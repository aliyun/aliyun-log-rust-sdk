//! Coordinated, checkpointed consumption of a Logstore.
//!
//! The consumer uses the SLS consumer-group APIs to distribute shards between
//! workers. One asynchronous task is created per assigned shard. Checkpoints
//! are only advanced when the processor explicitly calls
//! [`CheckpointTracker::save_checkpoint`].
//!
//! ```no_run
//! # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), Box<dyn std::error::Error>> {
//! use aliyun_log_rust_sdk::consumer::{
//!     ConsumerConfig, ConsumerWorker, CursorPosition, ProcessFn, ProcessOutcome,
//! };
//!
//! let config = ConsumerConfig::new(
//!     "my-project",
//!     "my-logstore",
//!     "my-consumer-group",
//!     "consumer-1",
//! )
//! .cursor_position(CursorPosition::Begin);
//! let processor = ProcessFn::new(|shard, log_groups, checkpoint| async move {
//!     println!("shard {shard}: {} log groups", log_groups.len());
//!     checkpoint.save_checkpoint(false).await?;
//!     Ok::<_, aliyun_log_rust_sdk::consumer::Error>(ProcessOutcome::Continue)
//! });
//!
//! let mut worker = ConsumerWorker::new(client, config, processor)?;
//! worker.start().await?;
//! worker.stop_and_wait().await?;
//! # Ok(())
//! # }
//! ```

mod checkpoint;
mod config;
mod processor;
mod worker;

pub use aliyun_log_sdk_protobuf::LogGroup;
pub(crate) use checkpoint::CheckpointCommitter;
pub use checkpoint::CheckpointTracker;
pub use config::{ConsumerConfig, CursorPosition};
pub use processor::{
    BoxProcessorError, LogGroupBatch, ProcessFn, ProcessOutcome, Processor, ProcessorFuture,
    ProcessorResult,
};
pub use worker::ConsumerWorker;

/// Result returned by consumer-library operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced while configuring or running a consumer worker.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid consumer configuration: {0}")]
    InvalidConfig(String),

    #[error(transparent)]
    Sdk(Box<crate::Error>),

    #[error("consumer worker has already been started")]
    AlreadyStarted,

    #[error("consumer task failed: {0}")]
    Task(#[from] tokio::task::JoinError),

    #[error("consumer heartbeat failed beyond the configured timeout: {0}")]
    Heartbeat(#[source] Box<crate::Error>),

    #[error("processor for shard {shard_id} failed repeatedly: {source}")]
    Processor {
        shard_id: i32,
        #[source]
        source: BoxProcessorError,
    },

    #[error("processor for shard {shard_id} panicked")]
    ProcessorPanicked { shard_id: i32 },

    #[error("graceful shutdown for shard {shard_id} exceeded {timeout:?}")]
    ShutdownTimeout {
        shard_id: i32,
        timeout: std::time::Duration,
    },

    #[error("checkpoint token for shard {shard_id} no longer belongs to the active batch")]
    StaleCheckpoint { shard_id: i32 },
}

impl From<crate::Error> for Error {
    fn from(error: crate::Error) -> Self {
        Self::Sdk(Box::new(error))
    }
}
