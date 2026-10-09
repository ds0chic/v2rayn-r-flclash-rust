//! HTTPS trust source for the update pipeline (SP-25).
//!
//! Mirrors upstream v2rayN 7.25.4 `GuiItem.RootCertProvider` semantics (see
//! `platform::cert`): `system` uses the OS/native roots, while `chrome` and
//! `mozilla` trust exactly a bundled PEM collection and never the OS store.
//! The contract is owned by `platform::http` and shared with subscription
//! downloads (`subscriptions::tls`).

pub use platform::http::{apply_trust, trust_failure_in_text, trust_failure_of, HttpsTrust};

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
