use std::fmt;

use aliyun_log_rust_sdk::Error;

/// A final delivery failure for an accepted submission, reported through
/// [`crate::DeliveryResult`] after retry handling. Admission errors are returned
/// separately by [`crate::LogstoreWriter::send`] as [`crate::ProducerError`].
#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DeliveryError {
    /// SLS returned an error and no further retry will be made.
    Server {
        /// HTTP status of the service response.
        http_status: u32,
        /// Service error code, preserving the server's spelling.
        error_code: String,
        /// Error description supplied by the service.
        message: String,
        /// Service request ID, when available.
        request_id: Option<String>,
    },

    /// An HTTP transport failure remained after allowed retries.
    Network(String),

    /// Credentials could not be obtained after allowed retries.
    /// Service authentication errors use [`Self::Server`].
    Credentials(String),

    /// The overall [`crate::ProducerConfig::delivery_timeout`] expired.
    /// It includes batching, processing, HTTP requests and retry delays.
    Timeout,

    /// The service response could not be parsed after allowed retries.
    InvalidResponse(String),

    /// Local processing or another internal delivery operation failed.
    Internal(String),
}

impl DeliveryError {
    /// HTTP status for [`Self::Server`]; `None` for every other variant.
    pub fn http_status(&self) -> Option<u32> {
        match self {
            Self::Server { http_status, .. } => Some(*http_status),
            _ => None,
        }
    }

    /// Service error code for [`Self::Server`]; `None` for every other variant.
    pub fn error_code(&self) -> Option<&str> {
        match self {
            Self::Server { error_code, .. } => Some(error_code),
            _ => None,
        }
    }

    /// Service request ID for [`Self::Server`]; `None` for every other variant.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Server { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Server { .. } => "Server",
            Self::Network(_) => "Network",
            Self::Credentials(_) => "Credentials",
            Self::Timeout => "Timeout",
            Self::InvalidResponse(_) => "InvalidResponse",
            Self::Internal(_) => "Internal",
        }
    }

    #[cold]
    pub(crate) fn from_client(error: &Error) -> Self {
        match error {
            Error::Server {
                http_status,
                error_code,
                error_message,
                request_id,
            } => Self::Server {
                http_status: *http_status,
                error_code: error_code.clone(),
                message: error_message.clone(),
                request_id: request_id.clone(),
            },
            Error::Network(_) => Self::Network(error.to_string()),
            Error::Credentials(_) => Self::Credentials(error.to_string()),
            Error::ResponseParse(_) => Self::InvalidResponse(error.to_string()),
            _ => Self::Internal(error.to_string()),
        }
    }
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Server {
                http_status,
                error_code,
                message,
                request_id,
            } => {
                write!(
                    f,
                    "service error {error_code} (HTTP {http_status}): {message}"
                )?;
                if let Some(id) = request_id {
                    write!(f, "; request_id={id}")?;
                }
                Ok(())
            }
            Self::Network(reason) => write!(f, "network error: {reason}"),
            Self::Credentials(reason) => write!(f, "credentials error: {reason}"),
            Self::Timeout => f.write_str("delivery timed out"),
            Self::InvalidResponse(reason) => write!(f, "invalid service response: {reason}"),
            Self::Internal(reason) => write!(f, "internal delivery error: {reason}"),
        }
    }
}
