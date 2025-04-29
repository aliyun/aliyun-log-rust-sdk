use serde::Deserialize;

pub type Result<T, E = crate::Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// This error is caused by invalid configuration for the client, such as invalid endpoint, invalid access key, etc.
    #[error("Config error: {0}")]
    InvalidConfig(#[from] ConfigError),

    /// The request is invalid and thus will not be sent to the server, this may be caused by missing required
    /// parameters, invalid parameters, etc.
    #[error("Invalid request: {0}")]
    RequestPreparation(#[from] RequestError),

    /// The response from server is invalid, which can not be parsed correctly, this may be caused by
    /// network error, server error, or other reasons.
    #[error("Invalid response from server: {0}")]
    ResponseParse(#[from] ResponseError),

    /// This error is caused by network error, such as connection timeout, DNS resolution error, etc.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// The server returns an error response with error code and message.
    #[error("Server error: code={error_code}, message={error_message}, httpStatus={http_status}, requestId={request_id:?}")]
    Server {
        error_code: String,
        error_message: String,
        http_status: u32,
        request_id: Option<String>,
    },

    #[error("Other error: {0}")]
    Other(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("Invalid endpoint: {0}")]
    InvalidEndpoint(String),

    #[error("Invalid access key")]
    InvalidAccessKey,

    #[error("Invalid client configuration: {0}")]
    InvalidClientConfig(#[source] anyhow::Error),

    #[error("Invalid url: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Invalid client config: {0}")]
    ClientBuilder(#[from] reqwest::Error),

    #[error("Invalid configuration: {0}")]
    Other(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct RequestError(#[from] RequestErrorKind);

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum RequestErrorKind {
    #[error("Missing required parameter: {0}")]
    MissingRequiredParameter(String),

    #[error("Failed to compress data: {0}")]
    Compression(#[from] CompressionError),

    #[error("Failed to encode request to JSON: {0}")]
    JsonEncode(#[from] serde_json::Error),

    #[error("Failed to serialize protobuf: {0}")]
    ProtobufSerialize(#[from] aliyun_log_sdk_protobuf::Error),

    #[error("Signature error: {0}")]
    Signature(#[from] aliyun_log_sdk_sign::Error),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CompressionError {
    #[error("{0}")]
    Lz4(#[from] std::io::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum DecompressionError {
    #[error("{0}")]
    Lz4(#[from] std::io::Error),

    #[error("Unsupported compress type: {0}")]
    UnsupportedCompressType(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct ResponseError(#[from] ResponseErrorKind);

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ResponseErrorKind {
    #[error(
        "Failed to decompress: {source}, compress_type={compress_type}, request_id={request_id:?}"
    )]
    Decompression {
        #[source]
        source: DecompressionError,
        compress_type: String,
        request_id: Option<String>,
    },

    #[error("Failed to decode JSON response: {source}, request_id={request_id:?}")]
    JsonDecode {
        #[source]
        source: serde_json::Error,
        request_id: Option<String>,
    },

    #[error("Failed to deserialize protobuf: {source}, request_id={request_id:?}")]
    ProtobufDeserialize {
        #[source]
        source: aliyun_log_sdk_protobuf::Error,
        request_id: Option<String>,
    },
}

pub(crate) type ResponseResult<T> = std::result::Result<T, ResponseError>;

impl Error {
    pub(crate) fn server_error(
        status: http::StatusCode,
        request_id: Option<String>,
        body: &[u8],
    ) -> Self {
        let result: std::result::Result<ServerError, serde_json::Error> =
            serde_json::from_slice(body);
        match result {
            Ok(server_error) => Error::Server {
                error_code: server_error.error_code,
                error_message: server_error.error_message,
                http_status: status.as_u16() as u32,
                request_id,
            },
            Err(err) => ResponseError(ResponseErrorKind::JsonDecode {
                source: err,
                request_id,
            })
            .into(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct ServerError {
    #[serde(rename = "errorCode")]
    error_code: String,

    #[serde(rename = "errorMessage")]
    error_message: String,
}
