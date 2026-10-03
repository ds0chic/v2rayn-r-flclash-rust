//! Audit regression: frozen v2rayN InnerFmt wire shape, no network or file IO.
//! Source: InnerFmt.ToUriSingle/ResolveSingle and JsonUtils serialize options
//! at 7d6a967c18c697f28dc6917122ed3a4993fcf336.

use domain::{ConfigType, Profile};
use serde_json::{json, Value};
use subscriptions::{parse_content, to_inner_uri, ContentHint, ParseOptions};

#[test]
#[ignore = "PR-08: frozen upstream wire format mismatch; run with --ignored"]
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
#[ignore = "PR-08: frozen upstream wire format mismatch; run with --ignored"]
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
