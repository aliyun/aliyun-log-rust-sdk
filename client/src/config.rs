use crate::credentials::{CredentialsCache, DEFAULT_FETCH_TIMEOUT};
use crate::utils::is_empty_or_none;
use crate::ConfigError;
use crate::{static_credentials_provider, CredentialsProvider, SharedCredentialsProvider};
use lazy_static::lazy_static;
use regex::Regex;
use std::sync::Arc;

/// Configuration for the Aliyun Log Service client.
///
/// # Examples
///
/// ```
/// # async fn wrapper() -> aliyun_log_rust_sdk::Result<()> {
/// use aliyun_log_rust_sdk::FromConfig;
/// let config = aliyun_log_rust_sdk::Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .access_key("access_key_id", "access_key_secret")
///     .build()?;
/// let client = aliyun_log_rust_sdk::Client::from_config(config)?;
/// # Ok(())
/// # }
/// ```
///
#[derive(Clone)]
pub struct Config {
    pub(crate) endpoint: Endpoint,
    pub(crate) credentials: Arc<CredentialsCache>,
    pub(crate) connection_timeout: std::time::Duration,
    pub(crate) request_timeout: std::time::Duration,
    pub(crate) max_retry: u32,
    pub(crate) base_retry_backoff: std::time::Duration,
    pub(crate) max_retry_backoff: std::time::Duration,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }
}

/// Config builder for creating a new config.
///
/// # Examples
///
/// ```
/// # async fn wrapper() -> aliyun_log_rust_sdk::Result<()> {
/// use aliyun_log_rust_sdk::FromConfig;
/// let config = aliyun_log_rust_sdk::Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .access_key("access_key_id", "access_key_secret")
///     .request_timeout(std::time::Duration::from_secs(60))
///     .connection_timeout(std::time::Duration::from_secs(10))
///     .build()?;
/// let client = aliyun_log_rust_sdk::Client::from_config(config)?;
/// # Ok(())
/// # }
/// ```
///
#[derive(Default)]
pub struct ConfigBuilder {
    endpoint: Option<String>,
    access_key_id: Option<String>,
    access_key_secret: Option<String>,
    security_token: Option<String>,
    credentials_provider: Option<SharedCredentialsProvider>,
    credentials_fetch_timeout: Option<std::time::Duration>,
    connection_timeout: Option<std::time::Duration>,
    request_timeout: Option<std::time::Duration>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the endpoint for the Aliyun Log Service.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The endpoint, e.g. "cn-hangzhou.log.aliyuncs.com"
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the access key ID and secret for authentication.
    ///
    /// # Arguments
    ///
    /// * `access_key_id` - The access key ID
    /// * `access_key_secret` - The access key secret
    pub fn access_key(
        mut self,
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(access_key_id.into());
        self.access_key_secret = Some(access_key_secret.into());
        self
    }

    /// Set temporary security token for STS authentication.
    ///
    /// # Arguments
    ///
    /// * `access_key_id` - The temporary access key ID
    /// * `access_key_secret` - The temporary access key secret
    /// * `security_token` - The security token
    pub fn sts(
        mut self,
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
        security_token: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(access_key_id.into());
        self.access_key_secret = Some(access_key_secret.into());
        self.security_token = Some(security_token.into());
        self
    }

    /// Use a custom asynchronous credentials provider.
    ///
    /// Fetching is lazy. The SDK caches credentials, refreshes them before expiration,
    /// and falls back to old credentials (even expired ones) on fetch failure.
    /// Concurrent fetches are allowed. Each fetch round has at most three attempts;
    /// an exhausted round suppresses new rounds for 15 seconds.
    ///
    /// Cannot be combined with [`Self::access_key`] or [`Self::sts`]. Clones of the
    /// built [`Config`] share the cache, including its failure cooldown.
    pub fn credentials_provider(mut self, provider: impl CredentialsProvider) -> Self {
        self.credentials_provider = Some(SharedCredentialsProvider::new(provider));
        self
    }

    /// Set the timeout for each credentials fetch attempt (default: 5 seconds).
    ///
    /// Must be nonzero. Independent of the SLS HTTP request timeout; a fetch round
    /// takes at most three such timeouts plus 300ms of retry delays, provided the
    /// provider yields to the async runtime. Cancellation drops the provider future.
    pub fn credentials_fetch_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.credentials_fetch_timeout = Some(timeout);
        self
    }

    /// Set the connection timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The connection timeout duration
    pub fn connection_timeout(mut self, connection_timeout: std::time::Duration) -> Self {
        self.connection_timeout = Some(connection_timeout);
        self
    }

    /// Set the request timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The request timeout duration
    pub fn request_timeout(mut self, request_timeout: std::time::Duration) -> Self {
        self.request_timeout = Some(request_timeout);
        self
    }

    /// Build the client with the configured settings.
    pub fn build(self) -> Result<Config, ConfigError> {
        let endpoint = self.validate_endpoint()?;
        self.validate_credentials()?;

        let connection_timeout = self
            .connection_timeout
            .unwrap_or(DEFAULT_CONNECTION_TIMEOUT);

        let request_timeout = self.request_timeout.unwrap_or(DEFAULT_REQUEST_TIMEOUT);
        let fetch_timeout = self
            .credentials_fetch_timeout
            .unwrap_or(DEFAULT_FETCH_TIMEOUT);
        if fetch_timeout.is_zero() {
            return Err(ConfigError::Other(anyhow::anyhow!(
                "credentials fetch timeout must be nonzero"
            )));
        }
        let provider = match self.credentials_provider {
            Some(provider) => provider,
            None => SharedCredentialsProvider::new(
                static_credentials_provider(
                    self.access_key_id.unwrap(),
                    self.access_key_secret.unwrap(),
                    self.security_token,
                )
                .map_err(|_| ConfigError::InvalidAccessKey)?,
            ),
        };

        Ok(Config {
            endpoint,
            credentials: Arc::new(CredentialsCache::new(provider, fetch_timeout)),
            request_timeout,
            connection_timeout,
            max_retry: DEFAULT_MAX_RETRY,
            base_retry_backoff: DEFAULT_BASE_RETRY_BACKOFF,
            max_retry_backoff: DEFAULT_MAX_RETRY_BACKOFF,
        })
    }

    fn validate_endpoint(&self) -> Result<Endpoint, ConfigError> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or_else(|| ConfigError::InvalidEndpoint("Endpoint not provided".to_string()))?;

        if !ENDPOINT_REGEX.is_match(endpoint) {
            return Err(ConfigError::InvalidEndpoint(endpoint.to_string()));
        }

        if let Some(stripped) = endpoint.strip_prefix(SCHEME_HTTPS) {
            return Ok(Endpoint {
                domain: stripped.to_string(),
                scheme: SCHEME_HTTPS,
            });
        }

        if let Some(stripped) = endpoint.strip_prefix(SCHEME_HTTP) {
            return Ok(Endpoint {
                domain: stripped.to_string(),
                scheme: SCHEME_HTTP,
            });
        }

        // No scheme in the input, use default
        Ok(Endpoint {
            domain: endpoint.to_string(),
            scheme: DEFAULT_HTTP_SCHEME,
        })
    }

    fn validate_credentials(&self) -> Result<(), ConfigError> {
        if self.credentials_provider.is_some() {
            if self.access_key_id.is_some()
                || self.access_key_secret.is_some()
                || self.security_token.is_some()
            {
                return Err(ConfigError::Other(anyhow::anyhow!(
                    "credentials_provider cannot be combined with access_key or sts"
                )));
            }
            return Ok(());
        }
        if is_empty_or_none(&self.access_key_id) || is_empty_or_none(&self.access_key_secret) {
            return Err(ConfigError::InvalidAccessKey);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct Endpoint {
    pub(crate) domain: String,
    pub(crate) scheme: &'static str,
}

const DEFAULT_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const DEFAULT_CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const DEFAULT_MAX_RETRY: u32 = 3;
const DEFAULT_BASE_RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(1000);
const DEFAULT_MAX_RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(10);

lazy_static! {
    static ref ENDPOINT_REGEX: Regex =
        Regex::new(r"^(https?://)?([a-zA-Z0-9.-]+)(:\d+)?$").expect("endpoint regex is invalid");
}

const SCHEME_HTTP: &str = "http://";
const SCHEME_HTTPS: &str = "https://";
const DEFAULT_HTTP_SCHEME: &str = "http://";
