//! Mirrors of the pure `domain` types so FRB can translate them.
//!
//! The external types must be re-exported publicly, and the placeholder
//! definitions below are only used at code-generation time; a mismatch
//! against `domain` becomes a compile error.

use flutter_rust_bridge::frb;

pub use domain::{ConfigType, CoreType, ProfileSummary};

#[frb(mirror(CoreType))]
pub enum _CoreType {
    Xray,
    SingBox,
    Mihomo,
    Custom,
}

#[frb(mirror(ConfigType))]
pub enum _ConfigType {
    Vmess,
    Vless,
    Trojan,
    Shadowsocks,
    Socks,
    Http,
    Hysteria2,
    Tuic,
}

#[frb(mirror(ProfileSummary))]
pub struct _ProfileSummary {
    pub id: String,
    pub config_type: ConfigType,
    pub remarks: String,
    pub address: String,
    pub port: u16,
    pub network: String,
    pub stream_security: String,
    pub sub_remarks: String,
    pub delay: i32,
    pub speed: String,
    pub today_up: u64,
    pub ip_info: String,
    pub today_down: u64,
    pub total_up: u64,
    pub total_down: u64,
    pub core_type: CoreType,
}
