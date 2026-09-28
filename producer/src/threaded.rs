use crate::{state::Shared, BaseProducer, LogstoreWriter, ProducerConfig, ProducerError};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

/// Thread-safe producer with automatic batching, retries and callback dispatch.
///
/// Create one producer per endpoint and credentials configuration, then obtain
/// cloneable writers with [`Self::writer`]. Sending requires no caller runtime;
/// acceptance is local, and callbacks report the final delivery outcome.
///
/// Callbacks execute serially on one background thread. Clones and writers share
/// admission and lifecycle: closing any clone stops admission for all of them.
/// Explicitly complete [`Self::close`] or [`Self::close_blocking`] before exit;
/// dropping handles only requests best-effort background shutdown.
///
/// Async lifecycle waits work on any executor. Blocking variants need no runtime.
/// Delivery is unordered, retries may duplicate logs, and no disk persistence is used.
#[derive(Clone)]
pub struct Producer {
    pub(crate) base: BaseProducer,
}
impl Producer {
    /// Create a producer using the supplied configuration.
    /// No network requests or credential fetches are made during creation.
    /// Install the application metrics Recorder before this call; handles are cached.
    ///
    /// # Errors
    ///
    /// - [`ProducerError::Config`]: invalid configuration.
    /// - [`ProducerError::Creation`]: local resource initialization failed.
    ///
    /// Successful creation confirms local initialization. It does not check endpoint
    /// reachability, access-key validity, permissions or remote logstore existence;
    /// those failures are reported later through [`crate::DeliveryResult`].
    pub fn create(config: ProducerConfig) -> Result<Self, ProducerError> {
        Self::from_base(BaseProducer::create(config)?)
    }
    pub(crate) fn from_base(base: BaseProducer) -> Result<Self, ProducerError> {
        let shared = &base.inner.shared;
        shared.poll_worker.start(shared)?;
        Ok(Self { base })
    }
    /// Get a writer for a project/logstore on this producer's endpoint.
    /// No service request is made. Writers can be cloned and shared across threads.
    ///
    /// # Errors
    ///
    /// Invalid project/logstore names return [`ProducerError::InvalidInput`]
    /// immediately, with no logs attached. Remote existence and permissions are
    /// checked during delivery. Obtaining a writer after close does not reopen admission.
    pub fn writer(&self, project: &str, logstore: &str) -> Result<LogstoreWriter, ProducerError> {
        self.base.writer(project, logstore)
    }
    /// Wait indefinitely for delivery of submissions admitted before this call.
    /// Later submissions may continue while this waits. Does not wait for callbacks
    /// or report individual delivery failures; inspect those in delivery callbacks.
    ///
    /// # Errors
    ///
    /// Returns [`ProducerError::Internal`] if the pipeline failed, or
    /// [`ProducerError::Reentrant`] when awaited from this producer's callback.
    pub async fn flush(&self) -> Result<(), ProducerError> {
        self.base.flush().await
    }
    /// Blocking equivalent of [`Self::flush`], with the same completion and errors.
    /// Requires no caller runtime; IO continues on the producer's background workers.
    pub fn flush_blocking(&self) -> Result<(), ProducerError> {
        self.base.flush_blocking()
    }
    /// Stop admission and wait indefinitely for delivery, callbacks and worker shutdown.
    /// Idempotent and shared by all clones and writers. The poll thread dispatches
    /// callbacks; this method never executes them on the calling thread.
    ///
    /// Individual delivery failures are reported through callbacks, not this result.
    /// A callback that never returns prevents close from completing. Do not hold
    /// application locks needed by callbacks while waiting.
    ///
    /// # Errors
    ///
    /// Returns [`ProducerError::Internal`] after a pipeline failure and cleanup, or
    /// [`ProducerError::Reentrant`] if awaited from this producer's callback.
    pub async fn close(&self) -> Result<(), ProducerError> {
        let shared = &self.base.inner.shared;
        shared.check_callback()?;
        self.base.begin_close();
        let result = self.base.wait_closed().await;
        shared.poll_worker.join(shared).await;
        result
    }
    /// Blocking equivalent of [`Self::close`], including callback and worker draining.
    /// Requires no caller runtime. Returns the same errors as async close.
    pub fn close_blocking(&self) -> Result<(), ProducerError> {
        crate::blocking::wait(self.close())
    }
}

/// Automatic callback dispatch owns its thread lifecycle independently of the queue.
/// Workers retain Shared only, so the last user handle can still initiate shutdown.
#[derive(Default)]
pub(crate) struct PollWorker {
    automatic: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
    done: AtomicBool,
}

impl PollWorker {
    pub(crate) fn is_automatic(&self) -> bool {
        self.automatic.load(Ordering::Relaxed)
    }

    fn start(&self, shared: &Arc<Shared>) -> Result<(), ProducerError> {
        let lease = crate::events::PollLease::acquire(shared)?;
        self.automatic.store(true, Ordering::Relaxed);
        let shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name(format!("sls-{}-poll", shared.owner))
            .spawn(move || {
                let _lease = lease;
                loop {
                    crate::events::dispatch_wait(&shared, Duration::from_millis(50));
                    if shared.gate.lock().unwrap().state == crate::ProducerState::Closed {
                        break;
                    }
                }
                shared.poll_worker.done.store(true, Ordering::Release);
                shared.notify();
            });
        match worker {
            Ok(worker) => *self.worker.lock().unwrap() = Some(worker),
            Err(error) => {
                self.automatic.store(false, Ordering::Relaxed);
                return Err(ProducerError::Creation(error.to_string()));
            }
        }
        Ok(())
    }

    async fn join(&self, shared: &Shared) {
        let mut changes = shared.changes.subscribe();
        while !self.done.load(Ordering::Acquire) {
            changes.changed().await.expect("shared owns watch sender");
        }
        // Concurrent close calls all wait for the joining caller to finish.
        let mut worker = self.worker.lock().unwrap();
        if let Some(worker) = worker.take() {
            let _ = worker.join();
        }
    }
}
