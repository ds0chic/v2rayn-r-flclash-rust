//! T21 evidence runner: verify a real upstream `v2rayN` release asset against
//! its detached `.sig` with the production [`PgpDetachedVerifier`].
//!
//! Usage (PowerShell/bash, network already done by the caller):
//! ```text
//! cargo run -p updater --example t21_verify_release -- \
//!     --asset <file> --sig <file.sig> [--key <public.asc>] [--tamper]
//! ```
//!
//! `--tamper` flips one byte of the asset in memory and expects verification to
//! FAIL, proving the check is not a no-op. The default key is the bundled
//! `fixtures/keys/v2rayn-public-key.asc`.

use std::path::PathBuf;

use updater::download::sha256_of;
use updater::signature::{v2rayn_app_verifier, PgpDetachedVerifier, SignatureVerifier};
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
    let sig = PathBuf::from(arg("--sig").unwrap_or_else(|| fail("--sig is required")));
    let tamper = has_flag("--tamper");

    let verifier: Box<dyn SignatureVerifier> = match arg("--key") {
        Some(path) => Box::new(
            PgpDetachedVerifier::from_public_key_file(&PathBuf::from(path))
                .unwrap_or_else(|e| fail(&format!("key load: {e}"))),
        ),
        None => v2rayn_app_verifier().unwrap_or_else(|e| fail(&format!("bundled key: {e}"))),
    };
    println!("scheme          : {}", verifier.scheme());

    let mut asset_bytes = std::fs::read(&asset).unwrap_or_else(|e| fail(&format!("asset: {e}")));
    let sig_bytes = std::fs::read(&sig).unwrap_or_else(|e| fail(&format!("sig: {e}")));
    println!("asset           : {}", asset.display());
    println!("asset bytes     : {}", asset_bytes.len());
    println!("asset sha256    : {}", sha256_of(&asset_bytes));
    println!("sig bytes       : {}", sig_bytes.len());
    println!(
        "sig armor       : {}",
        sig_bytes.starts_with(b"-----BEGIN PGP SIGNATURE-----")
    );

    if tamper {
        // Flip a byte in the middle so the signature can no longer match.
        let index = asset_bytes.len() / 2;
        asset_bytes[index] ^= 0x01;
        match verifier.verify(&asset_bytes, &sig_bytes) {
            Err(UpdateError::SignatureInvalid(_)) => {
                println!("RESULT: FAIL (tampered asset rejected as expected)");
                return;
            }
            Err(other) => {
                println!("RESULT: FAIL (tampered asset rejected: {other})");
                return;
            }
            Ok(()) => fail("tampered asset unexpectedly verified"),
        }
    }

    match verifier.verify(&asset_bytes, &sig_bytes) {
        Ok(()) => println!("RESULT: PASS (detached signature verified)"),
        Err(e) => fail(&format!("signature verification failed: {e}")),
    }
}
