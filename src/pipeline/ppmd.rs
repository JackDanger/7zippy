//! PPMd coder — in-tree, backed directly by ppmd-rust.
//!
//! 7z's PPMd codec (method ID `[0x03, 0x04, 0x01]`) stores a PPMd7
//! (PPMdH) bitstream. The coder properties are 5 bytes:
//! - byte 0: `order` (PPMd model order; typically 6–16)
//! - bytes 1–4: `mem_size` as a little-endian u32 (bytes)
//!
//! The uncompressed size is stored in the archive and must be provided to
//! the decoder to know when to stop reading (PPMd7 has no end-of-stream
//! marker in the 7z variant).

use std::io::{Read, Write};

use ppmd_rust::{Ppmd7Decoder, Ppmd7Encoder};

use crate::container::MethodId;
use crate::error::{SevenZippyError, SevenZippyResult};
use crate::pipeline::Coder;

/// Default PPMd7 model order. Matches 7zz's default (`-mo=6`).
const DEFAULT_ORDER: u32 = 6;

/// Default memory size for encoding (16 MiB — good for typical archive payloads).
const DEFAULT_MEM_SIZE: u32 = 16 * 1024 * 1024;

/// PPMd coder backed by ppmd-rust.
pub struct PpmdCoder {
    /// PPMd7 model order.
    order: u32,
    /// PPMd7 model memory size in bytes.
    mem_size: u32,
}

impl PpmdCoder {
    /// Create a coder for encoding with the default parameters.
    pub fn new() -> Self {
        Self {
            order: DEFAULT_ORDER,
            mem_size: DEFAULT_MEM_SIZE,
        }
    }

    /// Create a coder from the 5-byte properties blob stored in a 7z archive.
    ///
    /// Returns an error if the properties are malformed.
    pub fn from_props(props: &[u8]) -> SevenZippyResult<Self> {
        if props.len() < 5 {
            return Err(SevenZippyError::Coder(
                format!(
                    "PPMd properties too short: expected 5 bytes, got {}",
                    props.len()
                )
                .into(),
            ));
        }
        let order = props[0] as u32;
        let mem_size = u32::from_le_bytes([props[1], props[2], props[3], props[4]]);
        Ok(Self { order, mem_size })
    }
}

impl Default for PpmdCoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Coder for PpmdCoder {
    fn decode(&self, packed: &[u8], unpacked_size: u64) -> SevenZippyResult<Vec<u8>> {
        let cursor = std::io::Cursor::new(packed);
        let mut decoder = Ppmd7Decoder::new(cursor, self.order, self.mem_size)
            .map_err(|e| SevenZippyError::Coder(e.to_string().into()))?;
        let mut out = vec![0u8; unpacked_size as usize];
        decoder
            .read_exact(&mut out)
            .map_err(|e| SevenZippyError::Coder(e.to_string().into()))?;
        Ok(out)
    }

    fn encode(&self, unpacked: &[u8]) -> SevenZippyResult<Vec<u8>> {
        let mut out = Vec::new();
        let mut encoder = Ppmd7Encoder::new(&mut out, self.order, self.mem_size)
            .map_err(|e| SevenZippyError::Coder(e.to_string().into()))?;
        encoder
            .write_all(unpacked)
            .map_err(|e| SevenZippyError::Coder(e.to_string().into()))?;
        encoder
            .finish(false)
            .map_err(|e| SevenZippyError::Coder(e.to_string().into()))?;
        Ok(out)
    }

    fn method_id(&self) -> MethodId {
        MethodId::ppmd()
    }

    fn properties(&self) -> Vec<u8> {
        let mut props = Vec::with_capacity(5);
        props.push(self.order as u8);
        props.extend_from_slice(&self.mem_size.to_le_bytes());
        props
    }
}
