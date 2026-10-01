//! Versioned migration V2 -> V3 -> V4.
//!
//! Faithful port of `AppManager.MigrateProfileExtra*`
//! (`work/.../ServiceLib/Manager/AppManager.cs`, commit `7d6a967`). Each stage
//! filters on `ConfigVersion` and rewrites the version on success, so replaying
//! a completed migration is a no-op (idempotency, `MIG-ENT-001..004`). A single
//! failing row keeps its old version and is retried on the next run; the
//! surrounding batch is never dropped.

use serde_json::Value;

use crate::blobs::{ProtocolExtraBlob, TransportExtraBlob};
use crate::error::Result;
use crate::rows::RawRow;

pub const CONFIG_TYPE_POLICY_GROUP: i64 = 101;
pub const CONFIG_TYPE_PROXY_CHAIN: i64 = 102;

pub const CONFIG_VERSION_LEGACY: i32 = 2;
pub const CONFIG_VERSION_PROTO: i32 = 3;
pub const CONFIG_VERSION_TRANSPORT: i32 = 4;

/// Legacy network token that upstream rewrites to `raw` before V3->V4.
const RAW_NETWORK_ALIAS: &str = "tcp";
const DEFAULT_NETWORK: &str = "raw";
const NETWORKS: &[&str] = &["raw", "xhttp", "kcp", "grpc", "ws", "httpupgrade"];
const DEFAULT_WG_MTU: i32 = 1280;
const DEFAULT_HOP_INTERVAL: i64 = 30;

/// Subset of `HysteriaItem` consumed by the V2->V3 conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HysteriaMigrationInput {
    pub up_mbps: Option<i64>,
    pub down_mbps: Option<i64>,
    pub hop_interval: Option<i64>,
}

fn nullable(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

/// `Utils.NullIfEmpty` equivalent: collapse empty/whitespace to an absent key.
fn null_if_empty(row: &RawRow, column: &str) -> Option<String> {
    nullable(row.string(column))
}

fn config_version(row: &RawRow) -> i32 {
    row.opt_i64("ConfigVersion").unwrap_or(0) as i32
}

fn is_group(row: &RawRow) -> bool {
    matches!(
        row.opt_i64("ConfigType"),
        Some(CONFIG_TYPE_POLICY_GROUP) | Some(CONFIG_TYPE_PROXY_CHAIN)
    )
}

/// `MIG-ENT-002`: PolicyGroup/ProxyChain V2 -> V3, taking group details from the
/// deprecated `ProfileGroupItem` table (which is not deleted).
pub fn migrate_group_v2_to_v3(mut row: RawRow, group: Option<&RawRow>) -> Result<RawRow> {
    let mut extra = ProtocolExtraBlob::parse(row.opt_string("ProtoExtra").as_deref())?;
    extra.group_type = Some(match row.opt_i64("ConfigType") {
        Some(CONFIG_TYPE_PROXY_CHAIN) => "ProxyChain".to_string(),
        _ => "PolicyGroup".to_string(),
    });
    if let Some(group) = group {
        let child_items = group.string("ChildItems");
        let sub_child_items = group.string("SubChildItems");
        let has_child = !child_items.trim().is_empty() || !sub_child_items.trim().is_empty();
        if has_child {
            extra.child_items = nullable(child_items);
            extra.sub_child_items = nullable(sub_child_items);
            extra.filter = nullable(group.string("Filter"));
            extra.multiple_load = group.opt_i64("MultipleLoad").map(|v| v as i32);
        }
    }
    row.set("ProtoExtra", Value::String(extra.to_json()?));
    row.set("ConfigVersion", Value::from(CONFIG_VERSION_PROTO));
    Ok(row)
}

/// `MIG-ENT-003`: normal node V2 -> V3, moving legacy columns into
/// `ProtocolExtra` per config type.
pub fn migrate_profile_v2_to_v3(
    mut row: RawRow,
    hysteria: HysteriaMigrationInput,
) -> Result<RawRow> {
    let mut extra = ProtocolExtraBlob::parse(row.opt_string("ProtoExtra").as_deref())?;
    let config_type = row.opt_i64("ConfigType").unwrap_or(0);
    match config_type {
        3 => {
            extra.ss_method = null_if_empty(&row, "Security");
        }
        1 => {
            extra.alter_id = Some(row.opt_i64("AlterId").unwrap_or(0).to_string());
            extra.vmess_security = null_if_empty(&row, "Security");
        }
        5 => {
            extra.flow = null_if_empty(&row, "Flow");
            extra.vless_encryption = Some(row.string("Security"));
        }
        7 => {
            extra.salamander_pass = null_if_empty(&row, "Path");
            extra.ports = null_if_empty(&row, "Ports");
            extra.up_mbps = Some(hysteria.up_mbps.unwrap_or(0) as i32);
            extra.down_mbps = Some(hysteria.down_mbps.unwrap_or(0) as i32);
            extra.hop_interval = Some(
                hysteria
                    .hop_interval
                    .unwrap_or(DEFAULT_HOP_INTERVAL)
                    .to_string(),
            );
        }
        8 => {
            extra.congestion_control = null_if_empty(&row, "HeaderType");
            let security = row.string("Security");
            row.set("Username", Value::String(row.string("Id")));
            row.set("Id", Value::String(security.clone()));
            row.set("Password", Value::String(security));
        }
        10 | 4 => {
            row.set("Username", Value::String(row.string("Security")));
        }
        9 => {
            extra.wg_public_key = null_if_empty(&row, "PublicKey");
            extra.wg_interface_address = null_if_empty(&row, "RequestHost");
            extra.wg_reserved = null_if_empty(&row, "Path");
            extra.wg_mtu = Some(
                row.opt_string("ShortId")
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .unwrap_or(DEFAULT_WG_MTU),
            );
        }
        _ => {}
    }
    row.set("ProtoExtra", Value::String(extra.to_json()?));
    // Universal tail of the upstream loop.
    row.set("Password", Value::String(row.string("Id")));
    row.set("ConfigVersion", Value::from(CONFIG_VERSION_PROTO));
    Ok(row)
}

/// `ProfileItem.GetNetwork()` normalization used by the V3->V4 stage.
fn effective_network(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !NETWORKS.contains(&trimmed) {
        DEFAULT_NETWORK.to_string()
    } else {
        trimmed.to_string()
    }
}

/// `MIG-ENT-004`: transport V3 -> V4, filling `TransportExtra` from the legacy
/// `HeaderType/RequestHost/Path/Extra` columns.
pub fn migrate_transport_v3_to_v4(mut row: RawRow) -> Result<RawRow> {
    if row.string("Network") == RAW_NETWORK_ALIAS {
        row.set("Network", Value::String(DEFAULT_NETWORK.to_string()));
    }
    let mut transport = TransportExtraBlob::parse(row.opt_string("TransportExtra").as_deref())?;
    let network = effective_network(&row.string("Network"));
    match network.as_str() {
        "raw" => {
            transport.raw_header_type = null_if_empty(&row, "HeaderType");
            transport.host = null_if_empty(&row, "RequestHost");
            transport.path = null_if_empty(&row, "Path");
        }
        "ws" | "httpupgrade" => {
            transport.host = null_if_empty(&row, "RequestHost");
            transport.path = null_if_empty(&row, "Path");
        }
        "xhttp" => {
            transport.host = null_if_empty(&row, "RequestHost");
            transport.path = null_if_empty(&row, "Path");
            transport.xhttp_mode = null_if_empty(&row, "HeaderType");
            transport.xhttp_extra = null_if_empty(&row, "Extra");
        }
        "grpc" => {
            transport.grpc_authority = null_if_empty(&row, "RequestHost");
            transport.grpc_service_name = null_if_empty(&row, "Path");
            transport.grpc_mode = null_if_empty(&row, "HeaderType");
        }
        "kcp" => {
            transport.kcp_header_type = null_if_empty(&row, "HeaderType");
            transport.kcp_seed = null_if_empty(&row, "Path");
        }
        _ => {
            row.set("Network", Value::String(DEFAULT_NETWORK.to_string()));
            transport.raw_header_type = null_if_empty(&row, "HeaderType");
            transport.host = null_if_empty(&row, "RequestHost");
        }
    }
    row.set("TransportExtra", Value::String(transport.to_json()?));
    row.set("ConfigVersion", Value::from(CONFIG_VERSION_TRANSPORT));
    Ok(row)
}

/// Why a row was left untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationFailure {
    pub migration_id: &'static str,
    pub source_id: String,
    pub message: String,
}

/// Aggregate result of a migration run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationStats {
    pub groups_migrated: u32,
    pub profiles_migrated: u32,
    pub transports_migrated: u32,
    pub already_current: u32,
    pub failures: Vec<MigrationFailure>,
    /// Migration ids that actually changed at least one row.
    pub applied: Vec<&'static str>,
}

impl MigrationStats {
    pub fn total_migrated(&self) -> u32 {
        self.groups_migrated + self.profiles_migrated + self.transports_migrated
    }
}

/// Apply the three migration stages in upstream order. Failing rows keep their
/// old version and are reported; the run never aborts.
pub fn run_migrations(
    mut rows: Vec<RawRow>,
    groups: &[RawRow],
    hysteria: HysteriaMigrationInput,
) -> (Vec<RawRow>, MigrationStats) {
    let mut stats = MigrationStats::default();

    // Stage 1: group nodes.
    let mut any_group = false;
    for row in rows.iter_mut() {
        if config_version(row) < CONFIG_VERSION_PROTO && is_group(row) {
            let id = row.string("IndexId");
            let group = groups.iter().find(|g| g.string("IndexId") == id);
            match migrate_group_v2_to_v3(row.clone(), group) {
                Ok(updated) => {
                    *row = updated;
                    stats.groups_migrated += 1;
                    any_group = true;
                }
                Err(err) => stats.failures.push(failure("MIG-ENT-002", &id, err)),
            }
        }
    }
    if any_group {
        stats.applied.push("MIG-ENT-002");
    }

    // Stage 2: normal nodes.
    let mut any_profile = false;
    for row in rows.iter_mut() {
        if config_version(row) < CONFIG_VERSION_PROTO && !is_group(row) {
            let id = row.string("IndexId");
            match migrate_profile_v2_to_v3(row.clone(), hysteria) {
                Ok(updated) => {
                    *row = updated;
                    stats.profiles_migrated += 1;
                    any_profile = true;
                }
                Err(err) => stats.failures.push(failure("MIG-ENT-003", &id, err)),
            }
        }
    }
    if any_profile {
        stats.applied.push("MIG-ENT-003");
    }

    // Stage 3: transport.
    let mut any_transport = false;
    for row in rows.iter_mut() {
        if config_version(row) == CONFIG_VERSION_PROTO {
            let id = row.string("IndexId");
            match migrate_transport_v3_to_v4(row.clone()) {
                Ok(updated) => {
                    *row = updated;
                    stats.transports_migrated += 1;
                    any_transport = true;
                }
                Err(err) => stats.failures.push(failure("MIG-ENT-004", &id, err)),
            }
        }
    }
    if any_transport {
        stats.applied.push("MIG-ENT-004");
    }

    stats.already_current = rows
        .iter()
        .filter(|r| config_version(r) >= CONFIG_VERSION_TRANSPORT)
        .count() as u32;

    (rows, stats)
}

fn failure(
    migration_id: &'static str,
    source_id: &str,
    err: crate::error::PersistenceError,
) -> MigrationFailure {
    MigrationFailure {
        migration_id,
        source_id: source_id.to_string(),
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(pairs: &[(&str, Value)]) -> RawRow {
        let mut r = RawRow::new("ProfileItem");
        for (k, v) in pairs {
            r.set(k, v.clone());
        }
        r
    }

    #[test]
    fn shadowsocks_security_moves_to_ss_method() {
        let r = row(&[
            ("IndexId", json!("a")),
            ("ConfigType", json!(3)),
            ("ConfigVersion", json!(2)),
            ("Security", json!("aes-256-gcm")),
            ("Id", json!("pw")),
        ]);
        let out = migrate_profile_v2_to_v3(r, HysteriaMigrationInput::default()).unwrap();
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.ss_method.as_deref(), Some("aes-256-gcm"));
        assert_eq!(out.i64("ConfigVersion"), 3);
        assert_eq!(out.string("Password"), "pw");
    }

    #[test]
    fn vmess_and_vless_field_mapping() {
        let vmess = row(&[
            ("ConfigType", json!(1)),
            ("AlterId", json!(64)),
            ("Security", json!("auto")),
            ("Id", json!("uuid")),
        ]);
        let out = migrate_profile_v2_to_v3(vmess, HysteriaMigrationInput::default()).unwrap();
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.alter_id.as_deref(), Some("64"));
        assert_eq!(extra.vmess_security.as_deref(), Some("auto"));

        let vless = row(&[
            ("ConfigType", json!(5)),
            ("Flow", json!("xtls-rprx-vision")),
            ("Security", json!("")),
        ]);
        let out = migrate_profile_v2_to_v3(vless, HysteriaMigrationInput::default()).unwrap();
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.flow.as_deref(), Some("xtls-rprx-vision"));
        assert_eq!(extra.vless_encryption.as_deref(), Some(""));
    }

    #[test]
    fn hysteria2_pulls_global_values_and_tuic_rewrites_credentials() {
        let hy = row(&[
            ("ConfigType", json!(7)),
            ("Path", json!("obfs-pass")),
            ("Ports", json!("1000-2000")),
        ]);
        let out = migrate_profile_v2_to_v3(
            hy,
            HysteriaMigrationInput {
                up_mbps: Some(50),
                down_mbps: Some(100),
                hop_interval: Some(15),
            },
        )
        .unwrap();
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.salamander_pass.as_deref(), Some("obfs-pass"));
        assert_eq!(extra.ports.as_deref(), Some("1000-2000"));
        assert_eq!(extra.up_mbps, Some(50));
        assert_eq!(extra.down_mbps, Some(100));
        assert_eq!(extra.hop_interval.as_deref(), Some("15"));

        let tuic = row(&[
            ("ConfigType", json!(8)),
            ("HeaderType", json!("bbr")),
            ("Id", json!("idtok")),
            ("Security", json!("secret")),
        ]);
        let out = migrate_profile_v2_to_v3(tuic, HysteriaMigrationInput::default()).unwrap();
        assert_eq!(out.string("Username"), "idtok");
        assert_eq!(out.string("Id"), "secret");
        assert_eq!(out.string("Password"), "secret");
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.congestion_control.as_deref(), Some("bbr"));
    }

    #[test]
    fn wireguard_mtu_parses_or_defaults() {
        let wg = row(&[
            ("ConfigType", json!(9)),
            ("PublicKey", json!("pk")),
            ("RequestHost", json!("10.0.0.2/32")),
            ("Path", json!("reserved")),
            ("ShortId", json!("1420")),
        ]);
        let out = migrate_profile_v2_to_v3(wg, HysteriaMigrationInput::default()).unwrap();
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.wg_mtu, Some(1420));
        assert_eq!(extra.wg_public_key.as_deref(), Some("pk"));

        let wg_unparsed = row(&[("ConfigType", json!(9)), ("ShortId", json!("oops"))]);
        let out = migrate_profile_v2_to_v3(wg_unparsed, HysteriaMigrationInput::default()).unwrap();
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.wg_mtu, Some(DEFAULT_WG_MTU));
    }

    #[test]
    fn group_v2_to_v3_copies_group_table() {
        let group = row(&[
            ("IndexId", json!("g1")),
            ("ChildItems", json!("a,b")),
            ("SubChildItems", json!("sub-1")),
            ("Filter", json!("^HK")),
            ("MultipleLoad", json!(3)),
        ]);
        let profile = row(&[
            ("IndexId", json!("g1")),
            ("ConfigType", json!(101)),
            ("ConfigVersion", json!(2)),
        ]);
        let out = migrate_group_v2_to_v3(profile, Some(&group)).unwrap();
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.group_type.as_deref(), Some("PolicyGroup"));
        assert_eq!(extra.child_items.as_deref(), Some("a,b"));
        assert_eq!(extra.sub_child_items.as_deref(), Some("sub-1"));
        assert_eq!(extra.filter.as_deref(), Some("^HK"));
        assert_eq!(extra.multiple_load, Some(3));
        assert_eq!(out.i64("ConfigVersion"), 3);
    }

    #[test]
    fn group_without_children_keeps_group_type_only() {
        let group = row(&[("IndexId", json!("g2")), ("ChildItems", json!(""))]);
        let profile = row(&[("IndexId", json!("g2")), ("ConfigType", json!(102))]);
        let out = migrate_group_v2_to_v3(profile, Some(&group)).unwrap();
        let extra = ProtocolExtraBlob::parse(out.opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.group_type.as_deref(), Some("ProxyChain"));
        assert_eq!(extra.child_items, None);
    }

    #[test]
    fn transport_v3_to_v4_branches_per_network() {
        let ws = row(&[
            ("ConfigType", json!(5)),
            ("ConfigVersion", json!(3)),
            ("Network", json!("ws")),
            ("RequestHost", json!("h")),
            ("Path", json!("/p")),
        ]);
        let out = migrate_transport_v3_to_v4(ws).unwrap();
        let t = TransportExtraBlob::parse(out.opt_string("TransportExtra").as_deref()).unwrap();
        assert_eq!(t.host.as_deref(), Some("h"));
        assert_eq!(t.path.as_deref(), Some("/p"));
        assert_eq!(out.i64("ConfigVersion"), 4);

        let grpc = row(&[
            ("ConfigVersion", json!(3)),
            ("Network", json!("grpc")),
            ("RequestHost", json!("auth")),
            ("Path", json!("svc")),
            ("HeaderType", json!("multi")),
        ]);
        let out = migrate_transport_v3_to_v4(grpc).unwrap();
        let t = TransportExtraBlob::parse(out.opt_string("TransportExtra").as_deref()).unwrap();
        assert_eq!(t.grpc_authority.as_deref(), Some("auth"));
        assert_eq!(t.grpc_service_name.as_deref(), Some("svc"));
        assert_eq!(t.grpc_mode.as_deref(), Some("multi"));

        let legacy_tcp = row(&[
            ("ConfigVersion", json!(3)),
            ("Network", json!("tcp")),
            ("RequestHost", json!("host")),
        ]);
        let out = migrate_transport_v3_to_v4(legacy_tcp).unwrap();
        assert_eq!(out.string("Network"), "raw");
        let t = TransportExtraBlob::parse(out.opt_string("TransportExtra").as_deref()).unwrap();
        assert_eq!(t.host.as_deref(), Some("host"));
    }

    #[test]
    fn run_migrations_is_idempotent() {
        let rows = vec![
            row(&[
                ("IndexId", json!("n1")),
                ("ConfigType", json!(1)),
                ("ConfigVersion", json!(2)),
                ("AlterId", json!(0)),
                ("Id", json!("u")),
            ]),
            row(&[
                ("IndexId", json!("g1")),
                ("ConfigType", json!(101)),
                ("ConfigVersion", json!(2)),
            ]),
            row(&[
                ("IndexId", json!("t1")),
                ("ConfigType", json!(5)),
                ("ConfigVersion", json!(3)),
                ("Network", json!("ws")),
                ("Path", json!("/x")),
            ]),
        ];
        let (first, stats) = run_migrations(rows.clone(), &[], HysteriaMigrationInput::default());
        assert_eq!(stats.profiles_migrated, 1);
        assert_eq!(stats.groups_migrated, 1);
        // The transport stage runs after the two V3 stages in the same pass, so
        // it picks up every row that was just promoted to ConfigVersion 3.
        assert_eq!(stats.transports_migrated, 3);
        assert_eq!(stats.total_migrated(), 5);
        assert!(stats.failures.is_empty());
        assert!(first.iter().all(|r| r.i64("ConfigVersion") == 4));

        let (second, stats2) = run_migrations(first, &[], HysteriaMigrationInput::default());
        assert_eq!(stats2.total_migrated(), 0);
        assert_eq!(second.len(), 3);
    }
}
