//! Content hashing used by the import fingerprint (plan §11: idempotency is
//! keyed by *source id + source content hash*).

use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of the given bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

/// SHA-256 of a file's contents, streamed so large DB copies are not buffered.
pub fn sha256_file(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Short, stable id derived from a namespace + source key. Used for the
/// old-id -> new-id mapping so that a re-import of the same source produces the
/// same candidate ids (plan §11: predictable, no merge-by-name).
pub fn derived_id(namespace: &str, source_key: &str) -> String {
    let digest = sha256_hex(format!("{namespace}\u{1f}{source_key}").as_bytes());
    format!("{namespace}-{}", &digest[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn derived_id_is_stable_and_namespaced() {
        assert_eq!(
            derived_id("profile", "old-1"),
            derived_id("profile", "old-1")
        );
        assert_ne!(
            derived_id("profile", "old-1"),
            derived_id("profile", "old-2")
        );
        assert_ne!(derived_id("profile", "old-1"), derived_id("sub", "old-1"));
    }
}
