use std::{
    fmt,
    sync::Arc,
    time::{Duration, SystemTime},
};

use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use tokio::time::{sleep, timeout, Instant};

#[cfg(test)]
mod tests;

/// An immutable set of access keys and optional STS metadata.
///
/// Missing expiration means the credentials never expire and are not automatically
/// refreshed. `update_time` is provider metadata and does not affect caching.
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
    pub fn with_security_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        self.security_token = (!token.is_empty()).then_some(token);
        self
    }

    /// Set the absolute expiration time. Already expired fetch results are rejected.
    pub fn with_expiration(mut self, expiration: SystemTime) -> Self {
        self.expiration = Some(expiration);
        self
    }

    /// Attach the provider's update time; currently unused by the SDK.
    pub fn with_update_time(mut self, update_time: SystemTime) -> Self {
        self.update_time = Some(update_time);
        self
    }

    pub fn access_key_id(&self) -> &str {
        &self.access_key_id
    }
    pub fn access_key_secret(&self) -> &str {
        &self.access_key_secret
    }
    pub fn security_token(&self) -> Option<&str> {
        self.security_token.as_deref()
    }
    pub fn expiration(&self) -> Option<SystemTime> {
        self.expiration
    }
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
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialsError {
    #[error("access key ID and secret must be nonempty")]
    InvalidAccessKey,
    #[error("provider returned already expired credentials")]
    Expired,
    #[error("credentials fetch timed out after {0:?}")]
    Timeout(Duration),
    #[error("credentials fetch suppressed after a failure; retry in {retry_after:?}")]
    Throttled {
        retry_after: Duration,
        #[source]
        source: Arc<CredentialsError>,
    },
    #[error("credentials provider failed: {0}")]
    Provider(#[source] Arc<dyn std::error::Error + Send + Sync>),
}

impl CredentialsError {
    /// Wrap a custom provider error, retaining its source chain.
    pub fn provider(error: impl Into<anyhow::Error>) -> Self {
        Self::Provider(error.into().into_boxed_dyn_error().into())
    }
}

impl From<anyhow::Error> for CredentialsError {
    fn from(error: anyhow::Error) -> Self {
        Self::provider(error)
    }
}

/// Fetch credentials once, asynchronously. The SDK owns caching and retries.
///
/// Implementations must support concurrent calls: refreshes are not single-flight.
/// The SDK drops the returned future on timeout or request cancellation, so providers
/// should use cancellation-safe async I/O and avoid blocking the executor.
///
/// ```
/// use aliyun_log_rust_sdk::{async_trait, Credentials, CredentialsError, CredentialsProvider};
///
/// struct EnvironmentProvider;
///
/// #[async_trait]
/// impl CredentialsProvider for EnvironmentProvider {
///     async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
///         let id = std::env::var("ALIBABA_CLOUD_ACCESS_KEY_ID")
///             .map_err(CredentialsError::provider)?;
///         let secret = std::env::var("ALIBABA_CLOUD_ACCESS_KEY_SECRET")
///             .map_err(CredentialsError::provider)?;
///         Credentials::new(id, secret)
///     }
/// }
/// ```
#[async_trait]
pub trait CredentialsProvider: Send + Sync + 'static {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError>;
}

#[async_trait]
impl<P: CredentialsProvider + ?Sized> CredentialsProvider for Arc<P> {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        (**self).fetch_credentials().await
    }
}

/// A clonable provider that always returns the same credentials without I/O.
///
/// Clones share the stored credentials. Optional expiration and update time are
/// preserved; this provider cannot renew expiring credentials. The SDK applies the
/// same cache and expiration rules as for any other provider.
#[derive(Debug, Clone)]
pub struct StaticCredentialsProvider(Arc<Credentials>);

impl StaticCredentialsProvider {
    /// Wrap an existing set of credentials, preserving all its metadata.
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

/// A cheaply clonable provider handle. Clones share the provider, not its SDK cache.
/// Clone [`crate::Config`] to share both a provider and its cache across clients.
#[derive(Clone)]
pub struct SharedCredentialsProvider(Arc<dyn CredentialsProvider>);

impl SharedCredentialsProvider {
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
