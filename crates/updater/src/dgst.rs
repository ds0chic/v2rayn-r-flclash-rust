//! Xray-style `.dgst` companion assets.
//!
//! Xray (and other cores) publish a `<asset>.dgst` file next to each release
//! artifact. It is a small key/value document whose `SHA2-256` line carries the
//! artifact's digest. This module parses that document and verifies a file on
//! disk against it, so the download path can prefer the live `.dgst` channel
//! over the optional GitHub `digest` field.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::UpdateError;

/// Parse the SHA-256 value out of a Xray-style `.dgst` document.
///
/// Accepts `SHA2-256 = <hex>`, `SHA2-256=<hex>` and `SHA256=<hex>` (the key is
/// normalized by removing `-`/spaces and upper-casing). Returns lowercase hex.
pub fn parse_dgst_sha256(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_uppercase().replace(['-', ' '], "");
        if key == "SHA2256" || key == "SHA256" {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_ascii_lowercase());
            }
        }
    }
    None
}

/// Lowercase hex SHA-256 of a file, streamed (no whole-file buffering).
pub fn sha256_file_sync(path: &Path) -> Result<String, UpdateError> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| UpdateError::Io(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|e| UpdateError::Io(e.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Verify `path` against the `SHA2-256` line of `dgst_text`.
///
/// Returns the file's lowercase hex digest on success, or
/// [`UpdateError::InvalidMetadata`] when the document has no digest and
/// [`UpdateError::DigestMismatch`] when it disagrees with the file.
pub fn verify_dgst_file(path: &Path, dgst_text: &str) -> Result<String, UpdateError> {
    let expected = parse_dgst_sha256(dgst_text).ok_or_else(|| {
        UpdateError::InvalidMetadata("no SHA2-256 entry in .dgst document".into())
    })?;
    let actual = sha256_file_sync(path)?;
    if actual.eq_ignore_ascii_case(&expected) {
        Ok(actual)
    } else {
        Err(UpdateError::DigestMismatch { expected, actual })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DGST: &str = "MD5= 402f65a8cccdf123a6c0d5c176ef5252\n\
SHA1= 42e5e9a66b970b5499c34e99d8da6c986c67c2cc\n\
SHA2-256= d004c39288ce9ada487c6f398c7c545f7d749e44bdfdd59dbc9f865afba4e1ad\n\
SHA2-512= 5b1356f07a91cbd4fb538fd7eccc494967ea045e770fb60887a9bec0412f0b3bce6148a6514ce7c661824daefe83929b7ffa8b059c22abfc1f1299d9d228712c\n";

    #[test]
    fn parses_sha2_256_line() {
        assert_eq!(
            parse_dgst_sha256(DGST),
            Some("d004c39288ce9ada487c6f398c7c545f7d749e44bdfdd59dbc9f865afba4e1ad".to_string())
        );
    }

    #[test]
    fn parses_compact_and_sha256_spellings() {
        assert_eq!(
            parse_dgst_sha256("SHA256=ABCDEF"),
            Some("abcdef".to_string())
        );
        assert_eq!(
            parse_dgst_sha256("SHA2-256 = AbCdEf"),
            Some("abcdef".to_string())
        );
        assert_eq!(parse_dgst_sha256("SHA2-512=deadbeef"), None);
        assert_eq!(parse_dgst_sha256("nonsense"), None);
    }

    #[test]
    fn verify_dgst_file_accepts_match_and_rejects_tamper() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("artifact.bin");
        std::fs::write(&file, b"xray-core-payload").unwrap();
        let digest = sha256_file_sync(&file).unwrap();
        let doc = format!("SHA2-256= {digest}\n");
        assert_eq!(verify_dgst_file(&file, &doc).unwrap(), digest);

        // Same expected digest but a different file must be rejected.
        let tampered = dir.path().join("tampered.bin");
        std::fs::write(&tampered, b"xray-core-payload!").unwrap();
        let err = verify_dgst_file(&tampered, &doc).unwrap_err();
        assert!(matches!(err, UpdateError::DigestMismatch { .. }));

        // A document without the line is invalid metadata, not a pass.
        assert!(matches!(
            verify_dgst_file(&file, "MD5=00\n").unwrap_err(),
            UpdateError::InvalidMetadata(_)
        ));
    }
}
