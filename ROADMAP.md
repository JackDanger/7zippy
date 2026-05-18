# 7zippy Roadmap

## Order of Attack

### Phase 1: Scaffolding (in progress)
- [x] Umbrella repo structure (7zippy)
- [x] Container parser skeleton
- [x] Copy coder (in-tree, proof of concept)
- [x] `STATUS.md` tracking table
- [ ] Finish: all CI green, oracle tests pass

### Phase 2: LZMA Range Coder (lazippy foundation)
- [ ] Range coder (decode + encode)
- [ ] Literal tree decoder
- [ ] Match decoder
- [ ] LZMA block header parsing
- [ ] Full LZMA round-trip against 7zz

### Phase 3: LZMA2 Wrapping (xzippy)
- [ ] LZMA2 block decoder
- [ ] Dictionary reset logic
- [ ] Streaming LZMA2 unpacking
- [ ] Full LZMA2 round-trip

### Phase 4: BZip2 (bzippy2)
- [ ] Huffman decoder
- [ ] Burrows-Wheeler transform inversion
- [ ] Run-length unpacking
- [ ] Full BZip2 round-trip

### Phase 5: DEFLATE Wiring (gzippy integration)
- [ ] gzippy `lib.rs` PR (library API on existing code)
- [ ] 7zippy integration harness
- [ ] Deflate + Deflate64 round-trip

### Phase 6: PPMd (pippyzippy)
- [ ] Context model
- [ ] Arithmetic decoder
- [ ] Full PPMd round-trip

### Phase 7: BCJ Family + Delta Filters (jumpzippy, deltazippy)
- [ ] x86 / ARM / ARM-Thumb / PPC / IA64 / SPARC branch transformers
- [ ] Delta filter
- [ ] Chained encoder/decoder pipelines

### Phase 8: BCJ2 + AES Encryption (jumpzippier, lockzippy)
- [ ] BCJ2 decoder (more complex than BCJ; requires look-ahead)
- [ ] AES-256 + SHA-256 decryption
- [ ] Encrypted archive round-trip

## Performance Goals (Target for Phase 10+)

- **Throughput**: >= 100 MB/s decode on modern CPUs for all algorithms
- **Latency**: < 1 GB memory overhead for archives up to 4 GB
- **Ratio**: match the canonical `7zz` output within 1% (same settings)
- **Compatibility**: decompress any standard 7z archive created by 7-Zip >= 9.0

## Milestones

| Milestone | Estimate | Gate |
|-----------|----------|------|
| Phase 1 complete | 1 week | Scaffolding PR merged, STATUS.md live |
| Phase 2 complete | 3 weeks | lazippy round-trip passing |
| Phase 3 complete | 2 weeks | xzippy wired, LZMA2 tests green |
| Phase 4 complete | 3 weeks | bzippy2 archive parsing + round-trip |
| Phase 5 complete | 2 weeks | gzippy lib PR merged, deflate wired |
| Phases 6–8 complete | 8 weeks | All major codecs live |
| **7zippy 1.0** | ~4 months | Full `cargo build && 7zippy file.7z` working end-to-end |

## Current Focus

**See [STATUS.md](STATUS.md) for the live position.** The "Current focus" line there is the authoritative next task.

Run `make status` to print it.
