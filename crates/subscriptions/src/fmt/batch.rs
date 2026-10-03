//! Batch / structured formats: `V2rayFmt`, `SingboxFmt`, `ClashFmt`,
//! `Hysteria2Fmt.ResolveFull2` and `HtmlPageFmt`.
//!
//! Custom/Outbound profiles keep their raw payload under
//! `extra["RawConfig"]` (see `docs/decisions/T09-fmt.md`); the upstream build
//! writes it to a temp file instead, which the pure parser deliberately avoids.
//! The bridge import pipeline consumes it with [`take_raw_config`] and
//! materializes the RT-08 file-type form (FIX-04B): `Address` points at
//! `<data>/config/<name>`, so codegen reads it back through
//! `AppEngine::custom_file_text`.

use domain::{ConfigType, CoreType, Profile};
use serde_json::{Map, Value};

use crate::util::url_encode;

/// Key under `Profile.extra` that carries a structured config's raw text.
pub const RAW_CONFIG_KEY: &str = "RawConfig";

/// Take (and remove) the structured config text a batch decoder stored under
/// [`RAW_CONFIG_KEY`].
///
/// The pure parser deliberately avoids IO, so the raw payload is parked here;
/// the bridge import pipeline consumes it and materializes the RT-08 file-type
/// form (`Address` -> `<data>/config/<name>`), after which codegen reads it
/// through `AppEngine::custom_file_text`.
pub fn take_raw_config(profile: &mut Profile) -> Option<String> {
    match profile.extra.remove(RAW_CONFIG_KEY) {
        Some(Value::String(text)) if !text.trim().is_empty() => Some(text),
        _ => None,
    }
}

/// `SaveCustomRawFileServer.DetectFileExtension`: `.json` for a JSON document,
/// `.yaml` for a `---` header or a top-level `key:` line, otherwise an empty
/// extension. Kept here so the bridge import path can name materialized files
/// exactly like the frozen source.
pub fn detect_config_extension(data: &str) -> &'static str {
    let trimmed = data.trim_start();
    if trimmed.is_empty() {
        return "";
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return ".json";
    }
    if trimmed.starts_with("---") {
        return ".yaml";
    }
    for line in trimmed.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(colon) = line.find(':') {
            if colon > 0 {
                let key = &line[..colon];
                if !key.contains(' ') && !key.contains('\t') {
                    let after = line[colon + 1..].chars().next();
                    if after.is_none() || matches!(after, Some(' ' | '\t' | '\r')) {
                        return ".yaml";
                    }
                }
            }
        }
    }
    ""
}

/// `HtmlPageFmt.IsHtmlPage` (all three markers must be present).
pub fn is_html_page(data: &str) -> bool {
    matches_all(data, &["<html", "<!doctype html", "<head"])
}

/// `ClashFmt.ResolveFull` detection (all three markers must be present).
pub fn is_clash_full(data: &str) -> bool {
    matches_all(data, &["rules", "-port", "proxies"])
}

/// `Hysteria2Fmt.ResolveFull2` detection (all five markers must be present).
pub fn is_hysteria2_full(data: &str) -> bool {
    matches_all(data, &["server", "auth", "up", "down", "listen"])
}

fn matches_all(data: &str, needles: &[&str]) -> bool {
    needles.iter().all(|needle| {
        data.to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase())
    })
}

/// Mirror `AddBatchServersDefaultCustom`: try every structured decoder in the
/// upstream priority order and return the first non-empty result.
pub fn resolve_custom(data: &str, sub_remarks: Option<&str>, max_depth: usize) -> Vec<Profile> {
    let mut profiles = v2ray_resolve(data, sub_remarks, false, max_depth);
    if profiles.is_empty() {
        profiles = singbox_resolve(data, sub_remarks, false, max_depth);
    }
    if profiles.is_empty() {
        profiles = v2ray_resolve(data, sub_remarks, true, max_depth);
    }
    if profiles.is_empty() {
        profiles = singbox_resolve(data, sub_remarks, true, max_depth);
    }
    if !profiles.is_empty() {
        return profiles;
    }
    if is_html_page(data) {
        return Vec::new();
    }
    if is_clash_full(data) {
        return vec![custom_profile(
            CoreType::Mihomo,
            sub_remarks.unwrap_or("clash_custom"),
            data,
        )];
    }
    if is_hysteria2_full(data) {
        return vec![custom_profile(
            CoreType::Hysteria2,
            sub_remarks.unwrap_or("hysteria2_custom"),
            data,
        )];
    }
    Vec::new()
}

fn custom_profile(core_type: CoreType, remarks: &str, data: &str) -> Profile {
    let mut profile = Profile {
        config_type: ConfigType::Custom,
        core_type: Some(core_type),
        remarks: remarks.to_string(),
        address: String::new(),
        ..Profile::default()
    };
    profile
        .extra
        .insert(RAW_CONFIG_KEY.to_string(), Value::String(data.to_string()));
    profile
}

fn outbound_profile(core_type: CoreType, remarks: String, data: &str) -> Profile {
    let mut profile = Profile {
        config_type: ConfigType::Outbound,
        core_type: Some(core_type),
        remarks,
        address: String::new(),
        ..Profile::default()
    };
    profile
        .extra
        .insert(RAW_CONFIG_KEY.to_string(), Value::String(data.to_string()));
    profile
}

/// Parse an Xray/V2ray JSON document (array, full config or bare outbound).
pub fn v2ray_resolve(
    data: &str,
    sub_remarks: Option<&str>,
    is_outbound: bool,
    max_depth: usize,
) -> Vec<Profile> {
    let Ok(value) = serde_json::from_str::<Value>(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    resolve_v2ray_node(&value, sub_remarks, is_outbound, max_depth, &mut out);
    out
}

fn resolve_v2ray_node(
    node: &Value,
    sub_remarks: Option<&str>,
    is_outbound: bool,
    depth: usize,
    out: &mut Vec<Profile>,
) {
    if depth == 0 {
        return;
    }
    match node {
        Value::Array(items) => {
            for item in items {
                resolve_v2ray_node(item, sub_remarks, is_outbound, depth - 1, out);
            }
        }
        Value::Object(obj) => {
            if !is_outbound {
                if let Some(full) = v2ray_full(obj, sub_remarks) {
                    out.push(full);
                    return;
                }
            }
            let full_outbounds = v2ray_full_to_outbound(obj, sub_remarks);
            if !full_outbounds.is_empty() {
                out.extend(full_outbounds);
                return;
            }
            if let Some(outbound) = v2ray_outbound(obj, sub_remarks) {
                out.push(outbound);
            }
        }
        _ => {}
    }
}

fn v2ray_full(obj: &Map<String, Value>, sub_remarks: Option<&str>) -> Option<Profile> {
    if obj.get("inbounds").is_none() || obj.get("outbounds").is_none() {
        return None;
    }
    let outbounds = obj.get("outbounds")?.as_array()?;
    if !outbounds.iter().any(is_valid_v2ray_outbound) {
        return None;
    }
    let remarks = string_field(obj, "remarks")
        .filter(|v| !v.is_empty())
        .or_else(|| sub_remarks.map(str::to_string))
        .unwrap_or_else(|| "v2ray_custom".to_string());
    Some(custom_profile(
        CoreType::Xray,
        &remarks,
        &Value::Object(obj.clone()).to_string(),
    ))
}

fn v2ray_full_to_outbound(obj: &Map<String, Value>, sub_remarks: Option<&str>) -> Vec<Profile> {
    let Some(outbounds) = obj.get("outbounds").and_then(Value::as_array) else {
        return Vec::new();
    };
    outbounds
        .iter()
        .filter_map(|outbound| {
            outbound
                .as_object()
                .and_then(|o| v2ray_outbound(o, sub_remarks))
        })
        .collect()
}

fn v2ray_outbound(obj: &Map<String, Value>, _sub_remarks: Option<&str>) -> Option<Profile> {
    if !is_valid_v2ray_outbound(&Value::Object(obj.clone())) {
        return None;
    }
    let protocol = string_field(obj, "protocol").unwrap_or_default();
    if protocol.is_empty()
        || matches!(
            protocol.as_str(),
            "freedom" | "blackhole" | "dns" | "loopback"
        )
    {
        return None;
    }
    let tag = string_field(obj, "tag").unwrap_or_default();
    Some(outbound_profile(
        CoreType::Xray,
        format!("{protocol}_{tag}"),
        &Value::Object(obj.clone()).to_string(),
    ))
}

fn is_valid_v2ray_outbound(node: &Value) -> bool {
    let Some(obj) = node.as_object() else {
        return false;
    };
    if string_field(obj, "protocol").unwrap_or_default().is_empty() {
        return false;
    }
    let mut matched = 1;
    for key in ["settings", "streamSettings", "tag", "mux"] {
        if node_text_nonempty(obj, key) {
            matched += 1;
        }
    }
    matched >= 3
}

/// Parse a sing-box JSON document (array, full config or bare outbound).
pub fn singbox_resolve(
    data: &str,
    sub_remarks: Option<&str>,
    is_outbound: bool,
    max_depth: usize,
) -> Vec<Profile> {
    let Ok(value) = serde_json::from_str::<Value>(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    resolve_singbox_node(&value, sub_remarks, is_outbound, max_depth, &mut out);
    out
}

fn resolve_singbox_node(
    node: &Value,
    sub_remarks: Option<&str>,
    is_outbound: bool,
    depth: usize,
    out: &mut Vec<Profile>,
) {
    if depth == 0 {
        return;
    }
    match node {
        Value::Array(items) => {
            for item in items {
                resolve_singbox_node(item, sub_remarks, is_outbound, depth - 1, out);
            }
        }
        Value::Object(obj) => {
            if !is_outbound {
                if let Some(full) = singbox_full(obj, sub_remarks) {
                    out.push(full);
                    return;
                }
            }
            let full_outbounds = singbox_full_to_outbound(obj, sub_remarks);
            if !full_outbounds.is_empty() {
                out.extend(full_outbounds);
                return;
            }
            if let Some(outbound) = singbox_outbound(obj, sub_remarks) {
                out.push(outbound);
            }
        }
        _ => {}
    }
}

fn singbox_full(obj: &Map<String, Value>, sub_remarks: Option<&str>) -> Option<Profile> {
    if obj.get("inbounds").is_none() || obj.get("outbounds").is_none() {
        return None;
    }
    let outbounds = obj.get("outbounds")?.as_array()?;
    if !outbounds.iter().any(is_valid_singbox_outbound) {
        return None;
    }
    let remarks = sub_remarks.unwrap_or("singbox_custom").to_string();
    Some(custom_profile(
        CoreType::SingBox,
        &remarks,
        &Value::Object(obj.clone()).to_string(),
    ))
}

fn singbox_full_to_outbound(obj: &Map<String, Value>, sub_remarks: Option<&str>) -> Vec<Profile> {
    let Some(outbounds) = obj.get("outbounds").and_then(Value::as_array) else {
        return Vec::new();
    };
    outbounds
        .iter()
        .filter_map(|outbound| {
            outbound
                .as_object()
                .and_then(|o| singbox_outbound(o, sub_remarks))
        })
        .collect()
}

fn singbox_outbound(obj: &Map<String, Value>, _sub_remarks: Option<&str>) -> Option<Profile> {
    if !is_valid_singbox_outbound(&Value::Object(obj.clone())) {
        return None;
    }
    let type_token = string_field(obj, "type").unwrap_or_default();
    if type_token.is_empty()
        || matches!(
            type_token.as_str(),
            "direct" | "block" | "dns" | "selector" | "urltest"
        )
    {
        return None;
    }
    let tag = string_field(obj, "tag").unwrap_or_default();
    Some(outbound_profile(
        CoreType::SingBox,
        format!("{type_token}_{tag}"),
        &Value::Object(obj.clone()).to_string(),
    ))
}

fn is_valid_singbox_outbound(node: &Value) -> bool {
    let Some(obj) = node.as_object() else {
        return false;
    };
    if string_field(obj, "type").unwrap_or_default().is_empty() {
        return false;
    }
    let mut matched = 1;
    for key in ["tag", "server", "server_port", "tls"] {
        if node_text_nonempty(obj, key) {
            matched += 1;
        }
    }
    matched >= 2
}

/// Upstream compares each field's `JsonNode.ToString()` -- an object/array/bool
/// is non-empty, a missing field or an empty string is not.
fn node_text_nonempty(obj: &Map<String, Value>, key: &str) -> bool {
    match obj.get(key) {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn string_field(obj: &Map<String, Value>, key: &str) -> Option<String> {
    match obj.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    }
}

/// Encode a remark fragment exactly like the share exporters.
pub fn remark_fragment(remarks: &str) -> String {
    if remarks.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(remarks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_xray_config_becomes_a_custom_profile() {
        let data = r#"{"inbounds":[{"port":1080,"protocol":"socks"}],"outbounds":[{"protocol":"vmess","tag":"proxy","settings":{},"streamSettings":{"network":"ws"}}]}"#;
        let profiles = v2ray_resolve(data, Some("sub"), false, 16);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].config_type, ConfigType::Custom);
        assert_eq!(profiles[0].core_type, Some(CoreType::Xray));
        assert_eq!(profiles[0].remarks, "sub");
        assert!(profiles[0].extra.contains_key(RAW_CONFIG_KEY));
    }

    #[test]
    fn bare_outbounds_are_extracted() {
        let data = r#"{"outbounds":[{"protocol":"vmess","tag":"a","settings":{},"streamSettings":{}},{"protocol":"freedom","tag":"direct"}]}"#;
        let profiles = v2ray_resolve(data, None, false, 16);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].config_type, ConfigType::Outbound);
        assert_eq!(profiles[0].remarks, "vmess_a");
    }

    #[test]
    fn singbox_full_and_outbound_detection() {
        let full = r#"{"inbounds":[],"outbounds":[{"type":"vless","tag":"p","server":"a","server_port":443}]}"#;
        let profiles = singbox_resolve(full, Some("s"), false, 16);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].core_type, Some(CoreType::SingBox));
    }

    #[test]
    fn html_and_clash_detection_require_all_markers() {
        assert!(is_html_page("<html><head></head><!doctype html>"));
        assert!(!is_html_page("<html><head></head>"));
        assert!(is_clash_full("rules:\n-port\nproxies:"));
        assert!(!is_clash_full("proxies: []"));
    }

    #[test]
    fn take_raw_config_removes_the_local_marker() {
        let mut profile = custom_profile(CoreType::Mihomo, "clash", "proxies:\n  - a\n");
        assert!(profile.extra.contains_key(RAW_CONFIG_KEY));
        let raw = take_raw_config(&mut profile).expect("raw text");
        assert!(raw.contains("proxies"));
        assert!(!profile.extra.contains_key(RAW_CONFIG_KEY));
        assert!(take_raw_config(&mut profile).is_none());
    }

    #[test]
    fn detect_config_extension_matches_upstream() {
        assert_eq!(detect_config_extension(r#"{"inbounds":[]}"#), ".json");
        assert_eq!(detect_config_extension("[1,2]"), ".json");
        assert_eq!(detect_config_extension("---\nproxies: []"), ".yaml");
        assert_eq!(detect_config_extension("proxies:\n  - a"), ".yaml");
        assert_eq!(detect_config_extension("# c\nmixed-port: 7890"), ".yaml");
        assert_eq!(detect_config_extension("plain text without colon"), "");
    }
}
