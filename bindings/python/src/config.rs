use std::time::Duration;

use crate::{arguments::Arguments, error::ConfigError};
use aliyun_log_producer::{Compression, ProducerConfig as RustConfig};
use aliyun_log_rust_sdk::static_credentials_provider;
use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};

pub(crate) fn duration(seconds: f64, name: &str) -> PyResult<Duration> {
    Duration::try_from_secs_f64(seconds).map_err(|_| {
        ConfigError::new_err(format!(
            "{name} must be finite, nonnegative and representable"
        ))
    })
}

/// Immutable configuration. Resource and endpoint validation completes at Producer creation.
#[pyclass(frozen, module = "aliyun_log_producer._native")]
pub(crate) struct ProducerConfig {
    pub(crate) inner: RustConfig,
    #[pyo3(get, name = "_credentials_provider")]
    pub(crate) credentials_provider: Option<Py<PyAny>>,
}

#[pymethods]
impl ProducerConfig {
    #[new]
    #[pyo3(signature = (*args, **kwargs), text_signature = "(*, endpoint, access_key_id=None, access_key_secret=None, security_token=None, credentials_provider=None, user_agent=None, compression='zstd', generate_pack_id=True, batch_size_threshold=None, batch_count_threshold=None, linger=None, buffer_bytes=None, processing_workers=None, callback_capacity=None, max_attempts=None, base_backoff=None, max_backoff=None, delivery_timeout=None)")]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let args = Arguments::new(
            args,
            kwargs,
            &[
                "endpoint",
                "access_key_id",
                "access_key_secret",
                "security_token",
                "credentials_provider",
                "user_agent",
                "compression",
                "generate_pack_id",
                "batch_size_threshold",
                "batch_count_threshold",
                "linger",
                "buffer_bytes",
                "processing_workers",
                "callback_capacity",
                "max_attempts",
                "base_backoff",
                "max_backoff",
                "delivery_timeout",
            ],
            0,
            ConfigError::new_err,
        )?;
        let endpoint: String = args.required("endpoint")?;
        let user_agent: Option<String> = args.optional("user_agent")?;
        let access_key_id: Option<String> = args.optional("access_key_id")?;
        let access_key_secret: Option<String> = args.optional("access_key_secret")?;
        let security_token: Option<String> = args.optional("security_token")?;
        let credentials_provider: Option<Bound<'_, PyAny>> =
            args.optional("credentials_provider")?;
        let compression = args.default("compression", "zstd".to_owned())?;
        let compression = match compression.as_str() {
            "zstd" => Compression::Zstd,
            "lz4" => Compression::Lz4,
            _ => return Err(ConfigError::new_err("compression must be 'zstd' or 'lz4'")),
        };
        let mut inner = RustConfig::default()
            .with_endpoint(endpoint)
            .with_user_agent(user_agent.as_deref().unwrap_or(concat!(
                "aliyun-log-python-producer/",
                env!("CARGO_PKG_VERSION")
            )))
            .with_compression(compression)
            .with_generate_pack_id(args.default("generate_pack_id", true)?);
        let credentials_provider = credentials_provider.map(Bound::unbind);
        inner = if credentials_provider.is_some() {
            if access_key_id.is_some() || access_key_secret.is_some() || security_token.is_some() {
                return Err(ConfigError::new_err(
                    "credentials_provider cannot be combined with access_key_id, access_key_secret or security_token",
                ));
            }
            inner
        } else {
            let (Some(id), Some(secret)) = (access_key_id, access_key_secret) else {
                return Err(ConfigError::new_err(
                    "provide credentials_provider or both access_key_id and access_key_secret",
                ));
            };
            let provider = static_credentials_provider(id, secret, security_token)
                .map_err(|error| ConfigError::new_err(error.to_string()))?;
            inner.with_credentials_provider(provider)
        };
        // Omitted options inherit the Rust defaults rather than a second set of defaults.
        macro_rules! option {
            ($value:ident, $setter:ident) => {
                if let Some(value) = args.optional(stringify!($value))? {
                    inner = inner.$setter(value);
                }
            };
            ($value:ident, $setter:ident, duration) => {
                if let Some(value) = args.optional(stringify!($value))? {
                    inner = inner.$setter(duration(value, stringify!($value))?);
                }
            };
        }
        option!(batch_size_threshold, with_batch_size_threshold);
        option!(batch_count_threshold, with_batch_count_threshold);
        option!(buffer_bytes, with_buffer_bytes);
        option!(processing_workers, with_processing_workers);
        option!(callback_capacity, with_callback_capacity);
        option!(max_attempts, with_max_attempts);
        option!(linger, with_linger, duration);
        option!(base_backoff, with_base_backoff, duration);
        option!(max_backoff, with_max_backoff, duration);
        option!(delivery_timeout, with_delivery_timeout, duration);
        Ok(Self {
            inner,
            credentials_provider,
        })
    }

    fn __repr__(&self) -> String {
        // Rust's Debug deliberately redacts all authentication data.
        format!("{:?}", self.inner)
    }
}
