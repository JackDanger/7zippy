//! LZMA2 coder — raw LZMA2 chunk stream via the `xzippy` sub-crate.
//!
//! 7z's LZMA2 codec (method ID `[0x21]`) stores a raw LZMA2 chunk stream with
//! a 1-byte properties blob encoding the dictionary size:
//!
//! - `b == 40` → `dict_size = 0xFFFF_FFFF`
//! - `b < 40`  → `dict_size = (2 | (b & 1)) << ((b >> 1) + 11)`
//!
//! LZMA2 extends LZMA with multi-chunk streaming and optional uncompressed
//! chunk passthrough, making it suitable for large files and multi-threaded
//! encoding. The `xzippy` sub-crate wraps `lzma-rust2`'s `Lzma2Writer` /
//! `Lzma2Reader` in Phase 1; Phase 2 will replace these with xzippy's
//! own native chunk-orchestration implementation.

use crate::container::MethodId;
use crate::error::{SevenZippyError, SevenZippyResult};
use crate::pipeline::Coder;

/// Default dictionary size for LZMA2 encode: 256 KiB.
///
/// This matches 7zz's default for small files (`7zz a -m0=lzma2`).
/// Use `Lzma2Coder::with_dict_size` to override for larger files.
const DEFAULT_DICT_SIZE: u32 = 262_144; // 256 KiB

/// LZMA2 coder backed by the `xzippy` sub-crate (Phase 1).
pub struct Lzma2Coder {
    /// The 7z properties byte from the archive header.
    /// `None` when constructing for encode (will use `dict_size`).
    props_byte: Option<u8>,
    /// Dictionary size for encoding. Ignored for decode (props_byte used).
    dict_size: u32,
}

impl Lzma2Coder {
    /// Construct an LZMA2 coder with the default dictionary size (256 KiB).
    pub fn new() -> Self {
        Self {
            props_byte: None,
            dict_size: DEFAULT_DICT_SIZE,
        }
    }

    /// Construct an LZMA2 coder from the 7z archive's properties bytes.
    ///
    /// `props_bytes` must be exactly 1 byte.
    pub fn with_props(props_bytes: Vec<u8>) -> SevenZippyResult<Self> {
        if props_bytes.len() != 1 {
            return Err(SevenZippyError::Coder(
                format!(
                    "LZMA2 expects exactly 1 props byte, got {}",
                    props_bytes.len()
                )
                .into(),
            ));
        }
        let b = props_bytes[0];
        if b > 40 {
            return Err(SevenZippyError::Coder(
                format!("LZMA2 props byte {b:#04x} is out of range (max 40)").into(),
            ));
        }
        // Compute dict_size for use in encode (if ever needed)
        let dict_size = if b == 40 {
            u32::MAX
        } else {
            (2u32 | (b as u32 & 1)) << ((b as u32 >> 1) + 11)
        };
        Ok(Self {
            props_byte: Some(b),
            dict_size,
        })
    }
}

impl Default for Lzma2Coder {
    fn default() -> Self {
        Self::new()
    }
}

impl Coder for Lzma2Coder {
    fn decode(&self, packed: &[u8], _unpacked_size: u64) -> SevenZippyResult<Vec<u8>> {
        #[cfg(feature = "lzma2")]
        {
            let props = self.properties();
            xzippy::decode::decode_7z(packed, &props, _unpacked_size)
                .map_err(|e| SevenZippyError::Coder(Box::new(e)))
        }
        #[cfg(not(feature = "lzma2"))]
        {
            Err(SevenZippyError::missing_coder("LZMA2"))
        }
    }

    fn encode(&self, unpacked: &[u8]) -> SevenZippyResult<Vec<u8>> {
        #[cfg(feature = "lzma2")]
        {
            let (_, compressed) = xzippy::encode::encode_7z(unpacked, self.dict_size)
                .map_err(|e| SevenZippyError::Coder(Box::new(e)))?;
            Ok(compressed)
        }
        #[cfg(not(feature = "lzma2"))]
        {
            Err(SevenZippyError::missing_coder("LZMA2"))
        }
    }

    fn method_id(&self) -> MethodId {
        MethodId::lzma2()
    }

    fn properties(&self) -> Vec<u8> {
        if let Some(b) = self.props_byte {
            return vec![b];
        }
        // Encode: compute the props byte from dict_size.
        let b = xzippy::encode::dict_size_to_props_byte(self.dict_size);
        vec![b]
    }
}
