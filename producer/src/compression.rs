use std::{cell::RefCell, io};

/// Compression algorithm used for log delivery.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum Compression {
    /// LZ4 compression.
    Lz4,
    /// Zstandard compression (default).
    #[default]
    Zstd,
}

thread_local! {
    // Reuse a native compression context per encoding thread. It is destroyed when
    // that producer-owned Rayon thread exits; no global pool or extra threads.
    static ZSTD: RefCell<Option<zstd::bulk::Compressor<'static>>> = const { RefCell::new(None) };
}

impl Compression {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Lz4 => "lz4",
            Self::Zstd => "zstd",
        }
    }

    pub(crate) fn compress(self, raw: &[u8]) -> io::Result<Vec<u8>> {
        match self {
            Self::Lz4 => lz4::block::compress(raw, None, false),
            Self::Zstd => ZSTD.with(|context| {
                let mut context = context.borrow_mut();
                if context.is_none() {
                    *context = Some(zstd::bulk::Compressor::new(1)?);
                }
                context.as_mut().expect("context initialized").compress(raw)
            }),
        }
    }
}
