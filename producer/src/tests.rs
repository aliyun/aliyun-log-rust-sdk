pub(crate) mod observability;
use super::*;
use aliyun_log_rust_sdk::Error;
use aliyun_log_sdk_protobuf::{LogGroup, LogGroupImpl};
use bytes::Bytes;
use observability::{capture_metrics, delivery_counts};
use quick_protobuf::{BytesReader, MessageRead};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
use tokio::sync::{oneshot, Semaphore};

struct Mock<F>(F);
#[async_trait::async_trait]
impl<F, Fut> runtime::Transport for Mock<F>
where
    F: Fn(Bytes, usize) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), Error>> + Send,
{
    async fn send(
        &self,
        _project: &str,
        _logstore: &str,
        data: Bytes,
        raw_size: usize,
        _compression: Compression,
    ) -> Result<(), Error> {
        (self.0)(data, raw_size).await
    }
}
fn mock<F, Fut>(handler: F) -> Arc<dyn runtime::Transport>
where
    F: Fn(Bytes, usize) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), Error>> + Send + 'static,
{
    Arc::new(Mock(handler))
}

// Existing per-target fixtures route through one shared transport, like the client.
struct TestTransports(Vec<Arc<dyn runtime::Transport>>);
#[async_trait::async_trait]
impl runtime::Transport for TestTransports {
    async fn send(
        &self,
        project: &str,
        logstore: &str,
        data: Bytes,
        raw_size: usize,
        compression: Compression,
    ) -> Result<(), Error> {
        let index: usize = logstore.strip_prefix("store-").unwrap().parse().unwrap();
        self.0[index]
            .send(project, logstore, data, raw_size, compression)
            .await
    }
}

fn config() -> ProducerConfig {
    ProducerConfig::default()
        .with_compression(Compression::Lz4)
        .with_generate_pack_id(false)
        .with_linger(Duration::from_secs(10))
        .with_base_backoff(Duration::from_millis(1))
        .with_max_backoff(Duration::from_millis(2))
        .with_delivery_timeout(Duration::from_secs(10))
}

fn start(
    config: ProducerConfig,
    targets: Vec<Arc<dyn runtime::Transport>>,
) -> (ThreadedProducer, Vec<LogstoreWriter>) {
    start_with_inflight_limit(config, targets, None)
}

fn start_with_inflight_limit(
    config: ProducerConfig,
    targets: Vec<Arc<dyn runtime::Transport>>,
    limit: Option<usize>,
) -> (ThreadedProducer, Vec<LogstoreWriter>) {
    config.validate().unwrap();
    let owner = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
    let names: Vec<_> = (0..targets.len())
        .map(|index| format!("store-{index}"))
        .collect();
    let mut shared = Shared::for_test(
        owner,
        config,
        names.iter().map(|name| ("project", name.as_str())),
    );
    if let Some(limit) = limit {
        Arc::get_mut(&mut shared)
            .unwrap()
            .runtime_config
            .max_inflight_batches = limit;
    }
    let (tx, rx) = mpsc::channel(shared.runtime_config.input_capacity);
    let count = targets.len();
    runtime::launch(shared.clone(), rx, move || {
        Ok(Arc::new(TestTransports(targets)))
    })
    .unwrap();
    let producer = ThreadedProducer::from_base(BaseProducer {
        inner: Arc::new(Frontend { shared, tx }),
    })
    .unwrap();
    let writers = (0..count)
        .map(|index| {
            producer
                .writer("project", &format!("store-{index}"))
                .unwrap()
        })
        .collect();
    (producer, writers)
}

fn entry(value: impl Into<String>) -> Log {
    let mut log = Log::from_unixtime(1_700_000_000);
    log.add_content_kv("message", value);
    log
}

#[test]
fn blocking_lifecycle_and_owned_callback_need_no_caller_runtime() {
    assert!(tokio::runtime::Handle::try_current().is_err());
    let (producer, writers) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    let caller = std::thread::current().id();
    let (tx, rx) = flume::bounded(1);
    // Cell is Send but not Sync; the callback may own such application context.
    let context = std::cell::Cell::new(String::from("context"));
    let options = SendOptions::default()
        .with_source("shared-host")
        .with_topic("shared-topic");
    writers[0]
        .send_with_options_and_callback(entry("observed"), options.clone(), move |result| {
            tx.send((
                result.clone(),
                context.into_inner(),
                std::thread::current().id(),
            ))
            .unwrap();
        })
        .unwrap();
    writers[0]
        .send_with_options(entry("unobserved"), options.clone())
        .unwrap();
    assert_eq!(options.source(), "shared-host");
    assert_eq!(options.topic(), "shared-topic");
    producer
        .test_flush_blocking(Duration::from_secs(5))
        .unwrap();
    producer
        .test_close_blocking(Duration::from_secs(5))
        .unwrap();
    let (result, context, thread) = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(result.is_ok());
    assert_eq!(context, "context");
    assert_ne!(thread, caller);
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        2
    );
    assert_eq!(producer.base.inner.shared.gate.lock().unwrap().callbacks, 0);
    producer.test_close_blocking(Duration::ZERO).unwrap();
    producer.test_flush_blocking(Duration::ZERO).unwrap();
}

#[test]
fn blocking_flush_ignores_callback_but_close_waits_and_stops_admission() {
    let (producer, writers) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    let (entered_tx, entered_rx) = flume::bounded(1);
    let (release_tx, release_rx) = flume::bounded(1);
    writers[0]
        .send_with_callback(entry("slow callback"), move |result| {
            entered_tx.send(result.clone()).unwrap();
            release_rx.recv().unwrap();
        })
        .unwrap();
    producer
        .test_flush_blocking(Duration::from_secs(5))
        .unwrap();
    entered_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
        .unwrap();
    assert!(matches!(
        producer.test_close_blocking(Duration::ZERO),
        Err(TestWaitError::Timeout)
    ));
    let (rejected_tx, rejected_rx) = flume::bounded(1);
    let error = writers[0]
        .send_with_callback(entry("rejected"), move |result| {
            rejected_tx.send(result.clone()).unwrap();
        })
        .unwrap_err();
    assert!(matches!(error, ProducerError::Closed { .. }));
    assert_eq!(error.log().unwrap().contents()[0].value(), "rejected");
    // Rejection releases the closure without invoking it.
    assert!(matches!(
        rejected_rx.try_recv(),
        Err(flume::TryRecvError::Disconnected)
    ));
    assert!(matches!(
        producer.test_close_blocking(Duration::from_millis(10)),
        Err(TestWaitError::Timeout)
    ));
    release_tx.send(()).unwrap();
    producer
        .test_close_blocking(Duration::from_secs(5))
        .unwrap();
}

#[test]
fn blocking_flush_timeout_does_not_cancel_delivery() {
    let release = Arc::new(Semaphore::new(0));
    let gate = release.clone();
    let (producer, writers) = start(
        config(),
        vec![mock(move |_, _| {
            let gate = gate.clone();
            async move {
                gate.acquire().await.unwrap().forget();
                Ok(())
            }
        })],
    );
    let (tx, rx) = flume::bounded(1);
    writers[0]
        .send_with_callback(entry("pending"), move |result| {
            tx.send(result.clone()).unwrap();
        })
        .unwrap();
    assert!(matches!(
        producer.test_flush_blocking(Duration::from_millis(10)),
        Err(TestWaitError::Timeout)
    ));
    assert_eq!(
        producer
            .base
            .inner
            .shared
            .gate
            .lock()
            .unwrap()
            .pending
            .len(),
        1
    );
    release.add_permits(1);
    producer
        .test_flush_blocking(Duration::from_secs(5))
        .unwrap();
    producer
        .test_close_blocking(Duration::from_secs(5))
        .unwrap();
    assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap().is_ok());
    assert!(matches!(
        rx.try_recv(),
        Err(flume::TryRecvError::Disconnected)
    ));
}

#[tokio::test]
async fn blocking_flush_timeout_includes_capacity_wait_and_can_mix_with_async_close() {
    let (producer, _) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    let permits = producer
        .base
        .inner
        .tx
        .reserve_many(producer.base.inner.shared.runtime_config.input_capacity)
        .await
        .unwrap();
    let control = producer.clone();
    let result = std::thread::spawn(move || {
        assert!(tokio::runtime::Handle::try_current().is_err());
        control.test_flush_blocking(Duration::from_millis(10))
    })
    .join()
    .unwrap();
    assert!(matches!(result, Err(TestWaitError::Timeout)));
    drop(permits);
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    producer.close_blocking().unwrap();
}

async fn send_retry(
    writer: &LogstoreWriter,
    mut log: Log,
    mut options: impl FnMut() -> SendOptions,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        match writer.send_with_options(log, options()) {
            Ok(()) => return,
            Err(error @ ProducerError::EnqueueFull { .. }) => {
                log = error.into_log().unwrap();
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "send retry timed out"
                );
                tokio::task::yield_now().await;
            }
            Err(error) => panic!("send failed: {error}"),
        }
    }
}

#[derive(Debug)]
struct Decoded {
    pack_id: Option<String>,
    source: String,
    topic: String,
    values: Vec<String>,
}
fn decode(data: &Bytes, raw_size: usize) -> Decoded {
    decode_with(data, raw_size, Compression::Lz4)
}
fn decode_with(data: &Bytes, raw_size: usize, compression: Compression) -> Decoded {
    let raw = match compression {
        Compression::Lz4 => lz4::block::decompress(data, Some(raw_size as i32)).unwrap(),
        Compression::Zstd => zstd::bulk::decompress(data, raw_size).unwrap(),
    };
    assert_eq!(raw.len(), raw_size);
    let mut reader = BytesReader::from_bytes(&raw);
    let group = LogGroupImpl::from_reader(&mut reader, &raw).unwrap();
    Decoded {
        pack_id: group
            .log_tags
            .iter()
            .find(|tag| tag.key == "__pack_id__")
            .map(|tag| tag.value.to_string()),
        source: group.source.unwrap().into_owned(),
        topic: group.topic.unwrap().into_owned(),
        values: group
            .logs
            .into_iter()
            .map(|l| l.contents[0].value.to_string())
            .collect(),
    }
}

fn server_error(status: u32) -> Error {
    Error::Server {
        error_code: if status == 503 {
            "ServerBusy"
        } else {
            "MissAccessKeyId"
        }
        .into(),
        error_message: "test error".into(),
        http_status: status,
        request_id: Some("test-request".into()),
    }
}

async fn eventually(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn combines_submissions_and_keeps_all_four_key_components_separate() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let targets = (0..3)
        .map(|target| {
            let seen = seen.clone();
            mock(move |data, size| {
                seen.lock().unwrap().push((target, decode(&data, size)));
                async { Ok(()) }
            })
        })
        .collect();
    let (producer, writers) = start(config(), targets);
    writers[0]
        .send_with_options(
            entry("single"),
            SendOptions::default().with_source("s").with_topic("t"),
        )
        .unwrap();
    for value in ["batch1", "batch2"] {
        writers[0]
            .send_with_options(
                entry(value),
                SendOptions::default().with_source("s").with_topic("t"),
            )
            .unwrap();
    }
    writers[0]
        .send_with_options(
            entry("source"),
            SendOptions::default().with_source("other").with_topic("t"),
        )
        .unwrap();
    writers[0]
        .send_with_options(
            entry("topic"),
            SendOptions::default().with_source("s").with_topic("other"),
        )
        .unwrap();
    for writer in &writers[1..] {
        writer
            .send_with_options(
                entry("target"),
                SendOptions::default().with_source("s").with_topic("t"),
            )
            .unwrap();
    }
    producer.test_flush(Duration::from_secs(5)).await.unwrap();
    let report = delivery_counts(&producer.base.inner.shared);
    assert_eq!(report.accepted_logs, 7);
    assert_eq!(report.succeeded_logs, 7);
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 5);
    assert!(seen.iter().all(|(_, batch)| batch.pack_id.is_none()));
    let combined = seen
        .iter()
        .find(|(target, d)| *target == 0 && d.source == "s" && d.topic == "t")
        .unwrap();
    assert_eq!(combined.1.values, ["single", "batch1", "batch2"]);
}

#[tokio::test]
async fn unset_and_empty_source_topic_share_one_batch() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let batches = seen.clone();
    let transport = mock(move |data, size| {
        batches.lock().unwrap().push(decode(&data, size));
        async { Ok(()) }
    });
    let (producer, writers) = start(config(), vec![transport]);
    let reports = Arc::new(Mutex::new(Vec::new()));
    let options = [
        SendOptions::default(),
        SendOptions::default().with_source(""),
        SendOptions::default().with_topic(""),
        SendOptions::default().with_source("").with_topic(""),
    ];
    for (index, options) in options.into_iter().enumerate() {
        let reports = reports.clone();
        let log = entry(index.to_string());
        writers[0]
            .send_with_options_and_callback(log, options, move |report| {
                reports.lock().unwrap().push(report.clone())
            })
            .unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        4
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].source, "");
    assert_eq!(seen[0].topic, "");
    assert_eq!(seen[0].values, ["0", "1", "2", "3"]);
    let reports = reports.lock().unwrap();
    assert_eq!(reports.len(), 4);
    assert!(reports.iter().all(Result::is_ok));
}

#[tokio::test]
async fn count_threshold_batches_logs_and_calls_back_once_per_log() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let batches = seen.clone();
    let transport = mock(move |data, size| {
        batches.lock().unwrap().push(decode(&data, size).values);
        async { Err(server_error(403)) }
    });
    let (producer, writers) = start(config().with_batch_count_threshold(2), vec![transport]);
    for index in 0..4 {
        let calls = calls.clone();
        writers[0]
            .send_with_callback(entry(index.to_string()), move |result| {
                let error = result.as_ref().unwrap_err();
                assert_eq!(error.http_status(), Some(403));
                assert_eq!(error.error_code(), Some("MissAccessKeyId"));
                assert_eq!(error.request_id(), Some("test-request"));
                calls.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    let mut batches = seen.lock().unwrap();
    batches.sort();
    assert_eq!(*batches, vec![vec!["0", "1"], vec!["2", "3"]]);
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(delivery_counts(&producer.base.inner.shared).failed_logs, 4);
    assert_eq!(
        producer.base.inner.shared.raw_bytes.load(Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn byte_threshold_seals_automatically_aggregated_logs() {
    let log = entry("threshold");
    let size = batch::measure_log(&log).0;
    let transport = mock(|data, bytes| {
        assert_eq!(decode(&data, bytes).values.len(), 2);
        async { Ok(()) }
    });
    let (producer, writers) = start(
        config().with_batch_size_threshold(size * 2),
        vec![transport],
    );
    for _ in 0..4 {
        writers[0].send(log.clone()).unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        4
    );
}

#[tokio::test]
async fn hard_byte_limit_rolls_over_between_logs() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let batches = seen.clone();
    let transport = mock(move |data, size| {
        assert!(size < 10 * 1024 * 1024);
        batches
            .lock()
            .unwrap()
            .push(decode(&data, size).values.len());
        async { Ok(()) }
    });
    let (producer, writers) = start(
        config().with_batch_size_threshold(crate::config::MAX_BATCH_BYTES),
        vec![transport],
    );
    for _ in 0..10 {
        writers[0].send(entry("x".repeat(1024 * 1024))).unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    let mut sizes = seen.lock().unwrap().clone();
    sizes.sort();
    assert_eq!(sizes, [3, 7]);
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        10
    );
}

#[tokio::test]
async fn hard_count_limit_rolls_over_between_logs() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let batches = seen.clone();
    let transport = mock(move |data, size| {
        batches
            .lock()
            .unwrap()
            .push(decode(&data, size).values.len());
        async { Ok(()) }
    });
    let (producer, writers) = start(
        config()
            .with_batch_count_threshold(crate::config::MAX_BATCH_LOGS)
            .with_batch_size_threshold(crate::config::MAX_BATCH_BYTES),
        vec![transport],
    );
    for _ in 0..50_000 {
        send_retry(&writers[0], entry("x"), SendOptions::default).await;
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    let mut sizes = seen.lock().unwrap().clone();
    sizes.sort();
    assert_eq!(sizes, [9040, 40960]);
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        50_000
    );
}

#[tokio::test]
async fn linger_flushes_low_volume_without_explicit_flush() {
    let (sent_tx, sent_rx) = flume::bounded(1);
    let transport = mock(move |_, _| {
        sent_tx.send(()).unwrap();
        async { Ok(()) }
    });
    let (producer, writers) = start(
        config().with_linger(Duration::from_millis(30)),
        vec![transport],
    );
    writers[0].send(entry("low volume")).unwrap();
    tokio::time::timeout(Duration::from_secs(2), sent_rx.recv_async())
        .await
        .unwrap()
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
}

#[tokio::test]
async fn retries_reuse_compressed_payload_and_enforce_attempt_limit() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let requests = seen.clone();
    let transport = mock(move |data, size| {
        requests.lock().unwrap().push((data, size));
        async { Err(server_error(503)) }
    });
    let (producer, writers) = start(config().with_max_attempts(3), vec![transport]);
    let (tx, rx) = oneshot::channel();
    writers[0]
        .send_with_callback(entry("retry"), move |r| {
            tx.send(r.clone()).unwrap();
        })
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    let report = rx.await.unwrap();
    let error = report.unwrap_err();
    assert_eq!(delivery_counts(&producer.base.inner.shared).failed_logs, 1);
    assert_eq!(error.request_id(), Some("test-request"));
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    assert!(seen
        .iter()
        .all(|(data, size)| data.as_ptr() == seen[0].0.as_ptr() && *size == seen[0].1));
}

#[tokio::test]
async fn unclassified_transport_errors_retry_until_attempt_limit() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let calls = attempts.clone();
    let transport = mock(move |_, _| {
        calls.fetch_add(1, Ordering::SeqCst);
        async {
            Err(Error::Other(
                std::io::Error::other("unknown transport failure").into(),
            ))
        }
    });
    let (producer, writers) = start(config().with_max_attempts(3), vec![transport]);
    let (tx, rx) = oneshot::channel();
    writers[0]
        .send_with_callback(entry("unknown failure"), move |report| {
            tx.send(report.clone()).unwrap();
        })
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    let report = rx.await.unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 3);

    assert_eq!(delivery_counts(&producer.base.inner.shared).failed_logs, 1);
    assert!(matches!(report.unwrap_err(), DeliveryError::Internal(_)));
}

#[tokio::test]
async fn deadline_reports_failure_and_does_not_hang_close() {
    let transport = mock(|_, _| async { std::future::pending().await });
    let (producer, writers) = start(
        config().with_delivery_timeout(Duration::from_millis(80)),
        vec![transport],
    );
    let (tx, rx) = oneshot::channel();
    writers[0]
        .send_with_callback(entry("timeout"), move |r| {
            tx.send(r.clone()).unwrap();
        })
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    assert_eq!(delivery_counts(&producer.base.inner.shared).failed_logs, 1);
    assert_eq!(
        producer
            .base
            .inner
            .shared
            .raw_bytes
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    let report = rx.await.unwrap();
    let error = report.unwrap_err();
    assert!(matches!(error, DeliveryError::Timeout));
    assert!(error.http_status().is_none());
}

#[tokio::test]
async fn send_rejects_full_queue_without_admission_and_can_be_retried() {
    let (producer, writers) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    let permits = producer
        .base
        .inner
        .tx
        .reserve_many(producer.base.inner.shared.runtime_config.input_capacity)
        .await
        .unwrap();
    // Exercise the public send from an ordinary thread with no Tokio context.
    let writer = writers[0].clone();
    let rejected = std::thread::spawn(move || {
        assert!(tokio::runtime::Handle::try_current().is_err());
        writer
            .send_with_callback(entry("full"), |_| panic!("rejected callback"))
            .unwrap_err()
    })
    .join()
    .unwrap();
    assert!(matches!(rejected, ProducerError::EnqueueFull { .. }));
    assert_eq!(rejected.log().unwrap().contents()[0].value(), "full");
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).accepted_logs,
        0
    );
    assert_eq!(producer.base.inner.shared.gate.lock().unwrap().callbacks, 0);
    drop(permits);
    let writer = writers[0].clone();
    std::thread::spawn(move || writer.send(rejected.into_log().unwrap()))
        .join()
        .unwrap()
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        1
    );
    assert_eq!(
        capture_metrics(&producer).counter("sls_producer_rejected_submissions_total"),
        1
    );
}

#[tokio::test]
async fn soft_budget_rejects_immediately_and_recovers_without_new_input() {
    let release = Arc::new(Semaphore::new(0));
    let gate = release.clone();
    let transport = mock(move |_, _| {
        let gate = gate.clone();
        async move {
            gate.acquire().await.unwrap().forget();
            Ok(())
        }
    });
    let (producer, writers) = start(
        config().with_buffer_bytes(30).with_batch_count_threshold(1),
        vec![transport],
    );
    writers[0].send(entry("first large payload")).unwrap();
    eventually(|| {
        producer
            .base
            .inner
            .shared
            .overloaded
            .load(Ordering::Relaxed)
    })
    .await;
    let rejected = writers[0]
        .send_with_callback(entry("second large payload"), |_| {
            panic!("rejected callback")
        })
        .unwrap_err();
    assert!(matches!(rejected, ProducerError::EnqueueFull { .. }));
    assert_eq!(
        rejected.log().unwrap().contents()[0].value(),
        "second large payload"
    );
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).accepted_logs,
        1
    );
    release.add_permits(1);
    eventually(|| {
        !producer
            .base
            .inner
            .shared
            .overloaded
            .load(Ordering::Relaxed)
    })
    .await;
    writers[0].send(rejected.into_log().unwrap()).unwrap();
    release.add_permits(1);
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        2
    );
    assert_eq!(
        producer.base.inner.shared.raw_bytes.load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        capture_metrics(&producer).counter("sls_producer_rejected_submissions_total"),
        1
    );
}

#[tokio::test]
async fn close_rejects_send_even_when_soft_budget_is_full() {
    let release = Arc::new(Semaphore::new(0));
    let gate = release.clone();
    let transport = mock(move |_, _| {
        let gate = gate.clone();
        async move {
            gate.acquire().await.unwrap().forget();
            Ok(())
        }
    });
    let (producer, writers) = start(
        config().with_buffer_bytes(1).with_batch_count_threshold(1),
        vec![transport],
    );
    writers[0].send(entry("accepted")).unwrap();
    eventually(|| {
        producer
            .base
            .inner
            .shared
            .overloaded
            .load(Ordering::Relaxed)
    })
    .await;
    assert!(matches!(
        producer.test_close(Duration::from_millis(20)).await,
        Err(TestWaitError::Timeout)
    ));
    let rejected = writers[0]
        .send_with_callback(entry("closed"), |_| panic!("rejected callback"))
        .unwrap_err();
    assert!(matches!(rejected, ProducerError::Closed { .. }));
    assert_eq!(rejected.log().unwrap().contents()[0].value(), "closed");
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).accepted_logs,
        1
    );
    release.add_permits(1);
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        1
    );
}

#[tokio::test]
async fn flush_observes_completed_delivery_while_callback_queue_is_full() {
    let release_transport = Arc::new(Semaphore::new(0));
    let transport_gate = release_transport.clone();
    let transport = mock(move |_, _| {
        let gate = transport_gate.clone();
        async move {
            gate.acquire().await.unwrap().forget();
            Ok(())
        }
    });
    let callback_gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let completed_callbacks = Arc::new(AtomicUsize::new(0));
    let settings = config().with_callback_capacity(1024);
    // One running callback, a full queue, a blocked publication, and one more
    // submission whose delivery must be recorded before that publication resumes.
    let count = settings.callback_capacity();
    let (producer, writers) = start(settings.with_batch_count_threshold(count), vec![transport]);
    for _ in 0..count {
        while producer.base.inner.tx.capacity() == 0 {
            tokio::task::yield_now().await;
        }
        let gate = callback_gate.clone();
        let completed = completed_callbacks.clone();
        writers[0]
            .send_with_callback(entry("callback"), move |result| {
                let (lock, wake) = &*gate;
                drop(
                    wake.wait_while(lock.lock().unwrap(), |released| !*released)
                        .unwrap(),
                );
                result.as_ref().unwrap();
                completed.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
    }
    let mut flush = Box::pin(producer.test_flush(Duration::from_secs(5)));
    assert!(futures_util::poll!(flush.as_mut()).is_pending());
    release_transport.add_permits(1);
    let result = tokio::time::timeout(Duration::from_secs(2), flush).await;
    let callbacks_pending = producer.base.inner.shared.gate.lock().unwrap().callbacks;
    let pending_logs = producer
        .base
        .inner
        .shared
        .gate
        .lock()
        .unwrap()
        .pending
        .len();
    // Release callbacks even if the progress assertion failed, so cleanup can finish.
    *callback_gate.0.lock().unwrap() = true;
    callback_gate.1.notify_all();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert!(callbacks_pending > 0);
    result.unwrap().unwrap();
    assert_eq!(pending_logs, 0);
    assert_eq!(completed_callbacks.load(Ordering::Relaxed), count);
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        count as u64
    );
}

#[tokio::test]
async fn flush_watermark_does_not_wait_for_later_submissions() {
    let early = Arc::new(Semaphore::new(0));
    let late = Arc::new(Semaphore::new(0));
    let gates = (early.clone(), late.clone());
    let transport = mock(move |data, size| {
        let gate = if decode(&data, size).values[0] == "early" {
            gates.0.clone()
        } else {
            gates.1.clone()
        };
        async move {
            gate.acquire().await.unwrap().forget();
            Ok(())
        }
    });
    let (producer, writers) = start(config().with_batch_count_threshold(1), vec![transport]);
    writers[0].send(entry("early")).unwrap();
    let mut flush = Box::pin(producer.test_flush(Duration::from_secs(5)));
    assert!(futures_util::poll!(flush.as_mut()).is_pending());
    writers[0].send(entry("late")).unwrap();
    early.add_permits(1);
    flush.await.unwrap();
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        1
    );
    assert_eq!(
        producer
            .base
            .inner
            .shared
            .gate
            .lock()
            .unwrap()
            .pending
            .len(),
        1
    );
    late.add_permits(1);
    producer.test_close(Duration::from_secs(5)).await.unwrap();
}

#[tokio::test]
async fn slow_and_panicking_callbacks_are_isolated_and_close_joins_them() {
    let (release_tx, release_rx) = flume::bounded(1);
    let entered = Arc::new(AtomicUsize::new(0));
    let transport = mock(|_, _| async { Ok(()) });
    let (producer, writers) = start(config(), vec![transport]);
    let flag = entered.clone();
    writers[0]
        .send_with_callback(entry("slow"), move |_| {
            flag.store(1, Ordering::SeqCst);
            release_rx.recv().unwrap();
            panic!("intentional callback panic");
        })
        .unwrap();
    producer.test_flush(Duration::from_secs(5)).await.unwrap();
    eventually(|| entered.load(Ordering::SeqCst) == 1).await;
    assert!(matches!(
        producer.test_close(Duration::from_millis(20)).await,
        Err(TestWaitError::Timeout)
    ));
    release_tx.send(()).unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    assert_eq!(producer.base.inner.shared.gate.lock().unwrap().callbacks, 0);
}

#[tokio::test]
async fn empty_log_contents_remain_accepted() {
    let (producer, writers) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    for log in [entry("valid"), Log::new()] {
        writers[0].send(log).unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        2
    );
}

#[tokio::test]
async fn zero_linger_sends_without_a_flush_or_full_batch() {
    let (sent, requests) = flume::bounded(2);
    let transport = mock(move |data, size| {
        sent.send(decode(&data, size).values.len()).unwrap();
        async { Ok(()) }
    });
    let (producer, writers) = start(config().with_linger(Duration::ZERO), vec![transport]);
    writers[0].send(entry("one")).unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), requests.recv_async())
            .await
            .unwrap()
            .unwrap(),
        1
    );
    writers[0].send(entry("two")).unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), requests.recv_async())
            .await
            .unwrap()
            .unwrap(),
        1
    );
    producer.test_close(Duration::from_secs(5)).await.unwrap();
}

#[test]
fn size_estimates_track_content_bytes_across_varint_boundaries() {
    for size in [0, 1, 127, 128, 16383, 16384, 100_000, 1024 * 1024] {
        let mut log = entry("x".repeat(size));
        log.add_content_kv("k".repeat(128), "v");
        let expected = batch::measure_log(&log);
        for time in [0, 127, 128, 1_700_000_000, u32::MAX] {
            log.set_time(time);
            assert_eq!(batch::measure_log(&log), expected);
        }
        log.set_time_ns(999_999_999);
        assert_eq!(batch::measure_log(&log), expected);

        // A one-byte payload increase costs exactly one estimated byte, even
        // when protobuf would change the number of length-prefix bytes.
        let mut longer = entry("x".repeat(size + 1));
        longer.add_content_kv("k".repeat(128), "v");
        let estimate = batch::measure_log(&longer);
        assert_eq!(estimate, (expected.0 + 1, expected.1 + 1));
    }
}

#[tokio::test]
async fn send_accepts_logs_without_content_validation() {
    let (producer, writers) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    let mut unusual_time = entry("time");
    unusual_time.set_time_ns(1_000_000_000);
    let mut large_key = Log::new();
    large_key.add_content_kv("k".repeat(crate::config::MAX_BATCH_BYTES), "v");
    let callbacks = Arc::new(AtomicUsize::new(0));
    for log in [
        Log::new(),
        unusual_time,
        entry("x".repeat(1024 * 1024 + 1)),
        large_key,
    ] {
        let callbacks = callbacks.clone();
        writers[0]
            .send_with_callback(log, move |result| {
                result.as_ref().unwrap();
                callbacks.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        producer.test_flush(Duration::from_secs(5)).await.unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert_eq!(callbacks.load(Ordering::SeqCst), 4);
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        4
    );
}

#[tokio::test]
async fn group_metadata_does_not_reduce_the_log_batch_budget() {
    let log = entry("metadata headroom");
    let (log_size, _) = batch::measure_log(&log);
    let limit = log_size * 2;
    for pack_id in [false, true] {
        let transport = mock(move |data, raw_size| {
            assert!(raw_size > limit);
            let group = decode(&data, raw_size);
            assert_eq!(group.values.len(), 2);
            assert_eq!(group.source, "s".repeat(256));
            assert_eq!(group.topic, "t".repeat(256));
            assert_eq!(group.pack_id.is_some(), pack_id);
            async { Ok(()) }
        });
        let (producer, writers) = start(
            config()
                .with_batch_size_threshold(limit)
                .with_generate_pack_id(pack_id),
            vec![transport],
        );
        let options = || {
            SendOptions::default()
                .with_source("s".repeat(256))
                .with_topic("t".repeat(256))
        };
        writers[0]
            .send_with_options(log.clone(), options())
            .unwrap();
        writers[0]
            .send_with_options(log.clone(), options())
            .unwrap();
        for _ in 0..2 {
            writers[0]
                .send_with_options(log.clone(), options())
                .unwrap();
        }
        producer.test_close(Duration::from_secs(5)).await.unwrap();

        assert_eq!(
            delivery_counts(&producer.base.inner.shared).succeeded_logs,
            4
        );
    }
}

#[tokio::test]
async fn oversized_group_reaches_transport_and_reports_service_error() {
    let transport = mock(|_, raw_size| {
        assert!(raw_size > 10 * 1024 * 1024);
        async { Err(server_error(403)) }
    });
    let (producer, writers) = start(config(), vec![transport]);
    let (tx, rx) = oneshot::channel();
    writers[0]
        .send_with_callback(entry("x".repeat(12 * 1024 * 1024)), move |result| {
            tx.send(result.clone()).unwrap();
        })
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert!(matches!(
        rx.await.unwrap().unwrap_err(),
        DeliveryError::Server { .. }
    ));
    assert_eq!(delivery_counts(&producer.base.inner.shared).failed_logs, 1);

    assert_eq!(
        producer.base.inner.shared.raw_bytes.load(Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn waiting_http_requests_do_not_block_processing_more_batches() {
    let release = Arc::new(Semaphore::new(0));
    let permits = release.clone();
    let (started, requests) = flume::bounded(8);
    let transport = mock(move |_, _| {
        let release = permits.clone();
        let started = started.clone();
        async move {
            started.send_async(()).await.unwrap();
            release.acquire().await.unwrap().forget();
            Ok(())
        }
    });
    let (producer, writers) = start(
        config()
            .with_processing_workers(1)
            .with_batch_count_threshold(1),
        vec![transport],
    );
    for _ in 0..8 {
        writers[0].send(entry("blocked HTTP")).unwrap();
    }
    // All eight batches must be encoded and start sending before any request
    // completes, even with one processing thread. The former two slots deadlock here.
    tokio::time::timeout(Duration::from_secs(5), async {
        for _ in 0..8 {
            requests.recv_async().await.unwrap();
        }
    })
    .await
    .unwrap();

    release.add_permits(8);
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        8
    );
}

#[tokio::test]
async fn internal_capacity_bounds_concurrency_and_survives_provider_panics() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let counters = (active.clone(), peak.clone());
    let transport = mock(move |data, size| {
        let panic = decode(&data, size).values[0] == "panic";
        let (active, peak) = counters.clone();
        async move {
            if panic {
                panic!("intentional provider panic");
            }
            let n = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(n, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(5)).await;
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        }
    });
    let (producer, writers) = start_with_inflight_limit(
        config().with_batch_count_threshold(1),
        vec![transport],
        Some(3),
    );
    let results = Arc::new(Mutex::new(Vec::new()));
    for log in (0..30)
        .map(|_| entry("ok"))
        .chain([entry("panic"), entry("ok")])
    {
        let results = results.clone();
        writers[0]
            .send_with_callback(log, move |result| {
                results.lock().unwrap().push(result.clone());
            })
            .unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    let results = results.lock().unwrap();
    assert_eq!(results.len(), 32);
    let errors: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], DeliveryError::Internal(_)));
    assert!(errors[0].to_string().contains("panicked"));

    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        31
    );
    assert_eq!(delivery_counts(&producer.base.inner.shared).failed_logs, 1);

    assert!(peak.load(Ordering::SeqCst) <= 3);
    assert!(peak.load(Ordering::SeqCst) > 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_sync_writes_and_close_account_for_every_accepted_submission() {
    let callbacks = Arc::new(AtomicUsize::new(0));
    let accepted = Arc::new(AtomicUsize::new(0));
    let transport = mock(|_, _| async { Ok(()) });
    let (producer, writers) = start(config().with_batch_count_threshold(20), vec![transport]);
    let mut threads = Vec::new();
    for _ in 0..6 {
        let writer = writers[0].clone();
        let accepted = accepted.clone();
        let callbacks = callbacks.clone();
        threads.push(std::thread::spawn(move || {
            for _ in 0..200 {
                let callbacks = callbacks.clone();
                match writer.send_with_callback(entry("concurrent"), move |_| {
                    callbacks.fetch_add(1, Ordering::SeqCst);
                }) {
                    Ok(_) => {
                        accepted.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(error) => assert!(matches!(
                        error,
                        ProducerError::Closed { .. } | ProducerError::EnqueueFull { .. }
                    )),
                }
            }
        }));
    }
    eventually(|| accepted.load(Ordering::SeqCst) > 0).await;
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    for thread in threads {
        thread.join().unwrap();
    }

    assert_eq!(
        delivery_counts(&producer.base.inner.shared).accepted_logs,
        accepted.load(Ordering::SeqCst) as u64
    );
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        delivery_counts(&producer.base.inner.shared).accepted_logs
    );
    assert_eq!(
        callbacks.load(Ordering::SeqCst),
        accepted.load(Ordering::SeqCst)
    );
    assert_eq!(
        producer
            .base
            .inner
            .shared
            .raw_bytes
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn last_user_handle_drop_drains_background_without_arc_cycle() {
    let transport = mock(|_, _| async { Ok(()) });
    let (producer, writers) = start(config(), vec![transport]);
    let shared = producer.base.inner.shared.clone();
    writers[0].send(entry("drop")).unwrap();
    drop(writers);
    drop(producer);
    eventually(|| shared.gate.lock().unwrap().state == ProducerState::Closed).await;
    assert_eq!(delivery_counts(&shared).succeeded_logs, 1);
    assert_eq!(
        shared.raw_bytes.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn config_is_validated_before_startup() {
    assert!(config().with_processing_workers(0).validate().is_err());
    assert!(config().with_max_attempts(0).validate().is_err());
    assert!(config()
        .with_batch_size_threshold(10 * 1024 * 1024)
        .validate()
        .is_err());
}

#[tokio::test]
async fn invalid_destinations_fail_at_writer_creation_without_admission() {
    let (producer, _writers) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    for (project, logstore) in [
        ("", "store-0"),
        ("project", ""),
        ("project", "bad/name"),
        ("bad.name", "store"),
    ] {
        let error = producer
            .writer(project, logstore)
            .err()
            .expect("invalid destination");
        assert!(matches!(&error, ProducerError::InvalidInput { .. }));
        assert!(error.log().is_none());
        assert!(error.into_log().is_none());
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).accepted_logs,
        0
    );

    assert_eq!(
        producer.base.inner.shared.raw_bytes.load(Ordering::Relaxed),
        0
    );
    assert_eq!(producer.base.inner.shared.gate.lock().unwrap().callbacks, 0);
    assert_eq!(
        capture_metrics(&producer).counter("sls_producer_rejected_submissions_total"),
        0
    );
}

#[tokio::test]
async fn writer_lookup_and_callback_confirm_delivery() {
    let (producer, writers) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).accepted_logs,
        0
    );
    let writer = producer.writer("project", "store-0").unwrap().clone();
    let (tx, rx) = oneshot::channel();
    writer
        .send_with_options_and_callback(
            entry("named destination"),
            SendOptions::default()
                .with_source("host")
                .with_topic("topic"),
            move |report| {
                tx.send(report.clone()).unwrap();
            },
        )
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    drop(writer);
    drop(writers);
    drop(producer);
    let report = rx.await.unwrap();
    report.unwrap();
}

#[test]
fn zero_worker_budgets_are_rejected_before_starting_runtime() {
    for (config, name) in [
        (
            ProducerConfig::default().with_processing_workers(0),
            "processing_workers",
        ),
        (
            ProducerConfig::default().with_callback_capacity(0),
            "callback_capacity",
        ),
    ] {
        let error = ThreadedProducer::create(config).err().unwrap();
        assert!(matches!(error, ProducerError::Config(message) if message.contains(name)));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_client_posts_both_codecs_and_producer_owns_http_retry_policy() {
    assert_eq!(ProducerConfig::default().compression, Compression::Zstd);
    for compression in [Compression::Zstd, Compression::Lz4] {
        for response in [
            (
                "503 Service Unavailable",
                r#"{"errorCode":"ServerBusy","errorMessage":"retry"}"#,
            ),
            (
                "401 Unauthorized",
                r#"{"errorCode":"Unauthorized","errorMessage":"retry"}"#,
            ),
            (
                "400 Bad Request",
                "<html>unrecognized error response</html>",
            ),
        ] {
            check_http_compression(compression, response, false, None).await;
        }
    }
}

async fn check_http_compression(
    compression: Compression,
    first_response: (&'static str, &'static str),
    terminal_error: bool,
    external: Option<ExternalManagedCredentials>,
) {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let updates = external.clone();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for attempt in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut received = Vec::new();
            let header_end = loop {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                received.extend_from_slice(&buffer[..count]);
                if let Some(position) = received.windows(4).position(|w| w == b"\r\n\r\n") {
                    break position + 4;
                }
            };
            let headers = String::from_utf8(received[..header_end].to_vec())
                .unwrap()
                .to_ascii_lowercase();
            let header = |name: &str| {
                headers
                    .lines()
                    .find_map(|line| line.strip_prefix(name))
                    .unwrap()
                    .trim()
                    .to_owned()
            };
            let length = header("content-length:").parse::<usize>().unwrap();
            let raw_size = header("x-log-bodyrawsize:").parse::<usize>().unwrap();
            assert!(headers.starts_with("post /logstores/store/shards/lb http/1.1\r\n"));
            assert_eq!(header("x-log-compresstype:"), compression.as_str());
            let expected_agent = match compression {
                Compression::Zstd => {
                    concat!("aliyun-log-rust-producer/", env!("CARGO_PKG_VERSION"))
                }
                Compression::Lz4 => "custom-producer/1.0",
            };
            assert_eq!(header("user-agent:"), expected_agent);
            if let Some(updates) = &updates {
                assert!(header("authorization:").starts_with(&format!("log external-{attempt}:")));
                assert_eq!(header("x-acs-security-token:"), format!("token-{attempt}"));
                updates.set(
                    Credentials::new("external-1", "secret-1")
                        .unwrap()
                        .with_security_token("token-1"),
                );
            } else {
                assert!(header("authorization:").starts_with("log test-id:"));
            }
            while received.len() < header_end + length {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                received.extend_from_slice(&buffer[..count]);
            }
            let data = Bytes::copy_from_slice(&received[header_end..header_end + length]);
            let decoded = decode_with(&data, raw_size, compression);
            assert_eq!(decoded.values, ["over-http"]);
            assert!(decoded.pack_id.is_some());
            requests.push(data);
            let (status, body) = if attempt == 0 || terminal_error {
                first_response
            } else {
                ("200 OK", "")
            };
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nx-log-requestid: local-{attempt}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
        requests
    });
    // Client prefixes project to endpoint: project 127 + endpoint 0.0.1 => loopback.
    let producer_config = config()
        .with_endpoint(format!("http://0.0.1:{port}"))
        .with_access_key("test-id", "test-secret")
        .with_compression(compression)
        .with_max_attempts(2)
        .with_generate_pack_id(true);
    let producer_config = if compression == Compression::Lz4 {
        producer_config.with_user_agent("custom-producer/1.0")
    } else {
        producer_config
    };
    let producer_config = match external {
        Some(credentials) => producer_config.with_external_managed_credentials(credentials),
        None => producer_config,
    };
    let producer = ThreadedProducer::create(producer_config).unwrap();
    let (tx, rx) = oneshot::channel();
    producer
        .writer("127", "store")
        .unwrap()
        .send_with_options_and_callback(
            entry("over-http"),
            SendOptions::default()
                .with_source("host")
                .with_topic("topic"),
            move |report| {
                tx.send(report.clone()).unwrap();
            },
        )
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    let report = rx.await.unwrap();
    if terminal_error {
        let error = report.unwrap_err();
        assert!(matches!(error, DeliveryError::InvalidResponse(_)));
        assert!(!error.to_string().is_empty());
        assert_eq!(delivery_counts(&producer.base.inner.shared).failed_logs, 1);
    } else {
        assert_eq!(
            delivery_counts(&producer.base.inner.shared).succeeded_logs,
            1
        );
        report.unwrap();
    }
    let requests = tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(requests[0], requests[1]);
}

#[tokio::test]
async fn queued_batches_expire_without_making_more_http_calls() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let transport = mock(move |_, _| {
        observed.fetch_add(1, Ordering::Relaxed);
        async { std::future::pending().await }
    });
    let (producer, writers) = start_with_inflight_limit(
        config()
            .with_batch_count_threshold(1)
            .with_delivery_timeout(Duration::from_millis(50)),
        vec![transport],
        Some(1),
    );
    let (tx, rx) = oneshot::channel();
    writers[0].send(entry("in-flight")).unwrap();
    writers[0]
        .send_with_callback(entry("queued"), move |report| {
            tx.send(report.clone()).unwrap();
        })
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    let report = rx.await.unwrap();

    assert_eq!(delivery_counts(&producer.base.inner.shared).failed_logs, 2);
    assert!(matches!(report.unwrap_err(), DeliveryError::Timeout));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn close_waiting_inside_callback_is_rejected_then_shutdown_completes() {
    let (producer, writers) = start(config(), vec![mock(|_, _| async { Ok(()) })]);
    let callback_producer = producer.clone();
    let (tx, rx) = oneshot::channel();
    writers[0]
        .send_with_callback(entry("reentrant"), move |_| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            let result = runtime.block_on(callback_producer.test_close(Duration::from_millis(20)));
            tx.send(matches!(
                result,
                Err(TestWaitError::Producer(ProducerError::Reentrant))
            ))
            .unwrap();
        })
        .unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert!(rx.await.unwrap());
}

#[tokio::test]
async fn automatic_pack_ids_survive_retries_and_respect_wire_limits() {
    assert!(ProducerConfig::default().generate_pack_id);
    let attempts = Arc::new(Mutex::new(
        std::collections::BTreeMap::<String, usize>::new(),
    ));
    let counts = attempts.clone();
    let transport = mock(move |data, raw_size| {
        let batch = decode(&data, raw_size);
        assert_eq!(batch.values.len(), 2);
        let id = batch.pack_id.unwrap();
        assert_eq!(id.len(), crate::pack_id::PACK_ID_LEN);
        let mut metadata = LogGroup::new();
        metadata
            .set_source("")
            .set_topic("")
            .add_log_tag_kv(crate::pack_id::PACK_ID_TAG, &id);
        assert!(raw_size - metadata.encode().unwrap().len() <= 100);
        let mut counts = counts.lock().unwrap();
        let count = counts.entry(id).or_default();
        *count += 1;
        let retry = *count == 1;
        async move {
            if retry {
                Err(server_error(503))
            } else {
                Ok(())
            }
        }
    });
    let (producer, writers) = start(
        config()
            .with_generate_pack_id(true)
            .with_batch_size_threshold(100)
            .with_batch_count_threshold(2),
        vec![transport],
    );
    for _ in 0..4 {
        writers[0].send(entry("x".repeat(20))).unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        delivery_counts(&producer.base.inner.shared).succeeded_logs,
        4
    );
    let attempts = attempts.lock().unwrap();
    assert_eq!(attempts.len(), 2);
    let mut prefix = None;
    for (index, (id, count)) in attempts.iter().enumerate() {
        assert_eq!(*count, 2);
        let (context, sequence) = id.split_once('-').unwrap();
        assert!(context
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)));
        assert_eq!(prefix.get_or_insert(context), &context);
        assert_eq!(u64::from_str_radix(sequence, 16).unwrap(), index as u64);
    }
}

#[tokio::test]
async fn malformed_responses_retry_then_report_invalid_response() {
    check_http_compression(
        Compression::Zstd,
        (
            "400 Bad Request",
            "<html>unrecognized error response</html>",
        ),
        true,
        None,
    )
    .await;
}

#[tokio::test]
async fn configuration_and_credential_categories_preserve_retry_behavior() {
    use aliyun_log_rust_sdk::{ConfigError, CredentialsError};
    type ErrorFactory = fn() -> Error;
    type ErrorPredicate = fn(&DeliveryError) -> bool;
    let cases: [(ErrorFactory, ErrorPredicate, usize); 2] = [
        (
            || Error::InvalidConfig(ConfigError::InvalidEndpoint("invalid endpoint".into())),
            |error| matches!(error, DeliveryError::Internal(_)),
            1,
        ),
        (
            || {
                Error::Credentials(CredentialsError::provider(std::io::Error::other(
                    "credentials unavailable",
                )))
            },
            |error| matches!(error, DeliveryError::Credentials(_)),
            3,
        ),
    ];
    for (make_error, matches_error, attempts) in cases {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let transport = mock(move |_, _| {
            observed.fetch_add(1, Ordering::Relaxed);
            async move { Err(make_error()) }
        });
        let (producer, writers) = start(config().with_max_attempts(3), vec![transport]);
        let (tx, rx) = oneshot::channel();
        writers[0]
            .send_with_callback(entry("test"), move |result| {
                tx.send(result.clone()).unwrap();
            })
            .unwrap();
        producer.test_close(Duration::from_secs(5)).await.unwrap();

        let error = rx.await.unwrap().unwrap_err();
        assert!(matches_error(&error));
        assert!(!error.to_string().is_empty());
        assert_eq!(calls.load(Ordering::Relaxed), attempts);
    }
}

#[tokio::test]
async fn destinations_created_after_start_route_through_one_transport() {
    struct RoutingProbe(Arc<Mutex<Vec<(String, String, Decoded)>>>);
    #[async_trait::async_trait]
    impl runtime::Transport for RoutingProbe {
        async fn send(
            &self,
            project: &str,
            logstore: &str,
            data: Bytes,
            raw_size: usize,
            compression: Compression,
        ) -> Result<(), Error> {
            self.0.lock().unwrap().push((
                project.to_owned(),
                logstore.to_owned(),
                decode_with(&data, raw_size, compression),
            ));
            Ok(())
        }
    }
    let seen = Arc::new(Mutex::new(Vec::new()));
    let transport = Arc::new(RoutingProbe(seen.clone()));
    let shared = Shared::new(
        NEXT_OWNER.fetch_add(1, Ordering::Relaxed),
        config().with_generate_pack_id(true),
    );
    let (tx, rx) = mpsc::channel(16);
    runtime::launch(shared.clone(), rx, move || Ok(transport)).unwrap();
    let producer = ThreadedProducer::from_base(BaseProducer {
        inner: Arc::new(Frontend { shared, tx }),
    })
    .unwrap();
    for (project, logstore) in [("a", "same"), ("b", "same"), ("a", "other"), ("a", "same")] {
        producer
            .writer(project, logstore)
            .unwrap()
            .send(entry(format!("{project}/{logstore}")))
            .unwrap();
        producer.test_flush(Duration::from_secs(5)).await.unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    let batches = seen.lock().unwrap();
    assert_eq!(batches.len(), 4);
    for (project, logstore, batch) in batches.iter() {
        assert_eq!(batch.values, [format!("{project}/{logstore}")]);
        assert!(batch.pack_id.is_some());
    }
    let first = batches[0].2.pack_id.as_ref().unwrap();
    let repeated = batches[3].2.pack_id.as_ref().unwrap();
    assert_eq!(
        first.split_once('-').unwrap().0,
        repeated.split_once('-').unwrap().0
    );
    assert_ne!(
        first, repeated,
        "reobtaining a writer must not reset the PackId sequence"
    );
    // Writer creation after close can cache identity, but cannot reopen admission.
    assert!(matches!(
        producer
            .writer("new", "after-close")
            .unwrap()
            .send(entry("closed")),
        Err(ProducerError::Closed { .. })
    ));
}

#[derive(Debug, thiserror::Error)]
enum TestWaitError {
    #[error("test wait timed out")]
    Timeout,
    #[error(transparent)]
    Producer(ProducerError),
}

// Bounded waits are a test harness concern; the public lifecycle API has no deadline.
impl ThreadedProducer {
    async fn test_flush(&self, deadline: Duration) -> Result<(), TestWaitError> {
        tokio::time::timeout(deadline, self.flush())
            .await
            .map_err(|_| TestWaitError::Timeout)?
            .map_err(TestWaitError::Producer)
    }
    async fn test_close(&self, deadline: Duration) -> Result<(), TestWaitError> {
        tokio::time::timeout(deadline, self.close())
            .await
            .map_err(|_| TestWaitError::Timeout)?
            .map_err(TestWaitError::Producer)
    }
    fn test_flush_blocking(&self, deadline: Duration) -> Result<(), TestWaitError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(self.test_flush(deadline))
    }
    fn test_close_blocking(&self, deadline: Duration) -> Result<(), TestWaitError> {
        self.base.inner.shared.begin_close();
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(self.test_close(deadline))
    }
}
mod polling;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_credentials_config_rotates_across_threaded_producer_retries() {
    let credentials = ExternalManagedCredentials::new(
        Credentials::new("external-0", "secret-0")
            .unwrap()
            .with_security_token("token-0"),
    );
    check_http_compression(
        Compression::Zstd,
        (
            "503 Service Unavailable",
            r#"{"errorCode":"ServerBusy","errorMessage":"retry"}"#,
        ),
        false,
        Some(credentials),
    )
    .await;
}
