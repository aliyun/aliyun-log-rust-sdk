//! Exercises the production build through the public API (without unit-test hooks).
use aliyun_log_producer::{DeliveryError, Log, Producer, ProducerConfig};
use aliyun_log_rust_sdk::{Credentials, CredentialsError, CredentialsProvider};
use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use std::{collections::HashMap, time::Duration};

struct UnavailableCredentials;
#[async_trait::async_trait]
impl CredentialsProvider for UnavailableCredentials {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        Err(CredentialsError::provider(std::io::Error::other(
            "offline test",
        )))
    }
}

fn start() -> Producer {
    start_with_writers(&[("project", "store")])
}

fn start_with_writers(destinations: &[(&str, &str)]) -> Producer {
    let config = ProducerConfig::default()
        .with_endpoint("https://cn-hangzhou.log.aliyuncs.com")
        .with_credentials_provider(UnavailableCredentials)
        .with_linger(Duration::from_secs(60))
        .with_delivery_timeout(Duration::from_secs(120))
        .with_max_attempts(2)
        .with_base_backoff(Duration::from_millis(1))
        .with_max_backoff(Duration::from_millis(2));
    let producer = Producer::create(config).unwrap();
    for &(project, logstore) in destinations {
        producer.writer(project, logstore).unwrap();
    }
    producer
}

fn entry() -> Log {
    let mut log = Log::from_unixtime(1_700_000_000);
    log.add_content_kv("message", "test");
    log
}

type Counts = HashMap<(String, String), u64>;

fn capture(recorder: &DebuggingRecorder, totals: &mut Counts) {
    for (key, unit, description, value) in recorder.snapshotter().snapshot().into_vec() {
        let expected_unit = if key.key().name() == "sls_producer_delivered_raw_bytes_total" {
            metrics::Unit::Bytes
        } else {
            metrics::Unit::Count
        };
        assert_eq!(unit, Some(expected_unit));
        assert!(description.is_some());
        let name = key.key().name();
        let labels: Vec<_> = key.key().labels().collect();
        let result = match name {
            "sls_producer_delivered_logs_total" => {
                assert_eq!(labels.len(), 1);
                assert_eq!(labels[0].key(), "result");
                assert!(matches!(labels[0].value(), "success" | "failed"));
                labels[0].value()
            }
            "sls_producer_accepted_logs_total"
            | "sls_producer_rejected_submissions_total"
            | "sls_producer_delivered_raw_bytes_total" => {
                assert!(labels.is_empty());
                ""
            }
            _ => panic!("unexpected metric: {name}"),
        };
        let DebugValue::Counter(delta) = value else {
            panic!("expected counter")
        };
        *totals
            .entry((name.to_owned(), result.to_owned()))
            .or_default() += delta;
    }
}

fn count(totals: &Counts, suffix: &str, result: &str) -> u64 {
    totals[&(format!("sls_producer_{suffix}"), result.to_owned())]
}

#[tokio::test]
async fn cached_handles_record_across_workers_and_aggregate_producers() {
    let recorder = DebuggingRecorder::new();
    let (first, second) = metrics::with_local_recorder(&recorder, || (start(), start()));
    // Workers retain the handles registered at creation after the local scope ends.
    for producer in [&first, &second] {
        producer
            .writer("project", "store")
            .unwrap()
            .send_with_callback(entry(), |result| {
                assert!(matches!(
                    result.as_ref().unwrap_err(),
                    DeliveryError::Credentials(_)
                ));
                panic!("intentional callback panic");
            })
            .unwrap();
    }
    first.close().await.unwrap();
    first.close().await.unwrap();
    let mut totals = Counts::new();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    capture(&recorder, &mut totals);
    assert_eq!(count(&totals, "accepted_logs_total", ""), 2);
    assert_eq!(count(&totals, "delivered_logs_total", "failed"), 1);
    assert_eq!(count(&totals, "delivered_logs_total", "success"), 0);

    second.close().await.unwrap();
    capture(&recorder, &mut totals);
    assert_eq!(count(&totals, "accepted_logs_total", ""), 2);
    assert_eq!(count(&totals, "delivered_logs_total", "failed"), 2);
}

#[tokio::test]
async fn lifecycle_waits_work_without_a_recorder_even_when_delivery_fails() {
    let producer = start();
    let (completed, result) = tokio::sync::oneshot::channel();
    producer
        .writer("project", "store")
        .unwrap()
        .send_with_callback(entry(), move |result| {
            completed.send(result.clone()).unwrap();
        })
        .unwrap();
    // Individual delivery fails; completing the lifecycle wait still returns Ok(()).
    producer.flush().await.unwrap();
    producer.close().await.unwrap();
    assert!(matches!(
        result.await.unwrap().unwrap_err(),
        DeliveryError::Credentials(_)
    ));
}

#[tokio::test]
async fn progress_sampling_continues_while_close_waits_for_a_callback() {
    let recorder = DebuggingRecorder::new();
    let producer = metrics::with_local_recorder(&recorder, start);
    let (release, wait) = std::sync::mpsc::channel();
    let (entered, running) = tokio::sync::oneshot::channel();
    producer
        .writer("project", "store")
        .unwrap()
        .send_with_callback(entry(), move |_| {
            entered.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
        })
        .unwrap();
    producer.flush().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .unwrap()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), producer.close())
            .await
            .is_err()
    );

    let mut totals = Counts::new();
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            capture(&recorder, &mut totals);
            if count(&totals, "delivered_logs_total", "failed") == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    release.send(()).unwrap();
    producer.close().await.unwrap();
    capture(&recorder, &mut totals);
    assert_eq!(count(&totals, "delivered_logs_total", "failed"), 1);
}

#[tokio::test]
async fn writer_destinations_do_not_add_metric_series() {
    let recorder = DebuggingRecorder::new();
    let producer = metrics::with_local_recorder(&recorder, || start_with_writers(&[]));
    let mut totals = Counts::new();
    capture(&recorder, &mut totals);
    assert_eq!(totals.len(), 5);
    let initial_keys: std::collections::HashSet<_> = totals.keys().cloned().collect();
    for (index, (project, logstore)) in [
        ("z-project", "same"),
        ("a-project", "same"),
        ("z-project", "other"),
    ]
    .into_iter()
    .enumerate()
    {
        let writer = producer.writer(project, logstore).unwrap();
        for _ in 0..=index {
            writer.send(entry()).unwrap();
        }
    }
    producer.writer("unused", "store").unwrap();
    producer.close().await.unwrap();
    capture(&recorder, &mut totals);
    assert_eq!(initial_keys, totals.keys().cloned().collect());
    assert_eq!(count(&totals, "accepted_logs_total", ""), 6);
    assert_eq!(count(&totals, "delivered_logs_total", "failed"), 6);
    assert_eq!(count(&totals, "delivered_raw_bytes_total", ""), 0);
}
