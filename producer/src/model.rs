use std::time::{Duration, Instant, SystemTime};

use tokio::sync::oneshot;

use crate::memory_limiter::MemoryPermit;
use crate::{CloseError, DeliveryError, FlushError};

#[derive(Debug, Clone)]
pub struct LogField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct LogRecord {
    pub timestamp: SystemTime,
    pub fields: Vec<LogField>,
}

impl LogRecord {
    pub fn new(timestamp: SystemTime) -> Self {
        Self {
            timestamp,
            fields: Vec::new(),
        }
    }

    pub fn field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push(LogField {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    pub fn estimated_bytes(&self) -> usize {
        let mut bytes = 16;
        for field in &self.fields {
            bytes += field.key.len() + field.value.len() + 8;
        }
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WhenFull {
    Block,
    ReturnError,
}

#[derive(Debug)]
#[must_use = "call .wait() to receive the delivery result"]
pub struct AckHandle {
    pub(crate) rx: oneshot::Receiver<Result<DeliveryReport, DeliveryError>>,
}

impl AckHandle {
    pub async fn wait(self) -> Result<DeliveryReport, DeliveryError> {
        self.rx.await.unwrap_or(Err(DeliveryError::Shutdown))
    }
}

#[derive(Debug, Clone)]
pub struct DeliveryReport {
    pub batch_id: u64,
    pub record_count: usize,
    pub encoded_bytes: usize,
    pub retry_count: usize,
    pub elapsed: Duration,
    pub request_id: Option<String>,
}

pub(crate) struct RecordEnvelope {
    pub record: LogRecord,
    pub estimated_bytes: usize,
    pub _permit: MemoryPermit,
    pub ack: Option<oneshot::Sender<Result<DeliveryReport, DeliveryError>>>,
}

pub(crate) enum IngressMessage {
    Record(RecordEnvelope),
    Records(Vec<RecordEnvelope>),
    Flush(oneshot::Sender<Result<(), FlushError>>),
    Close(oneshot::Sender<Result<(), CloseError>>),
}

pub(crate) struct Batch {
    pub id: u64,
    pub created_at: Instant,
    pub records: Vec<RecordEnvelope>,
    pub total_bytes: usize,
}

impl Batch {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            created_at: Instant::now(),
            records: Vec::new(),
            total_bytes: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn push(&mut self, envelope: RecordEnvelope) {
        self.total_bytes += envelope.estimated_bytes;
        self.records.push(envelope);
    }
}
