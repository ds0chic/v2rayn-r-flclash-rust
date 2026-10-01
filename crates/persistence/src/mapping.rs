//! Typed mapping from upstream rows to domain entities.
//!
//! Conversion is by column name and never by position. Legacy / unrecognised
//! columns are copied into the entity's `extra` bag so nothing is lost
//! (plan §11: "不能为了强类型而删除未知扩展").

use domain::{
    ConfigType, CoreType, DnsProfile, FullConfigTemplate, Profile, RoutingProfile, RoutingRule,
    RuleType, SecurityParams, Subscription, TrafficStats,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::blobs::{ProtocolExtraBlob, TransportExtraBlob};
use crate::error::{PersistenceError, Result};
use crate::rows::RawRow;

/// Columns consumed explicitly by [`map_profile`]; every other column is kept
/// in `Profile.extra`.
const PROFILE_KNOWN_COLUMNS: &[&str] = &[
    "IndexId",
    "ConfigType",
    "CoreType",
    "ConfigVersion",
    "Subid",
    "IsSub",
    "PreSocksPort",
    "DisplayLog",
    "Remarks",
    "Address",
    "Port",
    "Password",
    "Username",
    "Network",
    "MuxEnabled",
    "Finalmask",
    "ProtoExtra",
    "TransportExtra",
    "StreamSecurity",
    "AllowInsecure",
    "Sni",
    "Alpn",
    "Fingerprint",
    "PublicKey",
    "ShortId",
    "SpiderX",
    "Mldsa65Verify",
    "Cert",
    "CertSha",
    "EchConfigList",
    "VerifyPeerCertByName",
];

fn collect_unknown(row: &RawRow, known: &[&str]) -> Map<String, Value> {
    let mut extra = Map::new();
    for (key, value) in &row.values {
        if !known.contains(&key.as_str()) {
            extra.insert(key.clone(), value.clone());
        }
    }
    extra
}

fn config_type(row: &RawRow) -> Result<ConfigType> {
    let value = row.opt_i64("ConfigType").unwrap_or(0) as i32;
    ConfigType::from_value(value).ok_or_else(|| {
        PersistenceError::internal(format!(
            "unknown ConfigType {value} on {}",
            row.string("IndexId")
        ))
    })
}

fn core_type(row: &RawRow) -> Option<CoreType> {
    row.opt_i64("CoreType")
        .and_then(|v| CoreType::from_value(v as i32))
}

fn security_params(row: &RawRow) -> SecurityParams {
    SecurityParams {
        stream_security: nullable(row.opt_string("StreamSecurity")),
        allow_insecure: nullable(row.opt_string("AllowInsecure")),
        sni: nullable(row.opt_string("Sni")),
        alpn: nullable(row.opt_string("Alpn")),
        fingerprint: nullable(row.opt_string("Fingerprint")),
        public_key: nullable(row.opt_string("PublicKey")),
        short_id: nullable(row.opt_string("ShortId")),
        spider_x: nullable(row.opt_string("SpiderX")),
        mldsa65_verify: nullable(row.opt_string("Mldsa65Verify")),
        cert: nullable(row.opt_string("Cert")),
        cert_sha: nullable(row.opt_string("CertSha")),
        ech_config_list: nullable(row.opt_string("EchConfigList")),
        verify_peer_cert_by_name: nullable(row.opt_string("VerifyPeerCertByName")),
    }
}

fn nullable(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// Map a `ProfileItem` row to a [`Profile`].
pub fn map_profile(row: &RawRow) -> Result<Profile> {
    let proto_extra = ProtocolExtraBlob::parse(row.opt_string("ProtoExtra").as_deref())?;
    let transport_extra = TransportExtraBlob::parse(row.opt_string("TransportExtra").as_deref())?;
    Ok(Profile {
        index_id: row.string("IndexId"),
        config_type: config_type(row)?,
        core_type: core_type(row),
        config_version: row.opt_i64("ConfigVersion").unwrap_or(0) as i32,
        subid: row.string("Subid"),
        is_sub: row.bool("IsSub"),
        pre_socks_port: row.opt_i64("PreSocksPort").map(|v| v as i32),
        display_log: row.bool("DisplayLog"),
        remarks: row.string("Remarks"),
        address: row.string("Address"),
        port: row.opt_i64("Port").unwrap_or(0) as i32,
        password: row.string("Password"),
        username: row.string("Username"),
        network: row.string("Network"),
        mux_enabled: row.opt_i64("MuxEnabled").map(|v| v != 0),
        finalmask: nullable(row.opt_string("Finalmask")),
        outbound_tag: None,
        security: security_params(row),
        proto_extra: proto_extra.to_domain(),
        transport_extra: transport_extra.to_domain(),
        extra: collect_unknown(row, PROFILE_KNOWN_COLUMNS),
    })
}

/// Map a `SubItem` row to a [`Subscription`].
pub fn map_subscription(row: &RawRow) -> Subscription {
    Subscription {
        id: row.string("Id"),
        remarks: row.string("Remarks"),
        url: row.string("Url"),
        more_url: row.string("MoreUrl"),
        enabled: row.bool("Enabled"),
        user_agent: row.string("UserAgent"),
        request_headers: row.opt_string("RequestHeaders"),
        sort: row.opt_i64("Sort").unwrap_or(0) as i32,
        filter: row.opt_string("Filter"),
        auto_update_interval: row.opt_i64("AutoUpdateInterval").unwrap_or(0) as i32,
        update_time: row.opt_i64("UpdateTime").unwrap_or(0),
        convert_target: row.opt_string("ConvertTarget"),
        prev_profile: row.opt_string("PrevProfile"),
        next_profile: row.opt_string("NextProfile"),
        pre_socks_port: row.opt_i64("PreSocksPort").map(|v| v as i32),
        memo: row.opt_string("Memo"),
        custom_core_type: row
            .opt_i64("CustomCoreType")
            .and_then(|v| CoreType::from_value(v as i32)),
        extra: collect_unknown(
            row,
            &[
                "Id",
                "Remarks",
                "Url",
                "MoreUrl",
                "Enabled",
                "UserAgent",
                "RequestHeaders",
                "Sort",
                "Filter",
                "AutoUpdateInterval",
                "UpdateTime",
                "ConvertTarget",
                "PrevProfile",
                "NextProfile",
                "PreSocksPort",
                "Memo",
                "CustomCoreType",
            ],
        ),
    }
}

/// Map a `RoutingItem` row to a [`RoutingProfile`].
pub fn map_routing_profile(row: &RawRow) -> RoutingProfile {
    RoutingProfile {
        id: row.string("Id"),
        remarks: row.string("Remarks"),
        url: row.string("Url"),
        rule_set: row.string("RuleSet"),
        rule_num: row.opt_i64("RuleNum").unwrap_or(0) as i32,
        enabled: row.bool("Enabled"),
        locked: row.bool("Locked"),
        custom_icon: row.string("CustomIcon"),
        custom_ruleset_path4_singbox: row.string("CustomRulesetPath4Singbox"),
        domain_strategy: row.string("DomainStrategy"),
        domain_strategy4_singbox: row.string("DomainStrategy4Singbox"),
        sort: row.opt_i64("Sort").unwrap_or(0) as i32,
        is_active: row.bool("IsActive"),
        extra: Map::new(),
    }
}

/// Map a `DNSItem` row to a [`DnsProfile`].
pub fn map_dns(row: &RawRow) -> Result<DnsProfile> {
    let core_type = row
        .opt_i64("CoreType")
        .and_then(|v| CoreType::from_value(v as i32))
        .unwrap_or_default();
    Ok(DnsProfile {
        id: row.string("Id"),
        remarks: row.string("Remarks"),
        enabled: row.bool("Enabled"),
        core_type,
        use_system_hosts: row.bool("UseSystemHosts"),
        normal_dns: row.opt_string("NormalDNS"),
        tun_dns: row.opt_string("TunDNS"),
        domain_strategy4_freedom: row.opt_string("DomainStrategy4Freedom"),
        domain_dns_address: row.opt_string("DomainDNSAddress"),
        extra: Map::new(),
    })
}

/// Map a `FullConfigTemplateItem` row to a [`FullConfigTemplate`].
pub fn map_template(row: &RawRow) -> Result<FullConfigTemplate> {
    let core_type = row
        .opt_i64("CoreType")
        .and_then(|v| CoreType::from_value(v as i32))
        .unwrap_or_default();
    Ok(FullConfigTemplate {
        id: row.string("Id"),
        remarks: row.string("Remarks"),
        enabled: row.bool("Enabled"),
        core_type,
        config: row.opt_string("Config"),
        tun_config: row.opt_string("TunConfig"),
        add_proxy_only: row.opt_i64("AddProxyOnly").map(|v| v != 0),
        proxy_detour: row.opt_string("ProxyDetour"),
        extra: Map::new(),
    })
}

/// Map a `ServerStatItem` row to [`TrafficStats`].
pub fn map_traffic(row: &RawRow) -> TrafficStats {
    TrafficStats {
        index_id: row.string("IndexId"),
        total_up: row.opt_i64("TotalUp").unwrap_or(0),
        total_down: row.opt_i64("TotalDown").unwrap_or(0),
        today_up: row.opt_i64("TodayUp").unwrap_or(0),
        today_down: row.opt_i64("TodayDown").unwrap_or(0),
        date_now: row.opt_i64("DateNow").unwrap_or(0),
        extra: Map::new(),
    }
}

/// `ProfileExItem` (6 properties). No `domain` entity exists for it, so the
/// persistence layer owns the storage shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ProfileExRow {
    pub index_id: String,
    pub delay: i32,
    pub speed: f64,
    pub sort: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_info: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ProfileExRow {
    pub fn from_raw(row: &RawRow) -> Self {
        Self {
            index_id: row.string("IndexId"),
            delay: row.opt_i64("Delay").unwrap_or(0) as i32,
            speed: row.opt_f64("Speed").unwrap_or(0.0),
            sort: row.opt_i64("Sort").unwrap_or(0) as i32,
            message: row.opt_string("Message"),
            ip_info: row.opt_string("IpInfo"),
            extra: collect_unknown(
                row,
                &["IndexId", "Delay", "Speed", "Sort", "Message", "IpInfo"],
            ),
        }
    }
}

/// `ProfileGroupItem` (5 properties); migration source only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ProfileGroupRow {
    pub index_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_child_items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    pub multiple_load: i32,
}

/// `RulesItem` (13 properties) as embedded JSON inside `RoutingItem.RuleSet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct RulesItemStorage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "Type", skip_serializing_if = "Option::is_none")]
    pub rule_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbound_tag: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outbound_tag: Option<String>,
    #[serde(rename = "Ip", skip_serializing_if = "Option::is_none")]
    pub ip: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Vec<String>>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_type: Option<i32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl RulesItemStorage {
    pub fn to_domain(&self) -> RoutingRule {
        RoutingRule {
            id: self.id.clone().unwrap_or_default(),
            rule_kind: self.rule_kind.clone(),
            port: self.port.clone(),
            network: self.network.clone(),
            inbound_tag: self.inbound_tag.clone(),
            outbound_tag: self.outbound_tag.clone(),
            ip: self.ip.clone(),
            domain: self.domain.clone(),
            protocol: self.protocol.clone(),
            process: self.process.clone(),
            enabled: self.enabled,
            remarks: self.remarks.clone(),
            rule_type: self.rule_type.and_then(RuleType::from_value),
            extra: self.extra.clone(),
        }
    }

    pub fn from_domain(rule: &RoutingRule) -> Self {
        Self {
            id: Some(rule.id.clone()),
            rule_kind: rule.rule_kind.clone(),
            port: rule.port.clone(),
            network: rule.network.clone(),
            inbound_tag: rule.inbound_tag.clone(),
            outbound_tag: rule.outbound_tag.clone(),
            ip: rule.ip.clone(),
            domain: rule.domain.clone(),
            protocol: rule.protocol.clone(),
            process: rule.process.clone(),
            enabled: rule.enabled,
            remarks: rule.remarks.clone(),
            rule_type: rule.rule_type.map(RuleType::value),
            extra: rule.extra.clone(),
        }
    }
}

/// Parse a `RoutingItem.RuleSet` JSON array, preserving order. Empty text yields
/// an empty vector; malformed JSON is surfaced rather than silently dropped.
pub fn parse_rules(rule_set: &str) -> Result<Vec<RoutingRule>> {
    if rule_set.trim().is_empty() {
        return Ok(Vec::new());
    }
    let items: Vec<RulesItemStorage> = serde_json::from_str(rule_set)?;
    Ok(items.iter().map(RulesItemStorage::to_domain).collect())
}

/// Serialize rules back into the embedded text without reordering.
pub fn serialize_rules(rules: &[RoutingRule]) -> Result<String> {
    let items: Vec<RulesItemStorage> = rules.iter().map(RulesItemStorage::from_domain).collect();
    Ok(serde_json::to_string(&items)?)
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
    fn profile_unknown_columns_are_kept_in_extra() {
        let r = row(&[
            ("IndexId", json!("p1")),
            ("ConfigType", json!(5)),
            ("ConfigVersion", json!(4)),
            ("Remarks", json!("node")),
            ("Address", json!("192.0.2.1")),
            ("Port", json!(443)),
            ("LegacyUnknown", json!("keep-me")),
        ]);
        let profile = map_profile(&r).unwrap();
        assert_eq!(profile.config_type, ConfigType::Vless);
        assert_eq!(profile.extra.get("LegacyUnknown"), Some(&json!("keep-me")));
    }

    #[test]
    fn rules_roundtrip_order_and_reference_fields() {
        let text = r#"[
            {"Id":"r1","Type":"field","OutboundTag":"proxy","Domain":["a.com"],"Enabled":true,"RuleType":1},
            {"Id":"r2","Type":"field","OutboundTag":"HK-1","Ip":["1.1.1.1"],"Enabled":true}
        ]"#;
        let rules = parse_rules(text).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].outbound_tag.as_deref(), Some("proxy"));
        assert_eq!(rules[0].rule_type, Some(RuleType::Routing));
        assert_eq!(rules[1].ip.as_ref().unwrap()[0], "1.1.1.1");
        let back = serialize_rules(&rules).unwrap();
        let reparsed: Vec<Value> = serde_json::from_str(&back).unwrap();
        assert_eq!(reparsed[0]["Id"], "r1");
        assert_eq!(reparsed[1]["OutboundTag"], "HK-1");
    }

    #[test]
    fn subscription_maps_all_seventeen_columns() {
        let r = row(&[
            ("Id", json!("sub1")),
            ("Remarks", json!("订阅")),
            ("Url", json!("https://example.invalid/sub")),
            ("Enabled", json!(1)),
            ("Sort", json!(3)),
            ("UpdateTime", json!(1700000000)),
            ("CustomCoreType", json!(24)),
        ]);
        let sub = map_subscription(&r);
        assert_eq!(sub.id, "sub1");
        assert!(sub.enabled);
        assert_eq!(sub.update_time, 1_700_000_000);
        assert_eq!(sub.custom_core_type, Some(CoreType::SingBox));
    }

    #[test]
    fn unknown_config_type_is_an_error_not_a_panic() {
        let r = row(&[("IndexId", json!("x")), ("ConfigType", json!(250))]);
        assert!(map_profile(&r).is_err());
    }
}
