//! Signature-verification hook.
//!
//! The plan (§15) requires a signature capability. [`PgpDetachedVerifier`] is
//! the real OpenPGP backend used for the application's own update assets:
//! upstream `2dust/v2rayN` publishes an ASCII-armored detached `.sig` next to
//! every release artifact, and the public key is bundled at
//! `fixtures/keys/v2rayn-public-key.asc` (see `docs/decisions/T21-signature.md`
//! for the trust model). [`UnsupportedSignatureVerifier`] remains the explicit
//! fail-closed default when no key is available.
//!
//! Detached verification never silently succeeds: a missing key reports
//! [`UpdateError::SignatureUnsupported`], a bad/mismatched signature reports
//! [`UpdateError::SignatureInvalid`].

use std::path::Path;

use pgp::composed::{Deserializable, SignedPublicKey, StandaloneSignature};
use pgp::types::KeyDetails;

use crate::error::UpdateError;

/// The bundled upstream 2dust/v2rayN release-signing public key (public data).
pub const V2RAYN_PUBLIC_KEY_ASC: &str =
    include_str!("../../../fixtures/keys/v2rayn-public-key.asc");

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

/// Real OpenPGP (RFC 9580) detached-signature verifier backed by `rpgp`.
///
/// The key is parsed once at construction. Verification accepts both
/// ASCII-armored (`-----BEGIN PGP SIGNATURE-----`) and binary detached
/// signatures, and tries the primary key plus every signing subkey so a
/// release signed by a subkey still validates.
pub struct PgpDetachedVerifier {
    public_key: SignedPublicKey,
    fingerprint: String,
}

impl std::fmt::Debug for PgpDetachedVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgpDetachedVerifier")
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

impl PgpDetachedVerifier {
    /// Parse an ASCII-armored public key block.
    pub fn from_armored_public_key(key: &[u8]) -> Result<Self, UpdateError> {
        let (public_key, _headers) = SignedPublicKey::from_armor_single(key)
            .map_err(|e| UpdateError::SignatureUnsupported(format!("invalid public key: {e}")))?;
        let fingerprint = hex::encode_upper(public_key.fingerprint().as_bytes());
        Ok(Self {
            public_key,
            fingerprint,
        })
    }

    /// Parse an ASCII-armored public key from a file.
    pub fn from_public_key_file(path: &Path) -> Result<Self, UpdateError> {
        let bytes = std::fs::read(path).map_err(|e| {
            UpdateError::SignatureUnsupported(format!("read public key {}: {e}", path.display()))
        })?;
        Self::from_armored_public_key(&bytes)
    }

    /// Upper-case hex fingerprint of the primary key (for trust-root checks).
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// The parsed transferable public key.
    pub fn public_key(&self) -> &SignedPublicKey {
        &self.public_key
    }
}

impl SignatureVerifier for PgpDetachedVerifier {
    fn scheme(&self) -> &'static str {
        "openpgp-detached"
    }

    fn verify(&self, artifact: &[u8], signature: &[u8]) -> Result<(), UpdateError> {
        let (signature, _headers) =
            StandaloneSignature::from_reader_single(std::io::Cursor::new(signature))
                .map_err(classify_signature_parse)?;

        let mut last_error = None;
        if signature.verify(&self.public_key, artifact).is_ok() {
            return Ok(());
        }
        for subkey in &self.public_key.public_subkeys {
            match signature.verify(subkey, artifact) {
                Ok(()) => return Ok(()),
                Err(e) => last_error = Some(e),
            }
        }
        Err(match last_error {
            Some(e) => classify_verify(e),
            None => UpdateError::SignatureInvalid(
                "signature does not match the configured public key".into(),
            ),
        })
    }
}

fn classify_signature_parse(error: pgp::errors::Error) -> UpdateError {
    match error {
        pgp::errors::Error::Unimplemented { .. } | pgp::errors::Error::Unsupported { .. } => {
            UpdateError::SignatureUnsupported(format!("unsupported signature: {error}"))
        }
        _ => UpdateError::SignatureInvalid(format!("malformed detached signature: {error}")),
    }
}

fn classify_verify(error: pgp::errors::Error) -> UpdateError {
    match error {
        pgp::errors::Error::Unimplemented { .. } | pgp::errors::Error::Unsupported { .. } => {
            UpdateError::SignatureUnsupported(format!("unsupported signature algorithm: {error}"))
        }
        _ => UpdateError::SignatureInvalid(format!("signature verification failed: {error}")),
    }
}

/// OpenPGP v5 (LibrePGP) verifier backed by the GnuPG CLI.
///
/// The upstream 2dust trust root is an OpenPGP **v5** EdDSA key (generated by
/// GnuPG 2.4+); neither `rpgp` nor Sequoia parse v5 keys today. This backend
/// runs GnuPG in an isolated temporary home, imports the bundled public key and
/// requires `GOODSIG` plus a `VALIDSIG` fingerprint matching the bundled key, so
/// the v5 trust root is actually enforced. It fails closed when GnuPG is not
/// installed.
pub struct GpgCliVerifier {
    gpg: std::path::PathBuf,
    key_asc: Vec<u8>,
    fingerprint: String,
}

impl std::fmt::Debug for GpgCliVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpgCliVerifier")
            .field("gpg", &self.gpg)
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

impl GpgCliVerifier {
    /// Locate GnuPG and derive the bundled key fingerprint.
    pub fn new(key_asc: &[u8]) -> Result<Self, UpdateError> {
        let gpg = locate_gpg().ok_or_else(|| {
            UpdateError::SignatureUnsupported(
                "GnuPG (gpg) not found; required to verify the OpenPGP v5 upstream key".into(),
            )
        })?;
        let work = temp_work_dir()?;
        let home = work.join("home");
        std::fs::create_dir_all(&home).map_err(gpg_io_error)?;
        let key_path = work.join("key.asc");
        std::fs::write(&key_path, key_asc).map_err(gpg_io_error)?;
        let result = run_gpg(
            &gpg,
            &[
                "--homedir",
                home.to_string_lossy().as_ref(),
                "--batch",
                "--no-tty",
                "--with-colons",
                "--import-options",
                "show-only",
                "--import",
                key_path.to_string_lossy().as_ref(),
            ],
        );
        let _ = std::fs::remove_dir_all(&work);
        let out = result?;
        let fingerprint = parse_colon_fingerprint(&out).ok_or_else(|| {
            UpdateError::SignatureUnsupported("bundled key has no fingerprint".into())
        })?;
        Ok(Self {
            gpg,
            key_asc: key_asc.to_vec(),
            fingerprint,
        })
    }

    /// Upper-case hex fingerprint of the bundled primary key.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

impl SignatureVerifier for GpgCliVerifier {
    fn scheme(&self) -> &'static str {
        "openpgp-v5-gpg-cli"
    }

    fn verify(&self, artifact: &[u8], signature: &[u8]) -> Result<(), UpdateError> {
        let work = temp_work_dir()?;
        let home = work.join("home");
        std::fs::create_dir_all(&home).map_err(gpg_io_error)?;
        let key_path = work.join("key.asc");
        let sig_path = work.join("artifact.sig");
        let artifact_path = work.join("artifact.bin");
        std::fs::write(&key_path, &self.key_asc).map_err(gpg_io_error)?;
        std::fs::write(&sig_path, signature).map_err(gpg_io_error)?;
        std::fs::write(&artifact_path, artifact).map_err(gpg_io_error)?;

        let import = run_gpg(
            &self.gpg,
            &[
                "--homedir",
                home.to_string_lossy().as_ref(),
                "--batch",
                "--yes",
                "--no-tty",
                "--import",
                key_path.to_string_lossy().as_ref(),
            ],
        );
        let imported = match import {
            Ok(_) => true,
            Err(UpdateError::SignatureUnsupported(message)) => {
                let _ = std::fs::remove_dir_all(&work);
                return Err(UpdateError::SignatureUnsupported(message));
            }
            Err(_) => false,
        };
        if !imported {
            let _ = std::fs::remove_dir_all(&work);
            return Err(UpdateError::SignatureUnsupported(
                "GnuPG could not import the bundled key".into(),
            ));
        }

        let verify = run_gpg(
            &self.gpg,
            &[
                "--homedir",
                home.to_string_lossy().as_ref(),
                "--batch",
                "--no-tty",
                "--status-fd",
                "1",
                "--verify",
                sig_path.to_string_lossy().as_ref(),
                artifact_path.to_string_lossy().as_ref(),
            ],
        );
        let _ = std::fs::remove_dir_all(&work);
        let status_text = verify?;

        let mut good = false;
        let mut validsig: Option<String> = None;
        for line in status_text.lines() {
            let Some(rest) = line.strip_prefix("[GNUPG:] ") else {
                continue;
            };
            let mut parts = rest.split_whitespace();
            match parts.next() {
                Some("GOODSIG") => good = true,
                Some("VALIDSIG") => validsig = parts.next().map(str::to_string),
                Some("BADSIG") => {
                    return Err(UpdateError::SignatureInvalid(
                        "GnuPG reported BADSIG".into(),
                    ));
                }
                Some("NO_PUBKEY") => {
                    return Err(UpdateError::SignatureInvalid(
                        "signature made by an unknown key".into(),
                    ));
                }
                Some("ERRSIG") => {
                    return Err(UpdateError::SignatureInvalid(
                        "GnuPG could not verify the signature".into(),
                    ));
                }
                _ => {}
            }
        }
        if !good {
            return Err(UpdateError::SignatureInvalid(
                "GnuPG did not report a good signature".into(),
            ));
        }
        match validsig {
            Some(fpr) if fpr.eq_ignore_ascii_case(&self.fingerprint) => Ok(()),
            Some(fpr) => Err(UpdateError::SignatureInvalid(format!(
                "signature was made by untrusted key {fpr}"
            ))),
            None => Err(UpdateError::SignatureInvalid(
                "GnuPG reported no VALIDSIG fingerprint".into(),
            )),
        }
    }
}

fn locate_gpg() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("V2RAYN_R_GPG") {
        let candidate = std::path::PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let names: &[&str] = &["gpg", "gpg.exe"];
    for name in names {
        if std::process::Command::new(name)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(std::path::PathBuf::from(name));
        }
    }
    for candidate in [
        r"C:\Program Files\GnuPG\bin\gpg.exe",
        r"C:\Program Files (x86)\GnuPG\bin\gpg.exe",
    ] {
        let path = std::path::PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn temp_work_dir() -> Result<std::path::PathBuf, UpdateError> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("v2rayn-r-gpg-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(gpg_io_error)?;
    Ok(dir)
}

fn run_gpg(gpg: &std::path::Path, args: &[&str]) -> Result<String, UpdateError> {
    use std::process::{Command, Stdio};
    let mut child = Command::new(gpg)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| UpdateError::SignatureUnsupported(format!("spawn gpg: {e}")))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(UpdateError::SignatureUnsupported(
                    "gpg timed out during verification".into(),
                ));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(UpdateError::SignatureUnsupported(format!("wait gpg: {e}")));
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| UpdateError::SignatureUnsupported(format!("collect gpg output: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() && !stdout.contains("[GNUPG:] GOODSIG") {
        return Err(UpdateError::SignatureInvalid(format!(
            "gpg exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }
    Ok(format!("{stdout}\n{stderr}"))
}

fn parse_colon_fingerprint(output: &str) -> Option<String> {
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("fpr:") {
            if let Some(value) = rest.split(':').nth(8) {
                let cleaned = value.trim();
                if !cleaned.is_empty() {
                    return Some(cleaned.to_ascii_uppercase());
                }
            }
        }
    }
    None
}

fn gpg_io_error(error: std::io::Error) -> UpdateError {
    UpdateError::SignatureUnsupported(format!("gpg workspace: {error}"))
}

/// Build the verifier for application (`v2rayN`) updates from the bundled key.
///
/// Prefers the pure-Rust `rpgp` backend (OpenPGP v4/v6). The upstream key is
/// OpenPGP v5 (LibrePGP), which no pure-Rust backend parses today, so the
/// GnuPG CLI backend is selected for it. Fails closed with
/// [`UpdateError::SignatureUnsupported`] when no backend can verify the key, so
/// callers never proceed on an unverified app asset.
pub fn v2rayn_app_verifier() -> Result<Box<dyn SignatureVerifier>, UpdateError> {
    match PgpDetachedVerifier::from_armored_public_key(V2RAYN_PUBLIC_KEY_ASC.as_bytes()) {
        Ok(verifier) => Ok(Box::new(verifier)),
        Err(rust_error) => match GpgCliVerifier::new(V2RAYN_PUBLIC_KEY_ASC.as_bytes()) {
            Ok(verifier) => Ok(Box::new(verifier)),
            Err(gpg_error) => Err(UpdateError::SignatureUnsupported(format!(
                "bundled upstream key needs GnuPG ({rust_error}); {gpg_error}"
            ))),
        },
    }
}

/// Verify an application release asset against its detached `.sig` using the
/// bundled upstream trust root.
///
/// This is the wiring point for the app self-update path: callers must run it
/// after download and before staging the external upgrade. It fails closed
/// (`SignatureUnsupported`) when neither the pure-Rust backend nor GnuPG can
/// enforce the v5 trust root.
pub fn verify_app_release_asset(asset: &[u8], signature: &[u8]) -> Result<(), UpdateError> {
    let verifier = v2rayn_app_verifier()?;
    verifier.verify(asset, signature)
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

    use pgp::composed::{KeyType, SecretKeyParamsBuilder, SignedSecretKey};
    use pgp::packet::{SignatureConfig, SignatureType};
    use pgp::types::Password;
    use rand::thread_rng;

    /// Generate a throwaway Ed25519 (legacy, v4) key pair.
    fn generate_keypair() -> (SignedSecretKey, PgpDetachedVerifier) {
        let mut rng = thread_rng();
        let params = SecretKeyParamsBuilder::default()
            .key_type(KeyType::Ed25519Legacy)
            .can_certify(true)
            .can_sign(true)
            .primary_user_id("t21 test <t21@example.invalid>".into())
            .passphrase(None)
            .build()
            .expect("key params");
        let secret = params.generate(&mut rng).expect("generate");
        let signed = secret.sign(&mut rng, &Password::empty()).expect("sign key");
        let public = signed.signed_public_key();
        let armored = public.to_armored_bytes(None.into()).expect("armor key");
        let verifier =
            PgpDetachedVerifier::from_armored_public_key(&armored).expect("parse public key");
        (signed, verifier)
    }

    fn detached_armored(secret: &SignedSecretKey, data: &[u8]) -> Vec<u8> {
        let mut rng = thread_rng();
        let config =
            SignatureConfig::from_key(&mut rng, &secret.primary_key, SignatureType::Binary)
                .expect("config");
        let signature = config
            .sign(&secret.primary_key, &Password::empty(), data)
            .expect("sign");
        StandaloneSignature::new(signature)
            .to_armored_bytes(None.into())
            .expect("armor signature")
    }

    fn detached_binary(secret: &SignedSecretKey, data: &[u8]) -> Vec<u8> {
        let mut rng = thread_rng();
        let config =
            SignatureConfig::from_key(&mut rng, &secret.primary_key, SignatureType::Binary)
                .expect("config");
        let signature = config
            .sign(&secret.primary_key, &Password::empty(), data)
            .expect("sign");
        use pgp::ser::Serialize;
        StandaloneSignature::new(signature)
            .to_bytes()
            .expect("binary signature")
    }

    #[test]
    fn pgp_verifier_accepts_armored_signature() {
        let (secret, verifier) = generate_keypair();
        assert!(verifier.is_available());
        assert_eq!(verifier.scheme(), "openpgp-detached");
        let artifact = b"v2rayN-linux-loong64.deb payload";
        let sig = detached_armored(&secret, artifact);
        assert!(sig.starts_with(b"-----BEGIN PGP SIGNATURE-----"));
        verifier.verify(artifact, &sig).expect("valid signature");
    }

    #[test]
    fn pgp_verifier_accepts_binary_signature() {
        let (secret, verifier) = generate_keypair();
        let artifact = b"binary detached payload";
        let sig = detached_binary(&secret, artifact);
        verifier
            .verify(artifact, &sig)
            .expect("valid binary signature");
    }

    #[test]
    fn pgp_verifier_rejects_tampered_artifact() {
        let (secret, verifier) = generate_keypair();
        let sig = detached_armored(&secret, b"original artifact");
        let err = verifier.verify(b"tampered artifact", &sig).unwrap_err();
        assert!(matches!(err, UpdateError::SignatureInvalid(_)), "{err:?}");
    }

    #[test]
    fn pgp_verifier_rejects_signature_from_other_key() {
        let (secret, _verifier) = generate_keypair();
        let (_other_secret, other_verifier) = generate_keypair();
        let sig = detached_armored(&secret, b"cross-key payload");
        let err = other_verifier
            .verify(b"cross-key payload", &sig)
            .unwrap_err();
        assert!(matches!(err, UpdateError::SignatureInvalid(_)), "{err:?}");
    }

    #[test]
    fn pgp_verifier_rejects_malformed_armored_signature() {
        let (_secret, verifier) = generate_keypair();
        let bogus =
            b"-----BEGIN PGP SIGNATURE-----\n\nnot base64 at all\n-----END PGP SIGNATURE-----\n";
        let err = verifier.verify(b"payload", bogus).unwrap_err();
        assert!(matches!(err, UpdateError::SignatureInvalid(_)), "{err:?}");
    }

    #[test]
    fn pgp_verifier_rejects_empty_signature() {
        let (_secret, verifier) = generate_keypair();
        let err = verifier.verify(b"payload", b"").unwrap_err();
        assert!(
            matches!(
                err,
                UpdateError::SignatureInvalid(_) | UpdateError::SignatureUnsupported(_)
            ),
            "{err:?}"
        );
    }

    #[test]
    fn malformed_public_key_is_unsupported_not_invalid() {
        let err = PgpDetachedVerifier::from_armored_public_key(b"not a pgp key")
            .expect_err("must reject");
        assert!(
            matches!(err, UpdateError::SignatureUnsupported(_)),
            "{err:?}"
        );
    }

    #[test]
    fn fingerprint_is_stable_and_hex() {
        let (_secret, verifier) = generate_keypair();
        let fp = verifier.fingerprint();
        assert_eq!(fp.len(), 40, "fingerprint {fp:?}");
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn bundled_upstream_key_selects_a_real_backend_or_fails_closed() {
        match v2rayn_app_verifier() {
            Ok(verifier) => {
                assert!(verifier.is_available());
                assert!(
                    matches!(verifier.scheme(), "openpgp-detached" | "openpgp-v5-gpg-cli"),
                    "unexpected scheme {}",
                    verifier.scheme()
                );
            }
            Err(error) => {
                assert!(
                    matches!(error, UpdateError::SignatureUnsupported(_)),
                    "{error:?}"
                );
            }
        }
    }

    #[test]
    fn bundled_upstream_key_rejects_unrelated_signature() {
        // A signature made by a fresh key must not validate under the bundled
        // upstream key (a wrong-key negative case against the real trust root).
        let Ok(bundled) = v2rayn_app_verifier() else {
            eprintln!("skip: no signature backend available on this machine");
            return;
        };
        let (secret, _verifier) = generate_keypair();
        let sig = detached_armored(&secret, b"payload");
        let err = bundled.verify(b"payload", &sig).unwrap_err();
        assert!(matches!(err, UpdateError::SignatureInvalid(_)), "{err:?}");
    }

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
