//! Signature-verification hook.
//!
//! The plan (§15) requires a signature capability, but the project has not yet
//! fixed a scheme or a trusted public key. This module therefore defines the
//! trait and ships an explicit [`UnsupportedSignatureVerifier`] that fails
//! loudly instead of silently pretending the artifact is verified. Wiring a
//! real GPG/minisign backend is tracked as an open item in
//! `docs/evidence/T16-updater.md`.

use crate::error::UpdateError;

/// Verify a detached signature over an artifact.
///
/// `artifact` is the downloaded file's bytes, `signature` the detached
/// signature bytes (e.g. a `.sig` asset). Implementations must return
/// [`UpdateError::SignatureInvalid`] for a wrong signature and
/// [`UpdateError::SignatureUnsupported`] when the scheme is unavailable.
pub trait SignatureVerifier: Send + Sync {
    /// Human-readable scheme name for diagnostics.
    fn scheme(&self) -> &'static str;

    /// Verify; `Ok(())` means the artifact is authentic under this scheme.
    fn verify(&self, artifact: &[u8], signature: &[u8]) -> Result<(), UpdateError>;

    /// Whether this verifier can actually perform a check. Callers use this to
    /// decide between "verified" and "signature pending".
    fn is_available(&self) -> bool {
        true
    }
}

/// Default verifier: no trusted key configured yet, so verification is
/// explicitly `Unsupported` and never reported as success.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnsupportedSignatureVerifier;

impl SignatureVerifier for UnsupportedSignatureVerifier {
    fn scheme(&self) -> &'static str {
        "unsupported"
    }

    fn verify(&self, _artifact: &[u8], _signature: &[u8]) -> Result<(), UpdateError> {
        Err(UpdateError::SignatureUnsupported(
            "no trusted public key configured for signature verification".into(),
        ))
    }

    fn is_available(&self) -> bool {
        false
    }
}

/// A test/demo verifier that accepts bytes with a fixed SHA-256 prefix.
///
/// This is deliberately *not* a cryptographic signature scheme; it exists so
/// the pipeline's control flow (wrong vs. right signature) can be exercised
/// without shipping a half-baked crypto backend.
#[derive(Debug, Clone)]
pub struct PrefixHashVerifier {
    /// Required lowercase hex prefix of the SHA-256 over `artifact`.
    pub expected_prefix: String,
}

impl SignatureVerifier for PrefixHashVerifier {
    fn scheme(&self) -> &'static str {
        "prefix-hash-test"
    }

    fn verify(&self, artifact: &[u8], signature: &[u8]) -> Result<(), UpdateError> {
        use sha2::{Digest, Sha256};
        if signature != self.expected_prefix.as_bytes() {
            return Err(UpdateError::SignatureInvalid(
                "signature marker does not match".into(),
            ));
        }
        let digest = hex::encode(Sha256::digest(artifact));
        if digest.starts_with(&self.expected_prefix) {
            Ok(())
        } else {
            Err(UpdateError::SignatureInvalid(
                "artifact hash does not match signature marker".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_verifier_is_unsupported_not_success() {
        let verifier = UnsupportedSignatureVerifier;
        assert!(!verifier.is_available());
        let err = verifier.verify(b"artifact", b"sig").unwrap_err();
        assert!(matches!(err, UpdateError::SignatureUnsupported(_)));
    }

    #[test]
    fn prefix_verifier_accepts_matching_marker() {
        use sha2::{Digest, Sha256};
        let artifact = b"hello world";
        let digest = hex::encode(Sha256::digest(artifact));
        let prefix = digest[..16].to_string();
        let verifier = PrefixHashVerifier {
            expected_prefix: prefix.clone(),
        };
        assert!(verifier.verify(artifact, prefix.as_bytes()).is_ok());
    }

    #[test]
    fn prefix_verifier_rejects_tampered_artifact() {
        use sha2::{Digest, Sha256};
        let digest = hex::encode(Sha256::digest(b"hello world"));
        let prefix = digest[..16].to_string();
        let verifier = PrefixHashVerifier {
            expected_prefix: prefix.clone(),
        };
        let err = verifier.verify(b"tampered", prefix.as_bytes()).unwrap_err();
        assert!(matches!(err, UpdateError::SignatureInvalid(_)));
    }
}
