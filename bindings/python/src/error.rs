use aliyun_log_producer::{DeliveryError as RustDeliveryError, ProducerError as RustProducerError};
use pyo3::{
    create_exception,
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
};

create_exception!(aliyun_log_producer, ProducerError, PyRuntimeError);
create_exception!(aliyun_log_producer, EnqueueFullError, ProducerError);
create_exception!(aliyun_log_producer, ProducerClosedError, ProducerError);

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("EnqueueFullError", py.get_type::<EnqueueFullError>())?;
    m.add("ProducerClosedError", py.get_type::<ProducerClosedError>())?;
    m.add("ProducerError", py.get_type::<ProducerError>())?;
    Ok(())
}

pub(crate) fn producer_error(error: RustProducerError) -> PyErr {
    let message = error.to_string();
    match error {
        RustProducerError::Config(_) | RustProducerError::InvalidInput { .. } => {
            PyValueError::new_err(message)
        }
        RustProducerError::Closed { .. } => ProducerClosedError::new_err(message),
        RustProducerError::EnqueueFull { .. } => EnqueueFullError::new_err(message),
        _ => ProducerError::new_err(message),
    }
}

/// A terminal delivery failure, passed as callback data rather than raised.
#[pyclass(frozen, get_all, module = "aliyun_log_producer._native")]
pub(crate) struct DeliveryError {
    kind: &'static str,
    message: String,
    http_status: Option<u32>,
    error_code: Option<String>,
    request_id: Option<String>,
}

impl From<RustDeliveryError> for DeliveryError {
    fn from(error: RustDeliveryError) -> Self {
        let (kind, message) = match &error {
            RustDeliveryError::Server { message, .. } => ("server", message.clone()),
            RustDeliveryError::Network(message) => ("network", message.clone()),
            RustDeliveryError::Credentials(message) => ("credentials", message.clone()),
            RustDeliveryError::Timeout => ("timeout", error.to_string()),
            RustDeliveryError::InvalidResponse(message) => ("invalid_response", message.clone()),
            RustDeliveryError::Internal(message) => ("internal", message.clone()),
            _ => ("unknown", error.to_string()),
        };
        Self {
            kind,
            message,
            http_status: error.http_status(),
            error_code: error.error_code().map(str::to_owned),
            request_id: error.request_id().map(str::to_owned),
        }
    }
}

#[pymethods]
impl DeliveryError {
    fn __str__(&self) -> &str {
        &self.message
    }

    fn __repr__(&self) -> String {
        format!("DeliveryError(kind={:?}, message={:?}, http_status={:?}, error_code={:?}, request_id={:?})",
            self.kind, self.message, self.http_status, self.error_code, self.request_id)
    }
}
