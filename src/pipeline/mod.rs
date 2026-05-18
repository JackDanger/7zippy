//! Coder pipeline dispatch: given a `Folder` and packed bytes, produce unpacked bytes.
//!
//! The `pipeline` module is the bridge between the container parser (`container::`)
//! and the codec implementations (in-tree Copy + sibling crates). `dispatch.rs` is
//! the single place where each codec plugs in.

#[cfg(feature = "aes")]
pub mod aes_folder;
#[cfg(feature = "bcj")]
pub mod bcj;
#[cfg(feature = "bcj2")]
pub mod bcj2_folder;
#[cfg(feature = "bzip2")]
pub mod bzip2;
mod coder_trait;
mod copy;
#[cfg(feature = "deflate")]
pub mod deflate;
#[cfg(feature = "deflate64")]
pub mod deflate64;
#[cfg(feature = "delta")]
pub mod delta;
mod dispatch;
#[cfg(feature = "lzma")]
pub mod lzma;
#[cfg(feature = "lzma2")]
pub mod lzma2;
#[cfg(feature = "ppmd")]
pub mod ppmd;

pub use coder_trait::Coder;
pub use copy::CopyCoder;
pub use dispatch::{coder_for, coder_for_method};

use crate::container::crc::crc32;
use crate::container::{Coder as CoderMeta, Folder, MethodId};
use crate::error::{SevenZippyError, SevenZippyResult};

/// Decode one folder's packed bytes through its coder pipeline, producing
/// the final unpacked stream.
///
/// `packed_streams` contains the packed byte slices for this folder, one per
/// unbound input stream (as determined by `folder.packed_stream_indices`).
/// For single-coder folders, exactly one slice is expected. For BCJ2 folders,
/// exactly four slices are expected.
///
/// For AES-encrypted folders, use [`decode_folder_with_password`] instead;
/// this function returns `SevenZippyError::EncryptedContent` for such folders.
pub fn decode_folder(folder: &Folder, packed_streams: &[&[u8]]) -> SevenZippyResult<Vec<u8>> {
    // ── AES-encrypted folder ─────────────────────────────────────────────────
    #[cfg(feature = "aes")]
    if aes_folder::has_aes_coder(folder) {
        return Err(SevenZippyError::encrypted_content(
            "archive is AES-encrypted; use extract_with_password() and supply a password",
        ));
    }

    // ── BCJ2 multi-coder folder ──────────────────────────────────────────────
    #[cfg(feature = "bcj2")]
    if bcj2_folder::is_bcj2_lzma_folder(folder) {
        return bcj2_folder::decode_bcj2_folder(folder, packed_streams);
    }

    // ── Single-coder folder (all other codecs) ───────────────────────────────
    if folder.coders.len() != 1 || !folder.bonds.is_empty() {
        return Err(SevenZippyError::not_yet_implemented(
            "multi-coder folder pipeline",
        ));
    }
    let packed = packed_streams.first().copied().unwrap_or(&[]);
    let coder_meta = &folder.coders[0];
    let coder = coder_for(coder_meta)?;
    let unpack_size = folder
        .unpack_sizes
        .first()
        .copied()
        .unwrap_or(packed.len() as u64);
    coder.decode(packed, unpack_size)
}

/// Decode an AES-encrypted folder using the supplied password.
///
/// For non-AES folders, delegates to [`decode_folder`].
#[cfg(feature = "aes")]
pub fn decode_folder_with_password(
    folder: &Folder,
    packed_streams: &[&[u8]],
    password: &str,
) -> SevenZippyResult<Vec<u8>> {
    if aes_folder::has_aes_coder(folder) {
        let packed = packed_streams.first().copied().unwrap_or(&[]);
        aes_folder::decode_aes_folder(folder, packed, password)
    } else {
        decode_folder(folder, packed_streams)
    }
}

/// Encode a single byte slice into a single-coder folder using `coder`.
///
/// Returns `(packed_bytes, folder_metadata)`.
pub fn encode_single_coder_folder(
    coder: &dyn Coder,
    unpacked: &[u8],
) -> SevenZippyResult<(Vec<u8>, Folder)> {
    let packed = coder.encode(unpacked)?;
    let folder = Folder {
        coders: vec![CoderMeta {
            method_id: coder.method_id(),
            num_in_streams: 1,
            num_out_streams: 1,
            properties: coder.properties(),
        }],
        bonds: vec![],
        packed_stream_indices: vec![0],
        unpack_sizes: vec![unpacked.len() as u64],
        unpack_crc: Some(crc32(unpacked)),
    };
    Ok((packed, folder))
}

/// Convenience: encode using the in-tree Copy coder.
pub fn encode_copy_folder(unpacked: &[u8]) -> SevenZippyResult<(Vec<u8>, Folder)> {
    encode_single_coder_folder(&CopyCoder, unpacked)
}

/// Return the method ID for the Copy coder.
pub fn copy_method_id() -> MethodId {
    MethodId::copy()
}

/// Encode plaintext into an AES+LZMA2 two-coder folder.
///
/// Pipeline (innermost → outermost):
/// 1. LZMA2 compresses the plaintext.
/// 2. AES-256-CBC encrypts the compressed bytes.
///
/// The returned `Folder` has:
/// - 2 coders: AES (index 0, outer) then LZMA2 (index 1, inner).
/// - 1 bond: `Bond { in_index: 1, out_index: 0 }` — AES output feeds LZMA2 input.
/// - `unpack_sizes`: `[lzma2_compressed_size, plaintext_size]`.
/// - `packed_stream_indices`: `[0]` (implicit single packed stream).
///
/// This matches the folder topology emitted by `7zz` for AES-encrypted archives.
///
/// # Errors
///
/// Returns `NotYetImplemented` if either the `aes` or `lzma2` feature is not enabled.
/// Propagates LZMA2 compression and AES encryption errors.
#[cfg(feature = "aes")]
pub fn encode_aes_lzma2_folder(
    plaintext: &[u8],
    password: &str,
) -> SevenZippyResult<(Vec<u8>, Folder)> {
    use crate::container::Bond;

    let result = aes_folder::encode_aes_folder(plaintext, password)?;

    let folder = Folder {
        coders: vec![
            // Coder 0: AES (outer — reads the packed stream, outputs compressed bytes)
            CoderMeta {
                method_id: MethodId::aes_sha256(),
                num_in_streams: 1,
                num_out_streams: 1,
                properties: result.aes_props,
            },
            // Coder 1: LZMA2 (inner — reads AES output, outputs plaintext)
            CoderMeta {
                method_id: MethodId::lzma2(),
                num_in_streams: 1,
                num_out_streams: 1,
                properties: result.lzma2_props,
            },
        ],
        bonds: vec![Bond {
            // AES output (stream 0) feeds LZMA2 input (stream 1)
            in_index: 1,
            out_index: 0,
        }],
        packed_stream_indices: vec![0],
        unpack_sizes: vec![result.lzma2_compressed_size, result.unpacked_size],
        unpack_crc: Some(crc32(plaintext)),
    };

    Ok((result.ciphertext, folder))
}

/// Encode plaintext into a BCJ2+LZMA two-coder folder (4 packed streams).
///
/// Pipeline (encoder side, outermost → innermost):
/// 1. BCJ2 splits the input into 4 sub-streams: `[main, call, jump, rc]`.
/// 2. LZMA compresses the `main` sub-stream.
///
/// The returned `packed_streams` vector has 4 elements in archive order:
///   - `[0]`: LZMA-compressed main stream
///   - `[1]`: CALL offset stream (raw)
///   - `[2]`: JMP offset stream (raw)
///   - `[3]`: range-coder stream (raw)
///
/// The returned `Folder` has:
/// - 2 coders: `LZMA` (index 0) then `BCJ2` (index 1, `num_in=4`).
/// - 1 bond: `Bond { in_index: 1, out_index: 0 }` — LZMA output feeds BCJ2 main input.
/// - `packed_stream_indices`: `[0, 1, 2, 3]` (all 4 are packed).
/// - `unpack_sizes`: `[lzma_main_size, plaintext_size]`.
///
/// This topology matches the folder structure produced by `7zz` for BCJ2-filtered archives.
#[cfg(feature = "bcj2")]
pub fn encode_bcj2_folder(plaintext: &[u8]) -> SevenZippyResult<(Vec<Vec<u8>>, Folder)> {
    use crate::container::Bond;

    // Step 1: BCJ2 split.
    let [main_raw, call, jump, rc] = bcjzippy::encode::encode_4streams(plaintext);

    // Step 2: LZMA-compress the main stream.
    const DICT_SIZE: u32 = 1 << 20; // 1 MiB — fast in tests
    let (lzma_props, lzma_main) = lazippy::encode::encode_7z(&main_raw, DICT_SIZE)
        .map_err(|e| SevenZippyError::Coder(Box::new(e)))?;

    let main_raw_size = main_raw.len() as u64;
    let plaintext_size = plaintext.len() as u64;

    // The 4 packed streams, in 7z archive order:
    //   packed[0] = LZMA-compressed main
    //   packed[1] = CALL offsets (raw)
    //   packed[2] = JMP offsets  (raw)
    //   packed[3] = range-coder  (raw)
    let packed_streams = vec![lzma_main, call, jump, rc];

    let folder = Folder {
        coders: vec![
            // Coder 0: LZMA (reads packed[0], outputs raw main stream)
            CoderMeta {
                method_id: MethodId::lzma(),
                num_in_streams: 1,
                num_out_streams: 1,
                properties: lzma_props,
            },
            // Coder 1: BCJ2 (4 inputs → 1 output: decoded x86 bytes)
            CoderMeta {
                method_id: MethodId::bcj2(),
                num_in_streams: 4,
                num_out_streams: 1,
                properties: vec![],
            },
        ],
        bonds: vec![Bond {
            // LZMA output (stream 0) feeds BCJ2 input slot 1 (the main slot).
            in_index: 1,
            out_index: 0,
        }],
        // BCJ2 folder packed-stream indices (global in-stream indices of the
        // unbound inputs, in the order they map to packed streams):
        //   PSI[0]=0: LZMA input at global in-stream 0 → packed stream 0 (LZMA main)
        //   PSI[1]=2: BCJ2 CALL input at global in-stream 2 → packed stream 1 (CALL)
        //   PSI[2]=3: BCJ2 JUMP input at global in-stream 3 → packed stream 2 (JUMP)
        //   PSI[3]=4: BCJ2 RC   input at global in-stream 4 → packed stream 3 (RC)
        // Global in-stream 1 (BCJ2 slot 0 = MAIN) is bonded and not listed.
        // The PSI values 0,2,3,4 are global in-stream indices; pack stream i is
        // fed into BCJ2 at global in-stream PSI[i]. The 7z decoder uses these
        // values to route packed streams to coder inputs; 7zz always writes [0,2,3,4].
        packed_stream_indices: vec![0, 2, 3, 4],
        unpack_sizes: vec![main_raw_size, plaintext_size],
        unpack_crc: Some(crc32(plaintext)),
    };

    Ok((packed_streams, folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_copy_folder_identity() {
        let data = b"hello world";
        let (packed, folder) = encode_copy_folder(data).unwrap();
        let unpacked = decode_folder(&folder, &[packed.as_slice()]).unwrap();
        assert_eq!(unpacked, data);
    }

    #[test]
    fn encode_copy_folder_sets_crc() {
        let data = b"test";
        let (_, folder) = encode_copy_folder(data).unwrap();
        assert!(folder.unpack_crc.is_some());
    }

    #[test]
    fn multi_coder_folder_is_not_yet_implemented() {
        use crate::container::{Bond, Coder as CoderMeta};
        let folder = Folder {
            coders: vec![
                CoderMeta {
                    method_id: MethodId::copy(),
                    num_in_streams: 1,
                    num_out_streams: 1,
                    properties: vec![],
                },
                CoderMeta {
                    method_id: MethodId::copy(),
                    num_in_streams: 1,
                    num_out_streams: 1,
                    properties: vec![],
                },
            ],
            bonds: vec![Bond {
                in_index: 0,
                out_index: 1,
            }],
            packed_stream_indices: vec![0],
            unpack_sizes: vec![5],
            unpack_crc: None,
        };
        let result = decode_folder(&folder, &[b"hello" as &[u8]]);
        assert!(matches!(result, Err(SevenZippyError::NotYetImplemented(_))));
    }

    /// Verify that `encode_aes_lzma2_folder` produces a folder with the correct
    /// 2-coder + 1-bond topology (AES outer, LZMA2 inner).
    #[cfg(all(feature = "aes", feature = "lzma2"))]
    #[test]
    fn encode_aes_lzma2_folder_bonds_are_correct() {
        use crate::container::{Bond, MethodId};

        let plaintext = b"test payload for multi-coder bond check".repeat(5);
        let (ciphertext, folder) =
            encode_aes_lzma2_folder(&plaintext, "password123").expect("encode failed");

        // Must have 2 coders
        assert_eq!(folder.coders.len(), 2, "expected 2 coders");

        // Coder 0 = AES (outer)
        assert_eq!(folder.coders[0].method_id, MethodId::aes_sha256());
        assert_eq!(folder.coders[0].num_in_streams, 1);
        assert_eq!(folder.coders[0].num_out_streams, 1);
        // AES properties are 18 bytes (NumCyclesPower + ivSize + 16-byte IV)
        assert_eq!(
            folder.coders[0].properties.len(),
            18,
            "AES props must be 18 bytes"
        );

        // Coder 1 = LZMA2 (inner)
        assert_eq!(folder.coders[1].method_id, MethodId::lzma2());
        assert_eq!(folder.coders[1].num_in_streams, 1);
        assert_eq!(folder.coders[1].num_out_streams, 1);
        assert_eq!(
            folder.coders[1].properties.len(),
            1,
            "LZMA2 props must be 1 byte"
        );

        // Must have exactly 1 bond: AES output (0) → LZMA2 input (1)
        assert_eq!(folder.bonds.len(), 1, "expected 1 bond");
        assert_eq!(
            folder.bonds[0],
            Bond {
                in_index: 1,
                out_index: 0
            }
        );

        // Packed stream index is [0] (implicit single stream)
        assert_eq!(folder.packed_stream_indices, vec![0]);

        // unpack_sizes: [lzma2_compressed, plaintext]
        assert_eq!(folder.unpack_sizes.len(), 2);
        assert_eq!(folder.unpack_sizes[1], plaintext.len() as u64);

        // Ciphertext must be non-empty and a multiple of 16 (AES block size)
        assert!(!ciphertext.is_empty());
        assert_eq!(
            ciphertext.len() % 16,
            0,
            "ciphertext must be AES-block-aligned"
        );

        // The folder must round-trip through the existing AES decode path
        use crate::pipeline::aes_folder::decode_aes_folder;
        let decrypted = decode_aes_folder(&folder, &ciphertext, "password123")
            .expect("decode_aes_folder failed");
        assert_eq!(decrypted, plaintext, "round-trip mismatch");
    }
}
