//! sing-box geosite/geoip -> rule_set conversion (`SingboxRulesetService`).

use serde_json::{json, Value};

use crate::singbox::SboxState;
use crate::util::*;

pub(crate) fn convert_geo2_ruleset(state: &mut SboxState<'_>) {
    let input = state.input;
    let mut rule_sets: Vec<String> = Vec::new();

    // route rules
    if let Some(rules) = state
        .config
        .get_mut("route")
        .and_then(|r| r.get_mut("rules"))
        .and_then(Value::as_array_mut)
    {
        for rule in rules.iter_mut() {
            convert_rule(rule, &mut rule_sets);
        }
    }
    // dns rules (including nested logical rules)
    if let Some(rules) = state
        .config
        .get_mut("dns")
        .and_then(|d| d.get_mut("rules"))
        .and_then(Value::as_array_mut)
    {
        for rule in rules.iter_mut() {
            convert_rule(rule, &mut rule_sets);
            if let Some(nested) = rule.get("rules").and_then(Value::as_array) {
                for item in nested {
                    if let Some(list) = item.get("rule_set").and_then(Value::as_array) {
                        for value in list {
                            if let Some(tag) = value.as_str() {
                                push_unique(&mut rule_sets, tag);
                            }
                        }
                    }
                }
            }
        }
    }

    let custom_rulesets: Vec<Value> = input
        .routing
        .as_ref()
        .and_then(|r| r.custom_ruleset.as_ref())
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let custom_rulesets: Vec<Value> = custom_rulesets
        .into_iter()
        .filter(|r| {
            r.get("tag").and_then(Value::as_str).is_some()
                && r.get("type").and_then(Value::as_str).is_some()
                && r.get("format").and_then(Value::as_str).is_some()
        })
        .collect();

    let local_srss = join_path(&input.settings.bin_directory, "srss");
    let srs_url = input
        .settings
        .ruleset_url
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| SINGBOX_RULESET_URL.to_string());

    let mut generated: Vec<Value> = Vec::new();
    let mut contains_remote = false;
    for item in &rule_sets {
        if item.is_empty() {
            continue;
        }
        let custom = custom_rulesets
            .iter()
            .find(|r| r.get("tag").and_then(Value::as_str) == Some(item.as_str()))
            .cloned();
        let ruleset = if let Some(custom) = custom {
            custom
        } else if input.settings.local_srs_files.contains(item) {
            let path = join_path(&local_srss, &format!("{item}.srs"));
            json!({"type": "local", "format": "binary", "tag": item, "path": path})
        } else {
            contains_remote = true;
            let kind = if item.starts_with("geosite") {
                "geosite"
            } else {
                "geoip"
            };
            json!({
                "type": "remote",
                "format": "binary",
                "tag": item,
                "url": srs_url.replace("{0}", kind).replace("{1}", item),
                "http_client": SINGBOX_SRS_HTTP_CLIENT_TAG
            })
        };
        generated.push(ruleset);
    }

    if let Some(route) = state.config.get_mut("route").and_then(Value::as_object_mut) {
        route.insert("rule_set".into(), Value::Array(generated));
    }
    if contains_remote {
        let clients = state
            .config
            .entry("http_clients")
            .or_insert_with(|| json!([]));
        if let Some(list) = clients.as_array_mut() {
            list.retain(|c| {
                c.get("tag").and_then(Value::as_str) != Some(SINGBOX_SRS_HTTP_CLIENT_TAG)
            });
            list.push(json!({"tag": SINGBOX_SRS_HTTP_CLIENT_TAG, "detour": PROXY_TAG}));
        }
    }
}

fn convert_rule(rule: &mut Value, rule_sets: &mut Vec<String>) {
    let Some(map) = rule.as_object_mut() else {
        return;
    };
    if let Some(geosites) = map.remove("geosite") {
        if let Some(list) = geosites.as_array() {
            let sets = map.entry("rule_set").or_insert_with(|| json!([]));
            if let Some(sets_list) = sets.as_array_mut() {
                for value in list {
                    if let Some(name) = value.as_str() {
                        let tag = format!("geosite-{name}");
                        sets_list.push(json!(tag));
                        push_unique(rule_sets, &tag);
                    }
                }
            }
        }
    }
    if let Some(geoips) = map.remove("geoip") {
        if let Some(list) = geoips.as_array() {
            let sets = map.entry("rule_set").or_insert_with(|| json!([]));
            if let Some(sets_list) = sets.as_array_mut() {
                for value in list {
                    if let Some(name) = value.as_str() {
                        let tag = format!("geoip-{name}");
                        sets_list.push(json!(tag));
                        push_unique(rule_sets, &tag);
                    }
                }
            }
        }
    }
    if let Some(list) = map.get("rule_set").and_then(Value::as_array) {
        for value in list {
            if let Some(tag) = value.as_str() {
                push_unique(rule_sets, tag);
            }
        }
    }
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    if !list.iter().any(|item| item == value) {
        list.push(value.to_string());
    }
}
