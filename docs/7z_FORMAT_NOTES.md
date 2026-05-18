# 7z Format Notes

Quick reference for 7z archive structure and codec routing. See the official spec:
- **7zFormat.txt** — container structure, header properties, folder layout
- **Methods.txt** — method ID definitions and codec-specific property bytes

## Method IDs and Sub-Crates

| Algorithm | Method ID | Sub-Crate | Repo |
|-----------|-----------|-----------|------|
| **Copy** | `0x00` | 7zippy (in-tree) | — |
| **LZMA** | `03 01 01` | lazippy | JackDanger/lazippy |
| **LZMA2** | `21` | xzippy | JackDanger/xzippy |
| **PPMd** | `03 04 01` | pippyzippy | JackDanger/pippyzippy |
| **BZip2** | `04 02 02` | bzippy2 | JackDanger/bzippy2 |
| **Deflate** | `04 01 08` | gzippy | JackDanger/gzippy |
| **Deflate64** | `04 01 09` | gzippy | JackDanger/gzippy |
| **BCJ (x86)** | `03 03 01 00` | jumpzippy | JackDanger/jumpzippy |
| **BCJ (ARM)** | `03 03 01 01` | jumpzippy | JackDanger/jumpzippy |
| **BCJ (ARM-Thumb)** | `03 03 01 02` | jumpzippy | JackDanger/jumpzippy |
| **BCJ (PPC)** | `03 03 01 03` | jumpzippy | JackDanger/jumpzippy |
| **BCJ (IA64)** | `03 03 01 04` | jumpzippy | JackDanger/jumpzippy |
| **BCJ (SPARC)** | `03 03 01 05` | jumpzippy | JackDanger/jumpzippy |
| **BCJ2** | `03 03 01 1B` | jumpzippier | JackDanger/jumpzippier |
| **Delta** | `03` | deltazippy | JackDanger/deltazippy |
| **AES-256 + SHA-256** | `06 F1 07 01` | lockzippy | JackDanger/lockzippy |

## Container Structure (High-Level)

```
[Signature Header (32 bytes)]
  magic: 37 7A BC AF 27 1C
  version: (1 byte)
  start_header_crc: (4 bytes CRC32)
  next_header_offset: (8 bytes UINT64LE)
  next_header_size: (4 bytes UINT64LE)
  next_header_crc: (4 bytes CRC32)

[Next Header (variable)]
  Property stream:
    - Folders: how many, what coders, initial dict sizes
    - Streams: which folders encode which files, order
    - Files: names, timestamps, sizes, attributes
    - Misc: archive comment, etc.
```

## Property ID Enum (7zFormat.txt Section 5.2)

| ID | Name | Meaning |
|----|------|---------|
| `0x00` | End | End of properties (terminator) |
| `0x01` | Header | Start of next header |
| `0x04` | Folder | Folder properties (upcoming) |
| `0x05` | CodersUnpackSize | Folder unpacking sizes |
| `0x06` | CodersInfo | Coder method IDs + properties |
| `0x07` | MainStreamsInfo | Packed data layout |
| `0x08` | SubStreamsInfo | Sub-stream (file) boundaries |
| `0x09` | UName | UTF-16 file names |
| `0x0A` | StartPos | Start positions (sparse files) |
| `0x0B` | DummyU32 | Dummy property (skip) |
| `0x0C` | Dummy | Another dummy (skip) |
| `0x0E` | CTime | Creation timestamps |
| `0x0F` | ATime | Access timestamps |
| `0x10` | MTime | Modification timestamps |
| `0x11` | WinAttrib | Windows file attributes |
| `0x12` | Comment | Archive comment |
| `0x13` | EncodedHeader | Encoded (compressed) header |
| `0x14` | StartPos | Start position (legacy) |
| `0x15` | Dummy2 | Another dummy |
| `0x18` | Sha256 | SHA-256 checksum |
| `0x19` | Crc | CRC-32 (EndMarker implicit) |

See 7zFormat.txt for full definitions and how each property is encoded.

## Coder Chain

A folder can contain **multiple coders in sequence**. Each coder:
1. Reads its input from the previous coder's output (or packed data for the first)
2. Applies its transformation (e.g., LZMA decode, BCJ transform)
3. Writes output for the next coder (or unpacked file data if last)

**Example**: LZMA2 with optional BCJ preprocessing:
```
Packed bytes → BCJ decode → LZMA2 decode → File bytes
```

Both method IDs appear in the Folder's coder chain. They're called in sequence.

## Dispatch Routing (7zippy's src/pipeline/dispatch.rs)

Given a MethodId (parsed from Container), return a trait object implementing the `Coder` trait:

```rust
match method_id {
    MethodId::Copy => Box::new(copy::CopyCoder),
    MethodId::Lzma => Box::new(lazippy_adapter::LzmaAdapter),
    MethodId::Bzip2 => Box::new(bzippy2_adapter::Bzip2Adapter),
    // ...
}
```

Each adapter wraps the sub-crate's API (which varies by algorithm).

## Property Encoding

Most properties are variable-length encoded:

- **UINT64** — LEB128 encoding (variable-length big-endian integer)
- **BitVector** — (count: UINT64) followed by (data: bytes, padded to byte boundary)

Example: if a property says "4 folders with their sizes", you read:
1. UINT64 = 4 (count)
2. BitVector of size 4 = 1 byte (4 bits + 4 padding bits)
3. 4 × UINT64 = folder sizes

The Container parser (`src/container/`) handles this; your coder implementation gets the already-decoded values.

---

**See the official spec** for exact byte layouts. This is a quick cheat-sheet.

For unknown method IDs or edge cases, test against `7zz`:
```bash
$ 7zz t archive.7z -v    # verbose listing shows method IDs
$ 7zz x archive.7z       # attempt extraction (oracle feedback)
```
