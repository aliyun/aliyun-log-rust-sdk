use std::{
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use futures_util::{future::try_join, FutureExt};
use tokio::sync::mpsc;

use crate::{batch::Command, state::Shared, ProducerError, ProducerState};

mod aggregator;
mod dispatcher;
mod retry;
mod sender;

use aggregator::Aggregator;
use dispatcher::Dispatcher;
pub(crate) use sender::Transport;
use sender::{BatchSender, ClientTransport};

pub(crate) fn start(
    shared: Arc<Shared>,
    input: mpsc::Receiver<Command>,
    client_config: aliyun_log_rust_sdk::Config,
) -> Result<(), ProducerError> {
    launch(shared, input, move || {
        ClientTransport::new(client_config).map(|client| Arc::new(client) as Arc<dyn Transport>)
    })
}

/// Thread bootstrap is separate so synchronous callers can await local initialization.
/// Tests supply transports here without changing runtime ownership or task supervision.
pub(crate) fn launch(
    shared: Arc<Shared>,
    input: mpsc::Receiver<Command>,
    transport: impl FnOnce() -> Result<Arc<dyn Transport>, String> + Send + 'static,
) -> Result<(), ProducerError> {
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name(format!("sls-{}-io", shared.owner))
        .spawn(move || {
            let producer = match ProducerRuntime::new(shared, transport) {
                Ok(producer) => producer,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            if ready_tx.send(Ok(())).is_ok() {
                producer.run(input);
            }
        })
        .map_err(|error| ProducerError::Creation(error.to_string()))?;
    ready_rx
        .recv()
        .map_err(|_| ProducerError::Creation("initialization thread exited".into()))?
        .map_err(ProducerError::Creation)
}

/// Owns the fixed execution resources from initialization through shutdown.
/// Batch state belongs to Aggregator; per-request processing belongs to BatchSender.
struct ProducerRuntime {
    shared: Arc<Shared>,
    transport: Arc<dyn Transport>,
    processing: ProcessingPool,
    runtime: tokio::runtime::Runtime,
}

impl ProducerRuntime {
    fn new(
        shared: Arc<Shared>,
        transport: impl FnOnce() -> Result<Arc<dyn Transport>, String>,
    ) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(shared.runtime_config.max_blocking_threads)
            .thread_name(format!("sls-{}-helper", shared.owner))
            .build()
            .map_err(|error| error.to_string())?;
        let transport = transport()?;
        let processing = ProcessingPool::new(shared.owner, shared.config.processing_workers)?;
        Ok(Self {
            shared,
            transport,
            processing,
            runtime,
        })
    }

    fn run(self, input: mpsc::Receiver<Command>) {
        let shared = self.shared.clone();
        let mut metrics = crate::metrics::ProgressReporter::new(&shared);
        self.runtime.block_on(async {
            let pipeline = async {
                let result = AssertUnwindSafe(self.run_pipeline(input))
                    .catch_unwind()
                    .await;
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error),
                    Err(_) => Some("producer pipeline panicked".to_owned()),
                };
                if let Some(error) = error {
                    log::error!("SLS producer stopped: {error}");
                    // The external poller remains available when delivery tasks fail.
                    shared.fail_pending(&error);
                }

                // Keep the runtime (and sampling) alive while callbacks drain.
                // The external poller releases callback slots only after dispatch.
                let mut changes = shared.changes.subscribe();
                while shared.gate.lock().unwrap().callbacks != 0 {
                    changes.changed().await.expect("shared owns watch sender");
                }
            };
            tokio::pin!(pipeline);
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = &mut pipeline => break,
                    _ = tick.tick() => metrics.record(),
                }
            }
            // Publish even short-lived producers and failures before close completes.
            metrics.record();
        });

        // All async workers have released their resource handles before joining pools.
        drop(self.transport);
        drop(self.processing); // Join every Rayon thread, including already running jobs.
        drop(self.runtime); // Wait for any Tokio helper threads too.
        self.shared.refresh_pressure();
        self.shared.gate.lock().unwrap().state = ProducerState::Closed;
        self.shared.notify();
    }

    async fn run_pipeline(&self, input: mpsc::Receiver<Command>) -> Result<(), String> {
        let (batch_tx, batch_rx) = flume::bounded(self.shared.runtime_config.batch_capacity);
        let sender = BatchSender::new(
            self.shared.clone(),
            self.transport.clone(),
            self.processing.pool(),
        );
        let aggregator = Aggregator::new(self.shared.clone(), input, batch_tx);
        let dispatcher = Dispatcher::new(self.shared.clone(), batch_rx);
        // A failed branch drops the other future; the outer unwind boundary
        // does the same for panics before pending submissions are completed.
        try_join(aggregator.run(), async move {
            dispatcher.run(sender).await;
            Ok::<(), String>(())
        })
        .await?;
        Ok(())
    }
}

struct ProcessingPool {
    pool: Option<Arc<rayon::ThreadPool>>,
    threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl ProcessingPool {
    fn new(owner: u64, count: usize) -> Result<Self, String> {
        let threads = Arc::new(Mutex::new(Vec::with_capacity(count)));
        let handles = threads.clone();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(count)
            .spawn_handler(move |thread| {
                let handle = std::thread::Builder::new()
                    .name(format!("sls-{owner}-process-{}", thread.index()))
                    .spawn(move || {
                        thread.run();
                    })?;
                handles.lock().unwrap().push(handle);
                Ok(())
            })
            .build();
        let mut result = Self {
            pool: None,
            threads,
        };
        result.pool = Some(Arc::new(pool.map_err(|e| e.to_string())?));
        Ok(result)
    }
    fn pool(&self) -> Arc<rayon::ThreadPool> {
        self.pool.as_ref().unwrap().clone()
    }
}

impl Drop for ProcessingPool {
    fn drop(&mut self) {
        self.pool.take();
        for handle in self.threads.lock().unwrap().drain(..) {
            let _ = handle.join();
        }
    }
}
