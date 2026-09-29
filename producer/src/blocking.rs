//! Drive only the producer's runtime-independent lifecycle waits on a caller thread.
//! IO and retries continue on the existing background workers.

use std::future::Future;

use crate::ProducerError;

// The future must use only channels/watch notifications, never Tokio timers or IO.
// Sharing that future with the async APIs keeps admission watermarks and shutdown
// behavior identical without creating another runtime or helper thread per wait.
pub(crate) fn wait(
    operation: impl Future<Output = Result<(), ProducerError>>,
) -> Result<(), ProducerError> {
    futures_lite::future::block_on(operation)
}
