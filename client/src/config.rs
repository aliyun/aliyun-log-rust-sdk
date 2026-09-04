use std::sync::Arc;
use std::time::Duration;

use crate::{ConfigError, Credentials, CredentialsProvider};

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
    pub(crate) credentials_provider: Arc<dyn CredentialsProvider>,
    pub(crate) connection_timeout: Duration,
    pub(crate) request_timeout: Duration,
    pub(crate) retry_policy: RetryPolicy,
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
    credentials_provider: Option<Arc<dyn CredentialsProvider>>,
    connection_timeout: Option<Duration>,
    request_timeout: Option<Duration>,
    retry_policy: Option<RetryPolicy>,
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
        self.security_token = None;
        self.credentials_provider = None;
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
        self.credentials_provider = None;
        self
    }

    /// Use a provider that can refresh credentials between request attempts.
    pub fn credentials_provider(mut self, provider: impl CredentialsProvider) -> Self {
        self.credentials_provider = Some(Arc::new(provider));
        self.access_key_id = None;
        self.access_key_secret = None;
        self.security_token = None;
        self
    }

    /// Set the connection timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The connection timeout duration
    pub fn connection_timeout(mut self, connection_timeout: Duration) -> Self {
        self.connection_timeout = Some(connection_timeout);
        self
    }

    /// Set the request timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The request timeout duration
    pub fn request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = Some(request_timeout);
        self
    }

    /// Configure retry behavior.
    pub fn retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = Some(retry_policy);
        self
    }

    /// Build the client with the configured settings.
    pub fn build(self) -> Result<Config, ConfigError> {
        let endpoint = self.validate_endpoint()?;
        let credentials_provider = self.build_credentials_provider()?;

        let connection_timeout = self
            .connection_timeout
            .unwrap_or(DEFAULT_CONNECTION_TIMEOUT);

        let request_timeout = self.request_timeout.unwrap_or(DEFAULT_REQUEST_TIMEOUT);
        if connection_timeout.is_zero() || request_timeout.is_zero() {
            return Err(ConfigError::InvalidClientConfig(anyhow::anyhow!(
                "connection_timeout and request_timeout must be greater than zero"
            )));
        }

        let retry_policy = self.retry_policy.unwrap_or_default();
        retry_policy.validate()?;

        Ok(Config {
            endpoint,
            credentials_provider,
            request_timeout,
            connection_timeout,
            retry_policy,
        })
    }

    fn validate_endpoint(&self) -> Result<Endpoint, ConfigError> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or_else(|| ConfigError::InvalidEndpoint("Endpoint not provided".to_string()))?;

        let endpoint = if endpoint.contains("://") {
            endpoint.to_string()
        } else {
            format!("{DEFAULT_SCHEME}{endpoint}")
        };
        let parsed = url::Url::parse(&endpoint)
            .map_err(|_| ConfigError::InvalidEndpoint(endpoint.clone()))?;
        let scheme = match parsed.scheme() {
            "http" => SCHEME_HTTP,
            "https" => SCHEME_HTTPS,
            _ => return Err(ConfigError::InvalidEndpoint(endpoint)),
        };
        if parsed.host().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(ConfigError::InvalidEndpoint(endpoint));
        }
        let domain = parsed[url::Position::BeforeHost..url::Position::AfterPort].to_string();
        Ok(Endpoint { domain, scheme })
    }

    fn build_credentials_provider(&self) -> Result<Arc<dyn CredentialsProvider>, ConfigError> {
        if let Some(provider) = &self.credentials_provider {
            return Ok(Arc::clone(provider));
        }
        let credentials = Credentials::new(
            self.access_key_id.clone().unwrap_or_default(),
            self.access_key_secret.clone().unwrap_or_default(),
            self.security_token
                .clone()
                .filter(|value| !value.is_empty()),
        );
        if !credentials.validate() {
            return Err(ConfigError::InvalidAccessKey);
        }
        Ok(Arc::new(credentials))
    }
}

#[derive(Clone)]
pub(crate) struct Endpoint {
    pub(crate) domain: String,
    pub(crate) scheme: &'static str,
}

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

const SCHEME_HTTP: &str = "http://";
const SCHEME_HTTPS: &str = "https://";
const DEFAULT_SCHEME: &str = "https://";

/// Retry behavior for SLS requests.
#[derive(Clone, Debug)]
pub struct RetryPolicy {
    pub(crate) max_retries: u32,
    pub(crate) base_backoff: Duration,
    pub(crate) max_backoff: Duration,
    pub(crate) max_elapsed: Duration,
    pub(crate) jitter: bool,
}

impl RetryPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_retries(mut self, value: u32) -> Self {
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

    pub fn max_elapsed(mut self, value: Duration) -> Self {
        self.max_elapsed = value;
        self
    }

    pub fn jitter(mut self, value: bool) -> Self {
        self.jitter = value;
        self
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.base_backoff > self.max_backoff {
            return Err(ConfigError::InvalidClientConfig(anyhow::anyhow!(
                "retry base_backoff must not exceed max_backoff"
            )));
        }
        if self.max_retries > 0 && self.max_elapsed.is_zero() {
            return Err(ConfigError::InvalidClientConfig(anyhow::anyhow!(
                "retry max_elapsed must be greater than zero"
            )));
        }
        Ok(())
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(10),
            max_elapsed: Duration::from_secs(90),
            jitter: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_defaults_to_https() {
        let config = Config::builder()
            .endpoint("cn-hangzhou.log.aliyuncs.com")
            .access_key("id", "secret")
            .build()
            .expect("valid config");
        assert_eq!(config.endpoint.scheme, "https://");
    }

    #[test]
    fn endpoint_rejects_paths_and_credentials() {
        for endpoint in ["https://example.com/path", "https://user@example.com"] {
            assert!(Config::builder()
                .endpoint(endpoint)
                .access_key("id", "secret")
                .build()
                .is_err());
        }
    }
}
