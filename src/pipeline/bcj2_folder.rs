//! BCJ2 multi-coder folder decoder.
//!
//! BCJ2 archives use a 2-coder folder topology:
//!
//! ```text
//! Coders (in order):
//!   [0] LZMA  (1 in, 1 out)  — compresses the main byte stream
//!   [1] BCJ2  (4 in, 1 out)  — reassembles 4 streams into x86 code
//!
//! Bonds:
//!   Bond { in_index: 1, out_index: 0 }
//!     → LZMA's output (stream index 0) feeds BCJ2's input slot 1 (main stream)
//!
//! Packed streams (in folder.packed_stream_indices order):
//!   packed[0] → LZMA input           (the LZMA-compressed main stream)
//!   packed[1] → BCJ2 input slot 2    (CALL offsets; may be LZMA-compressed)
//!   packed[2] → BCJ2 input slot 3    (JMP offsets; may be LZMA-compressed)
//!   packed[3] → BCJ2 input slot 0    (range-coder stream; raw)
//!
//! BCJ2 4-stream order (as bcjzippy expects):
//!   streams[0] = main      ← LZMA decode of packed[0]
//!   streams[1] = call      ← packed[1] (raw in 7zz defaults)
//!   streams[2] = jump      ← packed[2] (raw in 7zz defaults)
//!   streams[3] = range_coder ← packed[3] (raw)
//! ```
//!
//! Reference: LZMA SDK `CPP/7zip/Archive/7z/7zFolderOutStream.cpp`
//! and `CPP/7zip/Archive/7z/7zDecode.cpp`.

use crate::error::{SevenZippyError, SevenZippyResult};

/// Is this a canonical BCJ2+LZMA folder?
///
/// Returns `true` if the folder has:
/// - Exactly 2 coders: LZMA (method `[0x03,0x01,0x01]`) then BCJ2 (`[0x03,0x03,0x01,0x1B]`)
/// - Exactly 1 bond: in_index=1, out_index=0
/// - Exactly 4 packed stream indices
///
/// This is the only BCJ2 topology 7zz produces; other configurations are
/// rejected as `NotYetImplemented`.
pub fn is_bcj2_lzma_folder(folder: &crate::container::Folder) -> bool {
    use crate::container::MethodId;
    if folder.coders.len() != 2 {
        return false;
    }
    if folder.bonds.len() != 1 {
        return false;
    }
    if folder.packed_stream_indices.len() != 4 {
        return false;
    }
    let lzma_id = MethodId(vec![0x03, 0x01, 0x01]);
    let bcj2_id = MethodId(vec![0x03, 0x03, 0x01, 0x1B]);
    folder.coders[0].method_id == lzma_id && folder.coders[1].method_id == bcj2_id
}

/// Decode a BCJ2+LZMA folder from 4 packed stream byte slices.
///
/// `packed_streams` must contain exactly 4 slices in the order described by
/// `folder.packed_stream_indices`:
///
///   - `packed_streams[0]` — LZMA-compressed main stream
///   - `packed_streams[1]` — CALL offsets stream (raw or LZMA)
///   - `packed_streams[2]` — JMP offsets stream (raw or LZMA)
///   - `packed_streams[3]` — range-coder stream (raw)
///
/// # Errors
///
/// Returns `NotYetImplemented` if the folder is not a BCJ2+LZMA topology,
/// or propagates decode errors from the LZMA and BCJ2 layers.
#[cfg(feature = "bcj2")]
pub fn decode_bcj2_folder(
    folder: &crate::container::Folder,
    packed_streams: &[&[u8]],
) -> SevenZippyResult<Vec<u8>> {
    if !is_bcj2_lzma_folder(folder) {
        return Err(SevenZippyError::not_yet_implemented(
            "BCJ2 folder with non-LZMA inner coder",
        ));
    }
    if packed_streams.len() != 4 {
        return Err(SevenZippyError::invalid_argument(format!(
            "BCJ2 folder needs 4 packed streams, got {}",
            packed_streams.len()
        )));
    }

    // Step 1: LZMA-decode the main stream (packed_streams[0]).
    // LZMA coder properties are in folder.coders[0].properties.
    // unpack_sizes[0] is the LZMA output size (= BCJ2 main stream size).
    let lzma_coder = &folder.coders[0];
    let lzma_unpack_size = folder.unpack_sizes.first().copied().unwrap_or(0);

    let main_stream = {
        use crate::pipeline::lzma::LzmaCoder;
        use crate::pipeline::Coder;
        let c = LzmaCoder::with_props(lzma_coder.properties.clone());
        c.decode(packed_streams[0], lzma_unpack_size)?
    };

    // Step 2: BCJ2 reassembly.
    // bcjzippy expects [main, call, jump, range_coder].
    // packed_streams layout from 7z spec:
    //   packed_streams[0] was LZMA main → already decoded above as main_stream
    //   packed_streams[1] → call offsets
    //   packed_streams[2] → jump offsets
    //   packed_streams[3] → range_coder
    let bcj2_unpack_size = folder.unpack_sizes.last().copied().unwrap_or(0);

    bcjzippy::decode::decode_4streams(
        [
            main_stream.as_slice(),
            packed_streams[1],
            packed_streams[2],
            packed_streams[3],
        ],
        bcj2_unpack_size,
    )
    .map_err(|e| SevenZippyError::Coder(Box::new(e)))
}
