use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use aliyun_log_rust_sdk::{Client, Error as ClientError};
use aliyun_log_sdk_protobuf::{Log, LogGroup};
use async_trait::async_trait;
use bytes::Bytes;
use log::{debug, warn};

use crate::model::RecordEnvelope;
use crate::{DeliveryError, DeliveryReport, LogRecord};

#[derive(Debug, Clone)]
pub struct ExportBatch {
    pub batch_id: u64,
    pub records: Arc<[LogRecord]>,
    pub estimated_bytes: usize,
    pub retry_count: usize,
    pub elapsed: Duration,
}

#[async_trait]
pub trait LogSink: Send + Sync + 'static {
    async fn export(&self, batch: ExportBatch) -> Result<DeliveryReport, DeliveryError>;
}

// Only LZ4 compression is supported at this time; the compression type is
// not configurable and is hardcoded in encode_records / export_encoded.
pub(crate) struct SlsExporter {
    client: Arc<Client>,
    project: String,
    logstore: String,
    topic: Option<String>,
    source: Option<String>,
    log_tags: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub(crate) struct EncodedBatch {
    pub data: Bytes,
    pub raw_size: usize,
    pub record_count: usize,
}

impl SlsExporter {
    pub(crate) fn new(client: Arc<Client>, project: String, logstore: String) -> Self {
        Self {
            client,
            project,
            logstore,
            topic: None,
            source: None,
            log_tags: Vec::new(),
        }
    }

    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn add_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.log_tags.push((key.into(), value.into()));
        self
    }

    fn build_log_group<'a>(&self, records: impl IntoIterator<Item = &'a LogRecord>) -> LogGroup {
        let mut group = LogGroup::new();
        if let Some(topic) = &self.topic {
            group.set_topic(topic.clone());
        }
        if let Some(source) = &self.source {
            group.set_source(source.clone());
        }
        for (k, v) in &self.log_tags {
            group.add_log_tag_kv(k.clone(), v.clone());
        }
        for record in records {
            group.add_log(log_record_to_proto(record));
        }
        group
    }

    pub(crate) fn encode_batch(
        &self,
        records: &[RecordEnvelope],
    ) -> Result<EncodedBatch, DeliveryError> {
        self.encode_records(records.iter().map(|env| &env.record))
    }

    fn encode_records<'a>(
        &self,
        records: impl IntoIterator<Item = &'a LogRecord>,
    ) -> Result<EncodedBatch, DeliveryError> {
        let mut record_count = 0usize;
        let group = self.build_log_group(records.into_iter().inspect(|_| {
            record_count += 1;
        }));
        let encoded = group
            .encode()
            .map_err(|err| DeliveryError::Internal(format!("failed to encode log group: {err}")))?;
        let raw_size = encoded.len();
        let compressed = lz4::block::compress(encoded.as_slice(), None, false).map_err(|err| {
            DeliveryError::Internal(format!("failed to compress log group: {err}"))
        })?;

        Ok(EncodedBatch {
            data: Bytes::from(compressed),
            raw_size,
            record_count,
        })
    }

    pub(crate) async fn export_encoded(
        &self,
        batch_id: u64,
        encoded: &EncodedBatch,
        retry_count: usize,
        elapsed: Duration,
    ) -> Result<DeliveryReport, DeliveryError> {
        debug!(
            target: "aliyun_log_rust_sdk_producer::exporter",
            "export batch start: batch_id={}, record_count={}, raw_size={}, compressed_size={}, retry_count={}, elapsed_ms={}, project={}, logstore={}",
            batch_id,
            encoded.record_count,
            encoded.raw_size,
            encoded.data.len(),
            retry_count,
            elapsed.as_millis(),
            self.project,
            self.logstore,
        );

        let response = self
            .client
            .put_logs_raw(&self.project, &self.logstore)
            .data(encoded.data.clone())
            .raw_size(encoded.raw_size)
            .compress_type("lz4".to_string())
            .send()
            .await
            .map_err(|err| {
                let mapped = map_client_error(err);
                warn!(
                    target: "aliyun_log_rust_sdk_producer::exporter",
                    "export batch failed: batch_id={}, record_count={}, raw_size={}, compressed_size={}, retry_count={}, project={}, logstore={}, error={}",
                    batch_id,
                    encoded.record_count,
                    encoded.raw_size,
                    encoded.data.len(),
                    retry_count,
                    self.project,
                    self.logstore,
                    mapped
                );
                mapped
            })?;

        let request_id = response.get_request_id();
        debug!(
            target: "aliyun_log_rust_sdk_producer::exporter",
            "export batch success: batch_id={}, record_count={}, raw_size={}, compressed_size={}, retry_count={}, project={}, logstore={}, request_id={:?}",
            batch_id,
            encoded.record_count,
            encoded.raw_size,
            encoded.data.len(),
            retry_count,
            self.project,
            self.logstore,
            request_id,
        );

        Ok(DeliveryReport {
            batch_id,
            record_count: encoded.record_count,
            encoded_bytes: encoded.raw_size,
            retry_count,
            elapsed,
            request_id,
        })
    }
}

#[async_trait]
impl LogSink for SlsExporter {
    async fn export(&self, batch: ExportBatch) -> Result<DeliveryReport, DeliveryError> {
        let encoded = self.encode_records(batch.records.iter())?;
        self.export_encoded(batch.batch_id, &encoded, batch.retry_count, batch.elapsed)
            .await
    }
}

fn log_record_to_proto(record: &LogRecord) -> Log {
    let d = record.timestamp.duration_since(UNIX_EPOCH).unwrap_or_default();
    let mut log = Log::from_unixtime(d.as_secs().min(u32::MAX as u64) as u32);
    log.set_time_ns(d.subsec_nanos());
    for field in &record.fields {
        log.add_content_kv(field.key.clone(), field.value.clone());
    }
    log
}

// Retriable error codes (aligned with aliyun-log-java-producer RetriableErrors).
// Quota errors are also throttled and use max_backoff as a floor.
const RETRIABLE_ERROR_CODES: &[&str] = &[
    "RequestError",
    "Unauthorized",
    "WriteQuotaExceed",
    "ShardWriteQuotaExceed",
    "ExceedQuota",
    "InternalServerError",
    "ServerBusy",
    "BadResponse",
    "ProjectNotExists",
    "LogstoreNotExists",
    "SocketTimeout",
    "SignatureNotMatch",
];

const THROTTLE_ERROR_CODES: &[&str] =
    &["WriteQuotaExceed", "ShardWriteQuotaExceed", "ExceedQuota"];

fn map_client_error(err: ClientError) -> DeliveryError {
    match err {
        ClientError::Network(network_err) => {
            if network_err.is_timeout() {
                DeliveryError::Timeout
            } else {
                DeliveryError::Network(network_err.to_string())
            }
        }
        ClientError::Server {
            error_code,
            error_message,
            http_status: _,
            request_id,
        } => {
            let retryable = RETRIABLE_ERROR_CODES.contains(&error_code.as_str());
            let throttled = THROTTLE_ERROR_CODES.contains(&error_code.as_str());
            DeliveryError::Server {
                code: error_code,
                message: error_message,
                request_id,
                retryable,
                throttled,
            }
        }
        other => DeliveryError::Internal(other.to_string()),
    }
}

pub mod mock {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio::sync::Mutex;
    use tower::{Service, ServiceExt};

    use super::{ExportBatch, LogSink};
    use crate::{DeliveryError, DeliveryReport};

    pub struct TowerSink<S> {
        inner: Arc<Mutex<S>>,
    }

    impl<S> TowerSink<S> {
        pub fn new(service: S) -> Self {
            Self {
                inner: Arc::new(Mutex::new(service)),
            }
        }
    }

    #[async_trait]
    impl<S> LogSink for TowerSink<S>
    where
        S: Service<ExportBatch, Response = DeliveryReport, Error = DeliveryError> + Send + 'static,
        S::Future: Send + 'static,
    {
        async fn export(&self, batch: ExportBatch) -> Result<DeliveryReport, DeliveryError> {
            let mut service = self.inner.lock().await;
            service.ready().await?.call(batch).await
        }
    }
}
