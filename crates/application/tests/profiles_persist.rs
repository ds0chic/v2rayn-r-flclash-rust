//! T06a: real SQLite persistence tests for profile save/reopen, revision
//! conflicts, delete/copy and paged queries. All addresses are RFC 5737 /
//! reserved example domains.

use std::sync::Arc;

use application::runtime_client::NullRuntimeClient;
use application::{AppEngine, PageRequest, ProfileFilter, ProfileSort};
use domain::{ConfigType, CoreType, DesiredRevision, Profile};

fn open(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

fn base_profile(id: &str, config_type: ConfigType) -> Profile {
    Profile {
        index_id: id.to_string(),
        config_type,
        core_type: Some(CoreType::SingBox),
        remarks: format!("节点-{id}"),
        address: "192.0.2.10".into(),
        port: 443,
        ..Default::default()
    }
}

/// Fields populated per protocol so the round-trip covers the real extras.
fn protocol_profile(config_type: ConfigType, index: u32) -> Profile {
    let id = format!("t06a-{index:02}");
    let mut p = base_profile(&id, config_type);
    p.network = "ws".into();
    p.transport_extra.path = Some("/t06a".into());
    p.transport_extra.host = Some("example.com".into());
    p.security.stream_security = Some("tls".into());
    p.security.sni = Some("sni.example.com".into());
    p.security.alpn = Some("h2,http/1.1".into());
    p.security.fingerprint = Some("chrome".into());
    match config_type {
        ConfigType::Vmess => {
            p.proto_extra.alter_id = Some("0".into());
            p.proto_extra.vmess_security = Some("auto".into());
            p.password = "11111111-2222-3333-4444-555555555555".into();
        }
        ConfigType::Vless => {
            p.proto_extra.flow = Some("xtls-rprx-vision".into());
            p.proto_extra.vless_encryption = Some("none".into());
            p.password = "11111111-2222-3333-4444-555555555555".into();
        }
        ConfigType::Shadowsocks => {
            p.proto_extra.ss_method = Some("aes-256-gcm".into());
            p.password = "test-password".into();
        }
        ConfigType::Socks | ConfigType::Http => {
            p.username = "user".into();
            p.password = "pass".into();
        }
        ConfigType::Trojan => {
            p.password = "trojan-pass".into();
        }
        ConfigType::Hysteria2 => {
            p.proto_extra.salamander_pass = Some("obfs-pass".into());
            p.proto_extra.up_mbps = Some(120);
            p.proto_extra.down_mbps = Some(240);
            p.proto_extra.ports = Some("443,8443".into());
            p.password = "hy2-pass".into();
        }
        ConfigType::Tuic => {
            p.proto_extra.congestion_control = Some("bbr".into());
            p.proto_extra.uot = Some(true);
            p.password = "tuic-pass".into();
        }
        ConfigType::WireGuard => {
            p.proto_extra.wg_public_key = Some("pubkey".into());
            p.proto_extra.wg_interface_address = Some("10.0.0.2/32".into());
            p.proto_extra.wg_mtu = Some(1280);
            p.password = "private-key".into();
        }
        ConfigType::Anytls => {
            p.password = "anytls-pass".into();
        }
        ConfigType::Naive => {
            p.username = "naive-user".into();
            p.password = "naive-pass".into();
            p.proto_extra.insecure_concurrency = Some(4);
            p.proto_extra.naive_quic = Some(false);
        }
        other => panic!("unexpected protocol in test: {other:?}"),
    }
    p
}

const BASIC_11: [ConfigType; 11] = [
    ConfigType::Vmess,
    ConfigType::Vless,
    ConfigType::Shadowsocks,
    ConfigType::Socks,
    ConfigType::Http,
    ConfigType::Trojan,
    ConfigType::Hysteria2,
    ConfigType::Tuic,
    ConfigType::WireGuard,
    ConfigType::Anytls,
    ConfigType::Naive,
];

#[test]
fn save_reopen_round_trips_all_eleven_protocols() {
    let dir = tempfile::tempdir().unwrap();
    // `save_profile` normalizes ordinary protocols like upstream `Add*Server`
    // (RE-PROF-07), so the persisted contract is "reopen equals the saved,
    // normalized profile", not the raw fixture.
    let mut saved_profiles: Vec<Profile> = Vec::new();
    {
        let engine = open(dir.path());
        let mut revision = engine.snapshot().unwrap().revisions.desired;
        for (i, t) in BASIC_11.iter().enumerate() {
            let p = protocol_profile(*t, i as u32);
            let (saved, next) = engine.save_profile(p, revision).unwrap();
            saved_profiles.push(saved);
            revision = next;
        }
        assert_eq!(engine.profile_count(), 11);
    }
    // Reopen a fresh engine over the same directory: data must survive.
    let engine = open(dir.path());
    assert_eq!(engine.profile_count(), 11);
    for (i, t) in BASIC_11.iter().enumerate() {
        let id = format!("t06a-{i:02}");
        let loaded = engine.profile_by_id(&id).unwrap().expect("profile present");
        assert_eq!(
            loaded, saved_profiles[i],
            "round-trip mismatch for {t:?} (reopen must equal the normalized saved profile)"
        );
    }
}

#[test]
fn expected_revision_conflict_is_structured() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    let p = protocol_profile(ConfigType::Vless, 0);
    let (_, next) = engine
        .save_profile(p.clone(), DesiredRevision::ZERO)
        .unwrap();
    // Second save at the already-bumped revision succeeds.
    let mut p2 = p.clone();
    p2.remarks = "changed".into();
    engine.save_profile(p2, next).unwrap();
    // Reusing the stale revision is rejected with the stable code.
    let err = engine.save_profile(p, DesiredRevision::ZERO).unwrap_err();
    assert_eq!(err.code, domain::codes::REVISION_STALE);
}

#[test]
fn delete_and_copy_act_on_sets_and_persist() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    let mut revision = engine.snapshot().unwrap().revisions.desired;
    let mut ids = Vec::new();
    for i in 0..3 {
        let p = protocol_profile(ConfigType::Trojan, i);
        ids.push(p.index_id.clone());
        let (_, next) = engine.save_profile(p, revision).unwrap();
        revision = next;
    }
    let copies = engine.copy_profiles(&ids[..1]).unwrap();
    assert_eq!(copies.len(), 1);
    assert!(copies[0].remarks.contains("副本"));
    assert_eq!(engine.profile_count(), 4);

    let removed = engine
        .delete_profiles(&[ids[0].clone(), ids[1].clone()])
        .unwrap();
    assert_eq!(removed, 2);
    assert_eq!(engine.profile_count(), 2);

    // Reopen confirms the deletions/copies were committed.
    drop(engine);
    let reopened = open(dir.path());
    assert_eq!(reopened.profile_count(), 2);
}

#[test]
fn set_remarks_and_active_persist() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    let p = protocol_profile(ConfigType::Vmess, 0);
    let id = p.index_id.clone();
    engine.save_profile(p, DesiredRevision::ZERO).unwrap();
    engine.set_remarks(&id, "emoji 🚀 中文".into()).unwrap();
    engine.set_active(Some(id.clone())).unwrap();
    drop(engine);

    let reopened = open(dir.path());
    let loaded = reopened.profile_by_id(&id).unwrap().unwrap();
    assert_eq!(loaded.remarks, "emoji 🚀 中文");
    assert_eq!(reopened.active_profile().as_deref(), Some(id.as_str()));
}

#[test]
fn query_filters_sorts_and_pages_against_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    let mut revision = engine.snapshot().unwrap().revisions.desired;
    for i in 0..25 {
        let t = if i % 2 == 0 {
            ConfigType::Vless
        } else {
            ConfigType::Trojan
        };
        let p = protocol_profile(t, i);
        let (_, next) = engine.save_profile(p, revision).unwrap();
        revision = next;
    }

    let filter = ProfileFilter {
        text: None,
        config_types: vec![ConfigType::Vless],
        subid: None,
    };
    let page = engine
        .query_profiles(
            filter,
            ProfileSort::Remarks,
            PageRequest {
                cursor: 0,
                page_size: 5,
            },
        )
        .unwrap();
    assert_eq!(page.total, 13);
    assert_eq!(page.items.len(), 5);
    assert_eq!(page.next_cursor, Some(5));
    assert!(page
        .items
        .iter()
        .all(|p| p.config_type == ConfigType::Vless));

    let text_page = engine
        .query_profiles(
            ProfileFilter {
                text: Some("t06a-07".into()),
                config_types: vec![],
                subid: None,
            },
            ProfileSort::IndexId,
            PageRequest::default(),
        )
        .unwrap();
    assert_eq!(text_page.total, 1);
}
