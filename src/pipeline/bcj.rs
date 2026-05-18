//! BCJ (Branch/Call/Jump) family filter coders — pure-Rust, in-tree.
//!
//! BCJ filters are pre-conditioners that convert architecture-specific
//! relative branch offsets to absolute addresses, making byte patterns more
//! repetitive and thus improving subsequent compression (typically LZMA).
//!
//! Simple BCJ (6 architectures, 1-stream, stateless) is folded in-tree;
//! the complex BCJ2 (x86-only, 4-stream, range-coded) lives in `bcjzippy`.
//!
//! # 7z method IDs
//!
//! | Filter        | Method ID bytes                  |
//! |---------------|----------------------------------|
//! | BCJ x86       | `[0x03, 0x03, 0x01, 0x03]`       |
//! | BCJ PowerPC   | `[0x03, 0x03, 0x02, 0x05]`       |
//! | BCJ IA64      | `[0x03, 0x03, 0x04, 0x01]`       |
//! | BCJ ARM       | `[0x03, 0x03, 0x05, 0x01]`       |
//! | BCJ ARM-Thumb | `[0x03, 0x03, 0x07, 0x01]`       |
//! | BCJ SPARC     | `[0x03, 0x03, 0x08, 0x05]`       |
//!
//! # Properties
//!
//! BCJ filters in 7z carry an optional 4-byte start-position property. When
//! absent (the common case), `start_pos = 0`. When present, bytes 0–3 are a
//! LE u32.
//!
//! # Encoding vs decoding symmetry
//!
//! BCJ is a self-inverse filter: applying it twice returns the original data.

use std::io::{Read, Write};

use lzma_rust2::filter::bcj::{BcjReader, BcjWriter};

use crate::container::MethodId;
use crate::error::{SevenZippyError, SevenZippyResult};
use crate::pipeline::Coder;

// ── BCJ architecture variants ─────────────────────────────────────────────────

/// Which ISA the BCJ filter targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BcjArch {
    X86,
    PowerPc,
    Ia64,
    Arm,
    ArmThumb,
    Sparc,
}

impl BcjArch {
    /// Return the 7z method ID for this BCJ variant.
    pub fn method_id(self) -> MethodId {
        match self {
            BcjArch::X86 => MethodId(vec![0x03, 0x03, 0x01, 0x03]),
            BcjArch::PowerPc => MethodId(vec![0x03, 0x03, 0x02, 0x05]),
            BcjArch::Ia64 => MethodId(vec![0x03, 0x03, 0x04, 0x01]),
            BcjArch::Arm => MethodId(vec![0x03, 0x03, 0x05, 0x01]),
            BcjArch::ArmThumb => MethodId(vec![0x03, 0x03, 0x07, 0x01]),
            BcjArch::Sparc => MethodId(vec![0x03, 0x03, 0x08, 0x05]),
        }
    }
}

// ── in-tree BCJ encode/decode helpers ────────────────────────────────────────

fn bcj_encode(data: &[u8], arch: BcjArch, pc: usize) -> Vec<u8> {
    let mut w = match arch {
        BcjArch::X86 => BcjWriter::new_x86(Vec::new(), pc),
        BcjArch::PowerPc => BcjWriter::new_ppc(Vec::new(), pc),
        BcjArch::Ia64 => BcjWriter::new_ia64(Vec::new(), pc),
        BcjArch::Arm => BcjWriter::new_arm(Vec::new(), pc),
        BcjArch::ArmThumb => BcjWriter::new_arm_thumb(Vec::new(), pc),
        BcjArch::Sparc => BcjWriter::new_sparc(Vec::new(), pc),
    };
    w.write_all(data).expect("BcjWriter write_all");
    w.finish().expect("BcjWriter finish")
}

fn bcj_decode(data: &[u8], arch: BcjArch, pc: usize) -> Vec<u8> {
    let cursor = std::io::Cursor::new(data);
    let mut r = match arch {
        BcjArch::X86 => BcjReader::new_x86(cursor, pc),
        BcjArch::PowerPc => BcjReader::new_ppc(cursor, pc),
        BcjArch::Ia64 => BcjReader::new_ia64(cursor, pc),
        BcjArch::Arm => BcjReader::new_arm(cursor, pc),
        BcjArch::ArmThumb => BcjReader::new_arm_thumb(cursor, pc),
        BcjArch::Sparc => BcjReader::new_sparc(cursor, pc),
    };
    let mut out = Vec::with_capacity(data.len());
    r.read_to_end(&mut out).expect("BcjReader read_to_end");
    out
}

// ── BcjCoder ─────────────────────────────────────────────────────────────────

/// BCJ filter coder — pure-Rust, in-tree, backed by lzma-rust2's BCJ module.
pub struct BcjCoder {
    arch: BcjArch,
    /// Starting byte offset for the filter (LE u32 from the properties blob, or 0).
    start_pos: u32,
}

impl BcjCoder {
    /// Create a BCJ coder for the given architecture with `start_pos = 0`.
    pub fn new(arch: BcjArch) -> Self {
        Self { arch, start_pos: 0 }
    }

    /// Create a BCJ coder from the optional property blob stored in a 7z archive.
    ///
    /// The properties are either empty (→ `start_pos = 0`) or 4 bytes (LE u32).
    pub fn from_arch_props(arch: BcjArch, props: &[u8]) -> SevenZippyResult<Self> {
        let start_pos = match props.len() {
            0 => 0,
            4 => u32::from_le_bytes([props[0], props[1], props[2], props[3]]),
            n => {
                return Err(SevenZippyError::Coder(
                    format!("BCJ properties must be 0 or 4 bytes, got {n}").into(),
                ))
            }
        };
        Ok(Self { arch, start_pos })
    }
}

impl Coder for BcjCoder {
    fn decode(&self, packed: &[u8], _unpacked_size: u64) -> SevenZippyResult<Vec<u8>> {
        Ok(bcj_decode(packed, self.arch, self.start_pos as usize))
    }

    fn encode(&self, unpacked: &[u8]) -> SevenZippyResult<Vec<u8>> {
        Ok(bcj_encode(unpacked, self.arch, self.start_pos as usize))
    }

    fn method_id(&self) -> MethodId {
        self.arch.method_id()
    }

    fn properties(&self) -> Vec<u8> {
        if self.start_pos == 0 {
            Vec::new()
        } else {
            self.start_pos.to_le_bytes().to_vec()
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn self_inverse_round_trip(arch: BcjArch, data: &[u8]) {
        let coder = BcjCoder::new(arch);
        let encoded = coder.encode(data).unwrap();
        let decoded = coder.decode(&encoded, data.len() as u64).unwrap();
        assert_eq!(decoded, data, "BCJ {arch:?} round-trip failed");
    }

    #[test]
    fn x86_round_trip() {
        let data: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
        self_inverse_round_trip(BcjArch::X86, &data);
    }

    #[test]
    fn arm_round_trip() {
        let data: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
        self_inverse_round_trip(BcjArch::Arm, &data);
    }

    #[test]
    fn arm_thumb_round_trip() {
        let data: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
        self_inverse_round_trip(BcjArch::ArmThumb, &data);
    }

    #[test]
    fn ppc_round_trip() {
        let data: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
        self_inverse_round_trip(BcjArch::PowerPc, &data);
    }

    #[test]
    fn ia64_round_trip() {
        let data: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
        self_inverse_round_trip(BcjArch::Ia64, &data);
    }

    #[test]
    fn sparc_round_trip() {
        let data: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
        self_inverse_round_trip(BcjArch::Sparc, &data);
    }

    #[test]
    fn empty_input_round_trip() {
        for arch in [
            BcjArch::X86,
            BcjArch::Arm,
            BcjArch::ArmThumb,
            BcjArch::PowerPc,
            BcjArch::Ia64,
            BcjArch::Sparc,
        ] {
            self_inverse_round_trip(arch, &[]);
        }
    }

    #[test]
    fn from_arch_props_empty() {
        let coder = BcjCoder::from_arch_props(BcjArch::X86, &[]).unwrap();
        assert_eq!(coder.start_pos, 0);
        assert_eq!(coder.properties(), vec![]);
    }

    #[test]
    fn from_arch_props_four_bytes() {
        let coder = BcjCoder::from_arch_props(BcjArch::X86, &[0x00, 0x10, 0x00, 0x00]).unwrap();
        assert_eq!(coder.start_pos, 0x1000);
        assert_eq!(coder.properties(), vec![0x00, 0x10, 0x00, 0x00]);
    }

    #[test]
    fn from_arch_props_invalid_length_is_error() {
        let result = BcjCoder::from_arch_props(BcjArch::X86, &[0x01, 0x02]);
        assert!(result.is_err());
    }
}
