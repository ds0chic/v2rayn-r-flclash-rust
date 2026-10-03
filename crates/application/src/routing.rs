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
pub async fn fetch_rules_text(
    url: &str,
    via_proxy: bool,
    proxy_url: Option<&str>,
) -> Result<String, DomainError> {
    use subscriptions::{build_client, DownloadOptions, ProxyConfig};
    let proxy = if via_proxy {
        proxy_url
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| ProxyConfig { url: s.to_string() })
    } else {
        None
    };
    let options = DownloadOptions {
        proxy,
        timeout: std::time::Duration::from_secs(15),
        ..Default::default()
    };
    let downloader = build_client(&options).map_err(|e| {
        DomainError::new(domain::codes::UNAVAILABLE, "error.fetch_failed")
            .with_detail(e.to_string())
    })?;
    let cancellation = domain::CancellationToken::new();
    downloader
        .download(url, &cancellation)
        .await
        .map_err(|e| {
            DomainError::new(domain::codes::UNAVAILABLE, "error.fetch_failed")
                .with_detail(e.to_string())
        })
        .map(|d| d.body)
}

/// Import rules from text with replace/append semantics.
/// Returns the merged rule list (unsaved; the caller persists the profile).
pub fn merge_imported_rules(
    current: &[RoutingRule],
    text: &str,
    replace: bool,
) -> Result<Vec<RoutingRule>, DomainError> {
    let imported = domain::routing::parse_imported_rules(text)?;
    if replace {
        Ok(imported)
    } else {
        let mut merged = current.to_vec();
        merged.extend(imported);
        Ok(merged)
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
    domain::routing::export_rules(&selected)
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
}
