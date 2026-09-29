use std::{future::Future, sync::Arc};

use futures_util::{stream::FuturesUnordered, StreamExt};
use tokio_util::time::DelayQueue;

use super::sender::{AttemptResult, BatchSender, PendingBatch, Work};
use crate::{batch::Batch, state::Shared};

/// Owns both active attempts and delayed retries. A batch keeps its capacity slot
/// when moving between them, so retries never need to acquire additional capacity.
pub(super) struct Dispatcher {
    shared: Arc<Shared>,
    input: flume::Receiver<Batch>,
    delayed: DelayQueue<PendingBatch>,
}

impl Dispatcher {
    pub(super) fn new(shared: Arc<Shared>, input: flume::Receiver<Batch>) -> Self {
        Self {
            shared,
            input,
            delayed: DelayQueue::new(),
        }
    }

    pub(super) async fn run(self, sender: BatchSender) {
        self.run_with(|work| sender.process(work)).await
    }

    // A processing function also lets tests control attempt results and backoff
    // deterministically, without transport mocks or changes to the production policy.
    async fn run_with<F, Fut>(mut self, mut process: F)
    where
        F: FnMut(Work) -> Fut,
        Fut: Future<Output = AttemptResult>,
    {
        let mut active = FuturesUnordered::new();
        let mut input_closed = false;
        loop {
            let outstanding = active.len() + self.delayed.len();
            if input_closed && outstanding == 0 {
                return;
            }
            tokio::select! {
                result = active.next(), if !active.is_empty() => {
                    match result.expect("nonempty active set") {
                        AttemptResult::Finished => {}
                        AttemptResult::Retry { batch, ready_at } => {
                            self.delayed.insert_at(batch, tokio::time::Instant::from_std(ready_at));
                        }
                    }
                }
                expired = self.delayed.next(), if !self.delayed.is_empty() => {
                    let batch = expired.expect("nonempty delay queue").into_inner();
                    active.push(process(Work::Retry(batch)));
                }
                batch = self.input.recv_async(), if !input_closed
                    && outstanding < self.shared.runtime_config.max_inflight_batches => {
                    match batch {
                        Ok(batch) => active.push(process(Work::Fresh(batch))),
                        Err(_) => input_closed = true,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        batch::{Batch, Key},
        ProducerConfig,
    };
    use std::time::{Duration, Instant};

    fn fresh() -> Batch {
        Batch::new(
            Key {
                logstore: 0,
                source: String::new(),
                topic: String::new(),
            },
            Instant::now(),
        )
    }

    // Controlled worker replies keep this independent of randomized HTTP backoff.
    #[tokio::test(start_paused = true)]
    async fn retries_reserve_capacity_and_drain_on_close() {
        let mut shared = Shared::for_test(1, ProducerConfig::default(), [("project", "store-0")]);
        Arc::get_mut(&mut shared)
            .unwrap()
            .runtime_config
            .max_inflight_batches = 2;
        let (input, input_rx) = flume::bounded(4);
        let (work_tx, work) = flume::bounded(1);
        let (results, result_rx) = flume::bounded(1);
        let task = tokio::spawn(
            Dispatcher::new(shared.clone(), input_rx).run_with(move |work| {
                let work_tx = work_tx.clone();
                let result_rx = result_rx.clone();
                async move {
                    work_tx.send_async(work).await.unwrap();
                    result_rx.recv_async().await.unwrap()
                }
            }),
        );
        input.send(fresh()).unwrap();
        let Work::Fresh(first) = work.recv_async().await.unwrap() else {
            panic!("expected fresh batch")
        };
        results
            .send(AttemptResult::Retry {
                batch: PendingBatch::for_test(first, &shared),
                ready_at: (tokio::time::Instant::now() + Duration::from_secs(10)).into_std(),
            })
            .unwrap();
        input.send(fresh()).unwrap();
        let Work::Fresh(second) = tokio::time::timeout(Duration::from_secs(1), work.recv_async())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("a delayed retry blocked fresh work despite free capacity")
        };
        results
            .send(AttemptResult::Retry {
                batch: PendingBatch::for_test(second, &shared),
                ready_at: (tokio::time::Instant::now() + Duration::from_secs(20)).into_std(),
            })
            .unwrap();
        input.send(fresh()).unwrap();
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }

        assert!(work.is_empty());
        assert_eq!(input.len(), 1, "capacity must backpressure fresh batches");
        drop(input); // Shutdown must keep processing retries and upstream queued batches.
        tokio::time::advance(Duration::from_secs(10)).await;
        assert!(matches!(work.recv_async().await.unwrap(), Work::Retry(_)));
        results.send(AttemptResult::Finished).unwrap();
        assert!(matches!(work.recv_async().await.unwrap(), Work::Fresh(_)));
        results.send(AttemptResult::Finished).unwrap();
        tokio::time::advance(Duration::from_secs(10)).await;
        assert!(matches!(work.recv_async().await.unwrap(), Work::Retry(_)));
        results.send(AttemptResult::Finished).unwrap();
        task.await.unwrap();

        assert!(work.recv_async().await.is_err());
    }
}

#[cfg(test)]
mod failure_tests {
    use super::*;
    use crate::{batch::Key, ProducerConfig};
    use futures_util::FutureExt;
    use std::{
        sync::atomic::{AtomicBool, Ordering::Relaxed},
        time::Instant,
    };

    struct DropProbe(Arc<AtomicBool>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.store(true, Relaxed);
        }
    }

    #[tokio::test]
    async fn processing_failure_drops_other_attempts() {
        let shared = Shared::for_test(1, ProducerConfig::default(), [("project", "store-0")]);
        let (tx, rx) = flume::bounded(2);
        for _ in 0..2 {
            tx.send(Batch::new(
                Key {
                    logstore: 0,
                    source: String::new(),
                    topic: String::new(),
                },
                Instant::now(),
            ))
            .unwrap();
        }
        drop(tx);
        let dropped = Arc::new(AtomicBool::new(false));
        let mut first = true;
        let result =
            std::panic::AssertUnwindSafe(Dispatcher::new(shared.clone(), rx).run_with(|_| {
                let probe = first.then(|| DropProbe(dropped.clone()));
                first = false;
                async move {
                    if let Some(probe) = probe {
                        let _probe = probe;
                        std::future::pending::<()>().await;
                    }
                    panic!("processing failed")
                }
            }))
            .catch_unwind()
            .await;
        assert!(result.is_err());
        assert!(dropped.load(Relaxed));
    }
}
