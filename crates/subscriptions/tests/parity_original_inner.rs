//! Audit regression: frozen v2rayN InnerFmt wire shape, no network or file IO.
//! Source: InnerFmt.ToUriSingle/ResolveSingle and JsonUtils serialize options
//! at 7d6a967c18c697f28dc6917122ed3a4993fcf336.

use domain::{ConfigType, MultipleLoad, Profile};
use serde_json::{json, Value};
use subscriptions::{parse_content, to_inner_uri, ContentHint, ParseOptions};

fn fixture(name: &str) -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic/inner-wire")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {}", path.display()))
}

fn inner_uri(token: &str, payload: &str) -> String {
    format!(
        "v2rayn://{token}/{}",
        subscriptions::util::base64_urlsafe_nopad(payload.as_bytes())
    )
}

fn parse_inner_uris(text: &str, subid: &str) -> Vec<Profile> {
    let parsed = parse_content(
        text,
        ContentHint::Inner,
        &ParseOptions {
            subid: subid.into(),
            ..ParseOptions::default()
        },
    );
    assert!(
        parsed.errors.is_empty(),
        "synthetic inner URIs must parse: {:?}",
        parsed.errors
    );
    parsed.profiles
}

fn decode_inner_payload(uri: &str) -> Value {
    let encoded = uri.trim().split('/').next_back().unwrap();
    let decoded = subscriptions::util::base64_decode(encoded).unwrap();
    serde_json::from_str(&decoded).unwrap()
}

#[test]
fn frozen_pascal_case_inner_uri_preserves_vless_entity_and_extras() {
    let frozen_payload = json!({
        "IndexId": "synthetic-export-id",
        "ConfigType": 5,
        "CoreType": 2,
        "ConfigVersion": 4,
        "Remarks": "原版合成 VLESS",
        "Address": "node.example.invalid",
        "Port": 11980,
        "Password": "11111111-2222-3333-4444-555555555555",
        "Network": "ws",
        "StreamSecurity": "tls",
        "Sni": "tls.example.invalid",
        "ProtoExtraObj": {"VlessEncryption": "none", "Flow": ""},
        "TransportExtraObj": {"Host": "host.example.invalid", "Path": "/synthetic"}
    });
    let uri = format!(
        "v2rayn://vless/{}",
        subscriptions::util::base64_urlsafe_nopad(frozen_payload.to_string().as_bytes())
    );
    let parsed = parse_content(&uri, ContentHint::Inner, &ParseOptions::default());
    assert!(
        parsed.errors.is_empty(),
        "frozen-shape synthetic URI must parse: {:?}",
        parsed.errors
    );
    assert_eq!(parsed.profiles.len(), 1);
    let node = &parsed.profiles[0];
    assert_eq!(node.config_type, ConfigType::Vless);
    assert_eq!(node.remarks, "原版合成 VLESS");
    assert_eq!(node.address, "node.example.invalid");
    assert_eq!(node.port, 11980);
    assert_eq!(node.network, "ws");
    assert_eq!(node.proto_extra.vless_encryption.as_deref(), Some("none"));
    assert_eq!(node.transport_extra.path.as_deref(), Some("/synthetic"));
    assert_ne!(node.index_id, "synthetic-export-id");
}

#[test]
fn exported_inner_uri_uses_frozen_profile_property_names() {
    let node = Profile {
        index_id: "synthetic-local-id".into(),
        config_type: ConfigType::Vless,
        remarks: "导出合成 VLESS".into(),
        address: "node.example.invalid".into(),
        port: 11980,
        password: "11111111-2222-3333-4444-555555555555".into(),
        network: "ws".into(),
        ..Default::default()
    };
    let uri = to_inner_uri(&[node]).expect("synthetic VLESS is exportable");
    let encoded = uri.trim().split_once("v2rayn://vless/").unwrap().1;
    let decoded = subscriptions::util::base64_decode(encoded).unwrap();
    let payload: Value = serde_json::from_str(&decoded).unwrap();
    assert_eq!(payload["ConfigType"], 5);
    assert_eq!(payload["ConfigVersion"], 4);
    assert_eq!(payload["Remarks"], "导出合成 VLESS");
    assert_eq!(payload["Address"], "node.example.invalid");
    assert!(payload.get("Subid").is_none());
    assert!(payload.get("IsSub").is_none());
}

#[test]
fn fixture_vless_import_export_reimport_is_stable() {
    let payload = fixture("vless-node.json");
    let uri = inner_uri("vless", payload.trim());
    let first = parse_inner_uris(&uri, "");
    assert_eq!(first.len(), 1);
    let node = &first[0];
    assert_eq!(node.config_type, ConfigType::Vless);
    assert_eq!(node.remarks, "合成 VLESS");
    assert_eq!(node.address, "node.example.invalid");
    assert_eq!(node.port, 11980);
    assert_eq!(node.proto_extra.vless_encryption.as_deref(), Some("none"));
    assert_eq!(node.proto_extra.flow.as_deref(), Some("xtls-rprx-vision"));
    assert_eq!(node.transport_extra.path.as_deref(), Some("/synthetic"));
    assert_ne!(node.index_id, "fixture-vless-1");

    let exported = to_inner_uri(&first).expect("ordinary node is exportable");
    let wire: Value = decode_inner_payload(exported.trim());
    assert_eq!(wire["ConfigType"], 5);
    assert_eq!(wire["ConfigVersion"], 4);
    assert_eq!(wire["Remarks"], "合成 VLESS");
    assert_eq!(wire["Address"], "node.example.invalid");
    assert_eq!(wire["ProtoExtraObj"]["VlessEncryption"], "none");
    assert_eq!(wire["TransportExtraObj"]["Path"], "/synthetic");
    assert_eq!(wire["FutureUpstreamField"], "must-survive-roundtrip");
    assert!(wire.get("Subid").is_none());
    assert!(wire.get("IsSub").is_none());
    assert!(wire.get("ProtoExtra").is_none());
    assert!(wire.get("TransportExtra").is_none());

    let second = parse_inner_uris(exported.trim(), "");
    assert_eq!(second.len(), 1);
    let again = &second[0];
    assert_eq!(again.config_type, node.config_type);
    assert_eq!(again.remarks, node.remarks);
    assert_eq!(again.address, node.address);
    assert_eq!(again.port, node.port);
    assert_eq!(again.password, node.password);
    assert_eq!(again.proto_extra, node.proto_extra);
    assert_eq!(again.transport_extra, node.transport_extra);
    assert_eq!(
        again.extra.get("FutureUpstreamField"),
        node.extra.get("FutureUpstreamField")
    );
}

#[test]
fn fixture_group_self_sentinel_resolves_to_importing_subid() {
    let node_uri = inner_uri("vless", fixture("vless-node.json").trim());
    let group_uri = inner_uri("policygroup", fixture("group-node.json").trim());
    let text = format!("{group_uri}\n{node_uri}\n");
    let resolved = parse_inner_uris(&text, "sub-fixture");
    assert_eq!(resolved.len(), 2);
    let group = resolved
        .iter()
        .find(|p| p.config_type == ConfigType::PolicyGroup)
        .expect("group survives: child + subid both resolve");
    assert_eq!(
        group.proto_extra.sub_child_items.as_deref(),
        Some("sub-fixture")
    );
    assert_eq!(
        group.proto_extra.multiple_load,
        Some(MultipleLoad::RoundRobin)
    );
    let child = resolved
        .iter()
        .find(|p| p.config_type == ConfigType::Vless)
        .unwrap();
    assert_eq!(
        group.proto_extra.child_items.as_deref(),
        Some(child.index_id.as_str())
    );

    let exported = to_inner_uri(&resolved).expect("group round-trips");
    let group_line = exported
        .lines()
        .map(decode_inner_payload)
        .find(|v| v["ConfigType"] == 101)
        .expect("exported group line");
    assert_eq!(group_line["ProtoExtraObj"]["SubChildItems"], "self");
    assert_eq!(group_line["ProtoExtraObj"]["MultipleLoad"], 3);
}

#[test]
fn fixture_outbound_inline_content_never_becomes_a_file_path() {
    use subscriptions::fmt::wire::inline_outbound_text;

    let uri = inner_uri("outbound", fixture("outbound-inline.json").trim());
    let parsed = parse_inner_uris(&uri, "");
    assert_eq!(parsed.len(), 1);
    let node = &parsed[0];
    assert_eq!(node.config_type, ConfigType::Outbound);
    // Inline form: no file path is invented.
    assert!(node.address.is_empty());
    let inline = inline_outbound_text(node).expect("inline outbound text is preserved");
    assert!(inline.contains("synthetic-out"));

    let exported = to_inner_uri(&parsed).expect("inline outbound re-exports");
    let wire: Value = decode_inner_payload(exported.trim());
    assert_eq!(wire["ConfigType"], 13);
    assert_eq!(wire["CustomOutboundObj"]["tag"], "synthetic-out");
    assert!(wire.get("Address").is_none());

    let again = parse_inner_uris(exported.trim(), "");
    assert_eq!(again.len(), 1);
    assert!(again[0].address.is_empty());
    // Re-import re-issues index ids on both sides, so stability is compared
    // with `IndexId` removed: same wire shape, same content.
    let exported_again = to_inner_uri(&again).expect("second export");
    let mut first_wire = decode_inner_payload(exported.trim());
    let mut second_wire = decode_inner_payload(exported_again.trim());
    first_wire.as_object_mut().unwrap().remove("IndexId");
    second_wire.as_object_mut().unwrap().remove("IndexId");
    assert_eq!(first_wire, second_wire);
}

#[test]
fn fixture_outbound_file_path_round_trips_through_loader() {
    let mut node = Profile {
        index_id: "synthetic-file-out".into(),
        config_type: ConfigType::Outbound,
        remarks: "文件型出站".into(),
        address: "stored-outbound.json".into(),
        ..Default::default()
    };
    node.core_type = Some(domain::CoreType::Xray);
    let file_content = serde_json::json!({"tag": "from-file", "protocol": "freedom"});
    let loader = |path: &str| (path == "stored-outbound.json").then(|| file_content.clone());
    let exported = subscriptions::to_inner_uri_with_outbound_loader(&[node], &loader)
        .expect("file-backed outbound exports via loader");
    let wire: Value = decode_inner_payload(exported.trim());
    assert_eq!(wire["CustomOutboundObj"]["tag"], "from-file");
    assert!(wire.get("Address").is_none());

    // Without inline content and without a loadable file, the node is skipped,
    // like upstream returning null when the file is missing.
    let bare = Profile {
        config_type: ConfigType::Outbound,
        remarks: "无内容出站".into(),
        ..Default::default()
    };
    assert!(to_inner_uri(&[bare]).is_none());
}

#[test]
fn fixture_invalid_payloads_are_rejected() {
    for name in ["invalid-version.json", "invalid-enum.json"] {
        let uri = inner_uri("vless", fixture(name).trim());
        let parsed = parse_content(&uri, ContentHint::Inner, &ParseOptions::default());
        assert!(
            parsed.profiles.is_empty(),
            "{name} must not import any profile"
        );
    }
}

#[test]
fn ordinary_node_without_remarks_or_address_still_imports() {
    let payload = json!({
        "ConfigType": 5,
        "ConfigVersion": 4,
        "Remarks": "",
        "Address": "",
        "Port": 0,
        "Password": "11111111-2222-3333-4444-555555555555",
        "Network": "tcp"
    });
    let uri = inner_uri("vless", &payload.to_string());
    let parsed = parse_inner_uris(&uri, "");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].config_type, ConfigType::Vless);
}

#[test]
fn legacy_snake_case_inner_uri_still_imports() {
    let payload = json!({
        "index_id": "legacy-id",
        "config_type": 5,
        "config_version": 4,
        "remarks": "旧版导出",
        "address": "legacy.example.invalid",
        "port": 11983,
        "password": "11111111-2222-3333-4444-555555555555",
        "network": "tcp"
    });
    let uri = inner_uri("vless", &payload.to_string());
    let parsed = parse_inner_uris(&uri, "");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].config_type, ConfigType::Vless);
    assert_eq!(parsed[0].remarks, "旧版导出");
    assert_ne!(parsed[0].index_id, "legacy-id");
}
