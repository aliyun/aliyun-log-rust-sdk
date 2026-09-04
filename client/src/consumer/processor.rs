use std::{error::Error, future::Future, pin::Pin, sync::Arc};

use super::{CheckpointTracker, LogGroup};

/// A cheaply cloneable batch shared across processor retries.
pub type LogGroupBatch = Arc<[LogGroup]>;
pub type BoxProcessorError = Box<dyn Error + Send + Sync + 'static>;
pub type ProcessorResult<T = ProcessOutcome> = std::result::Result<T, BoxProcessorError>;
pub type ProcessorFuture<'a, T = ProcessOutcome> =
    Pin<Box<dyn Future<Output = ProcessorResult<T>> + Send + 'a>>;

/// Action to take after a batch has been processed successfully.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ProcessOutcome {
    /// Continue from the next cursor returned by SLS.
    #[default]
    Continue,
    /// Pull again from the supplied cursor.
    Rollback(String),
}

/// Processes batches assigned to a consumer.
///
/// The same processor can be called concurrently for different shards. Calls
/// for one shard are always sequential. Return an error to retry the same batch.
pub trait Processor: Send + Sync + 'static {
    fn process<'a>(
        &'a self,
        shard_id: i32,
        log_groups: LogGroupBatch,
        checkpoint: CheckpointTracker,
    ) -> ProcessorFuture<'a>;

    /// Called for every shard worker during graceful shutdown.
    ///
    /// Like the Go consumer library, an error causes this hook to be retried;
    /// implementations should therefore be idempotent.
    fn shutdown<'a>(&'a self, _checkpoint: CheckpointTracker) -> ProcessorFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

/// Adapts an async closure into a [`Processor`].
pub struct ProcessFn<F>(F);

impl<F> ProcessFn<F> {
    pub fn new<Fut, E>(process: F) -> Self
    where
        F: Fn(i32, LogGroupBatch, CheckpointTracker) -> Fut,
        Fut: Future<Output = std::result::Result<ProcessOutcome, E>>,
    {
        Self(process)
    }
}

impl<F, Fut, E> Processor for ProcessFn<F>
where
    F: Fn(i32, LogGroupBatch, CheckpointTracker) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = std::result::Result<ProcessOutcome, E>> + Send + 'static,
    E: Error + Send + Sync + 'static,
{
    fn process<'a>(
        &'a self,
        shard_id: i32,
        log_groups: LogGroupBatch,
        checkpoint: CheckpointTracker,
    ) -> ProcessorFuture<'a> {
        let future = (self.0)(shard_id, log_groups, checkpoint);
        Box::pin(async move {
            future
                .await
                .map_err(|error| Box::new(error) as BoxProcessorError)
        })
    }
}
