use std::fmt;

use thiserror::Error;

use crate::model::LogRecord;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BuildError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
}

impl From<aliyun_log_rust_sdk::ConfigError> for BuildError {
    fn from(err: aliyun_log_rust_sdk::ConfigError) -> Self {
        Self::InvalidConfig(err.to_string())
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RecordError {
    #[error("empty field key")]
    EmptyKey,
    #[error("record too large")]
    RecordTooLarge,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SendError {
    #[error("producer is closed")]
    Closed,
    #[error("queue is full")]
    QueueFull,
    #[error("memory limit exceeded")]
    MemoryLimitExceeded,
    #[error("record encode error: {0}")]
    Encode(#[from] RecordError),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TrySendError {
    #[error("producer is closed")]
    Closed,
    #[error("queue is full")]
    QueueFull,
    #[error("memory limit exceeded")]
    MemoryLimitExceeded,
    #[error("record encode error: {0}")]
    Encode(#[from] RecordError),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FlushError {
    #[error("producer is closed")]
    Closed,
    #[error("flush timeout")]
    Timeout,
}

#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum CloseError {
    #[error("close timeout")]
    Timeout,
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Error, Clone)]
#[non_exhaustive]
pub enum DeliveryError {
    #[error("request timeout")]
    Timeout,
    #[error("network error: {0}")]
    Network(String),
    #[error("server error: code={code}, message={message}, request_id={request_id:?}")]
    Server {
        code: String,
        message: String,
        request_id: Option<String>,
        retryable: bool,
        throttled: bool,
    },
    #[error("retriable error exceeded max retries: {last_error}")]
    RetriableExceeded { last_error: Box<DeliveryError> },
    #[error("producer shutdown")]
    Shutdown,
    #[error("internal error: {0}")]
    Internal(String),
}

impl DeliveryError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Timeout | Self::Network(_) => true,
            Self::Server { retryable, .. } => *retryable,
            Self::RetriableExceeded { .. } | Self::Shutdown | Self::Internal(_) => false,
        }
    }

    pub fn is_throttled(&self) -> bool {
        matches!(self, Self::Server { throttled: true, .. })
    }
}

#[derive(Debug)]
pub struct SendErrorWithRecord {
    pub error: SendError,
    pub record: LogRecord,
}

impl SendErrorWithRecord {
    pub(crate) fn new(error: SendError, record: LogRecord) -> Self {
        Self { error, record }
    }

    pub fn into_parts(self) -> (SendError, LogRecord) {
        (self.error, self.record)
    }
}

impl fmt::Display for SendErrorWithRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for SendErrorWithRecord {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[derive(Debug)]
pub struct TrySendErrorWithRecord {
    pub error: TrySendError,
    pub record: LogRecord,
}

impl TrySendErrorWithRecord {
    pub(crate) fn new(error: TrySendError, record: LogRecord) -> Self {
        Self { error, record }
    }

    pub fn into_parts(self) -> (TrySendError, LogRecord) {
        (self.error, self.record)
    }
}

impl fmt::Display for TrySendErrorWithRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for TrySendErrorWithRecord {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
