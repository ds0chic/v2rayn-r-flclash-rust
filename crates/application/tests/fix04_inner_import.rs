//! FIX-04: inner `v2rayn://` import persistence against real SQLite.
//!
//! Parses frozen-shaped synthetic URIs with the `subscriptions` crate, saves
//! them through the import-tolerant use case, and reopens a fresh engine over
//! the same directory. No network, no listeners, no user data.

use std::sync::Arc;

use application::runtime_client::NullRuntimeClient;
use application::AppEngine;
use domain::{ConfigType, DesiredRevision, Profile};

fn open(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

fn inner_uri(token: &str, payload: &str) -> String {
    format!(
        "v2rayn://{token}/{}",
        subscriptions::util::base64_urlsafe_nopad(payload.as_bytes())
    )
}

fn parse_inner(text: &str) -> Vec<Profile> {
    let parsed = subscriptions::parse_content(
        text,
        subscriptions::ContentHint::Inner,
        &subscriptions::ParseOptions::default(),
    );
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    parsed.profiles
}

#[test]
fn imported_vless_without_remarks_or_address_persists_and_reopens() {
    let payload = serde_json::json!({
        "IndexId": "fix04-bare",
        "ConfigType": 5,
        "ConfigVersion": 4,
        "Remarks": "",
        "Address": "",
        "Port": 0,
        "Password": "11111111-2222-3333-4444-555555555555",
        "Network": "tcp",
        "ProtoExtraObj": {"VlessEncryption": "none"}
    });
    let nodes = parse_inner(&inner_uri("vless", &payload.to_string()));
    assert_eq!(nodes.len(), 1);

    let dir = tempfile::tempdir().unwrap();
    let saved_id = {
        let engine = open(dir.path());
        // The editor contract still rejects this node; the import contract
        // must not.
        assert!(engine
            .save_profile(nodes[0].clone(), DesiredRevision::ZERO)
            .is_err());
        let (saved, _) = engine
            .save_imported_profile(nodes[0].clone(), DesiredRevision::ZERO)
            .expect("import save accepts empty remarks/address");
        // Persisted like upstream, but honestly flagged: address/port are
        // still required before the node can actually run.
        assert!(!saved.is_valid());
        saved.index_id.clone()
    };
    let reopened = open(dir.path());
    let loaded = reopened
        .profile_by_id(&saved_id)
        .unwrap()
        .expect("node survives reopen");
    assert_eq!(loaded.config_type, ConfigType::Vless);
    assert_eq!(loaded.password, "11111111-2222-3333-4444-555555555555");
    assert_eq!(loaded.proto_extra.vless_encryption.as_deref(), Some("none"));
}

#[test]
fn imported_outbound_inline_content_persists_and_reopens() {
    let payload = serde_json::json!({
        "IndexId": "fix04-out",
        "ConfigType": 13,
        "ConfigVersion": 4,
        "Remarks": "合成内联出站",
        "CustomOutboundObj": {"tag": "synthetic-out", "protocol": "freedom"}
    });
    let nodes = parse_inner(&inner_uri("outbound", &payload.to_string()));
    assert_eq!(nodes.len(), 1);
    assert!(nodes[0].address.is_empty());

    let dir = tempfile::tempdir().unwrap();
    let saved_id = {
        let engine = open(dir.path());
        let (saved, _) = engine
            .save_imported_profile(nodes[0].clone(), DesiredRevision::ZERO)
            .expect("inline outbound saves without a file path");
        saved.index_id.clone()
    };
    let reopened = open(dir.path());
    let loaded = reopened
        .profile_by_id(&saved_id)
        .unwrap()
        .expect("outbound survives reopen");
    assert_eq!(loaded.config_type, ConfigType::Outbound);
    assert!(loaded.address.is_empty());
    let inline = loaded
        .proto_extra
        .extra
        .get("customConfigText")
        .and_then(|v| v.as_str())
        .expect("inline text persisted");
    assert!(inline.contains("synthetic-out"));
}
