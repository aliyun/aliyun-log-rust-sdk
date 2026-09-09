use super::{Credentials, CredentialsError, CredentialsProvider, StaticCredentialsProvider};
use async_trait::async_trait;
use std::env::VarError;

/// A snapshot of access keys and an optional STS token read from environment variables.
///
/// Use [`environment_credentials_provider`] for default variable names, or
/// [`environment_credentials_provider_builder`] to customize individual names.
/// Both paths read and validate the environment when creating the provider.
/// Later environment changes do not affect the provider or its clones.
///
/// Both access keys must exist and be nonempty. A missing or empty STS token is
/// valid and means no token. Values are preserved without trimming whitespace.
/// Credentials have no expiration or update time. This provider cannot renew STS
/// credentials when they expire at the service.
///
/// # Examples
///
/// ```no_run
/// use aliyun_log_rust_sdk::{environment_credentials_provider_builder, Config};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let provider = environment_credentials_provider_builder()
///     .with_access_key_id_env("MY_ACCESS_KEY_ID")
///     .with_access_key_secret_env("MY_ACCESS_KEY_SECRET")
///     .build()?;
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .credentials_provider(provider)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct EnvironmentCredentialsProvider(StaticCredentialsProvider);

impl EnvironmentCredentialsProvider {
    /// Read and validate credentials immediately using the default variable names.
    ///
    /// Equivalent to `environment_credentials_provider()`. See
    /// [`EnvironmentCredentialsProviderBuilder::build`] for error conditions.
    pub fn new() -> Result<Self, CredentialsError> {
        environment_credentials_provider_builder().build()
    }

    /// Configure variable names before creating the provider.
    ///
    /// Equivalent to [`environment_credentials_provider_builder`]; see its examples.
    pub fn builder() -> EnvironmentCredentialsProviderBuilder {
        environment_credentials_provider_builder()
    }
}

#[async_trait]
impl CredentialsProvider for EnvironmentCredentialsProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        self.0.fetch_credentials().await
    }
}

/// Configures environment variable names before reading credentials.
///
/// Obtain this builder with [`environment_credentials_provider_builder`]. Override only
/// the names you need, then call [`build()`](Self::build) to create the provider.
/// No environment values are read before `build()`.
///
/// # Examples
///
/// ```no_run
/// use aliyun_log_rust_sdk::environment_credentials_provider_builder;
/// # fn main() -> Result<(), aliyun_log_rust_sdk::CredentialsError> {
/// let provider = environment_credentials_provider_builder()
///     .with_access_key_id_env("MY_ACCESS_KEY_ID")
///     .with_access_key_secret_env("MY_ACCESS_KEY_SECRET")
///     .with_security_token_env("MY_SECURITY_TOKEN")
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct EnvironmentCredentialsProviderBuilder {
    access_key_id_env: String,
    access_key_secret_env: String,
    security_token_env: String,
}

impl Default for EnvironmentCredentialsProviderBuilder {
    fn default() -> Self {
        Self {
            access_key_id_env: "ALIBABA_CLOUD_ACCESS_KEY_ID".into(),
            access_key_secret_env: "ALIBABA_CLOUD_ACCESS_KEY_SECRET".into(),
            security_token_env: "ALIBABA_CLOUD_SECURITY_TOKEN".into(),
        }
    }
}

impl EnvironmentCredentialsProviderBuilder {
    /// Set the environment variable name for the required AccessKey ID.
    ///
    /// `name` replaces `ALIBABA_CLOUD_ACCESS_KEY_ID`. Its value must exist and be
    /// nonempty when [`build()`](Self::build) is called. See the builder's example.
    pub fn with_access_key_id_env(mut self, name: impl Into<String>) -> Self {
        self.access_key_id_env = name.into();
        self
    }

    /// Set the environment variable name for the required AccessKey secret.
    ///
    /// `name` replaces `ALIBABA_CLOUD_ACCESS_KEY_SECRET`. Its value must exist and
    /// be nonempty when [`build()`](Self::build) is called. See the builder's example.
    pub fn with_access_key_secret_env(mut self, name: impl Into<String>) -> Self {
        self.access_key_secret_env = name.into();
        self
    }

    /// Set the environment variable name for the optional STS token.
    ///
    /// `name` replaces `ALIBABA_CLOUD_SECURITY_TOKEN`. A missing or empty value is
    /// treated as absent. Omit this method to keep the default name.
    /// See [`EnvironmentCredentialsProviderBuilder`] for an example.
    pub fn with_security_token_env(mut self, name: impl Into<String>) -> Self {
        self.security_token_env = name.into();
        self
    }

    /// Read and validate the environment, creating a credentials snapshot.
    ///
    /// # Returns
    ///
    /// A clonable provider whose credentials have no expiration or update time.
    /// It does not reread the environment or renew temporary STS credentials.
    /// See [`environment_credentials_provider`] for setup examples.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialsError::InvalidAccessKey`] for empty access keys, or
    /// [`CredentialsError::Provider`] for missing required variables or non-Unicode
    /// values. A missing or empty STS token is not an error. Values are not trimmed,
    /// and errors do not include their contents.
    pub fn build(self) -> Result<EnvironmentCredentialsProvider, CredentialsError> {
        let credentials = read_credentials(
            &self.access_key_id_env,
            &self.access_key_secret_env,
            &self.security_token_env,
            |name| std::env::var(name),
        )?;
        Ok(EnvironmentCredentialsProvider(
            StaticCredentialsProvider::new(credentials),
        ))
    }
}

/// Read environment credentials immediately using standard Alibaba Cloud variable names.
///
/// | Default variable | Requirement |
/// | --- | --- |
/// | `ALIBABA_CLOUD_ACCESS_KEY_ID` | Required and nonempty |
/// | `ALIBABA_CLOUD_ACCESS_KEY_SECRET` | Required and nonempty |
/// | `ALIBABA_CLOUD_SECURITY_TOKEN` | Optional; missing or empty means absent |
///
/// This function immediately reads and validates the variables. To override
/// individual names, use [`environment_credentials_provider_builder`]. No reads
/// occur during later fetches. Credentials have no expiration or update time;
/// an STS token may still expire at the service and cannot be renewed by this provider.
///
/// # Returns
///
/// A clonable [`EnvironmentCredentialsProvider`] ready for client configuration.
///
/// # Errors
///
/// Returns [`CredentialsError::InvalidAccessKey`] for empty access keys, or
/// [`CredentialsError::Provider`] for missing required variables or non-Unicode
/// values. A missing or empty STS token is not an error. Values are not trimmed.
///
/// # Examples
///
/// Set the required environment variables before creating the provider:
///
/// ```no_run
/// use aliyun_log_rust_sdk::{environment_credentials_provider, Client, Config, FromConfig};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .credentials_provider(environment_credentials_provider()?)
///     .build()?;
/// let client = Client::from_config(config)?;
/// # Ok(())
/// # }
/// ```
pub fn environment_credentials_provider() -> Result<EnvironmentCredentialsProvider, CredentialsError>
{
    EnvironmentCredentialsProvider::new()
}

/// Configure custom environment variable names before creating a provider.
///
/// Unchanged names keep the defaults documented by [`environment_credentials_provider`].
/// This helper does not read environment values. Call
/// [`build()`](EnvironmentCredentialsProviderBuilder::build) to read, validate, and
/// capture the credentials. See that method for error conditions.
///
/// # Returns
///
/// An [`EnvironmentCredentialsProviderBuilder`] supporting individual overrides.
///
/// # Examples
///
/// Override only the access key variables, keeping the default optional token name:
///
/// ```no_run
/// use aliyun_log_rust_sdk::environment_credentials_provider_builder;
/// # fn main() -> Result<(), aliyun_log_rust_sdk::CredentialsError> {
/// let provider = environment_credentials_provider_builder()
///     .with_access_key_id_env("MY_ACCESS_KEY_ID")
///     .with_access_key_secret_env("MY_ACCESS_KEY_SECRET")
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub fn environment_credentials_provider_builder() -> EnvironmentCredentialsProviderBuilder {
    EnvironmentCredentialsProviderBuilder::default()
}

fn read_credentials(
    id_env: &str,
    secret_env: &str,
    token_env: &str,
    mut read: impl FnMut(&str) -> Result<String, VarError>,
) -> Result<Credentials, CredentialsError> {
    let id = read(id_env).map_err(|err| environment_error(id_env, err))?;
    let secret = read(secret_env).map_err(|err| environment_error(secret_env, err))?;
    let credentials = Credentials::new(id, secret)?;
    match read(token_env) {
        Ok(token) => Ok(credentials.with_security_token(token)),
        Err(VarError::NotPresent) => Ok(credentials),
        Err(err) => Err(environment_error(token_env, err)),
    }
}

fn environment_error(name: &str, error: VarError) -> CredentialsError {
    // VarError::NotUnicode contains the value; do not retain or display it.
    let reason = match error {
        VarError::NotPresent => "is not set",
        VarError::NotUnicode(_) => "is not valid Unicode",
    };
    CredentialsError::provider(EnvironmentError {
        name: name.into(),
        reason,
    })
}

#[derive(Debug, thiserror::Error)]
#[error("credentials environment variable {name:?} {reason}")]
struct EnvironmentError {
    name: String,
    reason: &'static str,
}

#[cfg(test)]
mod tests;
