# 7-zippy

> **Pure-Rust 7z archive implementation** — read, write, and inspect `.7z` files.
> `cargo add sevenzippy`

7-zippy is both a CLI tool and a Rust library. It handles the full 7z container
format and delegates each codec to a dedicated sub-crate in the zippy family. The
sub-crates are also independently usable — each is a drop-in replacement for the
corresponding system tool.

## The zippy family

| Crate | Replaces | What it does |
|---|---|---|
| **sevenzippy** (`7zippy`) | `7z`, `7zz` | 7z archive read/write — the umbrella |
| [lazippy](https://github.com/JackDanger/lazippy) | `lzma` | `.lzma` files (LZMA encoder/decoder) |
| [xzippy](https://github.com/JackDanger/xzippy) | `xz`, `unxz`, `xzcat` | `.xz` files (LZMA2 encoder/decoder) |
| [gzippy](https://github.com/JackDanger/gzippy) | `gzip`, `gunzip`, `gzcat` | `.gz` files (Deflate encoder/decoder) |
| [bzippy2](https://github.com/JackDanger/bzippy2) | `bzip2`, `bunzip2` | `.bz2` files (BZip2 encoder/decoder) |
| [bcjzippy](https://github.com/JackDanger/bcjzippy) | — | BCJ2 x86 4-stream branch filter |
| [aeszippy](https://github.com/JackDanger/aeszippy) | — | AES-256-CBC encrypt/decrypt for 7z |

BCJ (simple 6-arch branch filter), PPMd, Copy, and Delta are folded in-tree.
LZMA2 in `.xz` files is handled by xzippy; inside 7z archives it goes through
the same lazippy → xzippy stack, since LZMA2 is built on LZMA.

### Why separate crates?

Each sub-crate is a **drop-in CLI replacement** for its system tool and gets its
own hyper-optimization CI. lazippy and xzippy will eventually be native
pure-Rust rewrites (Phase 2); the others wrap established Rust backends and are
unlikely to change much. Crates that run in under a minute of CI with no
standalone CLI use (PPMd, Delta, simple BCJ) are folded in-tree.

## Install

```bash
cargo install sevenzippy
```

Or use the library:

```toml
[dependencies]
sevenzippy = "0.0.1"
```

> Note: the crate is named `sevenzippy` on crates.io because Cargo package
> names cannot start with a digit. The binary is `7zippy`.

## Build and test

```bash
cargo build
cargo test --release
make oracle-check   # round-trip tests against the 7zz CLI
```

## Status

See [STATUS.md](STATUS.md) for the full codec × feature matrix.
