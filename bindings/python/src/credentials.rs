use aliyun_log_rust_sdk::{Credentials as RustCredentials, ExternalManagedCredentials};
use pyo3::{exceptions::PyValueError, prelude::*};
use std::time::{Duration, SystemTime};

/// One immutable, internally consistent credentials snapshot.
#[pyclass(frozen, module = "aliyun_log_producer._native")]
pub(crate) struct Credentials {
    pub(crate) inner: RustCredentials,
}

#[pymethods]
impl Credentials {
    #[new]
    #[pyo3(signature = (*, access_key_id, access_key_secret, security_token=None, expires_at=None))]
    fn new(
        access_key_id: String,
        access_key_secret: String,
        security_token: Option<String>,
        expires_at: Option<u64>,
    ) -> PyResult<Self> {
        let mut inner = RustCredentials::new(access_key_id, access_key_secret)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        if let Some(token) = security_token {
            inner = inner.with_security_token(token);
        }
        if let Some(expiration) = expires_at {
            let expiration = SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(expiration))
                .ok_or_else(|| {
                    PyValueError::new_err("expires_at is outside the supported timestamp range")
                })?;
            inner = inner.with_expiration(expiration);
        }
        Ok(Self { inner })
    }

    #[getter]
    fn access_key_id(&self) -> &str {
        self.inner.access_key_id()
    }
    #[getter]
    fn access_key_secret(&self) -> &str {
        self.inner.access_key_secret()
    }
    #[getter]
    fn security_token(&self) -> Option<&str> {
        self.inner.security_token()
    }
    #[getter]
    fn expires_at(&self) -> Option<u64> {
        self.inner.expiration().map(|time| {
            time.duration_since(SystemTime::UNIX_EPOCH)
                .expect("validated timestamp")
                .as_secs()
        })
    }
    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

/// Python owns refresh scheduling; this object only publishes Rust snapshots.
#[pyclass(
    name = "_ExternalCredentials",
    frozen,
    module = "aliyun_log_producer._native"
)]
pub(crate) struct ExternalCredentials {
    pub(crate) inner: ExternalManagedCredentials,
}

#[pymethods]
impl ExternalCredentials {
    #[new]
    fn new(initial: &Credentials) -> Self {
        Self {
            inner: ExternalManagedCredentials::new(initial.inner.clone()),
        }
    }

    fn set(&self, credentials: &Credentials) {
        self.inner.set(credentials.inner.clone());
    }
}
