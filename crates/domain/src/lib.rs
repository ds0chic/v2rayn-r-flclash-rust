//! Pure domain models for the T01 feasibility probe.
//!
//! This crate must stay free of Flutter, window and platform APIs. It only
//! carries the node summary shape needed by the 14-column profile table
//! (`compat/layouts.yaml` LAY-PROFILES-002) plus the owning core kind.

/// Core engine that owns the generated configuration for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreType {
    Xray,
    SingBox,
    Mihomo,
    Custom,
}

/// Protocol family of a node, expressed as the display token used by the
/// `ConfigType` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigType {
    Vmess,
    Vless,
    Trojan,
    Shadowsocks,
    Socks,
    Http,
    Hysteria2,
    Tuic,
}

impl ConfigType {
    /// Stable display token, matching the upstream `ConfigType` column text.
    pub fn as_str(self) -> &'static str {
        match self {
            ConfigType::Vmess => "vmess",
            ConfigType::Vless => "vless",
            ConfigType::Trojan => "trojan",
            ConfigType::Shadowsocks => "shadowsocks",
            ConfigType::Socks => "socks",
            ConfigType::Http => "http",
            ConfigType::Hysteria2 => "hysteria2",
            ConfigType::Tuic => "tuic",
        }
    }
}

/// One row of the profiles table. Field set mirrors the 14 columns of
/// `LAY-PROFILES-002` and adds the row identity plus the owning core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSummary {
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
