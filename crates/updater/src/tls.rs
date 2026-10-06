//! HTTPS trust source for the update pipeline (SP-25).
//!
//! Mirrors upstream v2rayN 7.25.4 `GuiItem.RootCertProvider` semantics (see
//! `platform::cert`): `system` uses the OS/native roots, while `chrome` and
//! `mozilla` trust exactly a bundled PEM collection and never the OS store.
//! Nothing here writes the OS certificate store.
//!
//! Pending the SP-00 `net_http` consolidation, `subscriptions::tls` carries an
//! identical contract for subscription downloads; keep the two in sync.

/// Root trust for outbound HTTPS.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HttpsTrust {
    /// OS/native roots only (upstream `system`).
    #[default]
    System,
    /// Exactly this PEM bundle, no OS roots (upstream `chrome`/`mozilla`).
    BundledPem(Vec<u8>),
}

impl HttpsTrust {
    /// Trust exactly `pem` (one or more PEM certificates), no OS roots.
    pub fn bundled(pem: &[u8]) -> Self {
        Self::BundledPem(pem.to_vec())
    }

    /// Map a `RootCertProvider` string onto trust roots. Unknown values fall
    /// back to `System`, matching upstream `ConfigHandler.LoadConfig`.
    pub fn from_provider(provider: &str, chrome_pem: &[u8], mozilla_pem: &[u8]) -> Self {
        match provider.trim().to_ascii_lowercase().as_str() {
            "chrome" => Self::bundled(chrome_pem),
            "mozilla" => Self::bundled(mozilla_pem),
            _ => Self::System,
        }
    }

    /// Upstream `CertPemManager.IsSystemRootCertProvider`.
    pub fn uses_system_store(&self) -> bool {
        matches!(self, Self::System)
    }

    /// Best-effort transport-string check for a TLS trust rejection (rustls
    /// `UnknownIssuer`, `NotValidForName`, expiry, revocation, handshake
    /// failures). Accept/reject behaviour per trust selection stays the
    /// primary proof; this only keeps the classification honest and stable.
    pub fn is_trust_failure(text: &str) -> bool {
        trust_failure_in_text(text)
    }
}

/// Apply `trust` to a `reqwest` client builder. `BundledPem` is exclusive:
/// built-in roots are disabled and only the bundle is trusted.
pub fn apply_trust(
    builder: reqwest::ClientBuilder,
    trust: &HttpsTrust,
) -> Result<reqwest::ClientBuilder, String> {
    match trust {
        HttpsTrust::System => Ok(builder),
        HttpsTrust::BundledPem(pem) => {
            let certs = reqwest::Certificate::from_pem_bundle(pem)
                .map_err(|e| format!("trust bundle parse: {e}"))?;
            if certs.is_empty() {
                return Err("trust bundle holds no certificate".to_string());
            }
            let mut builder = builder.tls_built_in_root_certs(false);
            for cert in certs {
                builder = builder.add_root_certificate(cert);
            }
            Ok(builder)
        }
    }
}

/// Transport-string check behind [`HttpsTrust::is_trust_failure`].
pub fn trust_failure_in_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "certificate",
        "unknownissuer",
        "notvalidforname",
        "handshake",
        "expired",
        "revoked",
        "untrusted",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// Walk a `reqwest` error's source chain for TLS trust evidence. Returns the
/// first matching source text (credential-free: certificate errors carry no
/// URLs or secrets).
pub fn trust_failure_of(err: &reqwest::Error) -> Option<String> {
    let mut current: Option<&dyn std::error::Error> = Some(err);
    while let Some(source) = current {
        let text = source.to_string();
        if trust_failure_in_text(&text) {
            return Some(text);
        }
        current = source.source();
    }
    None
}

/// Map a `reqwest` failure onto [`UpdateError`](crate::UpdateError), keeping a
/// rejected peer certificate distinguishable from timeouts and truncations.
pub fn classify_request(err: &reqwest::Error) -> crate::UpdateError {
    if err.is_timeout() {
        crate::UpdateError::Timeout
    } else if let Some(detail) = trust_failure_of(err) {
        crate::UpdateError::Download(format!("tls trust rejected: {detail}"))
    } else {
        crate::UpdateError::Download(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_mapping_matches_upstream_fallback() {
        assert_eq!(
            HttpsTrust::from_provider("system", b"c", b"m"),
            HttpsTrust::System
        );
        assert_eq!(
            HttpsTrust::from_provider("chrome", b"c", b"m"),
            HttpsTrust::BundledPem(b"c".to_vec())
        );
        assert_eq!(
            HttpsTrust::from_provider(" Mozilla ", b"c", b"m"),
            HttpsTrust::BundledPem(b"m".to_vec())
        );
        assert_eq!(
            HttpsTrust::from_provider("bogus", b"c", b"m"),
            HttpsTrust::System
        );
        assert_eq!(
            HttpsTrust::from_provider("", b"c", b"m"),
            HttpsTrust::System
        );
    }

    #[test]
    fn classifier_marks_only_tls_strings() {
        assert!(HttpsTrust::is_trust_failure(
            "invalid peer certificate: UnknownIssuer"
        ));
        assert!(!HttpsTrust::is_trust_failure("releases status 404"));
        assert!(!HttpsTrust::is_trust_failure("download timed out"));
    }
}
