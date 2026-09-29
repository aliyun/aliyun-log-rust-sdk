//! Cumulative counter snapshots for metric assertions.
use crate::{
    state::{DeliveryCounters, Shared},
    Producer,
};
use metrics_util::debugging::{DebugValue, Snapshotter};
use std::{collections::HashMap, sync::Mutex, time::Duration};

pub(crate) fn delivery_counts(shared: &Shared) -> DeliveryCounters {
    shared.gate.lock().unwrap().totals.clone()
}

pub(crate) fn capture_metrics(producer: &Producer) -> MetricSnapshot {
    producer.base.inner.shared.observer.snapshot()
}

type MetricKey = (String, Vec<(String, String)>);
#[derive(Clone, Default)]
pub(crate) struct MetricSnapshot(HashMap<MetricKey, u64>);
impl MetricSnapshot {
    pub fn counter(&self, name: &str) -> u64 {
        self.counter_with_labels(name, &[])
    }
    pub fn counter_with_labels(&self, name: &str, labels: &[(&str, &str)]) -> u64 {
        let values: Vec<_> = self
            .0
            .iter()
            .filter(|((metric, tags), _)| {
                metric == name
                    && labels
                        .iter()
                        .all(|(key, value)| tags.iter().any(|(k, v)| k == key && v == value))
            })
            .map(|(_, value)| *value)
            .collect();
        assert!(
            !values.is_empty(),
            "metric not registered: {name} {labels:?}"
        );
        values.into_iter().sum()
    }
}

pub(crate) struct Observer {
    recorder: Snapshotter,
    samples: Mutex<MetricSnapshot>,
}
impl Observer {
    pub fn new(recorder: Snapshotter) -> Self {
        Self {
            recorder,
            samples: Mutex::new(MetricSnapshot::default()),
        }
    }
    pub fn snapshot(&self) -> MetricSnapshot {
        let mut samples = self.samples.lock().unwrap();
        for (key, _, _, value) in self.recorder.snapshot().into_vec() {
            let mut labels: Vec<_> = key
                .key()
                .labels()
                .map(|l| (l.key().to_owned(), l.value().to_owned()))
                .collect();
            labels.sort();
            let DebugValue::Counter(delta) = value else {
                panic!("expected counter")
            };
            *samples
                .0
                .entry((key.key().name().to_owned(), labels))
                .or_default() += delta;
        }
        samples.clone()
    }
}

#[tokio::test]
async fn rejection_after_close_is_exported() {
    let (producer, writers) =
        super::start(super::config(), vec![super::mock(|_, _| async { Ok(()) })]);
    writers[0].send(super::entry("test")).unwrap();
    producer.test_close(Duration::from_secs(5)).await.unwrap();
    writers[0].send(super::entry("closed")).unwrap_err();

    assert_eq!(
        capture_metrics(&producer).counter("sls_producer_rejected_submissions_total"),
        1
    );
}

#[tokio::test]
async fn successful_send_and_submission_completion_do_not_write_metrics() {
    use crate::{batch::Command, state::Submission, Frontend};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    // No background sampler/aggregator: only exercise the user and submission paths.
    let shared = Shared::for_test(1, super::config(), [("project", "store")]);
    let (tx, mut rx) = mpsc::channel(8);
    let producer = Producer {
        base: crate::BaseProducer {
            inner: Arc::new(Frontend {
                shared: shared.clone(),
                tx,
            }),
        },
    };
    let writer = producer.writer("project", "store").unwrap();
    for _ in 0..128 {
        writer
            .send_with_callback(super::entry("single"), |r| assert!(r.is_ok()))
            .unwrap();
        let Command::Log(envelope) = rx.try_recv().unwrap() else {
            panic!("expected log")
        };
        let submission: Submission = envelope.submission;
        shared.complete([submission.id], Ok(()));
        assert_eq!(
            crate::events::poll_batch(&shared, Duration::ZERO)
                .unwrap()
                .dispatch(),
            1
        );
        for _ in 0..4 {
            writer.send(super::entry("single")).unwrap();
            let Command::Log(envelope) = rx.try_recv().unwrap() else {
                panic!("expected log");
            };
            shared.complete([envelope.submission.id], Ok(()));
        }
    }
    for (key, _, _, value) in shared.observer.recorder.snapshot().into_vec() {
        let DebugValue::Counter(n) = value else {
            panic!("expected counter")
        };
        assert_eq!(n, 0, "{}", key.key().name());
    }
    // A single background snapshot publishes the full result, once, with no scan
    // of individual submissions and no dependence on the submission API used.
    let mut reporter = crate::metrics::ProgressReporter::new(&shared);
    reporter.record();
    let captured = shared.observer.recorder.snapshot().into_vec();
    for name in [
        "sls_producer_accepted_logs_total",
        "sls_producer_delivered_logs_total",
    ] {
        let sum: u64 = captured
            .iter()
            .filter(|(key, _, _, _)| key.key().name() == name)
            .map(|(_, _, _, value)| match value {
                DebugValue::Counter(n) => *n,
                _ => panic!("expected counter"),
            })
            .sum();
        assert_eq!(sum, 128 * 5);
    }
    reporter.record();
    for (_, _, _, value) in shared.observer.recorder.snapshot().into_vec() {
        if let DebugValue::Counter(n) = value {
            assert_eq!(n, 0);
        }
    }
}

#[tokio::test]
async fn delivery_counts_logs_and_existing_raw_bytes_in_a_batch() {
    let (producer, writers) =
        super::start(super::config(), vec![super::mock(|_, _| async { Ok(()) })]);
    let log = super::entry("test");
    let expected_bytes = crate::batch::measure_log(&log).1 as u64 * 128;
    for _ in 0..128 {
        writers[0].send(log.clone()).unwrap();
    }
    producer.test_close(Duration::from_secs(5)).await.unwrap();

    assert_eq!(
        capture_metrics(&producer).counter("sls_producer_delivered_raw_bytes_total"),
        expected_bytes
    );
    assert_eq!(
        capture_metrics(&producer).counter_with_labels(
            "sls_producer_delivered_logs_total",
            &[("result", "success")]
        ),
        128
    );
}
