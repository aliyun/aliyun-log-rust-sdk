use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct MemoryLimiter {
    limit_bytes: usize,
    used_bytes: AtomicUsize,
}

#[derive(Clone)]
pub struct MemoryPermit {
    _inner: Arc<MemoryPermitInner>,
}

struct MemoryPermitInner {
    limiter: Arc<MemoryLimiter>,
    bytes: usize,
}

impl MemoryLimiter {
    pub fn new(limit_bytes: usize) -> Self {
        Self {
            limit_bytes,
            used_bytes: AtomicUsize::new(0),
        }
    }

    pub fn try_acquire(self: &Arc<Self>, bytes: usize) -> Option<MemoryPermit> {
        loop {
            let current = self.used_bytes.load(Ordering::Relaxed);
            let next = current.saturating_add(bytes);
            if next > self.limit_bytes {
                return None;
            }
            if self
                .used_bytes
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return Some(MemoryPermit {
                    _inner: Arc::new(MemoryPermitInner {
                        limiter: self.clone(),
                        bytes,
                    }),
                });
            }
        }
    }

    fn release(&self, bytes: usize) {
        self.used_bytes.fetch_sub(bytes, Ordering::Release);
    }
}

impl Drop for MemoryPermitInner {
    fn drop(&mut self) {
        self.limiter.release(self.bytes);
    }
}
