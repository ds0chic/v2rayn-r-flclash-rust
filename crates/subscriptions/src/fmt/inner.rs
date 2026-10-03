//! `InnerFmt` — the `v2rayn://<type>/<urlsafe-base64 json>` internal share
//! format. Index ids are always re-issued on import and remapped on export;
//! the `self` sentinel in `SubChildItems` resolves to the importing sub id.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use domain::{ConfigType, CoreType, Profile};
use serde_json::{Map, Value};

use super::base::INNER;
use super::wire::{self, OutboundLoader};
use crate::util::{base64_decode, base64_urlsafe_nopad};

/// `subid` replacement accepted by exported group nodes.
pub const SELF_SENTINEL: &str = "self";

/// Parse every `v2rayn://` line and re-issue index ids.
pub fn parse(input: &str, subid: &str) -> Vec<Profile> {
    let mut list: Vec<Profile> = Vec::new();
    let mut index_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for line in crate::util::split_lines(input) {
        if line.is_empty() {
            continue;
        }
        let trimmed = line.trim();
        if !trimmed
            .get(..INNER.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(INNER))
        {
            continue;
        }
        let Some(mut item) = parse_single(trimmed) else {
            continue;
        };
        if item.config_type == ConfigType::Custom {
            continue;
        }
        let new_id = new_id();
        if !item.index_id.is_empty() {
            index_map.insert(item.index_id.clone(), new_id.clone());
        }
        item.index_id = new_id;
        list.push(item);
    }

    let mut empty_groups: Vec<usize> = Vec::new();
    for (position, item) in list.iter_mut().enumerate() {
        if !is_group(item.config_type) {
            continue;
        }
        if item.proto_extra.sub_child_items.as_deref() == Some(SELF_SENTINEL) {
            item.proto_extra.sub_child_items = Some(subid.to_string());
        } else {
            item.proto_extra.sub_child_items = None;
        }
        if let Some(child_items) = item.proto_extra.child_items.as_deref() {
            let remapped: Vec<String> = crate::util::string2list(child_items)
                .into_iter()
                .filter_map(|id| index_map.get(&id).cloned())
                .collect();
            item.proto_extra.child_items = if remapped.is_empty() {
                None
            } else {
                Some(crate::util::list2string(&remapped))
            };
        } else {
            item.proto_extra.child_items = None;
        }
        if item.proto_extra.sub_child_items.is_none() && item.proto_extra.child_items.is_none() {
            empty_groups.push(position);
        }
    }
    for position in empty_groups.into_iter().rev() {
        list.remove(position);
    }
    list
}

/// Export profiles as newline-separated `v2rayn://` URIs. Returns `None` when
/// nothing exportable remains.
///
/// `Outbound` nodes without inline content are skipped here; callers that can
/// read the file behind `Address` should use [`emit_with`] instead.
pub fn emit(items: &[Profile]) -> Option<String> {
    emit_with(items, &|_| None)
}

/// [`emit`] with a resolver for file-backed `Outbound` addresses, mirroring
/// upstream `ToUriSingle` reading the file behind `Address`.
pub fn emit_with(items: &[Profile], outbound_loader: OutboundLoader<'_>) -> Option<String> {
    let mut out = String::new();
    for item in items {
        if item.config_type == ConfigType::Custom {
            continue;
        }
        let mut clone = item.clone();
        clone.index_id = reproducible_id(&clone.index_id);
        if is_group(clone.config_type) {
            if clone
                .proto_extra
                .sub_child_items
                .as_deref()
                .is_some_and(|v| !v.is_empty())
            {
                clone.proto_extra.sub_child_items = Some(SELF_SENTINEL.to_string());
            }
            if let Some(child_items) = clone.proto_extra.child_items.as_deref() {
                let remapped: Vec<String> = crate::util::string2list(child_items)
                    .into_iter()
                    .map(|id| reproducible_id(&id))
                    .filter(|id| !id.is_empty())
                    .collect();
                clone.proto_extra.child_items = Some(crate::util::list2string(&remapped));
            }
        }
        if let Some(uri) = emit_single(&clone, outbound_loader) {
            out.push_str(&uri);
            out.push('\n');
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn is_group(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::PolicyGroup | ConfigType::ProxyChain
    )
}

fn emit_single(item: &Profile, outbound_loader: OutboundLoader<'_>) -> Option<String> {
    let map = wire::profile_to_wire_object(item, outbound_loader)?;
    let mut value = Value::Object(map.into_iter().collect());
    remove_empty_json(&mut value);
    let json = serde_json::to_string(&value).ok()?;
    let encoded = base64_urlsafe_nopad(json.as_bytes());
    let token = item.config_type.as_str().to_ascii_lowercase();
    Some(format!("{INNER}{token}/{encoded}"))
}

fn parse_single(line: &str) -> Option<Profile> {
    let url = url::Url::parse(line).ok()?;
    let segment = url.path().trim_start_matches('/');
    if segment.is_empty() {
        return None;
    }
    let decoded = base64_decode(segment).ok()?;
    let value: Value = serde_json::from_str(&decoded).ok()?;
    let Value::Object(obj) = value else {
        return None;
    };

    // Frozen upstream shape first; legacy snake_case exports fall back below.
    // The gate matters: the legacy struct defaults (`config_version = 4`,
    // `config_type = Vmess`) would otherwise resurrect payloads the frozen
    // wire contract already rejected (wrong version / undefined enum).
    if obj.contains_key("ConfigType") || obj.contains_key("ConfigVersion") {
        if let Ok(wire_item) = serde_json::from_value::<wire::InnerProfile>(Value::Object(obj)) {
            return wire::wire_to_profile(wire_item);
        }
        return None;
    }
    parse_single_legacy(obj)
}

/// Pre-freeze spelling (`snake_case` storage names): still accepted so URIs
/// this app exported before the PascalCase wire DTO keep importing.
fn parse_single_legacy(obj: Map<String, Value>) -> Option<Profile> {
    let mut obj = obj;
    flatten_extra(&mut obj, "ProtoExtra", "ProtoExtraObj", "proto_extra");
    flatten_extra(
        &mut obj,
        "TransportExtra",
        "TransportExtraObj",
        "transport_extra",
    );

    let profile: Profile = serde_json::from_value(Value::Object(obj)).ok()?;
    if profile.config_version != 4 {
        return None;
    }
    if !matches!(
        profile.core_type,
        None | Some(CoreType::Xray) | Some(CoreType::SingBox)
    ) {
        return None;
    }
    if profile.config_type == ConfigType::Custom {
        return None;
    }
    if profile.config_type == ConfigType::Outbound
        && profile.extra.get("RawConfig").is_none()
        && profile.address.is_empty()
    {
        return None;
    }
    Some(profile)
}

/// Accept both our nested `proto_extra` object and the upstream string/Obj
/// spellings so older inner URIs still import.
fn flatten_extra(obj: &mut Map<String, Value>, string_key: &str, obj_key: &str, target: &str) {
    if matches!(obj.get(target), Some(Value::Object(_))) {
        return;
    }
    if let Some(Value::Object(inner)) = obj.remove(obj_key) {
        obj.insert(target.to_string(), Value::Object(inner));
        return;
    }
    if let Some(Value::String(raw)) = obj.get(string_key) {
        if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
            if parsed.is_object() {
                obj.insert(target.to_string(), parsed);
            }
        }
        obj.remove(string_key);
    }
}

fn remove_empty_json(node: &mut Value) {
    match node {
        Value::Object(map) => {
            for value in map.values_mut() {
                remove_empty_json(value);
            }
            map.retain(|_, value| !is_empty_json(value));
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                remove_empty_json(item);
            }
            items.retain(|value| !is_empty_json(value));
        }
        _ => {}
    }
}

fn is_empty_json(node: &Value) -> bool {
    match node {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Object(map) => map.is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

fn session_salt() -> &'static str {
    static SALT: OnceLock<String> = OnceLock::new();
    SALT.get_or_init(|| format!("{:x}", nanos()))
}

fn reproducible_id(original: &str) -> String {
    if original.is_empty() {
        return original.to_string();
    }
    let mut hasher = DefaultHasher::new();
    session_salt().hash(&mut hasher);
    original.hash(&mut hasher);
    let hash = (hasher.finish() & 0x7FFF_FFFF) as u32;
    let encoded = crate::util::base64_encode_nopad_bytes(&hash.to_le_bytes());
    encoded.replace('=', "")
}

fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}", nanos(), counter)
}

fn nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(remark: &str, id: &str) -> Profile {
        Profile {
            index_id: id.into(),
            config_type: ConfigType::Socks,
            remarks: remark.into(),
            address: "127.0.0.1".into(),
            port: 1080,
            username: "u".into(),
            password: "p".into(),
            ..Profile::default()
        }
    }

    #[test]
    fn group_references_are_remapped_and_self_becomes_subid() {
        let child_a = node("child-a", "id-a");
        let child_b = node("child-b", "id-b");
        let mut group = Profile {
            index_id: "group-1".into(),
            config_type: ConfigType::PolicyGroup,
            remarks: "group".into(),
            ..Profile::default()
        };
        group.proto_extra.child_items = Some("id-a,id-b".into());
        group.proto_extra.sub_child_items = Some("original-sub".into());

        let uri = emit(&[group.clone(), child_a.clone(), child_b.clone()]).unwrap();
        let resolved = parse(&uri, "sub-123");
        assert_eq!(resolved.len(), 3);
        let resolved_group = resolved.iter().find(|p| p.remarks == "group").unwrap();
        let resolved_a = resolved.iter().find(|p| p.remarks == "child-a").unwrap();
        let resolved_b = resolved.iter().find(|p| p.remarks == "child-b").unwrap();
        assert_eq!(
            resolved_group.proto_extra.sub_child_items.as_deref(),
            Some("sub-123")
        );
        assert_eq!(
            resolved_group.proto_extra.child_items.as_deref(),
            Some(format!("{},{}", resolved_a.index_id, resolved_b.index_id).as_str())
        );
        // Fresh ids, not the originals.
        assert_ne!(resolved_a.index_id, "id-a");
    }

    #[test]
    fn empty_group_is_dropped_and_custom_skipped() {
        let group = Profile {
            index_id: "g".into(),
            config_type: ConfigType::PolicyGroup,
            remarks: "empty".into(),
            ..Profile::default()
        };
        let custom = Profile {
            index_id: "c".into(),
            config_type: ConfigType::Custom,
            remarks: "custom".into(),
            ..Profile::default()
        };
        // Group has neither a resolvable child nor `self`, so it is removed.
        let uri = emit(&[group, custom, node("n", "n")]).unwrap();
        let resolved = parse(&uri, "sub");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].remarks, "n");
    }

    #[test]
    fn non_inner_lines_are_ignored() {
        let resolved = parse("vmess://abc\nrandom text\n", "sub");
        assert!(resolved.is_empty());
    }
}
