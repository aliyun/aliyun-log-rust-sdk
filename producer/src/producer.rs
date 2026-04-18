use std::sync::Arc;
use std::time::Duration;

use log::debug;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{self, Instant};

use crate::error::{
    CloseError, FlushError, RecordError, SendError, SendErrorWithRecord, TrySendError,
    TrySendErrorWithRecord,
};
use crate::memory_limiter::MemoryPermit;
use crate::model::{AckHandle, Batch, IngressMessage, LogRecord, RecordEnvelope, WhenFull};
use crate::shared::{Shared, STATE_CLOSED, STATE_RUNNING};
use crate::stats::ProducerStats;

/// An async log producer that batches records and delivers them to a sink.
///
/// `Producer` is cheaply cloneable (internally reference-counted).
///
/// # Shutdown
///
/// Call [`close`](Self::close) or [`close_and_wait`](Self::close_and_wait) to
/// flush pending records and shut down the background pipeline. If all clones
/// are dropped without calling `close`, the internal channel closes and the
/// batcher drains remaining records, but inflight batches in the dispatcher
/// may or may not complete before the tokio runtime shuts down. Always call
/// `close_and_wait` (or `close_timeout`) for a graceful shutdown.
#[derive(Clone)]
#[must_use]
pub struct Producer {
    inner: Arc<ProducerInner>,
}

pub(crate) struct ProducerInner {
    pub tx: mpsc::Sender<IngressMessage>,
    pub shared: Arc<Shared>,
}

impl Producer {
    pub(crate) fn new(inner: ProducerInner) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn builder() -> crate::ProducerBuilder {
        crate::ProducerBuilder::default()
    }

    pub async fn send(&self, record: LogRecord) -> Result<(), SendError> {
        self.send_result(record).await.map_err(|err| err.error)
    }

    pub fn try_send(&self, record: LogRecord) -> Result<(), TrySendError> {
        self.try_send_result(record).map_err(|err| err.error)
    }

    pub async fn send_with_ack(&self, record: LogRecord) -> Result<AckHandle, SendError> {
        self.send_with_ack_result(record)
            .await
            .map_err(|err| err.error)
    }

    pub async fn send_result(&self, record: LogRecord) -> Result<(), SendErrorWithRecord> {
        self.send_internal(record, None).await
    }

    pub fn try_send_result(&self, record: LogRecord) -> Result<(), TrySendErrorWithRecord> {
        self.try_send_internal(record, None)
    }

    pub async fn send_with_ack_result(
        &self,
        record: LogRecord,
    ) -> Result<AckHandle, SendErrorWithRecord> {
        let (tx, rx) = oneshot::channel();
        self.send_internal(record, Some(tx)).await?;
        Ok(AckHandle { rx })
    }

    pub async fn send_many<I>(&self, records: I) -> Result<(), SendError>
    where
        I: IntoIterator<Item = LogRecord>,
    {
        let records: Vec<LogRecord> = records.into_iter().collect();
        self.send_many_internal(records).await
    }

    pub async fn flush(&self) -> Result<(), FlushError> {
        debug!(target: "aliyun_log_rust_sdk_producer::producer", "flush requested");
        if self.inner.shared.is_closed() {
            return Err(FlushError::Closed);
        }
        let (tx, rx) = oneshot::channel();
        self.inner
            .tx
            .send(IngressMessage::Flush(tx))
            .await
            .map_err(|_| FlushError::Closed)?;
        rx.await.map_err(|_| FlushError::Closed)??;
        self.inner.shared.wait_until_drained().await;
        debug!(target: "aliyun_log_rust_sdk_producer::producer", "flush completed");
        Ok(())
    }

    pub async fn flush_timeout(&self, timeout: Duration) -> Result<(), FlushError> {
        time::timeout(timeout, self.flush())
            .await
            .unwrap_or(Err(FlushError::Timeout))
    }

    pub async fn close(&self) -> Result<(), CloseError> {
        debug!(target: "aliyun_log_rust_sdk_producer::producer", "close requested");
        loop {
            match self.inner.shared.state() {
                STATE_CLOSED => return self.inner.shared.close_result().unwrap_or(Ok(())),
                STATE_RUNNING => {
                    if self.inner.shared.begin_close() {
                        return self.initiate_close().await;
                    }
                }
                _ => {
                    return Ok(());
                }
            }
        }
    }

    pub async fn close_and_wait(&self) -> Result<(), CloseError> {
        debug!(target: "aliyun_log_rust_sdk_producer::producer", "close_and_wait requested");
        self.close().await?;
        let result = self.inner.shared.wait_until_closed().await;
        if result.is_ok() {
            debug!(
                target: "aliyun_log_rust_sdk_producer::producer",
                "close_and_wait completed"
            );
        }
        result
    }

    pub async fn close_timeout(&self, timeout: Duration) -> Result<(), CloseError> {
        time::timeout(timeout, self.close_and_wait())
            .await
            .unwrap_or(Err(CloseError::Timeout))
    }

    pub fn stats(&self) -> ProducerStats {
        self.inner.shared.stats()
    }

    async fn initiate_close(&self) -> Result<(), CloseError> {
        let (tx, rx) = oneshot::channel();
        if self.inner.tx.send(IngressMessage::Close(tx)).await.is_err() {
            let err = CloseError::Internal("batcher stopped".into());
            self.inner.shared.finish_close(Err(err.clone()));
            return Err(err);
        }

        let shared = self.inner.shared.clone();
        tokio::spawn(async move {
            let result = match rx.await {
                Ok(Ok(())) => {
                    shared.wait_until_drained().await;
                    Ok(())
                }
                Ok(Err(err)) => Err(err),
                Err(_) => Err(CloseError::Internal("close response dropped".into())),
            };
            shared.finish_close(result);
        });

        Ok(())
    }

    async fn send_internal(
        &self,
        record: LogRecord,
        ack: Option<DeliveryAck>,
    ) -> Result<(), SendErrorWithRecord> {
        if let Err(err) = validate_record(&record) {
            return Err(SendErrorWithRecord::new(SendError::Encode(err), record));
        }
        let estimated_bytes = record.estimated_bytes();
        let ack_requested = ack.is_some();
        debug!(
            target: "aliyun_log_rust_sdk_producer::producer",
            "send enqueue attempt: ack={}, estimated_bytes={}, queued_records={}, queued_bytes={}",
            ack_requested,
            estimated_bytes,
            self.inner.shared.stats().queued_records,
            self.inner.shared.stats().queued_bytes,
        );

        let permit = match self.acquire_memory(estimated_bytes).await {
            Ok(permit) => permit,
            Err(err) => return Err(SendErrorWithRecord::new(map_try_to_send(err), record)),
        };

        self.inner.shared.accept_records(1, estimated_bytes);
        self.enqueue_record(
            RecordEnvelope {
                record,
                estimated_bytes,
                _permit: permit,
                ack,
            },
            ack_requested,
        )
        .await
    }

    fn try_send_internal(
        &self,
        record: LogRecord,
        ack: Option<DeliveryAck>,
    ) -> Result<(), TrySendErrorWithRecord> {
        if let Err(err) = validate_record(&record) {
            return Err(TrySendErrorWithRecord::new(
                TrySendError::Encode(err),
                record,
            ));
        }
        let estimated_bytes = record.estimated_bytes();
        let permit = match self.try_acquire_memory(estimated_bytes) {
            Ok(permit) => permit,
            Err(err) => return Err(TrySendErrorWithRecord::new(err, record)),
        };

        self.inner.shared.accept_records(1, estimated_bytes);
        self.try_enqueue_record(RecordEnvelope {
            record,
            estimated_bytes,
            _permit: permit,
            ack,
        })
    }

    async fn send_many_internal(&self, records: Vec<LogRecord>) -> Result<(), SendError> {
        if records.is_empty() {
            return Ok(());
        }

        let mut total_bytes = 0usize;
        let mut records_with_size = Vec::with_capacity(records.len());
        for record in records {
            validate_record(&record).map_err(SendError::Encode)?;
            let estimated_bytes = record.estimated_bytes();
            total_bytes = total_bytes.saturating_add(estimated_bytes);
            records_with_size.push((record, estimated_bytes));
        }
        let record_count = records_with_size.len();

        let permit = self
            .acquire_memory(total_bytes)
            .await
            .map_err(map_try_to_send)?;

        self.inner.shared.accept_records(record_count, total_bytes);
        self.enqueue_many(
            records_with_size
                .into_iter()
                .map(|(record, estimated_bytes)| RecordEnvelope {
                    record,
                    estimated_bytes,
                    _permit: permit.clone(),
                    ack: None,
                })
                .collect(),
        )
        .await
    }

    async fn acquire_memory(&self, total_bytes: usize) -> Result<MemoryPermit, TrySendError> {
        loop {
            if !self.inner.shared.is_running() {
                return Err(TrySendError::Closed);
            }

            if let Some(permit) = self.inner.shared.memory_limiter.try_acquire(total_bytes) {
                return Ok(permit);
            }

            match self.inner.shared.when_full {
                WhenFull::Block => self.inner.shared.notify.notified().await,
                WhenFull::ReturnError => return Err(TrySendError::MemoryLimitExceeded),
            }
        }
    }

    fn try_acquire_memory(&self, total_bytes: usize) -> Result<MemoryPermit, TrySendError> {
        if !self.inner.shared.is_running() {
            return Err(TrySendError::Closed);
        }

        match self.inner.shared.memory_limiter.try_acquire(total_bytes) {
            Some(permit) => Ok(permit),
            None => Err(TrySendError::MemoryLimitExceeded),
        }
    }

    async fn enqueue_record(
        &self,
        env: RecordEnvelope,
        ack_requested: bool,
    ) -> Result<(), SendErrorWithRecord> {
        let estimated_bytes = env.estimated_bytes;
        match self.inner.tx.send(IngressMessage::Record(env)).await {
            Ok(_) => {
                debug!(
                    target: "aliyun_log_rust_sdk_producer::producer",
                    "send enqueued: ack={}, estimated_bytes={}, queued_records={}, queued_bytes={}",
                    ack_requested,
                    estimated_bytes,
                    self.inner.shared.stats().queued_records,
                    self.inner.shared.stats().queued_bytes,
                );
                Ok(())
            }
            Err(err) => {
                let IngressMessage::Record(env) = err.0 else {
                    unreachable!("unexpected send message variant");
                };
                self.inner.shared.release_records(1, env.estimated_bytes);
                Err(SendErrorWithRecord::new(SendError::Closed, env.record))
            }
        }
    }

    fn try_enqueue_record(&self, env: RecordEnvelope) -> Result<(), TrySendErrorWithRecord> {
        match self.inner.tx.try_send(IngressMessage::Record(env)) {
            Ok(_) => Ok(()),
            Err(tokio::sync::mpsc::error::TrySendError::Full(IngressMessage::Record(env))) => {
                self.inner.shared.release_records(1, env.estimated_bytes);
                Err(TrySendErrorWithRecord::new(
                    TrySendError::QueueFull,
                    env.record,
                ))
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(IngressMessage::Record(env))) => {
                self.inner.shared.release_records(1, env.estimated_bytes);
                Err(TrySendErrorWithRecord::new(
                    TrySendError::Closed,
                    env.record,
                ))
            }
            Err(_) => unreachable!("unexpected try_send message variant"),
        }
    }

    async fn enqueue_many(&self, envelopes: Vec<RecordEnvelope>) -> Result<(), SendError> {
        match self.inner.tx.send(IngressMessage::Records(envelopes)).await {
            Ok(_) => Ok(()),
            Err(err) => match err.0 {
                IngressMessage::Records(envelopes) => {
                    let total_bytes = envelopes.iter().map(|env| env.estimated_bytes).sum();
                    self.inner
                        .shared
                        .release_records(envelopes.len(), total_bytes);
                    Err(SendError::Closed)
                }
                IngressMessage::Record(env) => {
                    self.inner.shared.release_records(1, env.estimated_bytes);
                    Err(SendError::Closed)
                }
                _ => Err(SendError::Closed),
            },
        }
    }
}

type DeliveryAck = oneshot::Sender<Result<crate::DeliveryReport, crate::DeliveryError>>;

const MAX_RECORD_BYTES: usize = 3 * 1024 * 1024;

fn validate_record(record: &LogRecord) -> Result<(), RecordError> {
    if record.estimated_bytes() > MAX_RECORD_BYTES {
        return Err(RecordError::RecordTooLarge);
    }
    for field in &record.fields {
        if field.key.is_empty() {
            return Err(RecordError::EmptyKey);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) struct BatcherConfig {
    pub max_events: usize,
    pub max_bytes: usize,
    pub linger: Duration,
}

pub(crate) async fn run_batcher(
    rx: mpsc::Receiver<IngressMessage>,
    dispatcher: crate::dispatcher::Dispatcher,
    config: BatcherConfig,
) {
    Batcher::new(dispatcher, config).run(rx).await;
}

struct Batcher {
    dispatcher: crate::dispatcher::Dispatcher,
    config: BatcherConfig,
    next_batch_id: u64,
    current: Option<Batch>,
    deadline: Option<Instant>,
}

impl Batcher {
    fn new(dispatcher: crate::dispatcher::Dispatcher, config: BatcherConfig) -> Self {
        Self {
            dispatcher,
            config,
            next_batch_id: 1,
            current: None,
            deadline: None,
        }
    }

    async fn run(mut self, mut rx: mpsc::Receiver<IngressMessage>) {
        loop {
            if let Some(deadline) = self.deadline {
                tokio::select! {
                    _ = time::sleep_until(deadline) => self.flush_current().await,
                    msg = rx.recv() => {
                        if !self.handle_message(msg).await {
                            return;
                        }
                    }
                }
            } else if !self.handle_message(rx.recv().await).await {
                return;
            }
        }
    }

    async fn handle_message(&mut self, msg: Option<IngressMessage>) -> bool {
        match msg {
            Some(IngressMessage::Record(env)) => {
                self.push_envelope(env).await;
                true
            }
            Some(IngressMessage::Records(envs)) => {
                for env in envs {
                    self.push_envelope(env).await;
                }
                true
            }
            Some(IngressMessage::Flush(tx)) => {
                self.flush_current().await;
                let _ = tx.send(Ok(()));
                true
            }
            Some(IngressMessage::Close(tx)) => {
                self.flush_current().await;
                let _ = tx.send(Ok(()));
                false
            }
            None => false,
        }
    }

    async fn push_envelope(&mut self, env: RecordEnvelope) {
        self.ensure_batch();

        if self.should_flush_before_push(&env) {
            self.flush_current().await;
            self.ensure_batch();
        }

        let batch = self.current.as_mut().expect("batch exists");
        batch.push(env);

        if self.current_batch_is_full() {
            self.flush_current().await;
        }
    }

    fn ensure_batch(&mut self) {
        if self.current.is_none() {
            self.current = Some(Batch::new(self.alloc_batch_id()));
            self.deadline = Some(Instant::now() + self.config.linger);
        }
    }

    fn should_flush_before_push(&self, env: &RecordEnvelope) -> bool {
        let Some(batch) = self.current.as_ref() else {
            return false;
        };

        !batch.is_empty()
            && (batch.len() >= self.config.max_events
                || batch.total_bytes.saturating_add(env.estimated_bytes) > self.config.max_bytes)
    }

    fn current_batch_is_full(&self) -> bool {
        match self.current.as_ref() {
            Some(batch) => {
                batch.len() >= self.config.max_events || batch.total_bytes >= self.config.max_bytes
            }
            None => false,
        }
    }

    async fn flush_current(&mut self) {
        if let Some(batch) = self.current.take() {
            if !batch.is_empty() {
                self.dispatcher.dispatch(batch).await;
            }
        }
        self.deadline = None;
    }

    fn alloc_batch_id(&mut self) -> u64 {
        let id = self.next_batch_id;
        self.next_batch_id += 1;
        id
    }
}

fn map_try_to_send(err: TrySendError) -> SendError {
    match err {
        TrySendError::Closed => SendError::Closed,
        TrySendError::QueueFull => SendError::QueueFull,
        TrySendError::MemoryLimitExceeded => SendError::MemoryLimitExceeded,
        TrySendError::Encode(e) => SendError::Encode(e),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime};

    use tokio::time::sleep;
    use tower::service_fn;

    use crate::builder::ProducerBuilder;
    use crate::exporter::mock::TowerSink;
    use crate::exporter::ExportBatch;
    use crate::{DeliveryError, DeliveryReport, LogRecord, WhenFull};

    fn record(msg: &str) -> LogRecord {
        LogRecord::new(SystemTime::UNIX_EPOCH).field("message", msg)
    }

    #[tokio::test]
    async fn batches_by_max_bytes() {
        let batches = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let seen = batches.clone();
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| {
            let seen = seen.clone();
            async move {
                seen.lock().await.push(batch.records.len());
                Ok::<_, DeliveryError>(DeliveryReport {
                    batch_id: batch.batch_id,
                    record_count: batch.records.len(),
                    encoded_bytes: batch.estimated_bytes,
                    retry_count: batch.retry_count,
                    elapsed: batch.elapsed,
                    request_id: None,
                })
            }
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(10)
            .batch_max_bytes(90)
            .linger(Duration::from_secs(30))
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record("12345678901234567890")).await.unwrap();
        producer.send(record("abcdefghijabcdefghij")).await.unwrap();
        producer.send(record("xyz")).await.unwrap();
        producer.flush().await.unwrap();

        assert_eq!(*batches.lock().await, vec![1, 2]);
    }

    #[tokio::test]
    async fn linger_flushes_partial_batch() {
        let batches = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let seen = batches.clone();
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| {
            let seen = seen.clone();
            async move {
                seen.lock().await.push(batch.records.len());
                Ok::<_, DeliveryError>(DeliveryReport {
                    batch_id: batch.batch_id,
                    record_count: batch.records.len(),
                    encoded_bytes: batch.estimated_bytes,
                    retry_count: batch.retry_count,
                    elapsed: batch.elapsed,
                    request_id: None,
                })
            }
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(10)
            .linger(Duration::from_millis(20))
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record("a")).await.unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(*batches.lock().await, vec![1]);
    }

    #[tokio::test]
    async fn batches_by_max_events() {
        let batches = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let seen = batches.clone();
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| {
            let seen = seen.clone();
            async move {
                seen.lock().await.push(batch.records.len());
                Ok::<_, DeliveryError>(DeliveryReport {
                    batch_id: batch.batch_id,
                    record_count: batch.records.len(),
                    encoded_bytes: batch.estimated_bytes,
                    retry_count: batch.retry_count,
                    elapsed: batch.elapsed,
                    request_id: None,
                })
            }
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(2)
            .linger(Duration::from_secs(30))
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record("a")).await.unwrap();
        producer.send(record("b")).await.unwrap();
        producer.send(record("c")).await.unwrap();
        producer.flush().await.unwrap();

        assert_eq!(*batches.lock().await, vec![2, 1]);
    }

    #[tokio::test]
    async fn flush_waits_for_inflight_batches() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            sleep(Duration::from_millis(50)).await;
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(10)
            .linger(Duration::from_secs(10))
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record("a")).await.unwrap();
        let start = std::time::Instant::now();
        producer.flush().await.unwrap();
        assert!(start.elapsed() >= Duration::from_millis(45));
        assert_eq!(producer.stats().queued_records, 0);
        assert_eq!(producer.stats().inflight_batches, 0);
    }

    #[tokio::test]
    async fn close_returns_before_inflight_batches_finish() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            sleep(Duration::from_millis(80)).await;
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record("a")).await.unwrap();

        let start = std::time::Instant::now();
        producer.close().await.unwrap();
        assert!(
            start.elapsed() < Duration::from_millis(40),
            "close should return after shutdown is initiated, not after inflight delivery"
        );
        assert!(matches!(
            producer.send(record("b")).await,
            Err(crate::SendError::Closed)
        ));

        producer.close_and_wait().await.unwrap();
        assert_eq!(producer.stats().queued_records, 0);
        assert_eq!(producer.stats().inflight_batches, 0);
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts2 = attempts.clone();
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| {
            let attempts = attempts2.clone();
            async move {
                let current = attempts.fetch_add(1, Ordering::SeqCst);
                if current == 0 {
                    Err(DeliveryError::Server {
                        code: "InternalError".into(),
                        message: "boom".into(),
                        request_id: None,
                        retryable: true,
                        throttled: false,
                    })
                } else {
                    Ok(DeliveryReport {
                        batch_id: batch.batch_id,
                        record_count: batch.records.len(),
                        encoded_bytes: batch.estimated_bytes,
                        retry_count: batch.retry_count,
                        elapsed: batch.elapsed,
                        request_id: None,
                    })
                }
            }
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .base_backoff(Duration::from_millis(10))
            .max_backoff(Duration::from_millis(10))
            .max_retries(2)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        let ack = producer.send_with_ack(record("a")).await.unwrap();
        let report = ack.wait().await.unwrap();
        assert_eq!(report.retry_count, 1);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn block_when_full_waits_for_memory_release() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .memory_limit_bytes(80)
            .when_full(WhenFull::Block)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record(&"x".repeat(40))).await.unwrap();
        let start = std::time::Instant::now();
        producer.send(record(&"y".repeat(40))).await.unwrap();
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn send_returns_error_on_memory_limit_when_configured() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .memory_limit_bytes(80)
            .when_full(WhenFull::ReturnError)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record(&"x".repeat(40))).await.unwrap();
        let err = producer.send(record(&"y".repeat(40))).await.unwrap_err();
        producer.flush().await.unwrap();
        let stats = producer.stats();
        assert!(matches!(err, crate::SendError::MemoryLimitExceeded));
        assert_eq!(stats.accepted_records, 1);
        assert_eq!(stats.sent_records, 1);
    }

    #[tokio::test]
    async fn send_result_returns_record_when_enqueue_fails() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .memory_limit_bytes(80)
            .when_full(WhenFull::ReturnError)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record(&"x".repeat(40))).await.unwrap();
        let err = producer
            .send_result(record("kept"))
            .await
            .expect_err("send_result should return the record on enqueue failure");
        let (error, record) = err.into_parts();

        assert!(matches!(error, crate::SendError::MemoryLimitExceeded));
        assert_eq!(record.fields[0].value, "kept");
    }

    #[tokio::test]
    async fn try_send_result_returns_record_when_enqueue_fails() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .memory_limit_bytes(80)
            .when_full(WhenFull::ReturnError)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.try_send(record(&"x".repeat(40))).unwrap();
        let err = producer
            .try_send_result(record("kept"))
            .expect_err("try_send_result should return the record on enqueue failure");
        let (error, record) = err.into_parts();

        assert!(matches!(error, crate::TrySendError::MemoryLimitExceeded));
        assert_eq!(record.fields[0].value, "kept");
    }

    #[tokio::test]
    async fn send_with_ack_result_returns_record_when_closed() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.close().await.unwrap();
        let err = producer
            .send_with_ack_result(record("kept"))
            .await
            .expect_err("send_with_ack_result should return the record on enqueue failure");
        let (error, record) = err.into_parts();

        assert!(matches!(error, crate::SendError::Closed));
        assert_eq!(record.fields[0].value, "kept");
    }

    #[tokio::test]
    async fn close_unblocks_senders_waiting_on_memory_pressure() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(10)
            .linger(Duration::from_secs(30))
            .memory_limit_bytes(80)
            .when_full(WhenFull::Block)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.send(record(&"x".repeat(40))).await.unwrap();

        let producer2 = producer.clone();
        let blocked_send =
            tokio::spawn(async move { producer2.send(record(&"y".repeat(40))).await });

        sleep(Duration::from_millis(20)).await;

        producer
            .close_timeout(Duration::from_millis(200))
            .await
            .unwrap();

        assert!(matches!(
            blocked_send.await.unwrap(),
            Err(crate::SendError::Closed)
        ));
    }

    #[tokio::test]
    async fn memory_is_released_after_delivery_failure() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts2 = attempts.clone();
        let exporter = TowerSink::new(service_fn(move |_batch: ExportBatch| {
            let attempts = attempts2.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<DeliveryReport, _>(DeliveryError::Server {
                    code: "BadRequest".into(),
                    message: "bad".into(),
                    request_id: None,
                    retryable: false,
                    throttled: false,
                })
            }
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .memory_limit_bytes(80)
            .when_full(WhenFull::ReturnError)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        let ack = producer
            .send_with_ack(record(&"x".repeat(40)))
            .await
            .unwrap();
        let _ = ack.wait().await.unwrap_err();
        producer.try_send(record(&"y".repeat(40))).unwrap();
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn try_send_respects_memory_limit() {
        let exporter = TowerSink::new(service_fn(move |batch: ExportBatch| async move {
            sleep(Duration::from_millis(100)).await;
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: batch.elapsed,
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(10)
            .memory_limit_bytes(80)
            .when_full(WhenFull::ReturnError)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        producer.try_send(record(&"x".repeat(40))).unwrap();
        let err = producer.try_send(record(&"y".repeat(40))).unwrap_err();
        assert!(matches!(err, crate::TrySendError::MemoryLimitExceeded));
    }

    #[tokio::test]
    async fn exporter_panic_does_not_block_shutdown() {
        let exporter = TowerSink::new(service_fn(move |_batch: ExportBatch| async move {
            panic!("boom");
            #[allow(unreachable_code)]
            Ok::<_, DeliveryError>(DeliveryReport {
                batch_id: 0,
                record_count: 0,
                encoded_bytes: 0,
                retry_count: 0,
                elapsed: Duration::from_secs(0),
                request_id: None,
            })
        }));

        let producer = ProducerBuilder::default()
            .batch_max_events(1)
            .sink(Arc::new(exporter))
            .build()
            .await
            .unwrap();

        let ack = producer.send_with_ack(record("a")).await.unwrap();
        assert!(matches!(ack.wait().await, Err(DeliveryError::Shutdown)));
        producer
            .close_timeout(Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(producer.stats().queued_records, 0);
        assert_eq!(producer.stats().inflight_batches, 0);
    }
}
