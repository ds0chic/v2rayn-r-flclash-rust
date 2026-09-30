//! S3 feasibility spike: can a Rust TLS backend load browser root bundles from
//! the T00 fixtures and probe the operating system trust store?
//!
//! Read-only. No network access, no credential handling.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

fn sample_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("source")
        .join("upstream")
        .join("sample")
}

fn count_pem_certs(path: &PathBuf) -> Result<usize, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut count = 0usize;
    for item in rustls_pemfile::certs(&mut reader) {
        item.map_err(|e| format!("parse {}: {e}", path.display()))?;
        count += 1;
    }
    Ok(count)
}

#[test]
fn loads_browser_root_bundles() {
    let dir = sample_dir();
    for name in ["mozilla_roots_pem", "chrome_roots_pem"] {
        let path = dir.join(name);
        assert!(path.exists(), "fixture missing: {}", path.display());
        let count = count_pem_certs(&path).expect("pem bundle should parse");
        println!("T01-S3 {} certificates={count}", path.display());
        assert!(count > 0, "{name} should contain at least one certificate");
    }
}

#[test]
fn probes_native_cert_store() {
    let result = rustls_native_certs::load_native_certs();
    println!(
        "T01-S3 native_store certs={} errors={}",
        result.certs.len(),
        result.errors.len()
    );
    for err in &result.errors {
        println!("T01-S3 native_store error: {err}");
    }
    assert!(
        !result.certs.is_empty(),
        "native trust store probe returned no certificates"
    );
}
