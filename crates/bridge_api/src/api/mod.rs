//! FRB-exposed API surface. `rust_input: crate::api` in
//! `apps/desktop/flutter_rust_bridge.yaml` roots the scan here.

pub mod mirrors;
pub mod profiles;

pub use mirrors::*;
pub use profiles::*;
