//! PEM parsing contract locks for the rustls-pemfile -> rustls-pki-types
//! migration (W1, RUSTSEC-2025-0134).
//!
//! Locks the exact retired semantics that callers relied on:
//! - `certs(..).flatten()`: garbage/malformed sections are skipped, valid
//!   certificates on either side are still found, never panics.
//! - `private_key(..)`: first key wins; a malformed key section errors;
//!   input without a key yields `Ok(None)`.

use rustls_pki_types::pem::{Error as PemError, PemObject};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

const CA_PEM: &[u8] = include_bytes!("fixtures/tls/ca.pem");
const SERVER_PEM: &[u8] = include_bytes!("fixtures/tls/server.pem");
const SERVER_KEY_PEM: &[u8] = include_bytes!("fixtures/tls/server.key.pem");

fn first_private_key(pem: &[u8]) -> Result<Option<PrivateKeyDer<'static>>, PemError> {
    PrivateKeyDer::pem_slice_iter(pem).next().transpose()
}

#[test]
fn certs_skip_garbage_and_find_all() {
    let mut blob = b"this is not pem\n".to_vec();
    blob.extend_from_slice(CA_PEM);
    blob.extend_from_slice(
        b"-----BEGIN CERTIFICATE-----\n!!!not-base64!!!\n-----END CERTIFICATE-----\n",
    );
    blob.extend_from_slice(b"-----BEGIN FROBNICATOR-----\nQUJD\n-----END FROBNICATOR-----\n");
    blob.extend_from_slice(SERVER_PEM);
    blob.extend_from_slice(b"trailing garbage without end marker\n");

    let found: Vec<CertificateDer<'static>> =
        CertificateDer::pem_slice_iter(&blob).flatten().collect();
    assert_eq!(
        found.len(),
        2,
        "both valid certs must survive surrounding garbage"
    );
}

#[test]
fn private_key_first_key_wins_despite_leading_garbage() {
    let mut blob = b"garbage line\n".to_vec();
    blob.extend_from_slice(SERVER_KEY_PEM);
    let key = first_private_key(&blob).expect("key parse must not fail");
    assert!(key.is_some(), "key behind garbage must be found");
}

#[test]
fn private_key_malformed_section_errors() {
    let blob = b"-----BEGIN PRIVATE KEY-----\n!!!not-base64!!!\n-----END PRIVATE KEY-----\n";
    assert!(
        first_private_key(blob).is_err(),
        "malformed key section must error, not silently skip"
    );
}

#[test]
fn private_key_absent_is_none_not_error() {
    assert!(first_private_key(b"")
        .expect("empty must not fail")
        .is_none());
    assert!(first_private_key(b"just some text\nno sections here\n")
        .expect("keyless must not fail")
        .is_none());
}
