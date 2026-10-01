//! Mirrors of the pure `domain` types so FRB can translate them.
//!
//! The external types must be re-exported publicly, and the placeholder
//! definitions below are only used at code-generation time; a mismatch
//! against `domain` becomes a compile error.

use flutter_rust_bridge::frb;

pub use domain::{
    CancelOutcome, ConfigType, CoreType, JobState, ProfileSummary, RevisionState, RuntimeState,
};

#[frb(mirror(CoreType))]
pub enum _CoreType {
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

#[frb(mirror(ConfigType))]
pub enum _ConfigType {
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

#[frb(mirror(RevisionState))]
pub enum _RevisionState {
    Empty,
    InSync,
    Pending,
    Ahead,
}

#[frb(mirror(RuntimeState))]
pub enum _RuntimeState {
    Stopped,
    Validating,
    Preparing,
    Starting,
    Checking,
    Running,
    RollingBack,
    Degraded,
}

#[frb(mirror(JobState))]
pub enum _JobState {
    Running,
    Cancelling,
    Compensating,
    Done,
    Failed,
    Cancelled,
}

#[frb(mirror(CancelOutcome))]
pub enum _CancelOutcome {
    Requested,
    Compensating,
    AlreadyFinished,
    NotCancellable,
}
