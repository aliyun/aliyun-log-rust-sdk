use std::{cell::RefCell, panic::AssertUnwindSafe, sync::Arc, time::Instant};

use aliyun_log_rust_sdk::{Client, Error, FromConfig};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::FutureExt;
use tokio_rayon::AsyncThreadPool;

use super::retry::retryable;

use crate::{
    batch::Batch, registry::WriterTarget, state::Shared, Compression, DeliveryError,
    DeliveryResult, SubmissionId,
};

// The private processing pool bounds the number of retained encoding buffers.
thread_local! {
    static RAW_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}
// Clear on success, error or unwind; oversized logs must not pin large allocations.
struct EncodingBuffer<'a>(&'a mut Vec<u8>);
impl Drop for EncodingBuffer<'_> {
    fn drop(&mut self) {
        if self.0.capacity() > crate::config::MAX_BATCH_BYTES {
            *self.0 = Vec::new();
        } else {
            self.0.clear();
        }
    }
}

#[async_trait]
pub(crate) trait Transport: Send + Sync {
    async fn send(
        &self,
        project: &str,
        logstore: &str,
        data: Bytes,
        raw_size: usize,
        compression: Compression,
    ) -> Result<(), Error>;
}

pub(super) struct ClientTransport {
    client: Client,
}

impl ClientTransport {
    pub(super) fn new(config: aliyun_log_rust_sdk::Config) -> Result<Self, String> {
        let client = Client::from_config(config).map_err(|error| error.to_string())?;
        Ok(Self { client })
    }
}

#[async_trait]
impl Transport for ClientTransport {
    async fn send(
        &self,
        project: &str,
        logstore: &str,
        data: Bytes,
        raw_size: usize,
        compression: Compression,
    ) -> Result<(), Error> {
        self.client
            .put_logs_raw(project, logstore)
            .data(data)
            .raw_size(raw_size)
            .compress_type(compression.as_str().to_string())
            .send()
            .await
            .map(|_| ())
    }
}

/// Carries the already encoded payload and attempt state between attempts.
/// The original protobuf group is released after the first encoding. Source/topic
/// are already in the payload; retries retain the target identity.
pub(super) struct PendingBatch {
    context: BatchContext,
    data: Bytes,
    raw_size: usize,
}

/// Delivery metadata survives encoding and is consumed by terminal completion.
struct BatchContext {
    target: Arc<WriterTarget>,
    submission_ids: Vec<SubmissionId>,
    deadline: Instant,
    attempts: u32,
    pack_id: Option<Arc<str>>,
}

#[cfg(test)]
impl PendingBatch {
    pub(super) fn for_test(batch: Batch, shared: &Shared) -> Self {
        Self {
            context: BatchContext {
                target: shared.writers.by_index(batch.key.logstore),
                submission_ids: batch.submission_ids,
                deadline: batch.oldest + std::time::Duration::from_secs(60),
                attempts: 1,
                pack_id: batch.pack_id,
            },
            data: Bytes::new(),
            raw_size: 0,
        }
    }
}

pub(super) enum Work {
    Fresh(Batch),
    Retry(PendingBatch),
}

// Outstanding batches are bounded by internal capacity; keep retry state inline.
#[allow(clippy::large_enum_variant)]
pub(super) enum AttemptResult {
    Finished,
    Retry {
        batch: PendingBatch,
        ready_at: Instant,
    },
}

/// Shared dependencies for processing one batch attempt. Dispatcher bounds the
/// number of concurrent process futures and owns all delayed retries.
pub(super) struct BatchSender {
    shared: Arc<Shared>,
    transport: Arc<dyn Transport>,
    encoding: Arc<rayon::ThreadPool>,
}

impl BatchSender {
    pub(super) fn new(
        shared: Arc<Shared>,
        transport: Arc<dyn Transport>,
        encoding: Arc<rayon::ThreadPool>,
    ) -> Self {
        Self {
            shared,
            transport,
            encoding,
        }
    }

    pub(super) async fn process(&self, work: Work) -> AttemptResult {
        let pending = match work {
            Work::Fresh(batch) => self.encode(batch).await,
            Work::Retry(batch) => Some(batch),
        };
        let result = match pending {
            Some(batch) => self.attempt(batch).await,
            None => AttemptResult::Finished,
        };
        self.shared.refresh_pressure();
        result
    }

    async fn encode(&self, batch: Batch) -> Option<PendingBatch> {
        let target = self.shared.writers.by_index(batch.key.logstore);
        let context = BatchContext {
            target,
            submission_ids: batch.submission_ids,
            deadline: batch.oldest + self.shared.config.delivery_timeout,
            attempts: 0,
            pack_id: batch.pack_id,
        };
        if Instant::now() >= context.deadline {
            log::debug!("SLS producer delivery timed out before encoding");
            self.complete(context, Err(DeliveryError::Timeout));
            return None;
        }
        let group = batch.group;
        let size = batch.size;
        let compression = self.shared.config.compression;
        // Use our private pool. The bridge propagates panics through the returned
        // future; convert them to a per-batch error instead of stopping the dispatcher.
        let encoded = AssertUnwindSafe(self.encoding.spawn_fifo_async(
            move || -> Result<_, DeliveryError> {
                RAW_BUFFER.with(|buffer| {
                    let mut buffer = buffer.borrow_mut();
                    let raw = EncodingBuffer(&mut buffer);
                    raw.0.clear();
                    raw.0.reserve(size.min(crate::config::MAX_BATCH_BYTES));
                    group.encode_into(raw.0).map_err(|error| {
                        DeliveryError::Internal(format!("protobuf encoding failed: {error}"))
                    })?;
                    let compressed = compression.compress(raw.0).map_err(|error| {
                        DeliveryError::Internal(format!("compression failed: {error}"))
                    })?;
                    Ok((Bytes::from(compressed), raw.0.len()))
                })
            },
        ))
        .catch_unwind()
        .await
        .unwrap_or_else(|_| Err(DeliveryError::Internal("encoding job panicked".into())));
        match encoded {
            Ok((data, raw_size)) => Some(PendingBatch {
                context,
                data,
                raw_size,
            }),
            Err(error) => {
                self.complete(context, Err(error));
                None
            }
        }
    }

    async fn attempt(&self, mut batch: PendingBatch) -> AttemptResult {
        if Instant::now() >= batch.context.deadline {
            log::debug!("SLS producer delivery timed out before sending");
            self.complete(batch.context, Err(DeliveryError::Timeout));
            return AttemptResult::Finished;
        }
        let context = &mut batch.context;
        context.attempts += 1;
        let result = AssertUnwindSafe(async {
            tokio::time::timeout_at(
                tokio::time::Instant::from_std(context.deadline),
                self.transport.send(
                    &context.target.project,
                    &context.target.logstore,
                    batch.data.clone(),
                    batch.raw_size,
                    self.shared.config.compression,
                ),
            )
            .await
        })
        .catch_unwind()
        .await;
        let outcome = match result {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(error))) => {
                let details = DeliveryError::from_client(&error);
                if retryable(&error) && context.attempts < self.shared.config.max_attempts {
                    let multiplier = 1u32.checked_shl(context.attempts - 1).unwrap_or(u32::MAX);
                    let cap = self
                        .shared
                        .config
                        .base_backoff
                        .checked_mul(multiplier)
                        .unwrap_or(self.shared.config.max_backoff)
                        .min(self.shared.config.max_backoff);
                    let backoff = cap.mul_f64(fastrand::f64());
                    let ready_at = (Instant::now() + backoff).min(context.deadline);
                    log::warn!("SLS producer delivery failed; retry scheduled: project={}, logstore={}, logs={}, attempt={}, retry_delay_ms={}, kind={:?}, http_status={:?}, code={:?}, request_id={:?}, pack_id={:?}",
                        context.target.project, context.target.logstore, context.submission_ids.len(), context.attempts,
                        ready_at.saturating_duration_since(Instant::now()).as_millis(), details.name(),
                        details.http_status(), details.error_code(), details.request_id(), context.pack_id);
                    return AttemptResult::Retry { batch, ready_at };
                }
                Err(details)
            }
            Ok(Err(_)) => {
                log::debug!("SLS producer delivery timed out during request");
                Err(DeliveryError::Timeout)
            }
            Err(_) => Err(DeliveryError::Internal(
                "transport or credentials provider panicked".into(),
            )),
        };
        self.complete(batch.context, outcome);
        AttemptResult::Finished
    }

    fn complete(&self, context: BatchContext, outcome: DeliveryResult) {
        let BatchContext {
            target,
            submission_ids,
            attempts,
            pack_id,
            ..
        } = context;
        if let Err(error) = &outcome {
            // Message text is caller/service controlled: diagnostics retain only metadata.
            log::warn!("SLS producer delivery failed; no further retries: project={}, logstore={}, logs={}, attempts={}, kind={:?}, http_status={:?}, code={:?}, request_id={:?}, pack_id={:?}",
                target.project, target.logstore, submission_ids.len(), attempts, error.name(),
                error.http_status(), error.error_code(), error.request_id(), pack_id);
        }
        self.shared.complete(submission_ids, outcome);
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;

    #[test]
    fn scratch_is_cleared_on_unwind_and_oversized_capacity_is_released() {
        let mut buffer = Vec::with_capacity(128);
        let capacity = buffer.capacity();
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let scratch = EncodingBuffer(&mut buffer);
            scratch.0.extend_from_slice(b"partial encoding");
            panic!("encoding failed");
        }));
        assert!(result.is_err());
        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), capacity);

        buffer.reserve(crate::config::MAX_BATCH_BYTES + 1);
        buffer.extend_from_slice(b"oversized encoding");
        drop(EncodingBuffer(&mut buffer));
        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), 0);
    }
}
