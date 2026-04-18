use std::sync::Arc;
use std::time::Duration;

use aliyun_log_rust_sdk::{Client, Config, FromConfig};
use tokio::sync::mpsc;

use crate::callback::ProducerCallback;
use crate::dispatcher::Dispatcher;
use crate::error::BuildError;
use crate::exporter::{LogSink, SlsExporter};
use crate::memory_limiter::MemoryLimiter;
use crate::model::WhenFull;
use crate::producer::{run_batcher, BatcherConfig, Producer, ProducerInner};
use crate::shared::Shared;
use crate::stats::StatsInner;

#[derive(Clone)]
#[must_use]
pub struct ProducerBuilder {
    endpoint: Option<String>,
    credentials: Option<Credentials>,
    project: Option<String>,
    logstore: Option<String>,
    topic: Option<String>,
    source: Option<String>,
    log_tags: Vec<(String, String)>,

    batch_max_events: usize,
    batch_max_bytes: usize,
    linger: Duration,

    channel_capacity: usize,
    memory_limit_bytes: usize,
    when_full: WhenFull,

    concurrency: usize,
    export_timeout: Duration,
    max_retries: usize,
    base_backoff: Duration,
    max_backoff: Duration,

    callback: Option<Arc<dyn ProducerCallback>>,

    sink: Option<Arc<dyn LogSink>>,
}

#[derive(Clone)]
enum Credentials {
    AccessKey {
        access_key_id: String,
        access_key_secret: String,
    },
    Sts {
        access_key_id: String,
        access_key_secret: String,
        security_token: String,
    },
}

impl Default for ProducerBuilder {
    fn default() -> Self {
        Self {
            endpoint: None,
            credentials: None,
            project: None,
            logstore: None,
            topic: None,
            source: None,
            log_tags: Vec::new(),
            batch_max_events: 256,
            batch_max_bytes: 512 * 1024,
            linger: Duration::from_millis(200),
            channel_capacity: 1024,
            memory_limit_bytes: 64 * 1024 * 1024,
            when_full: WhenFull::Block,
            concurrency: 4,
            export_timeout: Duration::from_secs(5),
            max_retries: 3,
            base_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(50),
            callback: None,
            sink: None,
        }
    }
}

impl ProducerBuilder {
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    pub fn access_key(
        mut self,
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
    ) -> Self {
        self.credentials = Some(Credentials::AccessKey {
            access_key_id: access_key_id.into(),
            access_key_secret: access_key_secret.into(),
        });
        self
    }

    pub fn sts(
        mut self,
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
        security_token: impl Into<String>,
    ) -> Self {
        self.credentials = Some(Credentials::Sts {
            access_key_id: access_key_id.into(),
            access_key_secret: access_key_secret.into(),
            security_token: security_token.into(),
        });
        self
    }

    pub fn project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    pub fn logstore(mut self, logstore: impl Into<String>) -> Self {
        self.logstore = Some(logstore.into());
        self
    }

    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn add_log_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.log_tags.push((key.into(), value.into()));
        self
    }

    pub fn batch_max_events(mut self, value: usize) -> Self {
        self.batch_max_events = value;
        self
    }

    pub fn batch_max_bytes(mut self, value: usize) -> Self {
        self.batch_max_bytes = value;
        self
    }

    /// Maximum time to wait before flushing a partial batch.
    ///
    /// A new `Sleep` future is created for each batch, so sub-millisecond
    /// lingers will increase scheduler overhead proportionally.
    pub fn linger(mut self, value: Duration) -> Self {
        self.linger = value;
        self
    }

    pub fn channel_capacity(mut self, value: usize) -> Self {
        self.channel_capacity = value;
        self
    }

    pub fn memory_limit_bytes(mut self, value: usize) -> Self {
        self.memory_limit_bytes = value;
        self
    }

    pub fn when_full(mut self, value: WhenFull) -> Self {
        self.when_full = value;
        self
    }

    pub fn concurrency(mut self, value: usize) -> Self {
        self.concurrency = value;
        self
    }

    pub fn export_timeout(mut self, value: Duration) -> Self {
        self.export_timeout = value;
        self
    }

    pub fn max_retries(mut self, value: usize) -> Self {
        self.max_retries = value;
        self
    }

    pub fn base_backoff(mut self, value: Duration) -> Self {
        self.base_backoff = value;
        self
    }

    pub fn max_backoff(mut self, value: Duration) -> Self {
        self.max_backoff = value;
        self
    }

    pub fn callback(mut self, callback: Arc<dyn ProducerCallback>) -> Self {
        self.callback = Some(callback);
        self
    }

    pub fn sink(mut self, sink: Arc<dyn LogSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    pub async fn build(mut self) -> Result<Producer, BuildError> {
        self.validate()?;

        let memory_limiter = Arc::new(MemoryLimiter::new(self.memory_limit_bytes));
        let stats = Arc::new(StatsInner::default());
        let shared = Arc::new(Shared::new(
            self.when_full,
            memory_limiter,
            stats,
            self.callback.take(),
        ));
        let (tx, rx) = mpsc::channel(self.channel_capacity);

        let dispatcher = match self.sink.take() {
            Some(sink) => Dispatcher::new_custom(
                sink,
                shared.clone(),
                self.concurrency,
                self.export_timeout,
                self.max_retries,
                self.base_backoff,
                self.max_backoff,
            ),
            None => {
                let (client, project, logstore) = self.build_client_and_target()?;
                let mut exporter = SlsExporter::new(client, project, logstore);

                if let Some(topic) = self.topic.take() {
                    exporter = exporter.topic(topic);
                }
                if let Some(source) = self.source.take() {
                    exporter = exporter.source(source);
                }
                for (key, value) in self.log_tags.drain(..) {
                    exporter = exporter.add_tag(key, value);
                }

                Dispatcher::new_sls(
                    exporter,
                    shared.clone(),
                    self.concurrency,
                    self.export_timeout,
                    self.max_retries,
                    self.base_backoff,
                    self.max_backoff,
                )
            }
        };

        tokio::spawn(run_batcher(
            rx,
            dispatcher,
            BatcherConfig {
                max_events: self.batch_max_events,
                max_bytes: self.batch_max_bytes,
                linger: self.linger,
            },
        ));

        Ok(Producer::new(ProducerInner { tx, shared }))
    }

    fn validate(&self) -> Result<(), BuildError> {
        validate_non_zero("batch_max_events", self.batch_max_events)?;
        validate_non_zero("batch_max_bytes", self.batch_max_bytes)?;
        validate_non_zero("channel_capacity", self.channel_capacity)?;
        validate_non_zero("memory_limit_bytes", self.memory_limit_bytes)?;
        validate_non_zero("concurrency", self.concurrency)?;
        Ok(())
    }

    fn build_client_and_target(&self) -> Result<(Arc<Client>, String, String), BuildError> {
        let endpoint = self
            .endpoint
            .clone()
            .ok_or_else(|| BuildError::InvalidConfig("missing endpoint".into()))?;
        let project = self
            .project
            .clone()
            .ok_or_else(|| BuildError::InvalidConfig("missing project".into()))?;
        let logstore = self
            .logstore
            .clone()
            .ok_or_else(|| BuildError::InvalidConfig("missing logstore".into()))?;

        let client_config = match &self.credentials {
            Some(Credentials::AccessKey {
                access_key_id,
                access_key_secret,
            }) => Config::builder()
                .endpoint(endpoint)
                .access_key(access_key_id.clone(), access_key_secret.clone())
                .build(),
            Some(Credentials::Sts {
                access_key_id,
                access_key_secret,
                security_token,
            }) => Config::builder()
                .endpoint(endpoint)
                .sts(
                    access_key_id.clone(),
                    access_key_secret.clone(),
                    security_token.clone(),
                )
                .build(),
            None => return Err(BuildError::InvalidConfig("missing credentials".into())),
        }
        .map_err(|err| BuildError::InvalidConfig(err.to_string()))?;

        let client = Arc::new(
            Client::from_config(client_config)
                .map_err(|err| BuildError::InvalidConfig(err.to_string()))?,
        );

        Ok((client, project, logstore))
    }
}

fn validate_non_zero(name: &'static str, value: usize) -> Result<(), BuildError> {
    if value == 0 {
        return Err(BuildError::InvalidConfig(format!("{name} must be > 0")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tower::service_fn;

    use super::*;
    use crate::{DeliveryReport, TowerSink};

    #[tokio::test]
    async fn build_missing_endpoint_is_invalid_config() {
        let err = Producer::builder()
            .access_key("ak", "sk")
            .project("project")
            .logstore("logstore")
            .build()
            .await
            .err()
            .expect("build should require endpoint");

        assert!(matches!(err, BuildError::InvalidConfig(message) if message == "missing endpoint"));
    }

    #[tokio::test]
    async fn build_with_custom_sink_does_not_require_config() {
        let sink = TowerSink::new(service_fn(|batch: crate::ExportBatch| async move {
            Ok::<_, crate::DeliveryError>(DeliveryReport {
                batch_id: batch.batch_id,
                record_count: batch.records.len(),
                encoded_bytes: batch.estimated_bytes,
                retry_count: batch.retry_count,
                elapsed: Duration::default(),
                request_id: None,
            })
        }));

        let producer = Producer::builder()
            .sink(std::sync::Arc::new(sink))
            .build()
            .await;

        assert!(producer.is_ok(), "custom sink should bypass config checks");
    }

    #[tokio::test]
    async fn missing_project_is_reported_as_invalid_config() {
        let err = Producer::builder()
            .endpoint("cn-hangzhou.log.aliyuncs.com")
            .access_key("ak", "sk")
            .build()
            .await
            .err()
            .expect("missing target should be invalid config");

        assert!(matches!(
            err,
            BuildError::InvalidConfig(message) if message == "missing project"
        ));
    }
}
