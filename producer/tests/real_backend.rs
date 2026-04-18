mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};

use aliyun_log_rust_sdk_producer::AckHandle;

use crate::support::{
    build_client, init_logger, make_record, producer_builder, require_env, unique_run_id,
    wait_for_record_ids,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sparse_logs_flush_on_linger_and_future_resolves() {
    init_logger();
    let Some(env) = require_env() else {
        return;
    };

    let client = build_client(env);
    let run_id = unique_run_id("linger");
    let since = SystemTime::now();
    let linger = Duration::from_millis(900);
    let topic = unique_run_id("topic-linger");

    let producer = producer_builder(env)
        .topic(&topic)
        .batch_max_events(16)
        .batch_max_bytes(256 * 1024)
        .linger(linger)
        .build()
        .await
        .expect("producer should build");

    let ack = producer
        .send_with_ack(make_record(&run_id, "sparse", 0, 32))
        .await
        .expect("send_with_ack should enqueue");

    let report = ack.wait().await.expect("delivery should succeed");
    assert_eq!(report.record_count, 1);
    assert!(
        report.elapsed >= Duration::from_millis(700),
        "expected linger-based flush, got elapsed={:?}",
        report.elapsed
    );

    let ids = wait_for_record_ids(
        &client,
        env,
        &run_id,
        &topic,
        since,
        1,
        Duration::from_secs(30),
    )
    .await;
    assert_eq!(ids.len(), 1);

    producer
        .close_and_wait()
        .await
        .expect("close should succeed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dense_logs_flush_on_max_events_and_future_reports_match_batches() {
    init_logger();
    let Some(env) = require_env() else {
        return;
    };

    let client = build_client(env);
    let run_id = unique_run_id("max-events");
    let since = SystemTime::now();
    let topic = unique_run_id("topic-max-events");

    let producer = producer_builder(env)
        .topic(&topic)
        .batch_max_events(3)
        .batch_max_bytes(1024 * 1024)
        .linger(Duration::from_secs(30))
        .build()
        .await
        .expect("producer should build");

    let mut acks = Vec::new();
    for seq in 0..7 {
        acks.push(
            producer
                .send_with_ack(make_record(&run_id, "dense", seq, 24))
                .await
                .expect("send_with_ack should enqueue"),
        );
    }

    producer.flush().await.expect("flush should succeed");

    let reports = wait_all(acks).await;
    let mut batch_sizes: BTreeMap<u64, usize> = BTreeMap::new();
    for report in reports {
        batch_sizes
            .entry(report.batch_id)
            .or_insert(report.record_count);
    }

    let sizes: Vec<usize> = batch_sizes.into_values().collect();
    assert_eq!(sizes, vec![3, 3, 1]);

    let ids = wait_for_record_ids(
        &client,
        env,
        &run_id,
        &topic,
        since,
        7,
        Duration::from_secs(30),
    )
    .await;
    assert_eq!(ids.len(), 7);

    let stats = producer.stats();
    assert_eq!(stats.accepted_records, 7);
    assert_eq!(stats.sent_records, 7);
    assert_eq!(stats.sent_batches, 3);

    producer
        .close_and_wait()
        .await
        .expect("close should succeed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn large_records_flush_on_max_bytes() {
    init_logger();
    let Some(env) = require_env() else {
        return;
    };

    let client = build_client(env);
    let run_id = unique_run_id("max-bytes");
    let since = SystemTime::now();
    let topic = unique_run_id("topic-max-bytes");

    let probe = make_record(&run_id, "bytes", 0, 700);
    let batch_max_bytes = probe.estimated_bytes() + 32;

    let producer = producer_builder(env)
        .topic(&topic)
        .batch_max_events(16)
        .batch_max_bytes(batch_max_bytes)
        .linger(Duration::from_secs(30))
        .build()
        .await
        .expect("producer should build");

    let ack1 = producer
        .send_with_ack(probe)
        .await
        .expect("first send should succeed");
    let ack2 = producer
        .send_with_ack(make_record(&run_id, "bytes", 1, 700))
        .await
        .expect("second send should succeed");

    producer.flush().await.expect("flush should succeed");

    let reports = wait_all(vec![ack1, ack2]).await;
    assert_eq!(reports.len(), 2);
    assert_ne!(reports[0].batch_id, reports[1].batch_id);
    assert!(reports.iter().all(|report| report.record_count == 1));

    let ids = wait_for_record_ids(
        &client,
        env,
        &run_id,
        &topic,
        since,
        2,
        Duration::from_secs(30),
    )
    .await;
    assert_eq!(
        ids,
        BTreeSet::from([format!("{run_id}-bytes-0"), format!("{run_id}-bytes-1"),])
    );

    producer
        .close_and_wait()
        .await
        .expect("close should succeed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_many_plus_flush_persists_all_records() {
    init_logger();
    let Some(env) = require_env() else {
        return;
    };

    let client = build_client(env);
    let run_id = unique_run_id("send-many");
    let since = SystemTime::now();
    let topic = unique_run_id("topic-send-many");

    let producer = producer_builder(env)
        .topic(&topic)
        .batch_max_events(64)
        .batch_max_bytes(1024 * 1024)
        .linger(Duration::from_secs(30))
        .build()
        .await
        .expect("producer should build");

    let records: Vec<_> = (0..5)
        .map(|seq| make_record(&run_id, "send-many", seq, 48))
        .collect();

    producer
        .send_many(records)
        .await
        .expect("send_many should enqueue all records");
    producer.flush().await.expect("flush should succeed");

    let ids = wait_for_record_ids(
        &client,
        env,
        &run_id,
        &topic,
        since,
        5,
        Duration::from_secs(30),
    )
    .await;
    assert_eq!(ids.len(), 5);

    let stats = producer.stats();
    assert_eq!(stats.accepted_records, 5);
    assert_eq!(stats.sent_records, 5);
    assert_eq!(stats.failed_batches, 0);

    producer
        .close_and_wait()
        .await
        .expect("close should succeed");
}

async fn wait_all(acks: Vec<AckHandle>) -> Vec<aliyun_log_rust_sdk_producer::DeliveryReport> {
    let mut reports = Vec::with_capacity(acks.len());
    for ack in acks {
        reports.push(ack.wait().await.expect("delivery should succeed"));
    }
    reports
}
