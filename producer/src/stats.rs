use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Debug, Default)]
pub(crate) struct StatsInner {
    pub queued_records: AtomicUsize,
    pub queued_bytes: AtomicUsize,
    pub inflight_batches: AtomicUsize,
    pub accepted_records: AtomicU64,
    pub sent_records: AtomicU64,
    pub sent_batches: AtomicU64,
    pub failed_batches: AtomicU64,
    pub retry_count: AtomicU64,
}

#[derive(Debug, Clone, Default)]
pub struct ProducerStats {
    pub queued_records: usize,
    pub queued_bytes: usize,
    pub inflight_batches: usize,
    pub accepted_records: u64,
    pub sent_records: u64,
    pub sent_batches: u64,
    pub failed_batches: u64,
    pub retry_count: u64,
}

impl StatsInner {
    pub fn snapshot(&self) -> ProducerStats {
        ProducerStats {
            queued_records: self.queued_records.load(Ordering::Relaxed),
            queued_bytes: self.queued_bytes.load(Ordering::Relaxed),
            inflight_batches: self.inflight_batches.load(Ordering::Relaxed),
            accepted_records: self.accepted_records.load(Ordering::Relaxed),
            sent_records: self.sent_records.load(Ordering::Relaxed),
            sent_batches: self.sent_batches.load(Ordering::Relaxed),
            failed_batches: self.failed_batches.load(Ordering::Relaxed),
            retry_count: self.retry_count.load(Ordering::Relaxed),
        }
    }
}
