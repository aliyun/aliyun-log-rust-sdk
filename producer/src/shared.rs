use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

use crate::callback::ProducerCallback;
use crate::error::CloseError;
use crate::memory_limiter::MemoryLimiter;
use crate::model::WhenFull;
use crate::stats::{ProducerStats, StatsInner};

pub(crate) const STATE_RUNNING: u8 = 0;
pub(crate) const STATE_CLOSING: u8 = 1;
pub(crate) const STATE_CLOSED: u8 = 2;

// Stats fields use Relaxed ordering; the Notify wake/wait provides the
// happens-before edge that makes drain checks see up-to-date values.
pub(crate) struct Shared {
    pub state: AtomicU8,
    pub when_full: WhenFull,
    pub memory_limiter: Arc<MemoryLimiter>,
    pub stats: Arc<StatsInner>,
    pub notify: Notify,
    pub callback: Option<Arc<dyn ProducerCallback>>,
    pub close_result: Mutex<Option<Result<(), CloseError>>>,
}

impl Shared {
    pub fn new(
        when_full: WhenFull,
        memory_limiter: Arc<MemoryLimiter>,
        stats: Arc<StatsInner>,
        callback: Option<Arc<dyn ProducerCallback>>,
    ) -> Self {
        Self {
            state: AtomicU8::new(STATE_RUNNING),
            when_full,
            memory_limiter,
            stats,
            notify: Notify::new(),
            callback,
            close_result: Mutex::new(None),
        }
    }

    pub fn stats(&self) -> ProducerStats {
        self.stats.snapshot()
    }

    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_RUNNING
    }

    pub fn is_closed(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_CLOSED
    }

    pub fn state(&self) -> u8 {
        self.state.load(Ordering::SeqCst)
    }

    pub async fn wait_until_drained(&self) {
        loop {
            let snapshot = self.stats();
            if snapshot.queued_records == 0 && snapshot.inflight_batches == 0 {
                return;
            }
            self.notify.notified().await;
        }
    }

    pub async fn wait_until_closed(&self) -> Result<(), CloseError> {
        loop {
            if let Some(result) = self.close_result() {
                return result;
            }
            self.notify.notified().await;
        }
    }

    pub fn signal_progress(&self) {
        self.notify.notify_waiters();
    }

    pub fn accept_records(&self, count: usize, total_bytes: usize) {
        self.stats
            .accepted_records
            .fetch_add(count as u64, Ordering::Relaxed);
        self.stats
            .queued_records
            .fetch_add(count, Ordering::Relaxed);
        self.stats
            .queued_bytes
            .fetch_add(total_bytes, Ordering::Relaxed);
    }

    pub fn release_records(&self, count: usize, total_bytes: usize) {
        self.stats
            .queued_records
            .fetch_sub(count, Ordering::Relaxed);
        self.stats
            .queued_bytes
            .fetch_sub(total_bytes, Ordering::Relaxed);
        self.signal_progress();
    }

    pub fn begin_close(&self) -> bool {
        let did_transition = self
            .state
            .compare_exchange(
                STATE_RUNNING,
                STATE_CLOSING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok();
        if did_transition {
            self.signal_progress();
        }
        did_transition
    }

    pub fn finish_close(&self, result: Result<(), CloseError>) {
        let mut slot = self
            .close_result
            .lock()
            .expect("close_result lock poisoned");
        if slot.is_none() {
            *slot = Some(result);
        }
        self.state.store(STATE_CLOSED, Ordering::SeqCst);
        drop(slot);
        self.signal_progress();
    }

    pub fn close_result(&self) -> Option<Result<(), CloseError>> {
        self.close_result
            .lock()
            .expect("close_result lock poisoned")
            .clone()
    }
}
