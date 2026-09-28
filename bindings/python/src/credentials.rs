use crate::{arguments::Arguments, error::InvalidArgumentError};
use aliyun_log_rust_sdk::{Credentials as RustCredentials, ExternalManagedCredentials};
use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};
use std::time::{Duration, SystemTime};

/// One immutable, internally consistent credentials snapshot.
#[pyclass(frozen, module = "aliyun_log_producer._native")]
pub(crate) struct Credentials {
    pub(crate) inner: RustCredentials,
}

#[pymethods]
impl Credentials {
    #[new]
    #[pyo3(signature = (*args, **kwargs), text_signature = "(*, access_key_id, access_key_secret, security_token=None, expires_at=None)")]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let args = Arguments::new(
            args,
            kwargs,
            &[
                "access_key_id",
                "access_key_secret",
                "security_token",
                "expires_at",
            ],
            0,
            InvalidArgumentError::new_err,
        )?;
        let access_key_id: String = args.required("access_key_id")?;
        let access_key_secret: String = args.required("access_key_secret")?;
        let security_token: Option<String> = args.optional("security_token")?;
        let expires_at: Option<u64> = args.optional("expires_at")?;
        let mut inner = RustCredentials::new(access_key_id, access_key_secret)
            .map_err(|e| InvalidArgumentError::new_err(e.to_string()))?;
        if let Some(token) = security_token {
            inner = inner.with_security_token(token);
        }
        if let Some(expiration) = expires_at {
            let expiration = SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(expiration))
                .ok_or_else(|| {
                    InvalidArgumentError::new_err(
                        "expires_at is outside the supported timestamp range",
                    )
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
