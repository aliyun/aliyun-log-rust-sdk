use super::{Credentials, CredentialsError, CredentialsProvider};
use async_trait::async_trait;
use chrono::DateTime;
use serde::Deserialize;
use std::time::Duration;

const METADATA_ENDPOINT: &str = "http://100.100.100.200/latest/meta-data/ram/security-credentials/";

/// Retrieves temporary credentials for an explicitly named ECS instance RAM role.
///
/// Use [`ecs_ram_role_credentials_provider`] to create this provider, then pass it
/// to [`crate::ConfigBuilder::credentials_provider`]. The SDK obtains credentials
/// when sending requests and manages refreshes automatically. The provider supports
/// cloning and concurrent use.
///
/// # Prerequisites
///
/// * Run the application on an ECS instance with the specified RAM role attached.
/// * Grant the role the SLS permissions required by the application.
/// * Allow the application to access instance metadata in normal mode (IMDSv1).
///   Instances requiring IMDSv2 tokens are not supported.
///
/// See [Attach a RAM role to an ECS instance](https://www.alibabacloud.com/help/en/ecs/user-guide/attach-an-instance-ram-role-to-an-ecs-instance)
/// for setup and metadata access modes.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use aliyun_log_rust_sdk::{ecs_ram_role_credentials_provider, Client, Config, FromConfig};
///
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .credentials_provider(ecs_ram_role_credentials_provider("my-ecs-role")?)
///     .build()?;
/// let client = Client::from_config(config)?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct EcsRamRoleCredentialsProvider {
    http_client: reqwest::Client,
    credentials_url: url::Url,
}

impl EcsRamRoleCredentialsProvider {
    /// Create a provider for a required RAM role name. Does not perform network I/O.
    ///
    /// Prefer [`ecs_ram_role_credentials_provider`] for convenient construction;
    /// it has the same arguments and errors. See its example for client setup.
    ///
    /// # Arguments
    ///
    /// * `role_name` - The attached role's nonempty name, not its ARN.
    ///   The provider does not automatically discover a role.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialsError::Provider`] if the role name is empty or the
    /// HTTP client cannot be created. Metadata access is checked when fetching
    /// credentials, not during construction.
    pub fn new(role_name: impl Into<String>) -> Result<Self, CredentialsError> {
        Self::with_endpoint(
            role_name.into(),
            url::Url::parse(METADATA_ENDPOINT).expect("metadata endpoint is a valid URL"),
        )
    }

    fn with_endpoint(role_name: String, mut endpoint: url::Url) -> Result<Self, CredentialsError> {
        if role_name.is_empty() {
            return Err(CredentialsError::provider(MetadataError::InvalidRoleName));
        }
        endpoint
            .path_segments_mut()
            .expect("metadata endpoint has a path")
            .pop_if_empty()
            .push(&role_name);
        let http_client = reqwest::Client::builder()
            // Instance metadata must not be sent through environment/system proxies
            // or redirected to a different endpoint.
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(1))
            .build()
            .map_err(CredentialsError::provider)?;
        Ok(Self {
            http_client,
            credentials_url: endpoint,
        })
    }
}

/// Create an ECS RAM role provider for an explicitly specified role.
///
/// Pass the result to [`crate::ConfigBuilder::credentials_provider`] to use
/// automatically refreshed role credentials without configuring access keys.
/// Creating the provider does not contact the metadata service.
///
/// # Prerequisites
///
/// The application must run on an ECS instance with this role attached, the role
/// must have the required SLS permissions, and normal metadata access (IMDSv1)
/// must be available. IMDSv2-only instances are not supported. Follow the
/// [official setup instructions](https://www.alibabacloud.com/help/en/ecs/user-guide/attach-an-instance-ram-role-to-an-ecs-instance).
///
/// # Arguments
///
/// * `role_name` - Required, nonempty role name, not an ARN.
///
/// # Returns
///
/// A clonable [`EcsRamRoleCredentialsProvider`] for the specified role.
///
/// # Errors
///
/// Returns [`CredentialsError::Provider`] for an empty name or HTTP client
/// construction failure. Later metadata access or response errors are reported
/// during requests as [`crate::Error::Credentials`] if no previous credentials
/// are available. Configure fetch timeouts with
/// [`crate::ConfigBuilder::credentials_fetch_timeout`].
///
/// # Examples
///
/// ```
/// use aliyun_log_rust_sdk::{ecs_ram_role_credentials_provider, Client, Config, FromConfig};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .credentials_provider(ecs_ram_role_credentials_provider("my-ecs-role")?)
///     .build()?;
/// let client = Client::from_config(config)?;
/// # Ok(())
/// # }
/// ```
pub fn ecs_ram_role_credentials_provider(
    role_name: impl Into<String>,
) -> Result<EcsRamRoleCredentialsProvider, CredentialsError> {
    EcsRamRoleCredentialsProvider::new(role_name)
}

#[async_trait]
impl CredentialsProvider for EcsRamRoleCredentialsProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        let response = self
            .http_client
            .get(self.credentials_url.clone())
            .send()
            .await
            .map_err(CredentialsError::provider)?;
        if !response.status().is_success() {
            return Err(CredentialsError::provider(MetadataError::HttpStatus(
                response.status(),
            )));
        }
        let body = response.bytes().await.map_err(CredentialsError::provider)?;
        parse_credentials(&body).map_err(CredentialsError::provider)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MetadataResponse {
    code: String,
    access_key_id: String,
    access_key_secret: String,
    security_token: Option<String>,
    expiration: String,
    last_updated: String,
}

fn parse_credentials(body: &[u8]) -> Result<Credentials, MetadataError> {
    // Do not include the raw response or serde's invalid-value details in errors:
    // malformed metadata can still contain credentials.
    let response: MetadataResponse = serde_json::from_slice(body)
        .map_err(|_| MetadataError::InvalidResponse("malformed JSON or missing/invalid fields"))?;
    if !response.code.eq_ignore_ascii_case("success") {
        return Err(MetadataError::InvalidResponse("Code is not Success"));
    }
    let expiration = DateTime::parse_from_rfc3339(&response.expiration)
        .map_err(|_| MetadataError::InvalidResponse("invalid Expiration"))?;
    let last_updated = DateTime::parse_from_rfc3339(&response.last_updated)
        .map_err(|_| MetadataError::InvalidResponse("invalid LastUpdated"))?;
    let mut credentials = Credentials::new(response.access_key_id, response.access_key_secret)
        .map_err(|_| MetadataError::InvalidResponse("access key ID and secret must be nonempty"))?
        .with_expiration(expiration.into())
        .with_update_time(last_updated.into());
    if let Some(token) = response.security_token {
        credentials = credentials.with_security_token(token);
    }
    Ok(credentials)
}

#[derive(Debug, thiserror::Error)]
enum MetadataError {
    #[error("ECS RAM role name must not be empty")]
    InvalidRoleName,
    #[error("ECS RAM role metadata returned HTTP {0}")]
    HttpStatus(reqwest::StatusCode),
    #[error("invalid ECS RAM role metadata: {0}")]
    InvalidResponse(&'static str),
}

#[cfg(test)]
mod tests;
