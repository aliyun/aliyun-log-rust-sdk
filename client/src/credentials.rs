use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Credentials used to sign an SLS request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credentials {
    access_key_id: String,
    access_key_secret: String,
    security_token: Option<String>,
}

impl Credentials {
    pub fn new(
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
        security_token: Option<String>,
    ) -> Self {
        Self {
            access_key_id: access_key_id.into(),
            access_key_secret: access_key_secret.into(),
            security_token,
        }
    }

    pub fn access_key(
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
    ) -> Self {
        Self::new(access_key_id, access_key_secret, None)
    }

    pub fn sts(
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
        security_token: impl Into<String>,
    ) -> Self {
        Self::new(
            access_key_id,
            access_key_secret,
            Some(security_token.into()),
        )
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

    pub(crate) fn validate(&self) -> bool {
        !self.access_key_id.is_empty() && !self.access_key_secret.is_empty()
    }
}

pub type CredentialsProviderError = Box<dyn Error + Send + Sync + 'static>;
pub type CredentialsFuture<'a> = Pin<
    Box<
        dyn Future<Output = std::result::Result<Credentials, CredentialsProviderError>> + Send + 'a,
    >,
>;

/// Supplies credentials immediately before each request attempt.
///
/// Implement this trait to refresh expiring STS credentials without rebuilding
/// the client.
pub trait CredentialsProvider: Send + Sync + 'static {
    fn credentials(&self) -> CredentialsFuture<'_>;
}

impl CredentialsProvider for Credentials {
    fn credentials(&self) -> CredentialsFuture<'_> {
        let credentials = self.clone();
        Box::pin(async move { Ok(credentials) })
    }
}

impl<T> CredentialsProvider for Arc<T>
where
    T: CredentialsProvider + ?Sized,
{
    fn credentials(&self) -> CredentialsFuture<'_> {
        (**self).credentials()
    }
}
