//! Signature verification hook: Unsupported default and test verifier paths.

use sha2::{Digest, Sha256};
use updater::{PrefixHashVerifier, SignatureVerifier, UnsupportedSignatureVerifier, UpdateError};

#[test]
fn default_verifier_reports_unsupported() {
    let verifier = UnsupportedSignatureVerifier;
    assert!(!verifier.is_available());
    assert_eq!(verifier.scheme(), "unsupported");
    let err = verifier.verify(b"any", b"sig").unwrap_err();
    assert!(matches!(err, UpdateError::SignatureUnsupported(_)));
}

#[test]
fn unsupported_verifier_never_reports_success() {
    // Even with an empty signature it must not pass.
    assert!(UnsupportedSignatureVerifier.verify(b"", b"").is_err());
}

#[test]
fn prefix_verifier_accepts_correct_signature() {
    let artifact = b"downloaded-core";
    let prefix = hex::encode(Sha256::digest(artifact))[..16].to_string();
    let verifier = PrefixHashVerifier {
        expected_prefix: prefix.clone(),
    };
    assert!(verifier.verify(artifact, prefix.as_bytes()).is_ok());
    assert_eq!(verifier.scheme(), "prefix-hash-test");
}

#[test]
fn prefix_verifier_rejects_tampered_signature() {
    let artifact = b"downloaded-core";
    let prefix = hex::encode(Sha256::digest(artifact))[..16].to_string();
    let verifier = PrefixHashVerifier {
        expected_prefix: prefix,
    };
    assert!(matches!(
        verifier.verify(artifact, b"wrong-marker").unwrap_err(),
        UpdateError::SignatureInvalid(_)
    ));
}

#[test]
fn prefix_verifier_rejects_tampered_artifact() {
    let prefix = hex::encode(Sha256::digest(b"original"))[..16].to_string();
    let verifier = PrefixHashVerifier {
        expected_prefix: prefix.clone(),
    };
    assert!(matches!(
        verifier.verify(b"modified", prefix.as_bytes()).unwrap_err(),
        UpdateError::SignatureInvalid(_)
    ));
}
