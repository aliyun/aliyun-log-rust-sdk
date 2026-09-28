//! A thread-safe asynchronous SLS producer with automatic batching and retries.
//!
//! Create a producer for one endpoint, clone writers across threads, then explicitly
//! complete `close` or `close_blocking` before process exit. Admission is not an acknowledgement from SLS.
//! Delivery is unordered and retries may duplicate logs. No disk persistence is used.
//!
//! `send` does not wait for capacity and can be used from synchronous threads.
//! Async flush and close can run on any executor.
//! `flush_blocking` and `close_blocking` need no caller runtime.
//!
//! Metrics use the `metrics` facade. Install your application Recorder before
//! `Producer::create`; counter handles are cached across worker threads.
//! Without a Recorder, instruments are no-op. The library installs no exporter.
//!
//! See the crate README for configuration, lifecycle, and callback semantics.

mod batch;
mod blocking;
mod compression;
mod config;
mod delivery_error;
mod error;
mod events;
mod logs;
mod metrics;
mod pack_id;
mod registry;
mod runtime;
mod state;
mod threaded;
mod types;
mod writer;

pub use aliyun_log_rust_sdk::{Credentials, CredentialsError, CredentialsProvider};
pub use aliyun_log_sdk_protobuf::Log;
pub use compression::Compression;
pub use config::ProducerConfig;
pub use delivery_error::DeliveryError;
pub use error::ProducerError;
pub use events::EventBatch;
pub use logs::{log, log_at, log_now, IntoLog};
pub use threaded::ThreadedProducer;

/// Producer with automatic batching, retries and delivery callbacks.
/// Create with [`Producer::create`] and close explicitly before exiting.
pub type Producer = ThreadedProducer;
pub use types::*;
pub use writer::LogstoreWriter;

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::sync::mpsc;

use batch::Command;
use state::Shared;

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

struct Frontend {
    shared: Arc<Shared>,
    tx: mpsc::Sender<Command>,
}

impl Drop for Frontend {
    fn drop(&mut self) {
        // Background work owns Shared, not Frontend, so dropping the last user handle
        // starts draining without blocking the dropping thread or relying on Arc counts.
        self.shared.begin_close();
        if !self.shared.poll_worker.is_automatic() {
            self.shared.events.abandon();
        }
    }
}

/// Low-level producer for bindings and application-owned event dispatch.
/// Prefer [`Producer`] for automatic callbacks. `poll_batch` retrieves
/// events without running callbacks. To shut down, call `begin_close`, keep consuming
/// and dispatching batches until `is_closed`, then obtain the result with `wait_closed`.
/// Stopping admission through any clone affects every writer and clone.
#[derive(Clone)]
pub struct BaseProducer {
    inner: Arc<Frontend>,
}

impl BaseProducer {
    /// Create a producer with manual event dispatch.
    /// See [`Producer::create`] for validation and creation errors.
    pub fn create(config: ProducerConfig) -> Result<BaseProducer, ProducerError> {
        config.validate()?;
        let client_config = config.client_config()?;
        let shared = Shared::new(NEXT_OWNER.fetch_add(1, Ordering::Relaxed), config);
        let (tx, rx) = mpsc::channel(shared.runtime_config.input_capacity);
        runtime::start(shared.clone(), rx, client_config)?;
        Ok(BaseProducer {
            inner: Arc::new(Frontend { shared, tx }),
        })
    }

    /// Obtain a writer; see [`Producer::writer`] for validation semantics.
    pub fn writer(&self, project: &str, logstore: &str) -> Result<LogstoreWriter, ProducerError> {
        LogstoreWriter::new(self.clone(), project, logstore)
    }

    /// Wait for prior delivery, without dispatching callbacks.
    /// See [`Producer::flush`] for completion and error semantics.
    pub async fn flush(&self) -> Result<(), ProducerError> {
        self.inner.shared.check_callback()?;
        let shared = &self.inner.shared;
        let mut changes = shared.changes.subscribe();
        let watermark = loop {
            {
                let gate = shared.gate.lock().unwrap();
                if gate.state != ProducerState::Running {
                    break SubmissionId(gate.next_id - 1);
                }
            }
            tokio::select! {
                permit = self.inner.tx.reserve() => {
                    let gate = shared.gate.lock().unwrap();
                    let watermark = SubmissionId(gate.next_id - 1);
                    if gate.state == ProducerState::Running {
                        match permit {
                            Ok(permit) => permit.send(Command::Flush),
                            Err(_) => return Err(ProducerError::Internal("aggregation queue disconnected".into())),
                        }
                    }
                    break watermark;
                }
                _ = changes.changed() => {}
            }
        };
        shared.wait_delivery(watermark).await
    }

    /// Blocking flush, without requiring a caller runtime.
    pub fn flush_blocking(&self) -> Result<(), ProducerError> {
        blocking::wait(self.flush())
    }

    /// Stop admission without waiting. An external event consumer must keep polling
    /// until `is_closed()` becomes true. Useful to implement language bindings.
    pub fn begin_close(&self) {
        self.inner.shared.begin_close();
    }

    /// True after delivery, callbacks and background resources have drained.
    pub fn is_closed(&self) -> bool {
        self.inner.shared.gate.lock().unwrap().state == ProducerState::Closed
    }

    /// Wait for an external event consumer to finish shutdown; does not poll.
    /// Call `begin_close()` first and keep that consumer running throughout this wait.
    pub async fn wait_closed(&self) -> Result<(), ProducerError> {
        self.inner.shared.check_callback()?;
        let shared = &self.inner.shared;
        let mut changes = shared.changes.subscribe();
        loop {
            {
                let gate = shared.gate.lock().unwrap();
                if gate.state == ProducerState::Closed {
                    return gate
                        .fatal
                        .as_ref()
                        .map_or(Ok(()), |error| Err(ProducerError::Internal(error.clone())));
                }
            }
            changes.changed().await.expect("shared owns watch sender");
        }
    }

    /// Blocking equivalent of `wait_closed`, for wrappers with their own poll thread.
    pub fn wait_closed_blocking(&self) -> Result<(), ProducerError> {
        blocking::wait(self.wait_closed())
    }

    /// Wait without invoking user code. The owned batch holds the single-consumer
    /// lease until dispatched or dropped. Dropping it returns undispatched events to
    /// the queue; dequeueing alone never completes a callback.
    pub fn poll_batch(&self, timeout: Duration) -> Result<EventBatch, ProducerError> {
        events::poll_batch(&self.inner.shared, timeout)
    }
}

#[cfg(test)]
mod tests;

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme {}
