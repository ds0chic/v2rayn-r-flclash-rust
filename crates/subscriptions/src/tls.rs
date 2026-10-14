//! HTTPS trust source for subscription downloads (SP-25).
//!
//! Mirrors upstream v2rayN 7.25.4 `GuiItem.RootCertProvider` semantics (see
//! `platform::cert`): `system` uses the OS/native roots, while `chrome` and
//! `mozilla` trust exactly a bundled PEM collection and never the OS store.
//! The contract is owned by `platform::http` and shared with the update
//! pipeline (`updater::tls`).

pub use platform::http::{apply_trust, trust_failure_in_text, trust_failure_of, HttpsTrust};
