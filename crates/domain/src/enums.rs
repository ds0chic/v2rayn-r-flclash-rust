//! Upstream enums modelled with their exact numeric values so that persisted
//! guiNConfig.json / guiNDB.db data round-trips without renumbering.
//!
//! Numeric values follow `compat/features.yaml` (`config_types`) and the
//! frozen upstream sources under `work/research-v2rayn/source-latest/`.

use serde::{Deserialize, Serialize};

/// `EConfigType` (15 values). Values 1..=13 are protocols/custom kinds,
/// 101/102 are composite kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(try_from = "i32", into = "i32")]
pub enum ConfigType {
    #[default]
    Vmess,
    Custom,
    Shadowsocks,
    Socks,
    Vless,
    Trojan,
    Hysteria2,
    Tuic,
    WireGuard,
    Http,
    Anytls,
    Naive,
    Outbound,
    PolicyGroup,
    ProxyChain,
}

impl ConfigType {
    pub const ALL: [ConfigType; 15] = [
        ConfigType::Vmess,
        ConfigType::Custom,
        ConfigType::Shadowsocks,
        ConfigType::Socks,
        ConfigType::Vless,
        ConfigType::Trojan,
        ConfigType::Hysteria2,
        ConfigType::Tuic,
        ConfigType::WireGuard,
        ConfigType::Http,
        ConfigType::Anytls,
        ConfigType::Naive,
        ConfigType::Outbound,
        ConfigType::PolicyGroup,
        ConfigType::ProxyChain,
    ];

    pub const fn value(self) -> i32 {
        match self {
            ConfigType::Vmess => 1,
            ConfigType::Custom => 2,
            ConfigType::Shadowsocks => 3,
            ConfigType::Socks => 4,
            ConfigType::Vless => 5,
            ConfigType::Trojan => 6,
            ConfigType::Hysteria2 => 7,
            ConfigType::Tuic => 8,
            ConfigType::WireGuard => 9,
            ConfigType::Http => 10,
            ConfigType::Anytls => 11,
            ConfigType::Naive => 12,
            ConfigType::Outbound => 13,
            ConfigType::PolicyGroup => 101,
            ConfigType::ProxyChain => 102,
        }
    }

    pub fn from_value(value: i32) -> Option<Self> {
        Some(match value {
            1 => ConfigType::Vmess,
            2 => ConfigType::Custom,
            3 => ConfigType::Shadowsocks,
            4 => ConfigType::Socks,
            5 => ConfigType::Vless,
            6 => ConfigType::Trojan,
            7 => ConfigType::Hysteria2,
            8 => ConfigType::Tuic,
            9 => ConfigType::WireGuard,
            10 => ConfigType::Http,
            11 => ConfigType::Anytls,
            12 => ConfigType::Naive,
            13 => ConfigType::Outbound,
            101 => ConfigType::PolicyGroup,
            102 => ConfigType::ProxyChain,
            _ => return None,
        })
    }

    /// Composite/group kinds share the ChildItems reference semantics.
    pub const fn is_complex(self) -> bool {
        matches!(
            self,
            ConfigType::PolicyGroup
                | ConfigType::ProxyChain
                | ConfigType::Custom
                | ConfigType::Outbound
        )
    }

    /// Stable display token matching the upstream `ConfigType` column text.
    pub const fn as_str(self) -> &'static str {
        match self {
            ConfigType::Vmess => "VMess",
            ConfigType::Custom => "Custom",
            ConfigType::Shadowsocks => "Shadowsocks",
            ConfigType::Socks => "SOCKS",
            ConfigType::Vless => "VLESS",
            ConfigType::Trojan => "Trojan",
            ConfigType::Hysteria2 => "Hysteria2",
            ConfigType::Tuic => "TUIC",
            ConfigType::WireGuard => "WireGuard",
            ConfigType::Http => "HTTP",
            ConfigType::Anytls => "Anytls",
            ConfigType::Naive => "Naive",
            ConfigType::Outbound => "Outbound",
            ConfigType::PolicyGroup => "PolicyGroup",
            ConfigType::ProxyChain => "ProxyChain",
        }
    }
}

impl From<ConfigType> for i32 {
    fn from(v: ConfigType) -> i32 {
        v.value()
    }
}

impl TryFrom<i32> for ConfigType {
    type Error = crate::error::DomainError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        ConfigType::from_value(value)
            .ok_or_else(|| crate::error::DomainError::invalid_enum("ConfigType", value.to_string()))
    }
}

/// `ECoreType`. `V2rayN = 99` is the application-update identity, not a proxy
/// core; it is kept because it appears in persisted CoreTypeItem rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub enum CoreType {
    V2fly,
    Xray,
    V2flyV5,
    Mihomo,
    Hysteria,
    NaiveProxy,
    Tuic,
    SingBox,
    Juicity,
    Hysteria2,
    Brook,
    OverTls,
    ShadowQuic,
    Mieru,
    App,
}

impl CoreType {
    pub const ALL: [CoreType; 15] = [
        CoreType::V2fly,
        CoreType::Xray,
        CoreType::V2flyV5,
        CoreType::Mihomo,
        CoreType::Hysteria,
        CoreType::NaiveProxy,
        CoreType::Tuic,
        CoreType::SingBox,
        CoreType::Juicity,
        CoreType::Hysteria2,
        CoreType::Brook,
        CoreType::OverTls,
        CoreType::ShadowQuic,
        CoreType::Mieru,
        CoreType::App,
    ];

    /// Proxy cores only, without the `v2rayN=99` application identity.
    pub const PROXY_CORES: [CoreType; 14] = [
        CoreType::V2fly,
        CoreType::Xray,
        CoreType::V2flyV5,
        CoreType::Mihomo,
        CoreType::Hysteria,
        CoreType::NaiveProxy,
        CoreType::Tuic,
        CoreType::SingBox,
        CoreType::Juicity,
        CoreType::Hysteria2,
        CoreType::Brook,
        CoreType::OverTls,
        CoreType::ShadowQuic,
        CoreType::Mieru,
    ];

    pub const fn value(self) -> i32 {
        match self {
            CoreType::V2fly => 1,
            CoreType::Xray => 2,
            CoreType::V2flyV5 => 4,
            CoreType::Mihomo => 13,
            CoreType::Hysteria => 21,
            CoreType::NaiveProxy => 22,
            CoreType::Tuic => 23,
            CoreType::SingBox => 24,
            CoreType::Juicity => 25,
            CoreType::Hysteria2 => 26,
            CoreType::Brook => 27,
            CoreType::OverTls => 28,
            CoreType::ShadowQuic => 29,
            CoreType::Mieru => 30,
            CoreType::App => 99,
        }
    }

    pub fn from_value(value: i32) -> Option<Self> {
        Some(match value {
            1 => CoreType::V2fly,
            2 => CoreType::Xray,
            4 => CoreType::V2flyV5,
            13 => CoreType::Mihomo,
            21 => CoreType::Hysteria,
            22 => CoreType::NaiveProxy,
            23 => CoreType::Tuic,
            24 => CoreType::SingBox,
            25 => CoreType::Juicity,
            26 => CoreType::Hysteria2,
            27 => CoreType::Brook,
            28 => CoreType::OverTls,
            29 => CoreType::ShadowQuic,
            30 => CoreType::Mieru,
            99 => CoreType::App,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            CoreType::V2fly => "v2fly",
            CoreType::Xray => "Xray",
            CoreType::V2flyV5 => "v2fly_v5",
            CoreType::Mihomo => "mihomo",
            CoreType::Hysteria => "hysteria",
            CoreType::NaiveProxy => "naiveproxy",
            CoreType::Tuic => "tuic",
            CoreType::SingBox => "sing_box",
            CoreType::Juicity => "juicity",
            CoreType::Hysteria2 => "hysteria2",
            CoreType::Brook => "brook",
            CoreType::OverTls => "overtls",
            CoreType::ShadowQuic => "shadowquic",
            CoreType::Mieru => "mieru",
            CoreType::App => "v2rayN",
        }
    }
}

impl Default for CoreType {
    /// No persisted row has a meaningful zero; the app default run core is
    /// Xray, matching `GuiItem`'s default selection.
    fn default() -> Self {
        CoreType::Xray
    }
}

impl From<CoreType> for i32 {
    fn from(v: CoreType) -> i32 {
        v.value()
    }
}

impl TryFrom<i32> for CoreType {
    type Error = crate::error::DomainError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        CoreType::from_value(value)
            .ok_or_else(|| crate::error::DomainError::invalid_enum("CoreType", value.to_string()))
    }
}

/// `ETransport`. Serialized by name (serde default) which matches the stored
/// `Network` string column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    #[default]
    Raw,
    Kcp,
    Ws,
    #[serde(rename = "httpupgrade")]
    HttpUpgrade,
    Xhttp,
    H2,
    Http,
    Quic,
    Grpc,
}

impl Network {
    pub const ALL: [Network; 9] = [
        Network::Raw,
        Network::Kcp,
        Network::Ws,
        Network::HttpUpgrade,
        Network::Xhttp,
        Network::H2,
        Network::Http,
        Network::Quic,
        Network::Grpc,
    ];
}

/// `ProfileItem.StreamSecurity` values. Stored as free text upstream, but the
/// generator only branches on the two names below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    Tls,
    Reality,
}

impl Security {
    pub const fn as_str(self) -> &'static str {
        match self {
            Security::Tls => "tls",
            Security::Reality => "reality",
        }
    }
}

/// `ERuleType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub enum RuleType {
    All,
    Routing,
    Dns,
}

impl RuleType {
    pub const fn value(self) -> i32 {
        match self {
            RuleType::All => 0,
            RuleType::Routing => 1,
            RuleType::Dns => 2,
        }
    }

    pub fn from_value(value: i32) -> Option<Self> {
        Some(match value {
            0 => RuleType::All,
            1 => RuleType::Routing,
            2 => RuleType::Dns,
            _ => return None,
        })
    }
}

impl From<RuleType> for i32 {
    fn from(v: RuleType) -> i32 {
        v.value()
    }
}

impl TryFrom<i32> for RuleType {
    type Error = crate::error::DomainError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        RuleType::from_value(value)
            .ok_or_else(|| crate::error::DomainError::invalid_enum("RuleType", value.to_string()))
    }
}

/// `EMultipleLoad` (no explicit values upstream; ordinal order preserved).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MultipleLoad {
    #[default]
    LeastPing,
    Fallback,
    Random,
    RoundRobin,
    LeastLoad,
}

/// `ESysProxyType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(try_from = "i32", into = "i32")]
pub enum SysProxyType {
    #[default]
    ForcedClear,
    ForcedChange,
    Unchanged,
    Pac,
}

impl SysProxyType {
    pub const fn value(self) -> i32 {
        match self {
            SysProxyType::ForcedClear => 0,
            SysProxyType::ForcedChange => 1,
            SysProxyType::Unchanged => 2,
            SysProxyType::Pac => 3,
        }
    }

    pub fn from_value(value: i32) -> Option<Self> {
        Some(match value {
            0 => SysProxyType::ForcedClear,
            1 => SysProxyType::ForcedChange,
            2 => SysProxyType::Unchanged,
            3 => SysProxyType::Pac,
            _ => return None,
        })
    }
}

impl From<SysProxyType> for i32 {
    fn from(v: SysProxyType) -> i32 {
        v.value()
    }
}

impl TryFrom<i32> for SysProxyType {
    type Error = crate::error::DomainError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        SysProxyType::from_value(value).ok_or_else(|| {
            crate::error::DomainError::invalid_enum("SysProxyType", value.to_string())
        })
    }
}

/// `EGirdOrientation`. Persisted as an integer like every other upstream enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(try_from = "i32", into = "i32")]
pub enum GirdOrientation {
    Horizontal,
    #[default]
    Vertical,
    Tab,
}

impl GirdOrientation {
    pub const fn value(self) -> i32 {
        match self {
            GirdOrientation::Horizontal => 0,
            GirdOrientation::Vertical => 1,
            GirdOrientation::Tab => 2,
        }
    }

    pub fn from_value(value: i32) -> Option<Self> {
        Some(match value {
            0 => GirdOrientation::Horizontal,
            1 => GirdOrientation::Vertical,
            2 => GirdOrientation::Tab,
            _ => return None,
        })
    }
}

impl From<GirdOrientation> for i32 {
    fn from(v: GirdOrientation) -> i32 {
        v.value()
    }
}

impl TryFrom<i32> for GirdOrientation {
    type Error = crate::error::DomainError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        GirdOrientation::from_value(value).ok_or_else(|| {
            crate::error::DomainError::invalid_enum("GirdOrientation", value.to_string())
        })
    }
}

/// `ESpeedActionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpeedTestAction {
    Tcping,
    Realping,
    UdpTest,
    Speedtest,
    Mixedtest,
    FastRealping,
}

/// `ERuleMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub enum RuleMode {
    Rule,
    Global,
    Direct,
}

impl RuleMode {
    pub const fn value(self) -> i32 {
        match self {
            RuleMode::Rule => 0,
            RuleMode::Global => 1,
            RuleMode::Direct => 2,
        }
    }

    pub fn from_value(value: i32) -> Option<Self> {
        Some(match value {
            0 => RuleMode::Rule,
            1 => RuleMode::Global,
            2 => RuleMode::Direct,
            _ => return None,
        })
    }
}

impl From<RuleMode> for i32 {
    fn from(v: RuleMode) -> i32 {
        v.value()
    }
}

impl TryFrom<i32> for RuleMode {
    type Error = crate::error::DomainError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        RuleMode::from_value(value)
            .ok_or_else(|| crate::error::DomainError::invalid_enum("RuleMode", value.to_string()))
    }
}

/// `EInboundProtocol`. The inbound port offset equals the numeric value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(try_from = "i32", into = "i32")]
pub enum InboundProtocol {
    #[default]
    Socks,
    Socks2,
    Socks3,
    Pac,
    Api,
    Api2,
    Mixed,
    Speedtest,
}

impl InboundProtocol {
    pub const fn value(self) -> i32 {
        match self {
            InboundProtocol::Socks => 0,
            InboundProtocol::Socks2 => 1,
            InboundProtocol::Socks3 => 2,
            InboundProtocol::Pac => 3,
            InboundProtocol::Api => 4,
            InboundProtocol::Api2 => 5,
            InboundProtocol::Mixed => 6,
            InboundProtocol::Speedtest => 21,
        }
    }

    pub fn from_value(value: i32) -> Option<Self> {
        Some(match value {
            0 => InboundProtocol::Socks,
            1 => InboundProtocol::Socks2,
            2 => InboundProtocol::Socks3,
            3 => InboundProtocol::Pac,
            4 => InboundProtocol::Api,
            5 => InboundProtocol::Api2,
            6 => InboundProtocol::Mixed,
            21 => InboundProtocol::Speedtest,
            _ => return None,
        })
    }
}

impl From<InboundProtocol> for i32 {
    fn from(v: InboundProtocol) -> i32 {
        v.value()
    }
}

impl TryFrom<i32> for InboundProtocol {
    type Error = crate::error::DomainError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        InboundProtocol::from_value(value).ok_or_else(|| {
            crate::error::DomainError::invalid_enum("InboundProtocol", value.to_string())
        })
    }
}
