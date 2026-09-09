use std::{
    fmt,
    sync::Arc,
    time::{Duration, SystemTime},
};

use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use tokio::time::{sleep, timeout, Instant};

mod ecs_ram_role;
mod environment;
#[cfg(test)]
mod tests;
pub use ecs_ram_role::{ecs_ram_role_credentials_provider, EcsRamRoleCredentialsProvider};
pub use environment::{
    environment_credentials_provider, environment_credentials_provider_builder,
    EnvironmentCredentialsProvider, EnvironmentCredentialsProviderBuilder,
};

/// An immutable set of access keys and optional STS metadata.
///
/// Custom [`CredentialsProvider`] implementations return this type. AccessKey ID
/// and secret are required. The STS token, expiration, and update time are optional.
/// Missing expiration means nonexpiring credentials; `update_time` is metadata only.
/// Supply the actual expiration from your source for temporary credentials.
///
/// To configure fixed credentials directly, use [`static_credentials_provider`].
/// For ECS role credentials, use [`ecs_ram_role_credentials_provider`].
///
/// # Examples
///
/// Construct the value returned by a custom provider:
///
/// ```
/// # fn main() -> Result<(), aliyun_log_rust_sdk::CredentialsError> {
/// use aliyun_log_rust_sdk::Credentials;
/// use std::time::{Duration, SystemTime};
///
/// // In a provider, use the expiration returned by the credentials source.
/// let expiration = SystemTime::now() + Duration::from_secs(3600);
/// let credentials = Credentials::new("access_key_id", "access_key_secret")?
///     .with_security_token("sts_token")
///     .with_expiration(expiration)
///     .with_update_time(SystemTime::now());
/// assert_eq!(credentials.security_token(), Some("sts_token"));
/// # Ok(())
/// # }
#[derive(Clone)]
pub struct Credentials {
    access_key_id: String,
    access_key_secret: String,
    security_token: Option<String>,
    expiration: Option<SystemTime>,
    update_time: Option<SystemTime>,
}

impl Credentials {
    /// Create credentials. Both the access key ID and secret must be nonempty.
    ///
    /// # Arguments
    ///
    /// * `access_key_id` - AccessKey ID supplied by the credentials source.
    /// * `access_key_secret` - Corresponding AccessKey secret.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialsError::InvalidAccessKey`] if either key is empty.
    /// Optional fields are initially absent; see [`Credentials`] for an example.
    pub fn new(
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
    ) -> Result<Self, CredentialsError> {
        let credentials = Self {
            access_key_id: access_key_id.into(),
            access_key_secret: access_key_secret.into(),
            security_token: None,
            expiration: None,
            update_time: None,
        };
        if credentials.access_key_id.is_empty() || credentials.access_key_secret.is_empty() {
            return Err(CredentialsError::InvalidAccessKey);
        }
        Ok(credentials)
    }

    /// Attach an STS token. An empty token is treated as absent.
    ///
    /// # Arguments
    ///
    /// * `token` - The STS token associated with this AccessKey pair.
    pub fn with_security_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        self.security_token = (!token.is_empty()).then_some(token);
        self
    }

    /// Set the absolute expiration time. Already expired fetch results are rejected.
    ///
    /// # Arguments
    ///
    /// * `expiration` - The actual expiration returned by the credentials source.
    ///   Do not extend the validity of an existing credential locally.
    pub fn with_expiration(mut self, expiration: SystemTime) -> Self {
        self.expiration = Some(expiration);
        self
    }

    /// Attach the provider's update time; currently unused by the SDK.
    ///
    /// # Arguments
    ///
    /// * `update_time` - The timestamp supplied by the credentials source, such as
    ///   ECS metadata's `LastUpdated` field.
    pub fn with_update_time(mut self, update_time: SystemTime) -> Self {
        self.update_time = Some(update_time);
        self
    }

    /// Return the AccessKey ID.
    pub fn access_key_id(&self) -> &str {
        &self.access_key_id
    }
    /// Return the AccessKey secret. Avoid including it in logs or error messages.
    pub fn access_key_secret(&self) -> &str {
        &self.access_key_secret
    }
    /// Return the optional STS token. Avoid including it in logs or error messages.
    pub fn security_token(&self) -> Option<&str> {
        self.security_token.as_deref()
    }
    /// Return the expiration, or `None` for nonexpiring credentials.
    pub fn expiration(&self) -> Option<SystemTime> {
        self.expiration
    }
    /// Return the source's optional update time; it does not determine expiration.
    pub fn update_time(&self) -> Option<SystemTime> {
        self.update_time
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("access_key_id", &"[REDACTED]")
            .field("access_key_secret", &"[REDACTED]")
            .field(
                "security_token",
                &self.security_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expiration", &self.expiration)
            .field("update_time", &self.update_time)
            .finish()
    }
}

/// An error obtaining credentials. Provider error sources are shared when cloned.
///
/// Helper functions can return this error during construction. During client
/// requests, it is wrapped in [`crate::Error::Credentials`] when fetching fails
/// and no previous credentials exist. See [`CredentialsError::provider`] for
/// converting errors from a custom source.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialsError {
    /// The AccessKey ID or secret is empty.
    #[error("access key ID and secret must be nonempty")]
    InvalidAccessKey,
    /// The credentials source returned credentials whose expiration has passed.
    #[error("provider returned already expired credentials")]
    Expired,
    /// A fetch attempt exceeded its configured timeout.
    #[error("credentials fetch timed out after {0:?}")]
    Timeout(Duration),
    /// Fetching is temporarily suppressed after a previous failure.
    #[error("credentials fetch suppressed after a failure; retry in {retry_after:?}")]
    Throttled {
        /// Remaining duration before fetching may be attempted again.
        retry_after: Duration,
        /// The failure that caused suppression.
        #[source]
        source: Arc<CredentialsError>,
    },
    /// An error from provider construction, transport, or response validation.
    #[error("credentials provider failed: {0}")]
    Provider(#[source] Arc<dyn std::error::Error + Send + Sync>),
}

impl CredentialsError {
    /// Wrap a custom provider error, retaining its source chain.
    ///
    /// # Arguments
    ///
    /// * `error` - A source error convertible to `anyhow::Error`.
    ///
    /// # Examples
    ///
    /// ```
    /// use aliyun_log_rust_sdk::CredentialsError;
    ///
    /// let error = CredentialsError::provider(std::io::Error::new(
    ///     std::io::ErrorKind::TimedOut,
    ///     "credentials source timed out",
    /// ));
    /// assert!(matches!(error, CredentialsError::Provider(_)));
    /// ```
    pub fn provider(error: impl Into<anyhow::Error>) -> Self {
        Self::Provider(error.into().into_boxed_dyn_error().into())
    }
}

impl From<anyhow::Error> for CredentialsError {
    fn from(error: anyhow::Error) -> Self {
        Self::provider(error)
    }
}

/// Fetch credentials asynchronously from a built-in or application-defined source.
///
/// Prefer [`ecs_ram_role_credentials_provider`] for ECS,
/// [`environment_credentials_provider`] for environment variables, or
/// [`static_credentials_provider`] for fixed credentials. Implement this trait when
/// your application obtains credentials from another source, and expose a helper
/// function to create your provider. The SDK re-exports [`crate::async_trait`], so
/// a separate macro dependency is not required.
///
/// # Implementation Requirements
///
/// * Return an AccessKey pair with its optional STS token and actual expiration.
///   Missing expiration means the SDK need not refresh the credentials.
/// * Return source failures as [`CredentialsError`]; the SDK manages fetch retries.
/// * Support concurrent calls and cancellation-safe async I/O. A timeout or request
///   cancellation can drop the fetch future. Do not block the async executor.
/// * The provider must be `Send + Sync + 'static`, but need not implement `Clone`.
///   `Arc<YourProvider>` and [`SharedCredentialsProvider`] provide shared handles.
///
/// # Examples
///
/// This example obtains temporary credentials from an application-managed HTTPS
/// service. Adapt the response fields and authentication to your service; the
/// expiration is an RFC 3339 string. For environment variables, use the built-in
/// [`environment_credentials_provider`] instead.
///
/// ```
/// use aliyun_log_rust_sdk::{
///     async_trait, Config, Credentials, CredentialsError, CredentialsProvider,
/// };
/// use serde::Deserialize;
///
/// struct HttpCredentialsProvider {
///     endpoint: String,
///     client: reqwest::Client,
/// }
///
/// // JSON returned by your application's credentials service.
/// #[derive(Deserialize)]
/// struct SourceCredentials {
///     access_key_id: String,
///     access_key_secret: String,
///     security_token: Option<String>,
///     expiration: String,
/// }
///
/// #[async_trait]
/// impl CredentialsProvider for HttpCredentialsProvider {
///     async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
///         let body = self.client.get(&self.endpoint).send().await
///             .map_err(CredentialsError::provider)?
///             .error_for_status().map_err(CredentialsError::provider)?
///             .bytes().await.map_err(CredentialsError::provider)?;
///         let source: SourceCredentials = serde_json::from_slice(&body)
///             .map_err(CredentialsError::provider)?;
///         let expiration = chrono::DateTime::parse_from_rfc3339(&source.expiration)
///             .map_err(CredentialsError::provider)?;
///         let mut credentials = Credentials::new(source.access_key_id, source.access_key_secret)?
///             .with_expiration(expiration.into());
///         if let Some(token) = source.security_token {
///             credentials = credentials.with_security_token(token);
///         }
///         Ok(credentials)
///     }
/// }
///
/// fn http_credentials_provider(endpoint: impl Into<String>) -> impl CredentialsProvider {
///     HttpCredentialsProvider { endpoint: endpoint.into(), client: reqwest::Client::new() }
/// }
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = Config::builder()
///         .endpoint("cn-hangzhou.log.aliyuncs.com")
///         .credentials_provider(http_credentials_provider("https://credentials.example.com/current"))
///         .build()?;
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait CredentialsProvider: Send + Sync + 'static {
    /// Obtain one set of credentials from the source.
    ///
    /// # Returns
    ///
    /// Fresh [`Credentials`], with the source's actual expiration for temporary keys.
    ///
    /// # Errors
    ///
    /// Return [`CredentialsError`] if the source is unavailable or its credentials
    /// are invalid. Use [`CredentialsError::provider`] to wrap a source error.
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError>;
}

#[async_trait]
impl<P: CredentialsProvider + ?Sized> CredentialsProvider for Arc<P> {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        (**self).fetch_credentials().await
    }
}

/// A clonable provider for a fixed set of credentials.
///
/// Use [`static_credentials_provider`] for convenient construction. This provider
/// cannot renew temporary credentials. Use [`ecs_ram_role_credentials_provider`]
/// or a custom [`CredentialsProvider`] when credentials must be renewed.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use aliyun_log_rust_sdk::{static_credentials_provider, Config};
///
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .credentials_provider(static_credentials_provider("access_key_id", "access_key_secret", None)?)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct StaticCredentialsProvider(Arc<Credentials>);

impl StaticCredentialsProvider {
    /// Wrap an existing set of credentials, preserving all its metadata.
    ///
    /// Prefer [`static_credentials_provider`] when supplying keys and an optional
    /// token. Use this constructor when an existing [`Credentials`] value also has
    /// expiration or update metadata. This provider does not renew those credentials.
    ///
    /// # Arguments
    ///
    /// * `credentials` - The fixed credentials to return on every fetch.
    pub fn new(credentials: Credentials) -> Self {
        Self(Arc::new(credentials))
    }
}

#[async_trait]
impl CredentialsProvider for StaticCredentialsProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        Ok((*self.0).clone())
    }
}

/// Create a static provider from access keys and an optional STS token.
///
/// Both keys must be nonempty. An empty token is treated as absent. Credentials
/// created by this helper have no expiration or update time. To supply those fields,
/// use [`StaticCredentialsProvider::new`] with a [`Credentials`] value instead.
///
/// # Arguments
///
/// * `access_key_id` - Required, nonempty AccessKey ID.
/// * `access_key_secret` - Required, nonempty corresponding secret.
/// * `security_token` - Optional STS token. Pass `None` for long-term access keys.
///   Supplying a token does not enable automatic renewal of temporary credentials.
///
/// # Returns
///
/// A clonable [`StaticCredentialsProvider`] ready to pass to
/// [`crate::ConfigBuilder::credentials_provider`].
///
/// # Errors
///
/// Returns [`CredentialsError::InvalidAccessKey`] if either access key is empty.
///
/// # Examples
///
/// ```
/// use aliyun_log_rust_sdk::{static_credentials_provider, Client, Config, FromConfig};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = static_credentials_provider("access_key_id", "access_key_secret", None)?;
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .credentials_provider(provider)
///     .build()?;
/// let client = Client::from_config(config)?;
/// # Ok(())
/// # }
/// ```
///
/// Supply an STS token for a fixed temporary credential:
///
/// ```
/// # fn main() -> Result<(), aliyun_log_rust_sdk::CredentialsError> {
/// use aliyun_log_rust_sdk::static_credentials_provider;
///
/// let provider = static_credentials_provider(
///     "temporary_access_key_id",
///     "temporary_access_key_secret",
///     Some("sts_token".to_string()),
/// )?;
/// # Ok(())
/// # }
/// ```
pub fn static_credentials_provider(
    access_key_id: impl Into<String>,
    access_key_secret: impl Into<String>,
    security_token: Option<String>,
) -> Result<StaticCredentialsProvider, CredentialsError> {
    let mut credentials = Credentials::new(access_key_id, access_key_secret)?;
    if let Some(token) = security_token {
        credentials = credentials.with_security_token(token);
    }
    Ok(StaticCredentialsProvider::new(credentials))
}

/// A clonable handle for a built-in or custom [`CredentialsProvider`].
///
/// Use this type when you need a common handle for different provider types.
/// A custom provider does not need to implement `Clone` to be wrapped by this type.
/// Built-in providers can also be passed directly to the configuration builder.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use aliyun_log_rust_sdk::{
///     ecs_ram_role_credentials_provider, Config, SharedCredentialsProvider,
/// };
///
/// let provider = SharedCredentialsProvider::new(
///     ecs_ram_role_credentials_provider("my-ecs-role")?,
/// );
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .credentials_provider(provider.clone())
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SharedCredentialsProvider(Arc<dyn CredentialsProvider>);

impl SharedCredentialsProvider {
    /// Create a shared handle for a provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - The provider to share. Creating this handle does not fetch credentials.
    ///
    /// See [`SharedCredentialsProvider`] for an example using a creation helper.
    pub fn new(provider: impl CredentialsProvider) -> Self {
        Self(Arc::new(provider))
    }
}

#[async_trait]
impl CredentialsProvider for SharedCredentialsProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        self.0.fetch_credentials().await
    }
}

pub(crate) const DEFAULT_FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const FAILURE_COOLDOWN: Duration = Duration::from_secs(15);
const MAX_FETCH_ATTEMPTS: u32 = 3;

struct CachedCredentials {
    credentials: Arc<Credentials>,
    fetched_at: Instant,
    refresh_after: Option<Duration>,
}

impl CachedCredentials {
    fn new(credentials: Credentials) -> Result<Self, CredentialsError> {
        let fetched_at = Instant::now();
        let refresh_after = credentials
            .expiration
            .map(|expiration| -> Result<Duration, CredentialsError> {
                let remaining = expiration
                    .duration_since(SystemTime::now())
                    .ok()
                    .filter(|duration| !duration.is_zero())
                    .ok_or(CredentialsError::Expired)?;
                let window = (remaining / 5).min(Duration::from_secs(300));
                let advance = window.mul_f64(0.5 + fastrand::f64() * 0.5);
                Ok(remaining - advance)
            })
            .transpose()?;
        Ok(Self {
            credentials: Arc::new(credentials),
            fetched_at,
            refresh_after,
        })
    }

    fn needs_refresh(&self) -> bool {
        self.refresh_after
            .is_some_and(|delay| self.fetched_at.elapsed() >= delay)
    }
}

struct FetchFailure {
    failed_at: Instant,
    error: Arc<CredentialsError>,
}

pub(crate) struct CredentialsCache {
    provider: SharedCredentialsProvider,
    current: ArcSwapOption<CachedCredentials>,
    failure: ArcSwapOption<FetchFailure>,
    fetch_timeout: Duration,
}

impl CredentialsCache {
    pub(crate) fn new(provider: SharedCredentialsProvider, fetch_timeout: Duration) -> Self {
        Self {
            provider,
            current: ArcSwapOption::empty(),
            failure: ArcSwapOption::empty(),
            fetch_timeout,
        }
    }

    pub(crate) async fn get(&self) -> Result<Arc<Credentials>, CredentialsError> {
        if let Some(current) = self.current.load_full() {
            if !current.needs_refresh() {
                return Ok(current.credentials.clone());
            }
        }
        match self.fetch().await {
            Ok(credentials) => Ok(credentials),
            // Read the latest snapshot: another concurrent fetch may have succeeded.
            Err(error) => self
                .current
                .load_full()
                .map(|current| current.credentials.clone())
                .ok_or(error),
        }
    }

    async fn fetch(&self) -> Result<Arc<Credentials>, CredentialsError> {
        if let Some(failure) = self.failure.load_full() {
            let elapsed = failure.failed_at.elapsed();
            if elapsed < FAILURE_COOLDOWN {
                return Err(CredentialsError::Throttled {
                    retry_after: FAILURE_COOLDOWN - elapsed,
                    source: failure.error.clone(),
                });
            }
        }

        // No single-flight gate: concurrent callers may each execute a retry loop.
        // Cooldown is checked once, before the loop, and updated only on overall failure.
        for attempt in 0..MAX_FETCH_ATTEMPTS {
            let result = match timeout(self.fetch_timeout, self.provider.fetch_credentials()).await
            {
                Ok(result) => result.and_then(CachedCredentials::new),
                Err(_) => Err(CredentialsError::Timeout(self.fetch_timeout)),
            };
            let error = match result {
                Ok(current) => {
                    let credentials = current.credentials.clone();
                    // Last successful completion wins. Failures never replace credentials.
                    self.current.store(Some(Arc::new(current)));
                    return Ok(credentials);
                }
                Err(error) => error,
            };
            if attempt + 1 == MAX_FETCH_ATTEMPTS {
                let failure = Arc::new(FetchFailure {
                    failed_at: Instant::now(),
                    error: Arc::new(error.clone()),
                });
                // A delayed concurrent writer must not shorten a more recent cooldown.
                self.failure.rcu(|previous| match previous {
                    Some(previous) if previous.failed_at > failure.failed_at => {
                        Some(previous.clone())
                    }
                    _ => Some(failure.clone()),
                });
                log::warn!("Credentials fetch failed after three attempts; suppressing new fetches for 15 seconds");
                return Err(error);
            }
            sleep(Duration::from_millis(100 * (1 << attempt))).await;
        }
        unreachable!("fetch loop always returns on its last attempt")
    }
}
