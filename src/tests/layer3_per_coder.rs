//! Layer 3: per-coder sub-crate smoke tests.
//!
//! Each test in this module delegates to a sibling crate's own test suite
//! by spawning `cargo test -p <crate> --lib` as a subprocess and asserting
//! exit 0. This provides a quick cross-package sanity check from 7zippy's
//! perspective: "do lazippy's own tests still pass?"
//!
//! Tests are skipped if `cargo` is not found on `$PATH`.
//!
//! # Future additions
//!
//! As more sibling crates land, add a test row
//! here modelled on `lazippy_lib_tests_pass`.

use std::process::Command;

/// Find `cargo` on `$PATH`, returning `None` if not found.
fn find_cargo() -> Option<std::path::PathBuf> {
    // Use CARGO env var if set (common in test environments).
    if let Some(cargo) = std::env::var_os("CARGO") {
        return Some(std::path::PathBuf::from(cargo));
    }
    // Walk $PATH manually.
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join("cargo");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Verify that `lazippy`'s own lib tests pass from 7zippy's perspective.
///
/// This test is skipped (`return` early) if:
/// - `cargo` is not on PATH, or
/// - `lazippy` is not present at the expected `../lazippy` path relative to
///   the workspace root.
///
/// When lazippy is present, we run its tests with `--no-default-features`
/// so we don't require its optional dependencies.
#[test]
fn lazippy_lib_tests_pass() {
    let cargo = match find_cargo() {
        Some(c) => c,
        None => {
            eprintln!("[skip] cargo not found — per-coder smoke test skipped");
            return;
        }
    };

    // Resolve the lazippy path relative to the 7zippy workspace root.
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lazippy_path = workspace_root.join("../lazippy");
    if !lazippy_path.exists() {
        eprintln!(
            "[skip] lazippy not found at {} — per-coder smoke test skipped",
            lazippy_path.display()
        );
        return;
    }

    let output = Command::new(&cargo)
        .arg("test")
        .arg("--lib")
        .arg("--no-default-features")
        .arg("--manifest-path")
        .arg(lazippy_path.join("Cargo.toml"))
        .output()
        .expect("failed to spawn cargo test for lazippy");

    assert!(
        output.status.success(),
        "lazippy lib tests failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

// PPMd is now in-tree; its tests live in src/pipeline/ppmd.rs.

/// Verify that `xzippy`'s own lib tests pass from 7zippy's perspective.
#[test]
fn xzippy_lib_tests_pass() {
    let cargo = match find_cargo() {
        Some(c) => c,
        None => {
            eprintln!("[skip] cargo not found — per-coder smoke test skipped");
            return;
        }
    };

    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let xzippy_path = workspace_root.join("../xzippy");
    if !xzippy_path.exists() {
        eprintln!(
            "[skip] xzippy not found at {} — per-coder smoke test skipped",
            xzippy_path.display()
        );
        return;
    }

    let output = Command::new(&cargo)
        .arg("test")
        .arg("--lib")
        .arg("--no-default-features")
        .arg("--manifest-path")
        .arg(xzippy_path.join("Cargo.toml"))
        .output()
        .expect("failed to spawn cargo test for xzippy");

    assert!(
        output.status.success(),
        "xzippy lib tests failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn bzippy2_lib_tests_pass() {
    let cargo = match find_cargo() {
        Some(c) => c,
        None => {
            eprintln!("[skip] cargo not found — per-coder smoke test skipped");
            return;
        }
    };

    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let bzippy2_path = workspace_root.join("../bzippy2");
    if !bzippy2_path.exists() {
        eprintln!(
            "[skip] bzippy2 not found at {} — per-coder smoke test skipped",
            bzippy2_path.display()
        );
        return;
    }

    let output = Command::new(&cargo)
        .arg("test")
        .arg("--lib")
        .arg("--manifest-path")
        .arg(bzippy2_path.join("Cargo.toml"))
        .output()
        .expect("failed to spawn cargo test for bzippy2");

    assert!(
        output.status.success(),
        "bzippy2 lib tests failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Verify that gzippy 0.7's raw-deflate and Deflate64 API functions are
/// callable and produce correct round-trips. gzippy is a published crate
/// (not a sibling path dep), so we call its public API directly rather than
/// spawning a subprocess.
#[test]
#[cfg(feature = "deflate")]
fn gzippy_lib_tests_pass() {
    let original = b"hello from gzippy deflate round-trip test".repeat(10);

    // Raw Deflate round-trip
    let compressed = gzippy::deflate_encode(&original, 6).expect("deflate_encode failed");
    let decompressed = gzippy::deflate_decode(&compressed).expect("deflate_decode failed");
    assert_eq!(decompressed, original, "gzippy deflate round-trip mismatch");

    // Deflate64 decode smoke-test: the fixture is tested in layer5 oracle tests;
    // here we just confirm the symbol is present and accepts valid input.
    // We re-use the Deflate-compressed bytes as a best-effort smoke (gzippy
    // returns an error on invalid Deflate64 input, which is fine for this test).
    let _ = gzippy::decompress_deflate64(&compressed); // result ignored intentionally
}
