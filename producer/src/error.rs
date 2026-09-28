use std::fmt;

use crate::Log;

/// Errors from producer/writer creation, admission, and flush/close waiting.
/// Admission errors retain the rejected log; callbacks use [`crate::DeliveryError`].
#[derive(Clone, thiserror::Error)]
#[non_exhaustive]
pub enum ProducerError {
    // Configuration and creation.
    /// Invalid configuration, returned by [`crate::Producer::create`].
    #[error("invalid producer configuration: {0}")]
    Config(String),

    /// Local resource initialization failed in [`crate::Producer::create`].
    #[error("producer creation failed: {0}")]
    Creation(String),

    // Admission — no input is accepted and the callback is not invoked.
    /// The producer is closing, closed, or no longer accepting logs.
    #[error("producer is closing or closed")]
    Closed {
        /// The original rejected log.
        log: Log,
    },

    /// Admission capacity is unavailable. Send returns immediately without accepting the log.
    #[error("producer has no available admission capacity")]
    EnqueueFull {
        /// The original rejected log.
        log: Log,
    },

    /// Invalid project/logstore names at writer creation.
    #[error("invalid input: {reason}")]
    InvalidInput {
        /// The reason the input was rejected.
        reason: String,
    },

    /// Another poll batch owns the single event consumer.
    #[error("producer event consumer is already polling or dispatching")]
    PollBusy,
    /// Waiting from this producer's callback would prevent progress.
    #[error("cannot wait for or poll this producer from its callback")]
    Reentrant,

    /// An internal producer operation failed.
    #[error("internal producer error: {0}")]
    Internal(String),
}

impl ProducerError {
    /// Borrow the original rejected log. Non-admission errors return `None`.
    pub fn log(&self) -> Option<&Log> {
        match self {
            Self::Closed { log } | Self::EnqueueFull { log } => Some(log),
            _ => None,
        }
    }

    /// Recover the original rejected log without cloning. Non-admission errors
    /// return `None`.
    pub fn into_log(self) -> Option<Log> {
        match self {
            Self::Closed { log } | Self::EnqueueFull { log } => Some(log),
            _ => None,
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Config(_) => "Config",
            Self::Creation(_) => "Creation",
            Self::Closed { .. } => "Closed",
            Self::EnqueueFull { .. } => "EnqueueFull",
            Self::InvalidInput { .. } => "InvalidInput",
            Self::PollBusy => "PollBusy",
            Self::Reentrant => "Reentrant",
            Self::Internal(_) => "Internal",
        }
    }
}

// Keep rejected log contents out of Debug, including when errors are unwrapped.
impl fmt::Debug for ProducerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct(self.name());
        if let Self::InvalidInput { reason, .. } = self {
            debug.field("reason", reason);
        } else {
            debug.field("message", &format_args!("{self}"));
        }
        debug.finish()
    }
}
