use std::fmt::Display;

use crate::{CompressionError, DecompressionError};

#[non_exhaustive]
pub(crate) enum CompressType {
    Lz4,
}

impl Display for CompressType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressType::Lz4 => write!(f, "lz4"),
        }
    }
}

impl TryFrom<&str> for CompressType {
    type Error = DecompressionError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "lz4" => Ok(CompressType::Lz4),
            _ => Err(DecompressionError::UnsupportedCompressType(
                value.to_string(),
            )),
        }
    }
}

pub(crate) fn compress(
    body: impl AsRef<[u8]>,
    compress_type: &CompressType,
) -> std::result::Result<Vec<u8>, CompressionError> {
    match compress_type {
        CompressType::Lz4 => {
            let compressed = lz4::block::compress(body.as_ref(), None, false)?;
            Ok(compressed)
        }
    }
}

#[allow(dead_code)]
pub(crate) fn decompress(
    body: impl AsRef<[u8]>,
    compress_type: impl AsRef<str>,
    raw_size: usize,
) -> std::result::Result<Vec<u8>, DecompressionError> {
    let compress_type: CompressType = compress_type.as_ref().try_into()?;
    do_decompress(body, &compress_type, raw_size)
}

pub(crate) fn do_decompress(
    body: impl AsRef<[u8]>,
    compress_type: &CompressType,
    raw_size: usize,
) -> std::result::Result<Vec<u8>, DecompressionError> {
    match compress_type {
        CompressType::Lz4 => {
            let decompressed = lz4::block::decompress(body.as_ref(), Some(raw_size as i32))?;
            Ok(decompressed)
        }
    }
}
