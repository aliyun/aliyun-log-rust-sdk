use crate::state::{DeliveryCounters, Shared};
use ::metrics::{Counter, Unit};

/// Registered once at creation and shared across workers. The application owns export.
pub(crate) struct Metrics {
    pub rejected: Counter,
    accepted: Counter,
    succeeded: Counter,
    delivered_raw_bytes: Counter,
    failed: Counter,
}

impl Metrics {
    pub fn new() -> Self {
        for (name, description) in [
            ("sls_producer_accepted_logs_total", "Logs admitted locally"),
            (
                "sls_producer_delivered_logs_total",
                "Logs with a terminal delivery outcome",
            ),
            (
                "sls_producer_rejected_submissions_total",
                "Rejected send calls",
            ),
        ] {
            ::metrics::describe_counter!(name, Unit::Count, description);
        }
        ::metrics::describe_counter!(
            "sls_producer_delivered_raw_bytes_total",
            Unit::Bytes,
            "Estimated original bytes in successfully delivered logs"
        );
        Self {
            delivered_raw_bytes: ::metrics::counter!("sls_producer_delivered_raw_bytes_total"),
            rejected: ::metrics::counter!("sls_producer_rejected_submissions_total"),
            accepted: ::metrics::counter!("sls_producer_accepted_logs_total"),
            succeeded: ::metrics::counter!("sls_producer_delivered_logs_total", "result" => "success"),
            failed: ::metrics::counter!("sls_producer_delivered_logs_total", "result" => "failed"),
        }
    }
}

/// Publish cumulative progress once per interval and at shutdown. Recorder calls
/// stay outside the admission lock and off the per-log success path.
pub(crate) struct ProgressReporter<'a> {
    shared: &'a Shared,
    previous: DeliveryCounters,
}

impl<'a> ProgressReporter<'a> {
    pub fn new(shared: &'a Shared) -> Self {
        Self {
            shared,
            previous: DeliveryCounters::default(),
        }
    }

    pub fn record(&mut self) {
        let current = self.shared.gate.lock().unwrap().totals.clone();
        let metrics = &self.shared.metrics;
        for (handle, current, previous) in [
            (
                &metrics.accepted,
                current.accepted_logs,
                self.previous.accepted_logs,
            ),
            (
                &metrics.succeeded,
                current.succeeded_logs,
                self.previous.succeeded_logs,
            ),
            (
                &metrics.delivered_raw_bytes,
                current.delivered_raw_bytes,
                self.previous.delivered_raw_bytes,
            ),
            (
                &metrics.failed,
                current.failed_logs,
                self.previous.failed_logs,
            ),
        ] {
            let delta = current - previous;
            if delta != 0 {
                handle.increment(delta);
            }
        }
        self.previous = current;
    }
}
