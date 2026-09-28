use crate::{
    state::{CallbackJob, Shared},
    ProducerError,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct Queue {
    delivery: VecDeque<CallbackJob>,
    abandoned: bool,
}
#[derive(Default)]
pub(crate) struct EventQueue {
    queue: Mutex<Queue>,
    wake: Condvar,
    polling: AtomicBool,
}
impl EventQueue {
    pub fn extend(&self, jobs: Vec<CallbackJob>) {
        let mut queue = self.queue.lock().unwrap();
        if queue.abandoned {
            drop(queue);
            drop(jobs);
            return;
        }
        queue.delivery.extend(jobs);
        self.wake.notify_one();
    }
    pub fn abandon(&self) {
        let mut queue = self.queue.lock().unwrap();
        queue.abandoned = true;
        let jobs = std::mem::take(&mut queue.delivery);
        drop(queue);
        drop(jobs);
        self.wake.notify_all();
    }
    fn take(&self, timeout: Duration) -> Vec<CallbackJob> {
        let queue = self.queue.lock().unwrap();
        let (mut queue, _) = self
            .wake
            .wait_timeout_while(queue, timeout, |q| q.delivery.is_empty() && !q.abandoned)
            .unwrap();
        // Bound callback work per poll so consumers can interleave other work.
        let count = queue.delivery.len().min(64);
        queue.delivery.drain(..count).collect()
    }
}

pub(crate) struct PollLease(Arc<Shared>);
impl PollLease {
    pub fn acquire(shared: &Arc<Shared>) -> Result<Self, ProducerError> {
        shared.check_callback()?;
        shared
            .events
            .polling
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| ProducerError::PollBusy)?;
        Ok(Self(shared.clone()))
    }
}
impl Drop for PollLease {
    fn drop(&mut self) {
        self.0.events.polling.store(false, Ordering::Release);
    }
}

/// Owned batch, safe to fetch while a language runtime lock is released and then
/// dispatch after reacquiring it. Retaining the batch blocks other polls and holds its callbacks pending shutdown.
/// Dropping an undispatched batch requeues deliveries without executing user code.
///
/// Unstable and subject to change. Not intended for downstream application use.
#[doc(hidden)]
pub struct EventBatch {
    events: Vec<CallbackJob>,
    lease: PollLease,
}
impl EventBatch {
    /// Invoke delivery callbacks on this thread.
    /// Panics that unwind are isolated. Return values are ignored; completion means
    /// the synchronous invocation returned, not that externally scheduled work ended.
    pub fn dispatch(mut self) -> usize {
        let count = self.events.len();
        dispatch(self.events.drain(..), &self.lease.0);
        count
    }
}
impl Drop for EventBatch {
    fn drop(&mut self) {
        let mut queue = self.lease.0.events.queue.lock().unwrap();
        if queue.abandoned {
            return;
        }
        for job in self.events.drain(..).rev() {
            queue.delivery.push_front(job);
        }
        self.lease.0.events.wake.notify_one();
    }
}

pub(crate) fn poll_batch(
    shared: &Arc<Shared>,
    timeout: Duration,
) -> Result<EventBatch, ProducerError> {
    let lease = PollLease::acquire(shared)?;
    let events = shared.events.take(timeout);
    Ok(EventBatch { events, lease })
}
pub(crate) fn dispatch_wait(shared: &Arc<Shared>, timeout: Duration) {
    dispatch(shared.events.take(timeout), shared);
}
fn dispatch(events: impl IntoIterator<Item = CallbackJob>, shared: &Shared) {
    let _guard = CallbackGuard::enter(shared.owner);
    for job in events {
        job.run();
    }
}
thread_local! { static CALLBACKS: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) }; }
pub(crate) struct CallbackGuard;
impl CallbackGuard {
    fn enter(owner: u64) -> Self {
        CALLBACKS.with(|c| c.borrow_mut().push(owner));
        Self
    }
}
impl Drop for CallbackGuard {
    fn drop(&mut self) {
        CALLBACKS.with(|c| c.borrow_mut().pop());
    }
}
impl Shared {
    pub fn check_callback(&self) -> Result<(), ProducerError> {
        if CALLBACKS.with(|c| c.borrow().contains(&self.owner)) {
            Err(ProducerError::Reentrant)
        } else {
            Ok(())
        }
    }
}
