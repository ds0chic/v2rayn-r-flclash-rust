//! Routing profiles and rules (`RoutingItem` 13 + `RulesItem` 13).
//!
//! Mirrors `compat/fields.entities.yaml` (`FLD-ENT-125..150`) and the upstream
//! `ConfigHandler` routing region (`SaveRoutingItem` / `MoveRoutingRule` /
//! `SetDefaultRouting` / `InitBuiltinRouting`). Rule order is the array order
//! of `RoutingProfile.rule_set`; enabling / locking / activation semantics
//! follow the ViewModels (`RoutingSettingViewModel`, `RoutingRuleSettingViewModel`).

use serde::{Deserialize, Serialize};

use crate::entities::{RoutingProfile, RoutingRule};
use crate::enums::RuleType;
use crate::error::{codes, DomainError};
use crate::reference::{ReferenceExpr, ReferenceSource};

/// Built-in outbound tags (`Global.OutboundTags`).
pub const OUTBOUND_TAGS: &[&str] = &["proxy", "direct", "block"];
/// `Global.ProxyTag` fallback for dangling references.
pub const PROXY_TAG: &str = "proxy";
/// `Global.DomainStrategy` candidates (Xray).
pub const DOMAIN_STRATEGIES: &[&str] = &[
    "AsIs",
    "UseIP",
    "UseIPv4v6",
    "UseIPv6v4",
    "UseIPv4",
    "UseIPv6",
    "",
];
/// `Global.DomainStrategies4Sbox` candidates (sing-box).
pub const DOMAIN_STRATEGIES_SBOX: &[&str] =
    &["", "prefer_ipv4", "prefer_ipv6", "ipv4_only", "ipv6_only"];
/// `Global.RuleProtocols` candidates for the rule details form.
pub const RULE_PROTOCOLS: &[&str] = &["http", "tls", "quic", "bittorrent"];
/// `Global.RuleNetworks` candidates.
pub const RULE_NETWORKS: &[&str] = &["", "tcp", "udp", "tcp,udp"];
/// `Global.InboundTags` candidates for the rule details form.
pub const RULE_INBOUND_TAGS: &[&str] = &["tun", "socks", "socks2", "socks3"];
/// `ERuleType` names accepted by the rule details form.
pub const RULE_TYPE_NAMES: &[&str] = &["ALL", "Routing", "DNS"];

/// Embedded upstream rule templates (`fixtures/source/upstream/sample/`).
pub const BUILTIN_WHITE_RULES: &str =
    include_str!("../../../fixtures/source/upstream/sample/custom_routing_white");
pub const BUILTIN_BLACK_RULES: &str =
    include_str!("../../../fixtures/source/upstream/sample/custom_routing_black");
pub const BUILTIN_GLOBAL_RULES: &str =
    include_str!("../../../fixtures/source/upstream/sample/custom_routing_global");

/// Built-in routing scheme names (`ConfigHandler.InitBuiltinRouting`, `V4-` prefix).
pub const BUILTIN_WHITE_REMARKS: &str = "V4-绕过大陆(Whitelist)";
pub const BUILTIN_BLACK_REMARKS: &str = "V4-黑名单(Blacklist)";
pub const BUILTIN_GLOBAL_REMARKS: &str = "V4-全局(Global)";

/// Structured warning for a dangling `OutboundTag` reference.
///
/// Upstream (`CoreConfigContextBuilder.cs:63-98`) falls back to
/// `Global.ProxyTag` with a UI warning; T06b made the fallback explicit via
/// the `routing_dangling_reference` codegen diagnostic. This type carries the
/// same semantics at the engine layer so the UI can surface it before
/// generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingWarning {
    pub code: String,
    pub message: String,
    pub field_path: Option<String>,
}

impl RoutingWarning {
    pub fn dangling(rule_remarks: &str, outbound_tag: &str) -> Self {
        Self {
            code: "routing_dangling_reference".to_string(),
            message: format!(
                "routing rule '{rule_remarks}' references outbound '{outbound_tag}' \
                 that cannot be resolved; falling back to '{PROXY_TAG}'"
            ),
            field_path: Some("routing.ruleSet[].outboundTag".to_string()),
        }
    }

    pub fn empty_outbound(rule_remarks: &str) -> Self {
        Self {
            code: "routing_empty_outbound".to_string(),
            message: format!(
                "routing rule '{rule_remarks}' has an empty outbound tag; \
                 falling back to '{PROXY_TAG}'"
            ),
            field_path: Some("routing.ruleSet[].outboundTag".to_string()),
        }
    }
}

/// Validate one routing rule. At least one match criterion is required
/// (upstream `RoutingRuleDetailsViewModel.SaveRulesAsync`: network / port /
/// protocol / domain / ip / process).
pub fn validate_rule(rule: &RoutingRule) -> Result<(), DomainError> {
    let has = rule.port.as_deref().is_some_and(|v| !v.trim().is_empty())
        || rule
            .network
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
        || rule.protocol.as_ref().is_some_and(|v| !v.is_empty())
        || rule.domain.as_ref().is_some_and(|v| !v.is_empty())
        || rule.ip.as_ref().is_some_and(|v| !v.is_empty())
        || rule.process.as_ref().is_some_and(|v| !v.is_empty())
        || rule.inbound_tag.as_ref().is_some_and(|v| !v.is_empty());
    if !has {
        return Err(
            DomainError::new(codes::FIELD_REQUIRED, "error.routing_rule_empty").with_field("rule"),
        );
    }
    if let Some(rule_type) = rule.rule_type {
        if !matches!(rule_type, RuleType::All | RuleType::Routing | RuleType::Dns) {
            return Err(
                DomainError::new(codes::FIELD_RANGE, "error.routing_rule_type_invalid")
                    .with_field("ruleType"),
            );
        }
    }
    Ok(())
}

/// Validate a routing profile draft (remarks required; rule JSON parseable).
pub fn validate_profile(profile: &RoutingProfile) -> Result<(), DomainError> {
    if profile.remarks.trim().is_empty() {
        return Err(
            DomainError::new(codes::FIELD_REQUIRED, "error.remarks_required").with_field("remarks"),
        );
    }
    let rules = profile.rules().map_err(|e| {
        DomainError::new(codes::FIELD_FORMAT, "error.routing_rules_invalid")
            .with_detail(e.to_string())
    })?;
    for rule in &rules {
        validate_rule(rule)?;
    }
    Ok(())
}

/// Parse the embedded rule array of a profile.
pub fn parse_rules(profile: &RoutingProfile) -> Result<Vec<RoutingRule>, DomainError> {
    profile.rules().map_err(|e| {
        DomainError::new(codes::FIELD_FORMAT, "error.routing_rules_invalid")
            .with_detail(e.to_string())
    })
}

/// Serialize rules back into the profile without reordering; syncs `rule_num`.
pub fn set_rules(profile: &mut RoutingProfile, rules: &[RoutingRule]) -> Result<(), DomainError> {
    profile.set_rules(rules).map_err(|e| {
        DomainError::new(codes::INTERNAL, "error.routing_rules_serialize")
            .with_detail(e.to_string())
    })
}

/// Move one rule inside the list (upstream `ConfigHandler.MoveRoutingRule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Top,
    Up,
    Down,
    Bottom,
}

impl MoveDirection {
    pub fn from_int(value: i32) -> Option<Self> {
        Some(match value {
            0 => MoveDirection::Top,
            1 => MoveDirection::Up,
            2 => MoveDirection::Down,
            3 => MoveDirection::Bottom,
            _ => return None,
        })
    }
}

/// Reorder `rules` in place. Out-of-range index is an error; a no-op move at
/// the boundary succeeds without touching the order.
pub fn move_rule(
    rules: &mut Vec<RoutingRule>,
    index: usize,
    direction: MoveDirection,
) -> Result<(), DomainError> {
    if index >= rules.len() {
        return Err(
            DomainError::new(codes::FIELD_RANGE, "error.routing_rule_index").with_field("index"),
        );
    }
    match direction {
        MoveDirection::Top => {
            if index == 0 {
                return Ok(());
            }
            let item = rules.remove(index);
            rules.insert(0, item);
        }
        MoveDirection::Up => {
            if index == 0 {
                return Ok(());
            }
            rules.swap(index, index - 1);
        }
        MoveDirection::Down => {
            if index + 1 >= rules.len() {
                return Ok(());
            }
            rules.swap(index, index + 1);
        }
        MoveDirection::Bottom => {
            if index + 1 >= rules.len() {
                return Ok(());
            }
            let item = rules.remove(index);
            rules.push(item);
        }
    }
    Ok(())
}

/// Parse imported rule JSON (file / clipboard / URL body). Each imported rule
/// gets a fresh id (upstream `AddBatchRoutingRules` assigns new GUIDs).
pub fn parse_imported_rules(text: &str) -> Result<Vec<RoutingRule>, DomainError> {
    if text.trim().is_empty() {
        return Err(DomainError::new(
            codes::FIELD_FORMAT,
            "error.routing_rules_empty",
        ));
    }
    let mut rules: Vec<RoutingRule> = serde_json::from_str(text).map_err(|e| {
        DomainError::new(codes::FIELD_FORMAT, "error.routing_rules_invalid")
            .with_detail(e.to_string())
    })?;
    for rule in &mut rules {
        rule.id = new_rule_id();
    }
    for rule in &rules {
        validate_rule(rule)?;
    }
    Ok(rules)
}

/// Export rules to indented JSON (clipboard / file). Ids are cleared so a
/// re-import assigns fresh ones (upstream `RuleExportSelectedAsync`).
pub fn export_rules(rules: &[RoutingRule]) -> Result<String, DomainError> {
    let mut copies = rules.to_vec();
    for rule in &mut copies {
        rule.id.clear();
    }
    serde_json::to_string_pretty(&copies).map_err(|e| {
        DomainError::new(codes::INTERNAL, "error.routing_rules_serialize")
            .with_detail(e.to_string())
    })
}

/// Resolve an `OutboundTag`: built-in tags pass through, otherwise the remarks
/// must match a live profile. Returns the effective tag plus an optional
/// structured warning (upstream: fall back to `proxy` with a warning).
pub fn resolve_outbound_tag(
    rule: &RoutingRule,
    profile_remarks: &[String],
) -> (String, Option<RoutingWarning>) {
    let remarks = rule.remarks.as_deref().unwrap_or("");
    let raw = rule.outbound_tag.as_deref().unwrap_or("").trim();
    if raw.is_empty() {
        return (
            PROXY_TAG.to_string(),
            Some(RoutingWarning::empty_outbound(remarks)),
        );
    }
    if OUTBOUND_TAGS.contains(&raw) {
        return (raw.to_string(), None);
    }
    if profile_remarks.iter().any(|r| r == raw) {
        return (raw.to_string(), None);
    }
    (
        PROXY_TAG.to_string(),
        Some(RoutingWarning::dangling(remarks, raw)),
    )
}

/// Collect dangling-reference warnings for a whole rule set.
pub fn collect_warnings(rules: &[RoutingRule], profile_remarks: &[String]) -> Vec<RoutingWarning> {
    rules
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|r| resolve_outbound_tag(r, profile_remarks).1)
        .collect()
}

/// Build the three built-in routing profiles (white / black / global).
pub fn builtin_profiles() -> Vec<(RoutingProfile, Vec<RoutingRule>)> {
    let mut out = Vec::new();
    for (remarks, template, sort) in [
        (BUILTIN_WHITE_REMARKS, BUILTIN_WHITE_RULES, 1),
        (BUILTIN_BLACK_REMARKS, BUILTIN_BLACK_RULES, 2),
        (BUILTIN_GLOBAL_REMARKS, BUILTIN_GLOBAL_RULES, 3),
    ] {
        let rules: Vec<RoutingRule> = serde_json::from_str(template).unwrap_or_default();
        let mut profile = RoutingProfile {
            id: String::new(),
            remarks: remarks.to_string(),
            url: String::new(),
            rule_set: String::new(),
            rule_num: rules.len() as i32,
            enabled: true,
            locked: false,
            custom_icon: String::new(),
            custom_ruleset_path4_singbox: String::new(),
            domain_strategy: String::new(),
            domain_strategy4_singbox: String::new(),
            sort,
            is_active: false,
            extra: Default::default(),
        };
        let _ = profile.set_rules(&rules);
        out.push((profile, rules));
    }
    out
}

/// Fresh stable id for imported rules (matches `application::new_index_id`
/// shape: millisecond timestamp + counter + random suffix).
pub fn new_rule_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("r{millis:x}-{seq:04x}")
}

/// Remarks reference for an outbound tag (REF-ENT-001).
pub fn outbound_reference(raw: &str) -> Option<ReferenceExpr> {
    if raw.trim().is_empty() {
        return None;
    }
    Some(ReferenceExpr::remarks(raw, ReferenceSource::Imported))
}
