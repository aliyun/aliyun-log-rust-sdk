use super::*;

fn base(config: ProducerConfig) -> (BaseProducer, LogstoreWriter) {
    let shared = Shared::new(NEXT_OWNER.fetch_add(1, Ordering::Relaxed), config);
    let (tx, rx) = mpsc::channel(shared.runtime_config.input_capacity);
    runtime::launch(shared.clone(), rx, || Ok(mock(|_, _| async { Ok(()) }))).unwrap();
    let producer = BaseProducer {
        inner: Arc::new(Frontend { shared, tx }),
    };
    let writer = producer.writer("project", "store").unwrap();
    (producer, writer)
}

#[tokio::test]
async fn base_delivery_requires_poll_and_dropped_batch_returns_capacity_only_after_dispatch() {
    let (producer, writer) = base(config().with_callback_capacity(2));
    let calls = Arc::new(AtomicUsize::new(0));
    let caller = std::thread::current().id();
    for _ in 0..2 {
        let calls = calls.clone();
        writer
            .send_with_callback(entry("one"), move |r| {
                r.as_ref().unwrap();
                assert_eq!(std::thread::current().id(), caller);
                calls.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
    }
    tokio::time::timeout(Duration::from_secs(3), producer.flush())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(matches!(
        writer.send_with_callback(entry("full"), |_| panic!("rejected")),
        Err(ProducerError::EnqueueFull { .. })
    ));
    writer.send(entry("plain admission still works")).unwrap();
    let batch = producer.poll_batch(Duration::from_secs(1)).unwrap();
    assert!(matches!(
        producer.poll_batch(Duration::ZERO),
        Err(ProducerError::PollBusy)
    ));
    producer.begin_close();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), producer.wait_closed())
            .await
            .is_err()
    );
    drop(batch);
    assert_eq!(producer.inner.shared.gate.lock().unwrap().callbacks, 2);
    producer
        .poll_batch(Duration::from_secs(1))
        .unwrap()
        .dispatch();
    tokio::time::timeout(Duration::from_secs(3), finish(&producer))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(producer.inner.shared.gate.lock().unwrap().callbacks, 0);
    finish(&producer).await.unwrap();
}

#[tokio::test]
async fn manual_polling_drains_multiple_batches_and_isolates_panics_and_reentrant_waits() {
    let (producer, writer) = base(config());
    let calls = Arc::new(AtomicUsize::new(0));
    for index in 0..200 {
        let control = producer.clone();
        let calls = calls.clone();
        writer
            .send_with_callback(entry("many"), move |_| {
                calls.fetch_add(1, Ordering::Relaxed);
                assert!(matches!(
                    control.wait_closed_blocking(),
                    Err(ProducerError::Reentrant)
                ));
                assert!(matches!(
                    control.flush_blocking(),
                    Err(ProducerError::Reentrant)
                ));
                assert!(matches!(
                    control.poll_batch(Duration::ZERO),
                    Err(ProducerError::Reentrant)
                ));
                if index == 10 {
                    panic!("isolated");
                }
            })
            .unwrap();
    }
    tokio::time::timeout(Duration::from_secs(5), finish(&producer))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 200);
    assert!(producer.is_closed());
}

#[tokio::test]
async fn polling_batches_bound_dispatch_and_requeue_preserves_order() {
    let (producer, writer) = base(config().with_batch_count_threshold(80));
    let calls = Arc::new(Mutex::new(Vec::new()));
    for index in 0..80 {
        let calls = calls.clone();
        writer
            .send_with_callback(entry("delivery"), move |result| {
                result.as_ref().unwrap();
                calls.lock().unwrap().push(index);
            })
            .unwrap();
    }
    producer.flush().await.unwrap();
    // One network batch publishes all 80 callbacks together.
    let batch = producer.poll_batch(Duration::from_secs(3)).unwrap();
    drop(batch);
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(producer.poll_batch(Duration::ZERO).unwrap().dispatch(), 64);
    assert_eq!(producer.poll_batch(Duration::ZERO).unwrap().dispatch(), 16);
    assert_eq!(*calls.lock().unwrap(), (0..80).collect::<Vec<_>>());
    assert_eq!(producer.poll_batch(Duration::ZERO).unwrap().dispatch(), 0);
    tokio::time::timeout(Duration::from_secs(3), finish(&producer))
        .await
        .unwrap()
        .unwrap();
    // Metrics reporting is independent of polling and remains active through close.
    assert_eq!(
        producer
            .inner
            .shared
            .observer
            .snapshot()
            .counter("sls_producer_accepted_logs_total"),
        80
    );
    assert_eq!(
        producer
            .inner
            .shared
            .observer
            .snapshot()
            .counter_with_labels(
                "sls_producer_delivered_logs_total",
                &[("result", "success")]
            ),
        80
    );
}

#[test]
fn blocking_lifecycle_needs_no_runtime_and_threaded_close_keeps_callback_affinity() {
    let (base, writer) = base(config());
    let thread = std::thread::current().id();
    let (tx, rx) = flume::bounded(1);
    let producer = ThreadedProducer::from_base(base).unwrap();
    writer
        .send_with_callback(entry("blocking"), move |_| {
            tx.send(std::thread::current().id()).unwrap();
        })
        .unwrap();
    producer.flush_blocking().unwrap();
    producer.close_blocking().unwrap();
    assert_ne!(rx.recv().unwrap(), thread);
    producer.close_blocking().unwrap();
}

#[tokio::test]
async fn dropping_manual_producer_and_writers_does_not_leak_runtime_with_pending_callbacks() {
    let (producer, writer) = base(config());
    let shared = producer.inner.shared.clone();
    writer
        .send_with_callback(entry("abandoned"), |_| panic!("must not dispatch on drop"))
        .unwrap();
    drop(writer);
    drop(producer);
    eventually(|| shared.gate.lock().unwrap().state == ProducerState::Closed).await;
    assert_eq!(shared.gate.lock().unwrap().callbacks, 0);
}

#[test]
fn blocking_lifecycle_works_inside_another_executor() {
    futures_lite::future::block_on(async {
        let (base, writer) = base(config());
        let producer = ThreadedProducer::from_base(base).unwrap();
        let completed = Arc::new(AtomicUsize::new(0));
        let observed = completed.clone();
        writer
            .send_with_callback(entry("nested"), move |result| {
                result.as_ref().unwrap();
                observed.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        producer.flush_blocking().unwrap();
        producer.close_blocking().unwrap();
        assert_eq!(completed.load(Ordering::Relaxed), 1);
    });
}

// Application-owned shutdown: waiting alone never executes callbacks.
async fn finish(producer: &BaseProducer) -> Result<(), ProducerError> {
    producer.begin_close();
    while !producer.is_closed() {
        producer.poll_batch(Duration::ZERO)?.dispatch();
        tokio::task::yield_now().await;
    }
    producer.wait_closed().await
}
