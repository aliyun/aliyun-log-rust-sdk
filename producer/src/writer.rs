use std::sync::{atomic::Ordering, Arc};

use tokio::sync::mpsc;

use crate::{
    batch::{self, Command, Envelope, Key, PreparedLog},
    registry::WriterTarget,
    state::Submission,
    BaseProducer, Callback, DeliveryResult, Log, ProducerError, ProducerState, SendOptions,
    SubmissionId,
};

/// Cloneable, Send + Sync writer bound to a project/logstore.
/// Obtain a writer with [`crate::Producer::writer`] and share clones across threads.
#[derive(Clone)]
pub struct LogstoreWriter {
    producer: BaseProducer,
    target: Arc<WriterTarget>,
}

impl LogstoreWriter {
    pub(crate) fn new(
        producer: BaseProducer,
        project: &str,
        logstore: &str,
    ) -> Result<Self, ProducerError> {
        let valid = [project, logstore].iter().all(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        });
        if !valid {
            return Err(ProducerError::InvalidInput {
                reason:
                    "project/logstore must be nonempty names containing letters, digits, '-' or '_'"
                        .into(),
            });
        }
        let target = producer
            .inner
            .shared
            .writers
            .get_or_create(project, logstore);
        Ok(Self { producer, target })
    }

    /// Admit one log without waiting for capacity. No Tokio runtime is required.
    /// Accepts a [`Log`] or an application type implementing `Into<Log>`,
    /// typically through `From<MyType> for Log`.
    /// Encoding, network delivery and retries run on the producer's background workers.
    /// `Ok(())` means the input was accepted locally. Use [`Self::send_with_callback`]
    /// when the final delivery outcome is needed. This method creates no callback
    /// job and does not clone the delivery result for notification.
    ///
    /// # Errors
    ///
    /// - [`ProducerError::Closed`]: the producer is no longer accepting logs.
    /// - [`ProducerError::EnqueueFull`]: the input queue is full or soft-buffer
    ///   backpressure prevents immediate admission.
    ///
    /// Every returned error retains the converted [`Log`], recoverable with
    /// [`ProducerError::into_log`]; no input is admitted and no callback is invoked.
    pub fn send(&self, log: impl Into<Log>) -> Result<(), ProducerError> {
        self.send_with_options(log, SendOptions::default())
    }

    /// Send with explicit source/topic options. Acceptance and errors match [`Self::send`].
    pub fn send_with_options(
        &self,
        log: impl Into<Log>,
        options: SendOptions,
    ) -> Result<(), ProducerError> {
        self.enqueue(log.into(), options, None)
    }

    /// Admit one log without waiting for capacity, with a final delivery callback.
    /// No caller runtime is required. Like [`Self::send`], `Ok(())` means local
    /// acceptance; delivery, retries and notification continue in the background.
    ///
    /// Once accepted, polling invokes the closure once with `&DeliveryResult`.
    /// Callbacks for the same completed batch share a result. Clone it to retain
    /// an owned result after the callback returns, for example to send over a channel.
    /// Intermediate retries never invoke it. BaseProducer executes on the poll
    /// caller; Producer uses its single poll thread. Order is unspecified.
    /// The send call never invokes the callback inline. Capture owned context in
    /// the closure; `Sync` is not required. Notification capacity is reserved at
    /// admission and released only after callback execution.
    ///
    /// A callback panic that can unwind is caught and logged; it does
    /// not change the delivery result or retry the callback. Do not block waiting
    /// for this producer from the callback: close (async or blocking) waits for
    /// callbacks to return. Flush waits only for delivery. If a callback dispatches
    /// work elsewhere, close waits for the callback's return, not that external work.
    ///
    /// # Errors
    ///
    /// Returns [`ProducerError::Closed`] or [`ProducerError::EnqueueFull`] with the
    /// converted log, as in [`Self::send`]. Rejection drops the callback without
    /// invoking it. Retrying admission requires a new callback; options can be cloned.
    pub fn send_with_callback(
        &self,
        log: impl Into<Log>,
        callback: impl FnOnce(&DeliveryResult) + Send + 'static,
    ) -> Result<(), ProducerError> {
        self.send_with_options_and_callback(log, SendOptions::default(), callback)
    }

    /// Send with explicit options and a final delivery callback.
    /// Callback behavior and errors match [`Self::send_with_callback`].
    pub fn send_with_options_and_callback(
        &self,
        log: impl Into<Log>,
        options: SendOptions,
        callback: impl FnOnce(&DeliveryResult) + Send + 'static,
    ) -> Result<(), ProducerError> {
        self.enqueue(log.into(), options, Some(Box::new(callback)))
    }

    fn enqueue(
        &self,
        log: Log,
        options: SendOptions,
        callback: Option<Callback>,
    ) -> Result<(), ProducerError> {
        let (size, raw_bytes) = batch::measure_log(&log);
        let prepared = PreparedLog { log, size };
        let shared = &self.producer.inner.shared;
        let permit = match self.producer.inner.tx.try_reserve() {
            Ok(permit) => permit,
            Err(error) => {
                let closed = shared.gate.lock().unwrap().state != ProducerState::Running;
                return Err(
                    if closed || matches!(error, mpsc::error::TrySendError::Closed(_)) {
                        shared.reject(ProducerError::Closed { log: prepared.log })
                    } else {
                        shared.reject(ProducerError::EnqueueFull { log: prepared.log })
                    },
                );
            }
        };
        self.publish(permit, prepared, raw_bytes, options, callback)
    }

    fn publish(
        &self,
        permit: mpsc::Permit<'_, Command>,
        prepared: PreparedLog,
        raw_bytes: usize,
        options: SendOptions,
        callback: Option<Callback>,
    ) -> Result<(), ProducerError> {
        let shared = &self.producer.inner.shared;
        let mut gate = shared.gate.lock().unwrap();
        if gate.state != ProducerState::Running {
            return Err(shared.reject(ProducerError::Closed { log: prepared.log }));
        }
        if callback.is_some() && gate.callbacks >= shared.config.callback_capacity {
            return Err(shared.reject(ProducerError::EnqueueFull { log: prepared.log }));
        }
        if shared.overloaded.load(Ordering::Relaxed) {
            return Err(shared.reject(ProducerError::EnqueueFull { log: prepared.log }));
        }
        let id = SubmissionId(gate.next_id);
        gate.next_id += 1;
        let SendOptions { source, topic } = options;
        let key = Key {
            logstore: self.target.index,
            source,
            topic,
        };
        let submission = Submission::new(id, raw_bytes);
        gate.admit(&submission, callback);
        shared.raw_bytes.fetch_add(raw_bytes, Ordering::Relaxed);
        permit.send(Command::Log(Envelope {
            key,
            submission,
            log: prepared,
        }));
        Ok(())
    }
}
