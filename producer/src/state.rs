use std::{
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::Instant,
};

use tokio::sync::watch;

use crate::*;

pub(crate) struct Shared {
    pub events: crate::events::EventQueue,
    pub poll_worker: crate::threaded::PollWorker,
    pub owner: u64,
    pub config: ProducerConfig,
    pub runtime_config: crate::config::RuntimeConfig,
    /// Only admission, per-submission completion and lifecycle transitions take this lock.
    /// No logs, compression, callbacks, or awaits are processed while it is held.
    pub gate: Mutex<Progress>,
    pub changes: watch::Sender<()>,
    pub overloaded: AtomicBool,
    pub raw_bytes: AtomicUsize,
    pub metrics: crate::metrics::Metrics,
    pub writers: crate::registry::WriterCache,
    #[cfg(test)]
    pub observer: crate::tests::observability::Observer,
}

pub(crate) struct Progress {
    pub state: ProducerState,
    pub next_id: u64,
    pub pending: BTreeMap<SubmissionId, PendingSubmission>,
    pub totals: DeliveryCounters,
    pub callbacks: usize,
    pub fatal: Option<String>,
}

impl Shared {
    pub fn new(owner: u64, config: ProducerConfig) -> Arc<Self> {
        #[cfg(test)]
        let recorder = ::metrics_util::debugging::DebuggingRecorder::new();
        #[cfg(test)]
        let metrics = ::metrics::with_local_recorder(&recorder, crate::metrics::Metrics::new);
        #[cfg(not(test))]
        let metrics = crate::metrics::Metrics::new();
        Arc::new(Self {
            events: Default::default(),
            poll_worker: Default::default(),
            owner,
            runtime_config: crate::config::RuntimeConfig::new(&config),
            config,
            gate: Mutex::new(Progress {
                state: ProducerState::Running,
                next_id: 1,
                pending: BTreeMap::new(),
                totals: DeliveryCounters::default(),
                callbacks: 0,
                fatal: None,
            }),
            changes: watch::channel(()).0,
            overloaded: AtomicBool::new(false),
            raw_bytes: AtomicUsize::new(0),
            metrics,
            writers: crate::registry::WriterCache::default(),
            #[cfg(test)]
            observer: crate::tests::observability::Observer::new(recorder.snapshotter()),
        })
    }

    #[cfg(test)]
    pub fn for_test<'a>(
        owner: u64,
        config: ProducerConfig,
        names: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Arc<Self> {
        let shared = Self::new(owner, config);
        for (project, logstore) in names {
            shared.writers.get_or_create(project, logstore);
        }
        shared
    }

    pub fn reject(&self, error: ProducerError) -> ProducerError {
        self.metrics.rejected.increment(1);
        error
    }

    pub fn notify(&self) {
        self.changes.send_replace(());
    }

    pub fn begin_close(&self) {
        let mut gate = self.gate.lock().unwrap();
        if gate.state == ProducerState::Running {
            gate.state = ProducerState::Closing;
            drop(gate);
            self.notify();
        }
    }

    /// Update backpressure and return the sampled bytes for the next check policy.
    pub fn refresh_pressure(&self) -> usize {
        let bytes = self.raw_bytes.load(Ordering::Relaxed);
        let old = self.overloaded.load(Ordering::Relaxed);
        let new = if old {
            bytes > self.config.buffer_bytes / 5 * 4
        } else {
            bytes >= self.config.buffer_bytes
        };
        if new != old {
            self.overloaded.store(new, Ordering::Relaxed);
        }
        bytes
    }

    pub fn fatal_error(&self) -> Result<(), ProducerError> {
        match self.gate.lock().unwrap().fatal.clone() {
            Some(error) => Err(ProducerError::Internal(error)),
            None => Ok(()),
        }
    }

    /// Only the admission watermark is needed: completed submissions leave pending.
    /// This neither copies the pending map nor retains submissions during flush.
    pub async fn wait_delivery(&self, watermark: SubmissionId) -> Result<(), ProducerError> {
        let mut changes = self.changes.subscribe();
        loop {
            let complete = self
                .gate
                .lock()
                .unwrap()
                .pending
                .range(..=watermark)
                .next()
                .is_none();
            if complete {
                break;
            }
            changes.changed().await.expect("shared owns watch sender");
        }
        self.fatal_error()
    }

    /// Complete accepted logs once, publish their callbacks, then notify waiters.
    /// One gate acquisition and byte-budget update covers the entire batch.
    pub fn complete(
        self: &Arc<Self>,
        ids: impl IntoIterator<Item = SubmissionId>,
        result: DeliveryResult,
    ) {
        let mut jobs = Vec::new();
        let mut bytes = 0;
        let mut completed = 0;
        let mut gate = self.gate.lock().unwrap();
        for id in ids {
            // Removing the entry claims completion; repeated attempts do nothing.
            let Some(pending) = gate.pending.remove(&id) else {
                continue;
            };
            bytes += pending.raw_bytes;
            completed += 1;
            if let Some(callback) = pending.callback {
                jobs.push((id, callback));
            }
        }
        if result.is_ok() {
            gate.totals.succeeded_logs += completed;
            gate.totals.delivered_raw_bytes += bytes as u64;
        } else {
            gate.totals.failed_logs += completed;
        }
        self.raw_bytes.fetch_sub(bytes, Ordering::Relaxed);
        drop(gate);
        // Admission reserved callback capacity. Publication never waits for space
        // and must happen outside the gate (abandoned jobs release their slots).
        if !jobs.is_empty() {
            // One owned result per completed batch, shared even when polling
            // splits its callbacks across multiple EventBatches.
            let result = Arc::new(result);
            let shared = Arc::downgrade(self);
            self.events.extend(
                jobs.into_iter()
                    .map(|(id, callback)| CallbackJob {
                        callback: Some(callback),
                        result: result.clone(),
                        id,
                        shared: shared.clone(),
                    })
                    .collect(),
            );
        }
        self.notify();
    }

    #[cold]
    pub fn fail_pending(self: &Arc<Self>, error: &str) {
        let pending: Vec<_> = {
            let mut gate = self.gate.lock().unwrap();
            gate.state = ProducerState::Closing;
            gate.fatal = Some(error.to_owned());
            gate.pending.keys().copied().collect()
        };
        self.complete(pending, Err(DeliveryError::Internal(error.to_owned())));
    }
}

/// Internal cumulative counters sampled by metrics, independent of flush/close.
#[derive(Clone, Debug, Default)]
pub(crate) struct DeliveryCounters {
    pub accepted_logs: u64,
    pub succeeded_logs: u64,
    pub delivered_raw_bytes: u64,
    pub failed_logs: u64,
}

/// Admission metadata consumed when one log joins a batch; only its ID is retained.
pub(crate) struct Submission {
    pub id: SubmissionId,
    pub admitted: Instant,
    pub raw_bytes: usize,
}

impl Submission {
    pub fn new(id: SubmissionId, raw_bytes: usize) -> Self {
        Self {
            id,
            admitted: Instant::now(),
            raw_bytes,
        }
    }
}

/// Owned only by the pending table until terminal delivery claims it.
pub(crate) struct PendingSubmission {
    raw_bytes: usize,
    callback: Option<Callback>,
}

impl Progress {
    pub fn admit(&mut self, submission: &Submission, callback: Option<Callback>) {
        self.callbacks += usize::from(callback.is_some());
        self.totals.accepted_logs += 1;
        self.pending.insert(
            submission.id,
            PendingSubmission {
                raw_bytes: submission.raw_bytes,
                callback,
            },
        );
    }
}

pub(crate) struct CallbackJob {
    callback: Option<Callback>,
    result: Arc<DeliveryResult>,
    id: SubmissionId,
    shared: Weak<Shared>,
}

impl CallbackJob {
    pub fn run(mut self) {
        let callback = self.callback.take().expect("callback consumed once");
        let id = self.id;
        if catch_unwind(AssertUnwindSafe(|| callback(&self.result))).is_err() {
            log::error!("SLS producer callback panicked for {id:?}");
        }
    }
}

impl Drop for CallbackJob {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            shared.gate.lock().unwrap().callbacks -= 1;
            shared.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::observability::delivery_counts;

    #[test]
    fn batch_callbacks_share_result_across_polls_requeue_and_panic() {
        let shared = Shared::new(1, ProducerConfig::default());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let message = "shared batch failure".repeat(1024);
        let message_pointer = message.as_ptr() as usize;
        for id in 0..130 {
            let seen = seen.clone();
            shared.gate.lock().unwrap().admit(
                &Submission::new(SubmissionId(id), 1),
                Some(Box::new(move |result| {
                    let Err(DeliveryError::Internal(message)) = result else {
                        panic!("expected the batch failure");
                    };
                    seen.lock().unwrap().push((
                        id,
                        result as *const DeliveryResult as usize,
                        message.as_ptr() as usize,
                    ));
                    if id == 0 {
                        panic!("one callback must not prevent the rest from running");
                    }
                })),
            );
        }
        shared.raw_bytes.store(130, Ordering::Relaxed);
        shared.complete(
            (0..130).map(SubmissionId),
            Err(DeliveryError::Internal(message)),
        );

        // Dropping a fetched batch must preserve the shared result on requeue.
        drop(crate::events::poll_batch(&shared, Duration::ZERO).unwrap());
        assert!(seen.lock().unwrap().is_empty());
        for (count, remaining) in [(64, 66), (64, 2), (2, 0)] {
            assert_eq!(
                crate::events::poll_batch(&shared, Duration::ZERO)
                    .unwrap()
                    .dispatch(),
                count
            );
            assert_eq!(shared.gate.lock().unwrap().callbacks, remaining);
        }
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 130);
        for (index, &(id, result_pointer, pointer)) in seen.iter().enumerate() {
            assert_eq!(id, index as u64);
            assert_eq!(result_pointer, seen[0].1);
            // The error String itself was moved into the shared result, not cloned.
            assert_eq!(pointer, message_pointer);
        }
        assert_eq!(shared.gate.lock().unwrap().totals.failed_logs, 130);
        assert_eq!(shared.raw_bytes.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn terminal_submission_is_accounted_once_and_survives_failure_cleanup() {
        let shared = Shared::for_test(1, ProducerConfig::default(), [("project", "store-0")]);
        let completed = Submission::new(SubmissionId(1), 100);
        let pending = Submission::new(SubmissionId(2), 80);
        {
            let mut gate = shared.gate.lock().unwrap();
            gate.admit(&completed, None);
            gate.admit(&pending, None);
        }
        shared.raw_bytes.store(180, Ordering::Relaxed);
        shared.complete([completed.id], Ok(()));
        shared.complete(
            [completed.id],
            Err(DeliveryError::Internal("already finished".into())),
        );
        shared.fail_pending("worker failed");

        assert_eq!(delivery_counts(&shared).succeeded_logs, 1);
        assert_eq!(delivery_counts(&shared).delivered_raw_bytes, 100);
        assert_eq!(delivery_counts(&shared).failed_logs, 1);
        assert!(shared.gate.lock().unwrap().pending.is_empty());
        assert_eq!(shared.raw_bytes.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn batch_and_single_completion_race_account_once_and_release_callbacks() {
        for _ in 0..20 {
            let shared = Shared::new(1, ProducerConfig::default());
            let seen = Arc::new(AtomicUsize::new(0));
            let submissions: Vec<_> = (0..256)
                .map(|i| {
                    let seen = seen.clone();
                    let sub = Submission::new(SubmissionId(i), 100);
                    shared.gate.lock().unwrap().admit(
                        &sub,
                        Some(Box::new(move |_| {
                            seen.fetch_add(1, Ordering::Relaxed);
                        })),
                    );
                    sub
                })
                .collect();
            shared.raw_bytes.store(25600, Ordering::Relaxed);
            std::thread::scope(|scope| {
                scope.spawn(|| shared.complete(submissions.iter().map(|s| s.id), Ok(())));
                scope.spawn(|| {
                    for submission in &submissions {
                        shared.complete([submission.id], Ok(()));
                    }
                });
            });
            shared.complete(submissions.iter().map(|s| s.id), Ok(()));
            shared.fail_pending("already finished");
            assert_eq!(shared.gate.lock().unwrap().callbacks, 256);
            for _ in 0..4 {
                assert_eq!(
                    crate::events::poll_batch(&shared, Duration::ZERO)
                        .unwrap()
                        .dispatch(),
                    64
                );
            }
            let gate = shared.gate.lock().unwrap();
            assert!(gate.pending.is_empty());
            assert_eq!(gate.callbacks, 0);
            assert_eq!(gate.totals.succeeded_logs, 256);
            assert_eq!(gate.totals.failed_logs, 0);
            assert_eq!(shared.raw_bytes.load(Ordering::Relaxed), 0);
            assert_eq!(seen.load(Ordering::Relaxed), 256);
        }
    }
}
