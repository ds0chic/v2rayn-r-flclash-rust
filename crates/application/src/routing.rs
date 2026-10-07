//! Routing use cases (T11): `RoutingItem` CRUD, rule ordering, import/export,
//! outbound-tag resolution warnings and built-in seeding.
//!
//! Storage mirrors `compat/domain-map.yaml`: `RoutingItem` rows in `guiNDB.db`
//! with `RuleSet` kept as a JSON-array text column (order = array order).
//! The engine layer owns revision bumps and persistence; this module owns
//! validation, row mapping and the pure rule-list operations.

use std::sync::atomic::{AtomicU64, Ordering};

use domain::{DomainError, RoutingProfile, RoutingRule};
use persistence::RawRow;
use serde_json::{json, Value};

static ROUTING_ID_SEQ: AtomicU64 = AtomicU64::new(0);

/// Fresh routing-profile id.
pub fn new_routing_id() -> String {
    let seq = ROUTING_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("rt-{nanos:x}-{seq:x}")
}

#[allow(dead_code)]
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// Map a `RoutingProfile` onto a `RoutingItem` row.
pub fn routing_to_row(profile: &RoutingProfile) -> RawRow {
    let mut row = RawRow::new("RoutingItem");
    row.set("Id", json!(profile.id));
    row.set("Remarks", json!(profile.remarks));
    row.set("Url", json!(profile.url));
    row.set("RuleSet", json!(profile.rule_set));
    row.set("RuleNum", json!(profile.rule_num));
    row.set("Enabled", json!(i64::from(profile.enabled)));
    row.set("Locked", json!(i64::from(profile.locked)));
    row.set("CustomIcon", json!(profile.custom_icon));
    row.set(
        "CustomRulesetPath4Singbox",
        json!(profile.custom_ruleset_path4_singbox),
    );
    row.set("DomainStrategy", json!(profile.domain_strategy));
    row.set(
        "DomainStrategy4Singbox",
        json!(profile.domain_strategy4_singbox),
    );
    row.set("Sort", json!(profile.sort));
    row.set("IsActive", json!(i64::from(profile.is_active)));
    row
}

/// Map a `RoutingItem` row back onto the domain profile.
pub fn routing_from_row(row: &RawRow) -> RoutingProfile {
    RoutingProfile {
        id: row.string("Id"),
        remarks: row.string("Remarks"),
        url: row.string("Url"),
        rule_set: row.opt_string("RuleSet").unwrap_or_default(),
        rule_num: row.opt_i64("RuleNum").unwrap_or(0) as i32,
        enabled: row.bool("Enabled"),
        locked: row.bool("Locked"),
        custom_icon: row.string("CustomIcon"),
        custom_ruleset_path4_singbox: row.string("CustomRulesetPath4Singbox"),
        domain_strategy: row.string("DomainStrategy"),
        domain_strategy4_singbox: row.string("DomainStrategy4Singbox"),
        sort: row.opt_i64("Sort").unwrap_or(0) as i32,
        is_active: row.bool("IsActive"),
        extra: Default::default(),
    }
}

/// Normalize a draft before persisting: fresh id when empty, rule ids filled,
/// `rule_num` synced to the array length.
pub fn normalize_routing(mut profile: RoutingProfile) -> Result<RoutingProfile, DomainError> {
    if profile.id.trim().is_empty() {
        profile.id = new_routing_id();
    }
    let mut rules = domain::routing::parse_rules(&profile)?;
    for rule in &mut rules {
        if rule.id.trim().is_empty() {
            rule.id = domain::routing::new_rule_id();
        }
    }
    domain::routing::set_rules(&mut profile, &rules)?;
    domain::routing::validate_profile(&profile)?;
    Ok(profile)
}

/// Copy stored unknown-field extras into an incoming save.
///
/// The bridge DTOs have no room for `extra` keys, so a DTO round trip would
/// otherwise wipe them. Stored keys survive unless the draft overrides them;
/// per-rule extras are matched by rule id (upstream re-keys every rule id on
/// save, but this engine keeps draft ids stable, so the match is exact).
pub fn preserve_extras(existing: Option<&RoutingProfile>, incoming: &mut RoutingProfile) {
    let Some(stored) = existing else {
        return;
    };
    for (key, value) in &stored.extra {
        incoming
            .extra
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
    let Ok(mut incoming_rules) = incoming.rules() else {
        return;
    };
    if incoming_rules.is_empty() {
        return;
    }
    let stored_rules = stored.rules().unwrap_or_default();
    let by_id: std::collections::HashMap<&str, &RoutingRule> = stored_rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule))
        .collect();
    let mut touched = false;
    for rule in &mut incoming_rules {
        let Some(stored_rule) = by_id.get(rule.id.as_str()) else {
            continue;
        };
        for (key, value) in &stored_rule.extra {
            if !rule.extra.contains_key(key) {
                rule.extra.insert(key.clone(), value.clone());
                touched = true;
            }
        }
    }
    if touched {
        let _ = incoming.set_rules(&incoming_rules);
    }
}

/// Repository boundary for routing profiles.
pub trait RoutingRepository {
    fn list(&self) -> Result<Vec<RoutingProfile>, DomainError>;
    fn get(&self, id: &str) -> Result<Option<RoutingProfile>, DomainError>;
    fn upsert(&mut self, item: RoutingProfile) -> Result<(), DomainError>;
    fn remove(&mut self, id: &str) -> Result<bool, DomainError>;
    fn count(&self) -> usize;
}

/// In-memory routing repository (tests / non-persistent engines).
#[derive(Default)]
pub struct InMemoryRoutingRepository {
    items: Vec<RoutingProfile>,
}

impl InMemoryRoutingRepository {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_items(items: Vec<RoutingProfile>) -> Self {
        Self { items }
    }
}

impl RoutingRepository for InMemoryRoutingRepository {
    fn list(&self) -> Result<Vec<RoutingProfile>, DomainError> {
        let mut items = self.items.clone();
        items.sort_by_key(|r| r.sort);
        Ok(items)
    }

    fn get(&self, id: &str) -> Result<Option<RoutingProfile>, DomainError> {
        Ok(self.items.iter().find(|r| r.id == id).cloned())
    }

    fn upsert(&mut self, item: RoutingProfile) -> Result<(), DomainError> {
        if let Some(slot) = self.items.iter_mut().find(|r| r.id == item.id) {
            *slot = item;
        } else {
            self.items.push(item);
        }
        Ok(())
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        let before = self.items.len();
        self.items.retain(|r| r.id != id);
        Ok(self.items.len() != before)
    }

    fn count(&self) -> usize {
        self.items.len()
    }
}

/// Fetch URL body for rule import (loopback-friendly; short timeout).
///
/// Built on the shared `platform::http` async fetch API (FLD-CFG-087): the
/// client is cached per policy, the body is bounded and the whole operation
/// observes cooperative cancellation.
pub async fn fetch_rules_text(
    url: &str,
    via_proxy: bool,
    proxy_url: Option<&str>,
) -> Result<String, DomainError> {
    fetch_rules_text_cancel(url, via_proxy, proxy_url, &domain::CancellationToken::new()).await
}

/// [`fetch_rules_text`] with caller cancellation (download cancels cleanly and
/// never writes a rule).
pub async fn fetch_rules_text_cancel(
    url: &str,
    via_proxy: bool,
    proxy_url: Option<&str>,
    cancellation: &domain::CancellationToken,
) -> Result<String, DomainError> {
    let proxy = if via_proxy {
        proxy_url
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    } else {
        None
    };
    let policy = platform::http::HttpPolicy {
        timeout: std::time::Duration::from_secs(15),
        connect_timeout: std::time::Duration::from_secs(10),
        user_agent: None,
        proxy,
        trust: platform::http::HttpsTrust::System,
        redirect: platform::http::RedirectPolicy::None,
    };
    let client = platform::http::SharedHttpClient::shared(policy).map_err(|e| {
        DomainError::new(domain::codes::UNAVAILABLE, "error.fetch_failed").with_detail(e)
    })?;
    let options = platform::http::FetchOptions::default();
    platform::http::fetch_text(&client, url, &options, cancellation)
        .await
        .map_err(|e| {
            let code = match &e {
                platform::http::HttpError::Cancelled => domain::codes::CANCELLED,
                platform::http::HttpError::Timeout => domain::codes::TIMEOUT,
                _ => domain::codes::UNAVAILABLE,
            };
            DomainError::new(code, "error.fetch_failed").with_detail(e.to_string())
        })
}

/// Plan the `一键导入规则集` advanced import (upstream
/// `ConfigHandler.InitBuiltinRouting(config, true)`).
///
/// The advanced import appends the three built-in schemes instead of
/// short-circuiting on an existing store, and never changes the active/default
/// scheme. Sorts continue after the current maximum; every profile gets a fresh
/// id and fresh rule ids through [`normalize_routing`].
pub fn builtin_import_profiles(existing: &[RoutingProfile]) -> Vec<RoutingProfile> {
    let base = existing.len() as i32;
    domain::routing::builtin_profiles()
        .into_iter()
        .enumerate()
        .filter_map(|(offset, (mut profile, _))| {
            profile.id = new_routing_id();
            profile.sort = base + offset as i32 + 1;
            profile.is_active = false;
            normalize_routing(profile).ok()
        })
        .collect()
}

/// Import rules from text with replace/append semantics.
/// Returns the merged rule list (unsaved; the caller persists the profile).
pub fn merge_imported_rules(
    current: &[RoutingRule],
    text: &str,
    replace: bool,
) -> Result<Vec<RoutingRule>, DomainError> {
    let imported = parse_imported_rules_compat(text)?;
    if replace {
        Ok(imported)
    } else {
        let mut merged = current.to_vec();
        merged.extend(imported);
        Ok(merged)
    }
}

/// Parse imported rule JSON in the upstream camelCase `RulesItem` shape or the
/// stored snake_case shape. List members may be arrays or comma/newline
/// strings; `ruleType` may be the `ERuleType` ordinal or name. Unknown keys are
/// kept in `extra`, and every rule gets a fresh id (upstream
/// `AddBatchRoutingRules` assigns new GUIDs). Invalid input is an error so the
/// caller can leave its draft untouched.
pub fn parse_imported_rules_compat(text: &str) -> Result<Vec<RoutingRule>, DomainError> {
    if text.trim().is_empty() {
        return Err(invalid_rules_error("error.routing_rules_empty"));
    }
    let value: Value = serde_json::from_str(text).map_err(|e| {
        DomainError::new(domain::codes::FIELD_FORMAT, "error.routing_rules_invalid")
            .with_detail(e.to_string())
    })?;
    let Value::Array(entries) = value else {
        return Err(invalid_rules_error("error.routing_rules_invalid"));
    };
    if entries.is_empty() {
        return Err(invalid_rules_error("error.routing_rules_empty"));
    }
    let mut rules = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Object(map) = entry else {
            return Err(invalid_rules_error("error.routing_rules_invalid"));
        };
        rules.push(imported_rule_from_map(map)?);
    }
    for rule in &rules {
        domain::routing::validate_rule(rule)?;
    }
    Ok(rules)
}

fn invalid_rules_error(message_key: &str) -> DomainError {
    DomainError::new(domain::codes::FIELD_FORMAT, message_key)
}

/// Map one imported `RulesItem` object onto the stored snake_case fields, then
/// deserialize through the entity's serde form (so `extra` keeps unknown keys).
fn imported_rule_from_map(map: serde_json::Map<String, Value>) -> Result<RoutingRule, DomainError> {
    let mut fields = serde_json::Map::new();
    for (key, value) in map {
        let (target, value): (&str, Value) = match key.as_str() {
            "id" => continue,
            "type" => ("rule_kind", value),
            "inboundTag" | "inbound_tag" => {
                let Some(list) = normalize_string_list(value) else {
                    continue;
                };
                ("inbound_tag", json!(list))
            }
            "outboundTag" | "outbound_tag" => ("outbound_tag", value),
            "ruleType" | "rule_type" => {
                let Some(rule_type) = normalize_rule_type(&value) else {
                    continue;
                };
                ("rule_type", json!(rule_type))
            }
            "ip" | "domain" | "protocol" | "process" => {
                let Some(list) = normalize_string_list(value) else {
                    continue;
                };
                (key.as_str(), json!(list))
            }
            "enabled" => {
                if !value.is_boolean() {
                    continue;
                }
                ("enabled", value)
            }
            _ => (key.as_str(), value),
        };
        fields.insert(target.to_string(), value);
    }
    fields
        .entry("enabled".to_string())
        .or_insert(Value::Bool(true));
    let mut rule: RoutingRule = serde_json::from_value(Value::Object(fields)).map_err(|e| {
        DomainError::new(domain::codes::FIELD_FORMAT, "error.routing_rules_invalid")
            .with_detail(e.to_string())
    })?;
    rule.id = domain::routing::new_rule_id();
    Ok(rule)
}

/// Accept a JSON array of strings or a comma/newline separated string.
fn normalize_string_list(value: Value) -> Option<Vec<String>> {
    let list: Vec<String> = match value {
        Value::Null => return None,
        Value::Array(items) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(text) => {
                    let trimmed = text.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                }
                other => {
                    let text = other.to_string();
                    (!text.is_empty()).then_some(text)
                }
            })
            .collect(),
        Value::String(text) => text
            .split(['\r', '\n', ',', '，'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect(),
        other => {
            let text = other.to_string();
            if text.is_empty() {
                return None;
            }
            vec![text]
        }
    };
    (!list.is_empty()).then_some(list)
}

/// `ruleType` as an `ERuleType` ordinal or name (`ALL` / `Routing` / `DNS`).
fn normalize_rule_type(value: &Value) -> Option<i32> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .and_then(|v| i32::try_from(v).ok())
            .filter(|v| domain::RuleType::from_value(*v).is_some()),
        Value::String(text) => {
            let trimmed = text.trim();
            if let Ok(number) = trimmed.parse::<i32>() {
                return domain::RuleType::from_value(number).map(|_| number);
            }
            match trimmed.to_ascii_lowercase().as_str() {
                "all" => Some(0),
                "routing" | "route" => Some(1),
                "dns" => Some(2),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Export the selected rules (by id) to clipboard/file JSON.
pub fn export_selected_rules(rules: &[RoutingRule], ids: &[String]) -> Result<String, DomainError> {
    let selected: Vec<RoutingRule> = rules
        .iter()
        .filter(|r| ids.contains(&r.id))
        .cloned()
        .collect();
    if selected.is_empty() {
        return Err(
            DomainError::new(domain::codes::NOT_FOUND, "error.routing_no_selection")
                .with_field("ids"),
        );
    }
    export_rules_camel(&selected)
}

/// Export rules as the upstream camelCase `RulesItem` JSON (id omitted,
/// null/empty members omitted) so the output matches `RuleExportSelectedAsync`
/// and re-imports through [`parse_imported_rules_compat`].
pub fn export_rules_camel(rules: &[RoutingRule]) -> Result<String, DomainError> {
    let maps: Vec<Value> = rules.iter().map(export_rule_value).collect();
    serde_json::to_string_pretty(&maps).map_err(|e| {
        DomainError::new(domain::codes::INTERNAL, "error.routing_rules_serialize")
            .with_detail(e.to_string())
    })
}

fn export_rule_value(rule: &RoutingRule) -> Value {
    let mut map = serde_json::Map::new();
    fn put_str(map: &mut serde_json::Map<String, Value>, key: &str, value: Option<&str>) {
        if let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) {
            map.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    fn put_list(map: &mut serde_json::Map<String, Value>, key: &str, value: Option<&Vec<String>>) {
        if let Some(value) = value {
            let list: Vec<Value> = value
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| Value::String(s.to_string()))
                .collect();
            if !list.is_empty() {
                map.insert(key.to_string(), Value::Array(list));
            }
        }
    }
    put_str(&mut map, "type", rule.rule_kind.as_deref());
    put_str(&mut map, "port", rule.port.as_deref());
    put_str(&mut map, "network", rule.network.as_deref());
    put_list(&mut map, "inboundTag", rule.inbound_tag.as_ref());
    put_str(&mut map, "outboundTag", rule.outbound_tag.as_deref());
    put_list(&mut map, "ip", rule.ip.as_ref());
    put_list(&mut map, "domain", rule.domain.as_ref());
    put_list(&mut map, "protocol", rule.protocol.as_ref());
    put_list(&mut map, "process", rule.process.as_ref());
    map.insert("enabled".to_string(), Value::Bool(rule.enabled));
    put_str(&mut map, "remarks", rule.remarks.as_deref());
    if let Some(rule_type) = rule.rule_type {
        map.insert("ruleType".to_string(), json!(rule_type.value()));
    }
    Value::Object(map)
}

/// Read and parse `CustomRulesetPath4Singbox` content when the file exists.
/// Missing / unreadable / unparsable content yields `None` (the generator
/// then behaves as if no custom ruleset was set).
pub fn read_custom_ruleset(path: &str) -> Option<Value> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&text).ok()
}

fn opt_json(value: Option<&str>) -> Value {
    match value {
        Some(v) if !v.is_empty() => json!(v),
        _ => Value::Null,
    }
}

#[allow(dead_code)]
fn _opt_json_keepalive(value: Option<&str>) -> Value {
    opt_json(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with_rules() -> RoutingProfile {
        let mut profile = RoutingProfile {
            id: "rt-1".into(),
            remarks: "test".into(),
            ..Default::default()
        };
        let rules = vec![
            RoutingRule {
                id: "a".into(),
                port: Some("80".into()),
                enabled: true,
                ..Default::default()
            },
            RoutingRule {
                id: "b".into(),
                network: Some("tcp".into()),
                enabled: true,
                ..Default::default()
            },
        ];
        domain::routing::set_rules(&mut profile, &rules).unwrap();
        profile
    }

    #[test]
    fn row_round_trip_preserves_all_fields() {
        let profile = profile_with_rules();
        let row = routing_to_row(&profile);
        let loaded = routing_from_row(&row);
        assert_eq!(loaded, profile);
    }

    #[test]
    fn merge_import_append_and_replace() {
        let current = vec![RoutingRule {
            id: "a".into(),
            port: Some("80".into()),
            enabled: true,
            ..Default::default()
        }];
        let text = r#"[{"port": "443", "enabled": true}]"#;
        let appended = merge_imported_rules(&current, text, false).unwrap();
        assert_eq!(appended.len(), 2);
        let replaced = merge_imported_rules(&current, text, true).unwrap();
        assert_eq!(replaced.len(), 1);
    }

    #[test]
    fn import_accepts_upstream_camel_case_and_keeps_extras() {
        let text = r#"[
            {"type": "Routing", "outboundTag": "proxy", "domain": ["geosite:google"],
             "ruleType": "DNS", "enabled": true, "future_rule_flag": true},
            {"port": "443", "network": "tcp,udp"}
        ]"#;
        let rules = parse_imported_rules_compat(text).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].rule_kind.as_deref(), Some("Routing"));
        assert_eq!(rules[0].outbound_tag.as_deref(), Some("proxy"));
        assert_eq!(rules[0].domain, Some(vec!["geosite:google".to_string()]));
        assert_eq!(rules[0].rule_type, Some(domain::RuleType::Dns));
        assert!(rules[0].extra.contains_key("future_rule_flag"));
        assert!(!rules[0].id.is_empty());
        assert_eq!(rules[1].port.as_deref(), Some("443"));
        assert_eq!(rules[1].network.as_deref(), Some("tcp,udp"));
        assert!(rules[1].enabled);
    }

    #[test]
    fn export_is_camel_case_without_id_and_round_trips() {
        let rules = vec![RoutingRule {
            id: "r1".into(),
            rule_kind: Some("Routing".into()),
            port: Some("80".into()),
            outbound_tag: Some("proxy".into()),
            domain: Some(vec!["geosite:cn".into()]),
            rule_type: Some(domain::RuleType::Routing),
            enabled: true,
            remarks: Some("keep".into()),
            ..Default::default()
        }];
        let text = export_rules_camel(&rules).unwrap();
        let json: Vec<Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(json.len(), 1);
        let first = json[0].as_object().unwrap();
        assert!(!first.contains_key("id"));
        assert_eq!(first["type"], "Routing");
        assert_eq!(first["outboundTag"], "proxy");
        assert_eq!(first["domain"][0], "geosite:cn");
        assert_eq!(first["ruleType"], 1);
        assert_eq!(first["enabled"], true);

        let back = parse_imported_rules_compat(&text).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].rule_kind.as_deref(), Some("Routing"));
        assert_eq!(back[0].outbound_tag.as_deref(), Some("proxy"));
        assert_eq!(back[0].domain, Some(vec!["geosite:cn".to_string()]));
        assert_eq!(back[0].rule_type, Some(domain::RuleType::Routing));
        assert_ne!(back[0].id, rules[0].id);
    }

    #[test]
    fn builtin_import_appends_without_touching_default() {
        let existing = vec![
            RoutingProfile {
                id: "old-1".into(),
                remarks: "custom".into(),
                sort: 1,
                is_active: true,
                ..Default::default()
            },
            RoutingProfile {
                id: "old-2".into(),
                remarks: "custom-2".into(),
                sort: 2,
                ..Default::default()
            },
        ];
        let imported = builtin_import_profiles(&existing);
        assert_eq!(imported.len(), 3);
        assert_eq!(imported[0].remarks, domain::routing::BUILTIN_WHITE_REMARKS);
        assert_eq!(imported[1].remarks, domain::routing::BUILTIN_BLACK_REMARKS);
        assert_eq!(imported[2].remarks, domain::routing::BUILTIN_GLOBAL_REMARKS);
        // Sorts continue after the current maximum (upstream `maxSort = items.Count`).
        assert_eq!(imported[0].sort, 3);
        assert_eq!(imported[1].sort, 4);
        assert_eq!(imported[2].sort, 5);
        // Advanced import never promotes a scheme to default.
        assert!(imported.iter().all(|p| !p.is_active));
        // Fresh ids and rules are filled by `normalize_routing`.
        assert!(imported.iter().all(|p| !p.id.trim().is_empty()));
        assert!(imported.iter().all(|p| p.rule_num > 0));
        for profile in &imported {
            let rules = domain::routing::parse_rules(profile).unwrap();
            assert!(rules.iter().all(|r| !r.id.trim().is_empty()));
        }
    }

    #[test]
    fn import_rejects_invalid_and_empty() {
        assert!(parse_imported_rules_compat("").is_err());
        assert!(parse_imported_rules_compat("not json").is_err());
        assert!(parse_imported_rules_compat("[]").is_err());
        assert!(parse_imported_rules_compat("[{}]").is_err());
    }

    /// SP-13/CP-08: malformed rule text is rejected at the parse boundary and
    /// never reaches the repository (read failure must not become a write).
    #[test]
    fn sp13_malformed_import_never_writes() {
        let repo = InMemoryRoutingRepository::with_items(vec![RoutingProfile {
            id: "rt-keep".into(),
            remarks: "synthetic keep".into(),
            ..Default::default()
        }]);
        for bad in [
            "",
            "not json{{",
            "[]",
            "[{}]",
            r#"[{"port": "443", "enabled": true}, "oops"]"#,
        ] {
            assert!(parse_imported_rules_compat(bad).is_err(), "input: {bad}");
        }
        let kept = repo.list().unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, "rt-keep");
        assert_eq!(repo.count(), 1);
    }

    /// SP-13/CP-08: a partial multi-delete reconciles against the
    /// authoritative store. The committed delete stays gone, the failed id
    /// stays listed for retry, and a later full read never resurrects it.
    #[test]
    fn sp13_partial_delete_reconciles_against_authoritative() {
        let mut repo = InMemoryRoutingRepository::with_items(vec![
            RoutingProfile {
                id: "rt-a".into(),
                remarks: "synthetic a".into(),
                ..Default::default()
            },
            RoutingProfile {
                id: "rt-b".into(),
                remarks: "synthetic b".into(),
                ..Default::default()
            },
        ]);
        assert!(repo.remove("rt-a").unwrap());
        assert!(!repo.remove("rt-missing").unwrap());
        let ids: Vec<String> = repo.list().unwrap().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, vec!["rt-b".to_string()]);
        assert_eq!(repo.count(), 1);
        assert!(repo.get("rt-a").unwrap().is_none());
    }
}
