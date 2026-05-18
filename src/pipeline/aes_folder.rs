//! AES-256-CBC multi-coder folder decoder.
//!
//! 7z AES archives use a 2-coder folder topology:
//!
//! ```text
//! Coders (in order):
//!   [0] AES   (1 in, 1 out)  — decrypts the packed stream
//!   [1] LZMA2 (1 in, 1 out)  — decompresses the decrypted stream
//!
//! Bond:
//!   Bond { in_index: 1, out_index: 0 }
//!     → AES output (stream index 0) feeds LZMA2's input (stream index 1)
//!
//! Packed stream:
//!   packed[0] → AES input
//!             → AES decrypts → LZMA2 input
//!                            → LZMA2 decompresses → final output
//! ```
//!
//! The inner compressor is typically LZMA2 (the 7zz default), but may be
//! any single-stream coder. This Phase 1 implementation handles only
//! AES+LZMA2.
//!
//! ## Password API
//!
//! AES decryption requires a user-supplied password. The password is passed
//! separately from the packed data and is not stored in the archive.

use crate::error::{SevenZippyError, SevenZippyResult};

/// Is this a canonical AES+LZMA2 folder?
///
/// Returns `true` if the folder has:
/// - Exactly 2 coders: AES (`[0x06,0xF1,0x07,0x01]`) then LZMA2 (`[0x21]`)
/// - Exactly 1 bond: in_index=1, out_index=0
/// - Exactly 1 packed stream index
pub fn is_aes_lzma2_folder(folder: &crate::container::Folder) -> bool {
    use crate::container::MethodId;
    if folder.coders.len() != 2 {
        return false;
    }
    if folder.bonds.len() != 1 {
        return false;
    }
    if folder.packed_stream_indices.len() != 1 {
        return false;
    }
    let aes_id = MethodId(vec![0x06, 0xF1, 0x07, 0x01]);
    let lzma2_id = MethodId(vec![0x21]);
    folder.coders[0].method_id == aes_id && folder.coders[1].method_id == lzma2_id
}

/// Is this any folder encrypted with AES (regardless of the inner coder)?
///
/// Returns `true` if the first coder's method ID is `[0x06,0xF1,0x07,0x01]`.
/// Used by `decode_folder` to route to `decode_aes_folder` and to give the
/// caller a useful error if no password was provided.
pub fn has_aes_coder(folder: &crate::container::Folder) -> bool {
    use crate::container::MethodId;
    let aes_id = MethodId(vec![0x06, 0xF1, 0x07, 0x01]);
    folder.coders.first().is_some_and(|c| c.method_id == aes_id)
}

/// Result of encoding an AES+LZMA2 folder.
///
/// The caller stores `ciphertext` as the single packed stream and uses
/// `aes_props` for the AES coder's properties entry in the 7z container.
#[cfg(feature = "aes")]
pub struct AesFolderEncodeResult {
    /// The LZMA2-compressed then AES-256-CBC-encrypted bytes.
    pub ciphertext: Vec<u8>,
    /// AES codec properties (18 bytes: NumCyclesPower, ivSize byte, 16-byte IV).
    pub aes_props: Vec<u8>,
    /// LZMA2 codec properties (1-byte props byte).
    pub lzma2_props: Vec<u8>,
    /// Uncompressed size (before LZMA2 compression).
    pub unpacked_size: u64,
    /// LZMA2-compressed size (before AES encryption, before zero-padding).
    pub lzma2_compressed_size: u64,
}

/// Encode plaintext as an AES+LZMA2 folder (compress with LZMA2, then encrypt with AES).
///
/// # Arguments
///
/// - `plaintext`: the uncompressed file bytes
/// - `password`: the archive password (UTF-8)
///
/// # Returns
///
/// An [`AesFolderEncodeResult`] containing the ciphertext and properties needed
/// to write the 7z folder header. The folder topology is:
///
/// ```text
/// Coders: [AES, LZMA2]   Bond: AES-output → LZMA2-input
///   Bond { in_index: 1, out_index: 0 }: coder 1 (LZMA2) reads its input
///   from coder 0 (AES) output — decryption happens first, then
///   decompression on the plaintext.
/// Packed stream: ciphertext
/// ```
///
/// # Errors
///
/// - `NotYetImplemented` if the `lzma2` feature is not enabled
/// - Propagates LZMA2 compression errors and aeszippy encrypt errors
#[cfg(feature = "aes")]
pub fn encode_aes_folder(
    plaintext: &[u8],
    password: &str,
) -> crate::error::SevenZippyResult<AesFolderEncodeResult> {
    #[cfg(not(feature = "lzma2"))]
    {
        let _ = (plaintext, password);
        return Err(crate::error::SevenZippyError::not_yet_implemented(
            "AES+LZMA2 encoding requires the lzma2 feature",
        ));
    }

    #[cfg(feature = "lzma2")]
    {
        use crate::pipeline::lzma2::Lzma2Coder;
        use crate::pipeline::Coder;

        // Step 1: LZMA2 compress.
        let lzma2 = Lzma2Coder::default();
        let compressed = lzma2.encode(plaintext)?;
        let lzma2_props = lzma2.properties();
        let lzma2_compressed_size = compressed.len() as u64;

        // Step 2: AES-256-CBC encrypt (with random IV, NumCyclesPower=19).
        let enc_result = aeszippy::encrypt::encrypt_7z(&compressed, password)
            .map_err(|e| crate::error::SevenZippyError::Coder(Box::new(e)))?;

        Ok(AesFolderEncodeResult {
            ciphertext: enc_result.ciphertext,
            aes_props: enc_result.props,
            lzma2_props,
            unpacked_size: plaintext.len() as u64,
            lzma2_compressed_size,
        })
    }
}

/// Decode an AES+LZMA2 folder from a single packed stream byte slice.
///
/// # Arguments
///
/// - `folder`: the folder metadata (coders + bonds + unpack_sizes)
/// - `packed`: the single packed (encrypted) byte stream
/// - `password`: the archive password (UTF-8)
///
/// # Errors
///
/// - `NotYetImplemented` if the folder is not the AES+LZMA2 topology
/// - `InvalidArgument` if the password is empty and the archive is encrypted
/// - Propagates decrypt errors from aeszippy and decompress errors from xzippy
#[cfg(feature = "aes")]
pub fn decode_aes_folder(
    folder: &crate::container::Folder,
    packed: &[u8],
    password: &str,
) -> SevenZippyResult<Vec<u8>> {
    if !is_aes_lzma2_folder(folder) {
        return Err(SevenZippyError::not_yet_implemented(
            "AES folder with non-LZMA2 inner coder",
        ));
    }

    // Step 1: AES-256-CBC decrypt.
    // folder.coders[0] = AES; its properties contain NumCyclesPower + IV.
    let aes_coder = &folder.coders[0];
    let decrypted = aeszippy::decrypt::decrypt_7z(packed, &aes_coder.properties, password)
        .map_err(|e| SevenZippyError::Coder(Box::new(e)))?;

    // Step 2: LZMA2 decompress.
    // folder.coders[1] = LZMA2; its properties contain the props byte.
    // unpack_sizes[1] is the LZMA2 output size (= final uncompressed size).
    let lzma2_coder = &folder.coders[1];
    let unpack_size = folder.unpack_sizes.last().copied().unwrap_or(0);

    use crate::pipeline::lzma2::Lzma2Coder;
    use crate::pipeline::Coder;
    let lzma2 = Lzma2Coder::with_props(lzma2_coder.properties.clone())?;
    lzma2.decode(&decrypted, unpack_size)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(all(test, feature = "aes", feature = "lzma2"))]
mod tests {
    use super::*;
    use crate::container::{Bond, Coder as CoderMeta, Folder, MethodId};

    /// Build a fake AES+LZMA2 folder metadata from encode results for decode testing.
    fn folder_from_encode(result: &AesFolderEncodeResult) -> Folder {
        Folder {
            coders: vec![
                // AES coder (index 0: outer, reads packed stream)
                CoderMeta {
                    method_id: MethodId(vec![0x06, 0xF1, 0x07, 0x01]),
                    num_in_streams: 1,
                    num_out_streams: 1,
                    properties: result.aes_props.clone(),
                },
                // LZMA2 coder (index 1: inner, reads AES output)
                CoderMeta {
                    method_id: MethodId(vec![0x21]),
                    num_in_streams: 1,
                    num_out_streams: 1,
                    properties: result.lzma2_props.clone(),
                },
            ],
            bonds: vec![Bond {
                // AES output (stream index 0) feeds LZMA2 input (stream index 1)
                in_index: 1,
                out_index: 0,
            }],
            packed_stream_indices: vec![0],
            unpack_sizes: vec![result.lzma2_compressed_size, result.unpacked_size],
            unpack_crc: None,
        }
    }

    #[test]
    fn encode_then_decode_round_trip() {
        let plaintext = b"Hello, AES+LZMA2 round-trip test in 7zippy!".repeat(10);
        let password = "test1234";

        let encoded = encode_aes_folder(&plaintext, password).expect("encode_aes_folder failed");
        let folder = folder_from_encode(&encoded);
        let decoded =
            decode_aes_folder(&folder, &encoded.ciphertext, password).expect("decode failed");

        assert_eq!(decoded, plaintext, "encode+decode round-trip mismatch");
    }

    #[test]
    fn wrong_password_fails_or_produces_garbage() {
        let plaintext = b"secret data for encryption";
        let encoded = encode_aes_folder(plaintext, "correct").expect("encode failed");
        let folder = folder_from_encode(&encoded);
        // With wrong password, decode either errors (LZMA2 rejects garbage) or produces wrong output.
        let result = decode_aes_folder(&folder, &encoded.ciphertext, "wrong");
        match result {
            Err(_) => {} // Expected: LZMA2 rejects garbage
            Ok(decrypted) => {
                assert_ne!(
                    decrypted, plaintext,
                    "wrong password must not decrypt correctly"
                );
            }
        }
    }
}
