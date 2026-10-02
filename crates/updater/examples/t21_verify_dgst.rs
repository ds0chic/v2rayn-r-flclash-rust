//! T21 evidence runner: verify a locally staged core archive against its
//! Xray-style `.dgst` companion document (offline).
//!
//! Usage:
//! ```text
//! cargo run -p updater --example t21_verify_dgst -- \
//!     --asset <file> --dgst <file.dgst> [--tamper-expected]
//! ```
//!
//! `--tamper-expected` flips the first hex nibble of the `SHA2-256` line and
//! expects verification to FAIL, proving the digest is actually checked.

use std::path::PathBuf;

use updater::dgst::{parse_dgst_sha256, sha256_file_sync, verify_dgst_file};
use updater::UpdateError;

fn arg(flag: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(item) = args.next() {
        if item == flag {
            return args.next();
        }
    }
    None
}

fn has_flag(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}

fn fail(message: &str) -> ! {
    eprintln!("ERROR: {message}");
    std::process::exit(2);
}

fn main() {
    let asset = PathBuf::from(arg("--asset").unwrap_or_else(|| fail("--asset is required")));
    let dgst_path = PathBuf::from(arg("--dgst").unwrap_or_else(|| fail("--dgst is required")));
    let tamper = has_flag("--tamper-expected");

    let text = std::fs::read_to_string(&dgst_path).unwrap_or_else(|e| fail(&format!("dgst: {e}")));
    let parsed = parse_dgst_sha256(&text);
    println!("asset           : {}", asset.display());
    println!(
        "asset bytes     : {}",
        asset.metadata().map(|m| m.len()).unwrap_or(0)
    );
    println!(
        "asset sha256    : {}",
        sha256_file_sync(&asset).unwrap_or_else(|e| fail(&format!("hash: {e}")))
    );
    println!("dgst parsed     : {}", parsed.clone().unwrap_or_default());

    let effective = if tamper {
        match parsed.as_deref() {
            Some(hex) if !hex.is_empty() => {
                let mut bytes = hex.as_bytes().to_vec();
                bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
                let tampered = format!("SHA2-256= {}\n", String::from_utf8(bytes).unwrap());
                println!(
                    "dgst tampered   : {}",
                    parse_dgst_sha256(&tampered).unwrap_or_default()
                );
                tampered
            }
            _ => fail("dgst has no SHA2-256 to tamper"),
        }
    } else {
        text
    };

    match verify_dgst_file(&asset, &effective) {
        Ok(digest) if !tamper => {
            println!("RESULT: PASS (SHA2-256 matches: {digest})");
        }
        Err(UpdateError::DigestMismatch { expected, actual }) if tamper => {
            println!(
                "RESULT: FAIL (tampered digest rejected; expected {expected}, actual {actual})"
            );
        }
        other => fail(&format!("unexpected dgst result: {other:?}")),
    }
}
