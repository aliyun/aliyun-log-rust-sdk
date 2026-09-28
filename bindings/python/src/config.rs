use std::time::Duration;

use aliyun_log_producer::{Compression, ProducerConfig as RustConfig};
use aliyun_log_rust_sdk::static_credentials_provider;
use pyo3::{exceptions::PyValueError, prelude::*};

pub(crate) fn duration(seconds: f64, name: &str) -> PyResult<Duration> {
    Duration::try_from_secs_f64(seconds).map_err(|_| {
        PyValueError::new_err(format!(
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
    #[pyo3(signature = (*, endpoint, access_key_id=None, access_key_secret=None, security_token=None,
        credentials_provider=None, user_agent=None,
        compression="zstd", generate_pack_id=true, batch_size_threshold=None,
        batch_count_threshold=None, linger=None, buffer_bytes=None, processing_workers=None,
        callback_capacity=None, max_attempts=None, base_backoff=None, max_backoff=None,
        delivery_timeout=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        endpoint: String,
        access_key_id: Option<String>,
        access_key_secret: Option<String>,
        security_token: Option<String>,
        credentials_provider: Option<Bound<'_, PyAny>>,
        user_agent: Option<&str>,
        compression: &str,
        generate_pack_id: bool,
        batch_size_threshold: Option<usize>,
        batch_count_threshold: Option<usize>,
        linger: Option<f64>,
        buffer_bytes: Option<usize>,
        processing_workers: Option<usize>,
        callback_capacity: Option<usize>,
        max_attempts: Option<u32>,
        base_backoff: Option<f64>,
        max_backoff: Option<f64>,
        delivery_timeout: Option<f64>,
    ) -> PyResult<Self> {
        let compression = match compression {
            "zstd" => Compression::Zstd,
            "lz4" => Compression::Lz4,
            _ => return Err(PyValueError::new_err("compression must be 'zstd' or 'lz4'")),
        };
        let mut inner = RustConfig::default()
            .with_endpoint(endpoint)
            .with_user_agent(user_agent.unwrap_or(concat!(
                "aliyun-log-python-producer/",
                env!("CARGO_PKG_VERSION")
            )))
            .with_compression(compression)
            .with_generate_pack_id(generate_pack_id);
        let credentials_provider = credentials_provider.map(Bound::unbind);
        inner = if credentials_provider.is_some() {
            if access_key_id.is_some() || access_key_secret.is_some() || security_token.is_some() {
                return Err(PyValueError::new_err(
                    "credentials_provider cannot be combined with access_key_id, access_key_secret or security_token",
                ));
            }
            inner
        } else {
            let (Some(id), Some(secret)) = (access_key_id, access_key_secret) else {
                return Err(PyValueError::new_err(
                    "provide credentials_provider or both access_key_id and access_key_secret",
                ));
            };
            let provider = static_credentials_provider(id, secret, security_token)
                .map_err(|error| PyValueError::new_err(error.to_string()))?;
            inner.with_credentials_provider(provider)
        };
        // Omitted options inherit the Rust defaults rather than a second set of defaults.
        macro_rules! option {
            ($value:ident, $setter:ident) => {
                if let Some(value) = $value {
                    inner = inner.$setter(value);
                }
            };
            ($value:ident, $setter:ident, duration) => {
                if let Some(value) = $value {
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
