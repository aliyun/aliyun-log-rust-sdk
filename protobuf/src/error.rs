#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct DecodeError(#[from] quick_protobuf::Error);

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct EncodeError(#[from] quick_protobuf::Error);

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Fail to decode: {0}")]
    Decode(#[from] DecodeError),

    #[error("Fail to encode: {0}")]
    Encode(#[from] EncodeError),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
