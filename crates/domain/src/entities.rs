//! Non-profile persisted entities.
//!
//! Field coverage follows `compat/fields.entities.yaml`: `SubItem` (17),
//! `RoutingItem` (13), `RulesItem` (13), `DNSItem` (9),
//! `FullConfigTemplateItem` (8), `ServerStatItem` (6), plus the extra records
//! named in plan §11 (MigrationRecord, TaskRecord, CoreInstallation,
//! WindowState).

use serde::{Deserialize, Serialize};

use crate::enums::{ConfigType, CoreType, InboundProtocol, MultipleLoad, RuleType};
use crate::profile::ExtraMap;
use crate::reference::{ReferenceExpr, ReferenceSource};

/// One subscription (`SubItem`, 17 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Subscription {
    pub id: String,
    pub remarks: String,
    pub url: String,
    pub more_url: String,
    pub enabled: bool,
    pub user_agent: String,
    /// Raw HTTP headers block, as stored upstream.
    pub request_headers: Option<String>,
    pub sort: i32,
    pub filter: Option<String>,
    pub auto_update_interval: i32,
    /// Unix time of last successful update.
    pub update_time: i64,
    /// Conversion target (e.g. an external subconverter).
    pub convert_target: Option<String>,
    /// Remarks-based chain reference resolved to a profile (REF-ENT-002).
    pub prev_profile: Option<String>,
    pub next_profile: Option<String>,
    pub pre_socks_port: Option<i32>,
    pub memo: Option<String>,
    pub custom_core_type: Option<CoreType>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl Subscription {
    pub fn prev_profile_ref(&self) -> Option<ReferenceExpr> {
        self.prev_profile
            .as_deref()
            .map(|raw| ReferenceExpr::remarks(raw, ReferenceSource::Imported))
    }

    pub fn next_profile_ref(&self) -> Option<ReferenceExpr> {
        self.next_profile
            .as_deref()
            .map(|raw| ReferenceExpr::remarks(raw, ReferenceSource::Imported))
    }
}

/// One routing rule (`RulesItem`, 13 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RoutingRule {
    pub id: String,
    /// Upstream free-text `Type` (rule-kind discriminator).
    pub rule_kind: Option<String>,
    pub port: Option<String>,
    pub network: Option<String>,
    pub inbound_tag: Option<Vec<String>>,
    /// Remarks string of the target outbound profile (REF-ENT-001).
    pub outbound_tag: Option<String>,
    pub ip: Option<Vec<String>>,
    pub domain: Option<Vec<String>>,
    pub protocol: Option<Vec<String>>,
    pub process: Option<Vec<String>>,
    pub enabled: bool,
    pub remarks: Option<String>,
    pub rule_type: Option<RuleType>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl RoutingRule {
    pub fn outbound_ref(&self) -> Option<ReferenceExpr> {
        self.outbound_tag
            .as_deref()
            .map(|raw| ReferenceExpr::remarks(raw, ReferenceSource::Imported))
    }
}

/// One routing rule set / profile (`RoutingItem`, 13 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RoutingProfile {
    pub id: String,
    pub remarks: String,
    pub url: String,
    /// JSON array of [`RoutingRule`], preserved as text to match storage.
    pub rule_set: String,
    pub rule_num: i32,
    pub enabled: bool,
    pub locked: bool,
    pub custom_icon: String,
    pub custom_ruleset_path4_singbox: String,
    pub domain_strategy: String,
    pub domain_strategy4_singbox: String,
    pub sort: i32,
    pub is_active: bool,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl RoutingProfile {
    /// Deserialize the embedded rule array. Order is preserved.
    pub fn rules(&self) -> Result<Vec<RoutingRule>, serde_json::Error> {
        if self.rule_set.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&self.rule_set)
    }

    /// Serialize rules back into the embedded text without reordering.
    pub fn set_rules(&mut self, rules: &[RoutingRule]) -> Result<(), serde_json::Error> {
        self.rule_set = serde_json::to_string(rules)?;
        self.rule_num = rules.len() as i32;
        Ok(())
    }
}

/// One DNS profile (`DNSItem`, 9 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DnsProfile {
    pub id: String,
    pub remarks: String,
    pub enabled: bool,
    /// Owning core (DNS config is per-core).
    pub core_type: CoreType,
    pub use_system_hosts: bool,
    pub normal_dns: Option<String>,
    pub tun_dns: Option<String>,
    pub domain_strategy4_freedom: Option<String>,
    pub domain_dns_address: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl Default for DnsProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            enabled: false,
            core_type: CoreType::Xray,
            use_system_hosts: false,
            normal_dns: None,
            tun_dns: None,
            domain_strategy4_freedom: None,
            domain_dns_address: None,
            extra: ExtraMap::new(),
        }
    }
}

/// Full config template (`FullConfigTemplateItem`, 8 properties).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FullConfigTemplate {
    pub id: String,
    pub remarks: String,
    pub enabled: bool,
    pub core_type: CoreType,
    pub config: Option<String>,
    pub tun_config: Option<String>,
    pub add_proxy_only: Option<bool>,
    pub proxy_detour: Option<String>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl Default for FullConfigTemplate {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            enabled: false,
            core_type: CoreType::Xray,
            config: None,
            tun_config: None,
            add_proxy_only: None,
            proxy_detour: None,
            extra: ExtraMap::new(),
        }
    }
}

/// Per-node traffic counters (`ServerStatItem`, 6 properties).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TrafficStats {
    pub index_id: String,
    pub total_up: i64,
    pub total_down: i64,
    pub today_up: i64,
    pub today_down: i64,
    /// Day bucket marker as stored upstream (epoch day).
    pub date_now: i64,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// Record of a completed migration step (plan §11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationRecord {
    /// Migration id from `compat/fields.yaml` (`MIG-ENT-*`).
    pub migration_id: String,
    pub from_version: i32,
    pub to_version: i32,
    /// Unix time the migration was applied.
    pub applied_at: i64,
    /// Source content hash that was migrated, for idempotency.
    pub source_hash: String,
    /// Number of entities touched.
    pub entities_touched: u32,
    pub notes: Option<String>,
}

/// Lifecycle record of a long-running task (plan §11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub job_id: crate::job::JobId,
    /// Task kind token, e.g. `update_subscription`, `start_test`.
    pub kind: String,
    pub state: crate::job::JobState,
    pub created_at: i64,
    pub updated_at: i64,
    /// Redacted diagnostic detail.
    pub detail: Option<String>,
}

/// A core binary installation (plan §11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreInstallation {
    pub core_type: CoreType,
    /// Path relative to the install root whenever possible (plan §11 forbids
    /// hard-coding machine-absolute paths into every record).
    pub relative_path: String,
    pub version: Option<String>,
    /// SHA-256 of the installed binary, when known.
    pub sha256: Option<String>,
    /// Whether the app manages updates for this core.
    pub update_supported: bool,
    /// Whether this is the currently selected run core.
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

fn default_dest_override() -> Option<Vec<String>> {
    Some(vec!["http".to_string(), "tls".to_string()])
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

fn is_default_orientation(value: &crate::enums::GirdOrientation) -> bool {
    *value == crate::enums::GirdOrientation::Vertical
}

/// Persisted window geometry / layout state (plan §11, FLD-CFG-156..158).
///
/// Upstream `WindowSizeItem` only carries `TypeName`/`Width`/`Height`; the
/// layout extras are skipped while they hold their default so a generated
/// `guiNConfig.json` keeps the upstream shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct WindowState {
    /// Window type key (`MainWindow`, dialog names, ...).
    pub type_name: String,
    pub width: i32,
    pub height: i32,
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub main_grid_height1: i32,
    #[serde(skip_serializing_if = "is_zero_i32")]
    pub main_grid_height2: i32,
    #[serde(skip_serializing_if = "is_default_orientation")]
    pub orientation: crate::enums::GirdOrientation,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// A column definition (`ColumnItem`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ColumnDefinition {
    pub name: String,
    pub width: i32,
    pub index: i32,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// A global hotkey binding (`KeyEventItem`; `EGlobalHotkey` kept as int).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct GlobalHotkey {
    #[serde(rename = "EGlobalHotkey")]
    pub action: i32,
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub key_code: Option<i32>,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// Per-config-type core selection (`CoreTypeItem`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct CoreTypeBinding {
    pub config_type: ConfigType,
    pub core_type: CoreType,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

impl CoreTypeBinding {
    pub fn new(config_type: ConfigType, core_type: CoreType) -> Self {
        Self {
            config_type,
            core_type,
            extra: ExtraMap::new(),
        }
    }
}

/// An inbound listener row (`InItem`, 11 properties; stored in guiNConfig.json).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct InboundListener {
    pub local_port: i32,
    pub protocol: InboundProtocol,
    pub udp_enabled: bool,
    #[serde(default = "default_true")]
    pub sniffing_enabled: bool,
    #[serde(default = "default_dest_override")]
    pub dest_override: Option<Vec<String>>,
    pub route_only: bool,
    #[serde(rename = "AllowLANConn")]
    pub allow_lan_conn: bool,
    #[serde(rename = "NewPort4LAN")]
    pub new_port4_lan: bool,
    pub user: String,
    pub pass: String,
    pub second_local_port_enabled: bool,
    #[serde(flatten)]
    pub extra: ExtraMap,
}

/// `EMultipleLoad` carried alongside group nodes for convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GroupLoadPolicy(pub MultipleLoad);
