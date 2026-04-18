use std::sync::Arc;
use std::time::Duration;

use backon::{BackoffBuilder, ExponentialBuilder};
use log::error;
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::exporter::{ExportBatch, LogSink, SlsExporter};
use crate::model::{Batch, RecordEnvelope};
use crate::shared::Shared;

#[derive(Clone)]
enum DispatcherExporter {
    Sls(Arc<SlsExporter>),
    Custom(Arc<dyn LogSink>),
}

#[derive(Clone)]
pub(crate) struct Dispatcher {
    exporter: DispatcherExporter,
    shared: Arc<Shared>,
    concurrency: Arc<Semaphore>,
    export_timeout: Duration,
    max_retries: usize,
    base_backoff: Duration,
    max_backoff: Duration,
}

impl Dispatcher {
    pub fn new_sls(
        exporter: SlsExporter,
        shared: Arc<Shared>,
        concurrency: usize,
        export_timeout: Duration,
        max_retries: usize,
        base_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        Self::new(
            DispatcherExporter::Sls(Arc::new(exporter)),
            shared,
            concurrency,
            export_timeout,
            max_retries,
            base_backoff,
            max_backoff,
        )
    }

    pub fn new_custom(
        exporter: Arc<dyn LogSink>,
        shared: Arc<Shared>,
        concurrency: usize,
        export_timeout: Duration,
        max_retries: usize,
        base_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        Self::new(
            DispatcherExporter::Custom(exporter),
            shared,
            concurrency,
            export_timeout,
            max_retries,
            base_backoff,
            max_backoff,
        )
    }

    fn new(
        exporter: DispatcherExporter,
        shared: Arc<Shared>,
        concurrency: usize,
        export_timeout: Duration,
        max_retries: usize,
        base_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        Self {
            exporter,
            shared,
            concurrency: Arc::new(Semaphore::new(concurrency)),
            export_timeout,
            max_retries,
            base_backoff,
            max_backoff,
        }
    }

    pub async fn dispatch(&self, batch: Batch) {
        let inflight_guard = InflightBatchGuard::new(self.shared.clone());
        let this = self.clone();
        tokio::spawn(async move {
            let _inflight_guard = inflight_guard;
            let Ok(_permit) = this.concurrency.clone().acquire_owned().await else {
                return;
            };
            this.dispatch_inner(batch).await;
        });
    }

    async fn dispatch_inner(&self, batch: Batch) {
        let Batch {
            id,
            created_at,
            records,
            total_bytes,
        } = batch;
        let record_count = records.len();
        let mut finalizer = BatchFinalizer::new(self.shared.clone(), records);

        let result = match &self.exporter {
            DispatcherExporter::Sls(exporter) => {
                let encoded = match exporter.encode_batch(&finalizer.records) {
                    Ok(encoded) => encoded,
                    Err(err) => {
                        Self::finish_batch(&self.shared, &mut finalizer, record_count, Err(err));
                        return;
                    }
                };
                let encoded = Arc::new(encoded);
                let exporter = exporter.clone();
                self.retry_loop(move |retry_count| {
                    let exporter = exporter.clone();
                    let encoded = encoded.clone();
                    async move {
                        exporter
                            .export_encoded(id, &encoded, retry_count, created_at.elapsed())
                            .await
                    }
                })
                .await
            }
            DispatcherExporter::Custom(exporter) => {
                let records: Arc<[crate::LogRecord]> = finalizer
                    .records
                    .iter()
                    .map(|env| env.record.clone())
                    .collect::<Vec<_>>()
                    .into();
                let exporter = exporter.clone();
                self.retry_loop(move |retry_count| {
                    let exporter = exporter.clone();
                    let records = records.clone();
                    async move {
                        exporter
                            .export(ExportBatch {
                                batch_id: id,
                                records,
                                estimated_bytes: total_bytes,
                                retry_count,
                                elapsed: created_at.elapsed(),
                            })
                            .await
                    }
                })
                .await
            }
        };

        Self::finish_batch(&self.shared, &mut finalizer, record_count, result);
    }

    async fn retry_loop<F, Fut>(
        &self,
        mut attempt: F,
    ) -> Result<crate::DeliveryReport, crate::DeliveryError>
    where
        F: FnMut(usize) -> Fut,
        Fut: std::future::Future<Output = Result<crate::DeliveryReport, crate::DeliveryError>>,
    {
        let mut retry_count = 0usize;
        let mut backoff = ExponentialBuilder::default()
            .with_min_delay(self.base_backoff)
            .with_max_delay(self.max_backoff)
            .with_max_times(self.max_retries)
            .build();

        loop {
            let result = timeout(self.export_timeout, attempt(retry_count))
                .await
                .map_err(|_| crate::DeliveryError::Timeout)
                .and_then(|r| r);

            match result {
                Ok(report) => return Ok(report),
                Err(err) if err.is_retryable() => {
                    let Some(delay) = backoff.next() else {
                        return Err(crate::DeliveryError::RetriableExceeded {
                            last_error: Box::new(err),
                        });
                    };
                    let delay = if err.is_throttled() {
                        delay.max(self.max_backoff)
                    } else {
                        delay
                    };
                    retry_count += 1;
                    self.shared
                        .stats
                        .retry_count
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    tokio::time::sleep(delay).await;
                }
                Err(err) => return Err(err),
            }
        }
    }

    fn finish_batch(
        shared: &Arc<Shared>,
        finalizer: &mut BatchFinalizer,
        record_count: usize,
        result: Result<crate::DeliveryReport, crate::DeliveryError>,
    ) {
        match result {
            Ok(report) => {
                shared
                    .stats
                    .sent_records
                    .fetch_add(record_count as u64, std::sync::atomic::Ordering::Relaxed);
                shared
                    .stats
                    .sent_batches
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if let Some(callback) = &shared.callback {
                    invoke_callback("on_delivery", || callback.on_delivery(&report));
                }
                finalizer.finish(Ok(report));
            }
            Err(err) => {
                shared
                    .stats
                    .failed_batches
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if let Some(callback) = &shared.callback {
                    invoke_callback("on_error", || callback.on_error(&err));
                }
                finalizer.finish(Err(err));
            }
        }
    }
}

struct InflightBatchGuard {
    shared: Arc<Shared>,
}

impl InflightBatchGuard {
    fn new(shared: Arc<Shared>) -> Self {
        shared
            .stats
            .inflight_batches
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        shared.signal_progress();
        Self { shared }
    }
}

impl Drop for InflightBatchGuard {
    fn drop(&mut self) {
        self.shared
            .stats
            .inflight_batches
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        self.shared.signal_progress();
    }
}

struct BatchFinalizer {
    shared: Arc<Shared>,
    records: Vec<RecordEnvelope>,
}

impl BatchFinalizer {
    fn new(shared: Arc<Shared>, records: Vec<RecordEnvelope>) -> Self {
        Self { shared, records }
    }

    fn finish(&mut self, result: Result<crate::DeliveryReport, crate::DeliveryError>) {
        let mut total_bytes = 0usize;
        let count = self.records.len();
        for env in self.records.drain(..) {
            if let Some(tx) = env.ack {
                let _ = tx.send(result.clone());
            }
            total_bytes += env.estimated_bytes;
        }
        if count > 0 {
            self.shared.release_records(count, total_bytes);
        }
    }
}

impl Drop for BatchFinalizer {
    fn drop(&mut self) {
        if self.records.is_empty() {
            return;
        }

        let mut total_bytes = 0usize;
        let count = self.records.len();
        for env in self.records.drain(..) {
            if let Some(tx) = env.ack {
                let _ = tx.send(Err(crate::DeliveryError::Shutdown));
            }
            total_bytes += env.estimated_bytes;
        }
        self.shared.release_records(count, total_bytes);
    }
}

fn invoke_callback<F>(name: &'static str, f: F)
where
    F: FnOnce(),
{
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        error!(
            target: "aliyun_log_rust_sdk_producer::dispatcher",
            "producer callback panicked: {}",
            name
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::time::Instant;
    use tower::service_fn;

    use crate::builder::ProducerBuilder;
    use crate::exporter::mock::TowerSink;
    use crate::exporter::ExportBatch;
    use crate::{DeliveryError, LogRecord};

    fn record(msg: &str) -> LogRecord {
        LogRecord::new(std::time::SystemTime::UNIX_EPOCH).field("message", msg)
    }

    #[tokio::test]
    async fn non_retryable_error_does_not_retry() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts2 = attempts.clone();
        let exporter = TowerSink::new(service_fn(move |_batch: ExportBatch| {
            let attempts = attempts2.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<crate::DeliveryReport, _>(DeliveryError::Server {
                    code: "BadRequest".into(),
                    message: "nope".into(),
                    request_id: None,
                    retryable: false,
                    throttled: false,
                })
            }
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .max_retries(3)
            .base_backoff(Duration::from_millis(20))
            .max_backoff(Duration::from_millis(20))
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        let ack = producer.send_with_ack(record("a")).await.unwrap();
        let start = Instant::now();
        let err = ack.wait().await.unwrap_err();
        assert!(matches!(
            err,
            DeliveryError::Server {
                retryable: false,
                ..
            }
        ));
        assert!(start.elapsed() < Duration::from_millis(20));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(producer.stats().retry_count, 0);
    }

    #[tokio::test]
    async fn retry_policy_honors_backoff_timing() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts2 = attempts.clone();
        let exporter = TowerSink::new(service_fn(move |_batch: ExportBatch| {
            let attempts = attempts2.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<crate::DeliveryReport, _>(DeliveryError::Network("down".into()))
            }
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .max_retries(2)
            .base_backoff(Duration::from_millis(15))
            .max_backoff(Duration::from_millis(30))
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        let ack = producer.send_with_ack(record("a")).await.unwrap();
        let start = Instant::now();
        let err = ack.wait().await.unwrap_err();
        assert!(matches!(
            err,
            DeliveryError::RetriableExceeded {
                last_error
            } if matches!(*last_error, DeliveryError::Network(_))
        ));
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(40), "elapsed={elapsed:?}");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(producer.stats().retry_count, 2);
    }

    #[tokio::test]
    async fn throttled_error_retries_with_at_least_max_backoff() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts2 = attempts.clone();
        let exporter = TowerSink::new(service_fn(move |_batch: ExportBatch| {
            let attempts = attempts2.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<crate::DeliveryReport, _>(DeliveryError::Server {
                    code: "Throttled".into(),
                    message: "too many requests".into(),
                    request_id: None,
                    retryable: true,
                    throttled: true,
                })
            }
        }));

        let max_backoff = Duration::from_millis(50);
        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .max_retries(1)
            .base_backoff(Duration::from_millis(5))
            .max_backoff(max_backoff)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        let ack = producer.send_with_ack(record("a")).await.unwrap();
        let start = Instant::now();
        let _err = ack.wait().await.unwrap_err();
        let elapsed = start.elapsed();
        assert!(
            elapsed >= max_backoff,
            "throttled retry should wait at least max_backoff ({max_backoff:?}), got {elapsed:?}"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(producer.stats().retry_count, 1);
    }
}
