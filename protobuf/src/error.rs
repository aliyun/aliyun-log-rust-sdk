#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[cfg(feature = "prost")]
    #[error("Fail to decode: {0}")]
    Prost(#[from] prost::DecodeError),

    #[cfg(feature = "quick-protobuf")]
    #[error("Fail to decode: {0}")]
    Quick(#[from] quick_protobuf::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[cfg(feature = "prost")]
    #[error("Fail to encode: {0}")]
    Prost(#[from] prost::EncodeError),

    #[cfg(feature = "quick-protobuf")]
    #[error("Fail to encode: {0}")]
    Quick(#[from] quick_protobuf::Error),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Fail to decode: {0}")]
    Decode(#[from] DecodeError),

    #[error("Fail to encode: {0}")]
    Encode(#[from] EncodeError),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
