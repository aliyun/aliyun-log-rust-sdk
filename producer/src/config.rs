use std::{fmt, time::Duration};

use aliyun_log_rust_sdk::{Config, CredentialsProvider, SharedCredentialsProvider};

use crate::{Compression, ProducerError};

pub(crate) const MAX_BATCH_BYTES: usize = 8 * 1024 * 1024;

/// Maximum log count in one network batch.
pub(crate) const MAX_BATCH_LOGS: usize = 40960;

/// Connection, batching, retry and resource settings for a producer.
///
/// Configure with `ProducerConfig::default().with_processing_workers(4)` and other
/// `with_*` methods. Values are validated by [`crate::Producer::create`].
/// All setters are infallible; they only save values. [`crate::Producer::create`]
/// returns [`ProducerError::Config`] for invalid settings and
/// [`ProducerError::Creation`] for local resource initialization failures.
///
/// ```
/// use aliyun_log_producer::ProducerConfig;
/// let config = ProducerConfig::default()
///     .with_processing_workers(4);
/// assert_eq!(config.processing_workers(), 4);
/// ```
#[derive(Clone, Debug)]
pub struct ProducerConfig {
    endpoint: String,
    user_agent: String,
    authentication: Option<Authentication>,
    pub(crate) callback_capacity: usize,
    pub(crate) compression: Compression,
    pub(crate) generate_pack_id: bool,
    pub(crate) batch_size_threshold: usize,
    pub(crate) batch_count_threshold: usize,
    pub(crate) linger: Duration,
    pub(crate) buffer_bytes: usize,
    pub(crate) processing_workers: usize,
    pub(crate) max_attempts: u32,
    pub(crate) base_backoff: Duration,
    pub(crate) max_backoff: Duration,
    pub(crate) delivery_timeout: Duration,
}

#[derive(Clone)]
enum Authentication {
    AccessKey { id: String, secret: String },
    Provider(SharedCredentialsProvider),
}

impl fmt::Debug for Authentication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AccessKey { .. } => "AccessKey([redacted])",
            Self::Provider(_) => "CredentialsProvider(..)",
        })
    }
}

impl Default for ProducerConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            user_agent: concat!("aliyun-log-rust-producer/", env!("CARGO_PKG_VERSION")).into(),
            callback_capacity: 65536,
            authentication: None,
            compression: Compression::default(),
            generate_pack_id: true,
            batch_size_threshold: 1024 * 1024,
            batch_count_threshold: 4096,
            linger: Duration::from_millis(2000),
            buffer_bytes: 128 * 1024 * 1024,
            processing_workers: 2,
            max_attempts: 10,
            base_backoff: Duration::from_millis(200),
            max_backoff: Duration::from_secs(10),
            delivery_timeout: Duration::from_secs(600),
        }
    }
}

impl ProducerConfig {
    /// Maximum accepted callbacks awaiting execution, including pending delivery.
    /// Defaults to 65536. Plain send does not consume this capacity.
    pub fn with_callback_capacity(mut self, capacity: usize) -> Self {
        self.callback_capacity = capacity;
        self
    }
    /// Read the outstanding callback capacity.
    pub fn callback_capacity(&self) -> usize {
        self.callback_capacity
    }

    /// One regional endpoint shared by every writer, e.g. `cn-hangzhou.log.aliyuncs.com`.
    /// Validated by [`crate::Producer::create`]; this setter never contacts the service.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Read the endpoint set by [`Self::with_endpoint`].
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// HTTP User-Agent. Defaults to `aliyun-log-rust-producer/<version>`.
    /// Replaces the default; validated at producer creation.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Read the configured HTTP User-Agent.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Fixed access keys shared by every writer. Replaces any credentials provider.
    /// Empty keys are rejected at creation. Debug output redacts both values.
    pub fn with_access_key(mut self, id: impl Into<String>, secret: impl Into<String>) -> Self {
        self.authentication = Some(Authentication::AccessKey {
            id: id.into(),
            secret: secret.into(),
        });
        self
    }

    /// Use a credentials provider, replacing any previously configured credentials.
    /// The shared client manages credential caching and refresh for all writers.
    /// Creation does not fetch credentials; providers must use nonblocking async IO.
    pub fn with_credentials_provider(mut self, provider: impl CredentialsProvider) -> Self {
        self.authentication = Some(Authentication::Provider(SharedCredentialsProvider::new(
            provider,
        )));
        self
    }

    pub(crate) fn client_config(&self) -> Result<Config, ProducerError> {
        let builder = Config::builder()
            .endpoint(&self.endpoint)
            .user_agent(&self.user_agent)
            .max_retry(0);
        let builder = match &self.authentication {
            Some(Authentication::AccessKey { id, secret }) => builder.access_key(id, secret),
            Some(Authentication::Provider(provider)) => {
                builder.credentials_provider(provider.clone())
            }
            None => {
                return Err(ProducerError::Config(
                    "credentials must be configured".into(),
                ))
            }
        };
        builder
            .build()
            .map_err(|error| ProducerError::Config(error.to_string()))
    }

    /// Compression algorithm. Defaults to Zstd level 1; LZ4 is also supported.
    pub fn with_compression(mut self, value: Compression) -> Self {
        self.compression = value;
        self
    }

    /// Read the configured value; see [`Self::with_compression`].
    pub fn compression(&self) -> Compression {
        self.compression
    }

    /// Generate a stable PackId per network batch, retained across retries. Defaults to true.
    pub fn with_generate_pack_id(mut self, value: bool) -> Self {
        self.generate_pack_id = value;
        self
    }

    /// Read the configured value; see [`Self::with_generate_pack_id`].
    pub fn generate_pack_id(&self) -> bool {
        self.generate_pack_id
    }

    /// Estimated log-entry bytes that trigger sending, excluding group metadata.
    /// A complete submission may exceed this threshold; it is never split.
    /// Defaults to 1 MiB; must be at most 8 MiB.
    pub fn with_batch_size_threshold(mut self, value: usize) -> Self {
        self.batch_size_threshold = value;
        self
    }

    /// Read the configured value; see [`Self::with_batch_size_threshold`].
    pub fn batch_size_threshold(&self) -> usize {
        self.batch_size_threshold
    }

    /// Log count that triggers sending. Defaults to 4096, at most 40960.
    /// A complete submission may exceed this threshold; it is never split.
    pub fn with_batch_count_threshold(mut self, value: usize) -> Self {
        self.batch_count_threshold = value;
        self
    }

    /// Read the configured value; see [`Self::with_batch_count_threshold`].
    pub fn batch_count_threshold(&self) -> usize {
        self.batch_count_threshold
    }

    /// Batch accumulation delay from the oldest admission. Defaults to 2000 ms.
    /// Zero sends each submission without waiting to accumulate more logs.
    pub fn with_linger(mut self, value: Duration) -> Self {
        self.linger = value;
        self
    }

    /// Read the configured value; see [`Self::with_linger`].
    pub fn linger(&self) -> Duration {
        self.linger
    }

    /// Soft budget for estimated original bytes of accepted, unfinished logs. Defaults to 128 MiB; not an RSS limit.
    pub fn with_buffer_bytes(mut self, value: usize) -> Self {
        self.buffer_bytes = value;
        self
    }

    /// Read the configured value; see [`Self::with_buffer_bytes`].
    pub fn buffer_bytes(&self) -> usize {
        self.buffer_bytes
    }

    /// Number of threads used to prepare logs for delivery. Defaults to 2.
    pub fn with_processing_workers(mut self, value: usize) -> Self {
        self.processing_workers = value;
        self
    }

    /// Read the configured value; see [`Self::with_processing_workers`].
    pub fn processing_workers(&self) -> usize {
        self.processing_workers
    }

    /// Maximum delivery attempts, including the initial request. Defaults to 10; use 1 to disable retries.
    pub fn with_max_attempts(mut self, value: u32) -> Self {
        self.max_attempts = value;
        self
    }

    /// Read the configured value; see [`Self::with_max_attempts`].
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Initial exponential retry backoff with full jitter. Defaults to 200 ms.
    pub fn with_base_backoff(mut self, value: Duration) -> Self {
        self.base_backoff = value;
        self
    }

    /// Read the configured value; see [`Self::with_base_backoff`].
    pub fn base_backoff(&self) -> Duration {
        self.base_backoff
    }

    /// Maximum retry backoff before full jitter. Defaults to 10 seconds.
    pub fn with_max_backoff(mut self, value: Duration) -> Self {
        self.max_backoff = value;
        self
    }

    /// Read the configured value; see [`Self::with_max_backoff`].
    pub fn max_backoff(&self) -> Duration {
        self.max_backoff
    }

    /// Delivery deadline measured from the oldest admission in a batch. Defaults to 600 seconds.
    /// All submissions in a batch share this deadline; later arrivals inherit the
    /// remaining time. Includes batching, processing, HTTP requests and retries.
    pub fn with_delivery_timeout(mut self, value: Duration) -> Self {
        self.delivery_timeout = value;
        self
    }

    /// Read the configured value; see [`Self::with_delivery_timeout`].
    pub fn delivery_timeout(&self) -> Duration {
        self.delivery_timeout
    }

    pub(crate) fn validate(&self) -> Result<(), ProducerError> {
        for (name, value) in [
            ("callback_capacity", self.callback_capacity),
            ("batch_size_threshold", self.batch_size_threshold),
            ("batch_count_threshold", self.batch_count_threshold),
            ("buffer_bytes", self.buffer_bytes),
            ("processing_workers", self.processing_workers),
            ("max_attempts", self.max_attempts as usize),
        ] {
            if value == 0 {
                return Err(ProducerError::Config(format!("{name} must be nonzero")));
            }
        }
        // Leave headroom below the service's 10 MB raw request limit.
        if self.batch_size_threshold > MAX_BATCH_BYTES {
            return Err(ProducerError::Config(
                "batch_size_threshold must be <= 8 MiB".into(),
            ));
        }
        if self.batch_count_threshold > MAX_BATCH_LOGS {
            return Err(ProducerError::Config(
                "batch_count_threshold must be <= 40960".into(),
            ));
        }
        for (name, value, allow_zero) in [
            ("linger", self.linger, true),
            ("delivery_timeout", self.delivery_timeout, false),
            ("base_backoff", self.base_backoff, false),
            ("max_backoff", self.max_backoff, false),
        ] {
            if (!allow_zero && value.is_zero()) || value > Duration::from_secs(365 * 24 * 3600) {
                return Err(ProducerError::Config(format!(
                    "{name} must be in {}",
                    if allow_zero {
                        "[0, 365 days]"
                    } else {
                        "(0, 365 days]"
                    }
                )));
            }
        }
        if self.base_backoff > self.max_backoff {
            return Err(ProducerError::Config(
                "base_backoff exceeds max_backoff".into(),
            ));
        }
        Ok(())
    }
}

/// Implementation policy, derived once at creation and never exposed as user knobs.
/// Internal tests may lower capacities to exercise saturation without large loads.
#[derive(Debug)]
pub(crate) struct RuntimeConfig {
    pub tick_interval: Duration,
    pub input_capacity: usize,
    pub batch_capacity: usize,
    pub max_inflight_batches: usize,
    pub max_blocking_threads: usize,
}

impl RuntimeConfig {
    pub fn new(config: &ProducerConfig) -> Self {
        Self {
            tick_interval: if config.linger.is_zero() {
                Duration::from_millis(20)
            } else {
                config.linger.min(Duration::from_millis(20))
            },
            input_capacity: 1024,
            batch_capacity: 64,
            // Bound task and retry storage independently of execution thread counts.
            max_inflight_batches: 1024,
            max_blocking_threads: 2,
        }
    }
}
