use crate::utils::is_empty_or_none;
use crate::ConfigError;
use lazy_static::lazy_static;
use regex::Regex;

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
    pub(crate) access_key_id: String,
    pub(crate) access_key_secret: String,
    pub(crate) security_token: Option<String>,
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
        let security_token = if is_empty_or_none(&self.security_token) {
            None
        } else {
            self.security_token
        };

        let access_key_id = self.access_key_id.unwrap();
        let access_key_secret = self.access_key_secret.unwrap();

        Ok(Config {
            endpoint,
            access_key_id,
            access_key_secret,
            security_token,
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
        Regex::new(r"^(https?://)?([a-zA-Z0-9.-]+)(:\d+)?$").unwrap();
}

const SCHEME_HTTP: &str = "http://";
const SCHEME_HTTPS: &str = "https://";
const DEFAULT_HTTP_SCHEME: &str = "http://";
