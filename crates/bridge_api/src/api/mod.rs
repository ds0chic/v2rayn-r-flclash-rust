//! FRB-exposed API surface. `rust_input: crate::api` in
//! `apps/desktop/flutter_rust_bridge.yaml` roots the scan here.

pub mod contract;
pub mod dns;
pub mod engine;
pub mod groups;
pub mod mirrors;
pub mod monitor;
pub mod platform;
pub mod profiles;
pub mod routing;
pub mod settings;
pub mod speedtest;
pub mod stable;
pub mod subs;
pub mod t16;

pub use contract::*;
pub use dns::*;
pub use engine::*;
pub use groups::*;
pub use mirrors::*;
pub use monitor::*;
pub use platform::*;
pub use profiles::*;
pub use routing::*;
pub use settings::*;
pub use speedtest::*;
pub use subs::*;
pub use t16::*;
