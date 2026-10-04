//! RE-PROF-07: per-protocol backend normalization and validation for ordinary
//! nodes, mirroring the frozen `ConfigHandler.Add*Server` / `AddServerCommon`
//! contracts on both the editor save and the import save.
//!
//! Synthetic data only (RFC 5737 addresses, fake credentials); no network, no
//! listeners, no user data.

use application::AppEngine;
use domain::{codes, ConfigType, CoreType, DesiredRevision, Profile};

fn base(config_type: ConfigType) -> Profile {
    Profile {
        index_id: "re-prof-07".into(),
        config_type,
        remarks: "synthetic".into(),
        address: " 192.0.2.10 ".into(),
        port: 443,
        ..Default::default()
    }
}

#[allow(clippy::result_large_err)]
fn save(profile: Profile) -> Result<Profile, domain::DomainError> {
    AppEngine::in_memory()
        .save_profile(profile, DesiredRevision::ZERO)
        .map(|(saved, _)| saved)
}

#[allow(clippy::result_large_err)]
fn import(profile: Profile) -> Result<Profile, domain::DomainError> {
    AppEngine::in_memory()
        .save_imported_profile(profile, DesiredRevision::ZERO)
        .map(|(saved, _)| saved)
}

#[test]
fn tuic_editor_normalizes_like_upstream() {
    let mut profile = base(ConfigType::Tuic);
    profile.username = " user ".into();
    profile.password = " pass ".into();
    profile.network = "quic".into();
    profile.security.fingerprint = Some("chrome".into());
    profile.proto_extra.congestion_control = Some("bogus".into());

    let saved = save(profile).expect("tuic save");

    assert_eq!(saved.core_type, Some(CoreType::SingBox));
    assert_eq!(saved.address, "192.0.2.10");
    assert_eq!(saved.username, "user");
    assert_eq!(saved.password, "pass");
    assert_eq!(saved.network, "");
    assert!(saved.security.fingerprint.is_none());
    assert_eq!(
        saved.proto_extra.congestion_control.as_deref(),
        Some("cubic")
    );
    assert_eq!(saved.security.stream_security.as_deref(), Some("tls"));
    assert_eq!(saved.security.alpn.as_deref(), Some("h3"));
}

#[test]
fn tuic_import_applies_the_same_defaults() {
    let mut profile = base(ConfigType::Tuic);
    profile.index_id = String::new();
    profile.remarks = String::new();
    profile.address = String::new();
    profile.port = 0;
    profile.username = "u".into();
    profile.password = "p".into();

    let saved = import(profile).expect("tuic import");

    assert_eq!(saved.core_type, Some(CoreType::SingBox));
    assert_eq!(saved.security.stream_security.as_deref(), Some("tls"));
    assert_eq!(saved.security.alpn.as_deref(), Some("h3"));
    assert_eq!(
        saved.proto_extra.congestion_control.as_deref(),
        Some("cubic")
    );
}

#[test]
fn trojan_defaults_tls_and_trims_password() {
    let mut profile = base(ConfigType::Trojan);
    profile.password = " secret ".into();

    let saved = save(profile).expect("trojan save");

    assert_eq!(saved.password, "secret");
    assert_eq!(saved.security.stream_security.as_deref(), Some("tls"));
}

#[test]
fn hysteria2_clears_fingerprint_alpn_network() {
    let mut profile = base(ConfigType::Hysteria2);
    profile.password = "p".into();
    profile.network = "ws".into();
    profile.security.fingerprint = Some("chrome".into());
    profile.security.alpn = Some("h3".into());

    let saved = save(profile).expect("hysteria2 save");

    assert_eq!(saved.network, "");
    assert!(saved.security.fingerprint.is_none());
    assert!(saved.security.alpn.is_none());
    assert_eq!(saved.security.stream_security.as_deref(), Some("tls"));
}

#[test]
fn anytls_and_naive_force_singbox_and_clear_transport() {
    let mut anytls = base(ConfigType::Anytls);
    anytls.password = "p".into();
    anytls.network = "ws".into();
    let anytls = save(anytls).expect("anytls save");
    assert_eq!(anytls.core_type, Some(CoreType::SingBox));
    assert_eq!(anytls.network, "");
    assert_eq!(anytls.security.stream_security.as_deref(), Some("tls"));

    let mut naive = base(ConfigType::Naive);
    naive.username = "u".into();
    naive.password = "p".into();
    naive.network = "ws".into();
    naive.security.fingerprint = Some("chrome".into());
    naive.security.alpn = Some("h3".into());
    naive.security.allow_insecure = Some("true".into());
    let naive = save(naive).expect("naive save");
    assert_eq!(naive.core_type, Some(CoreType::SingBox));
    assert_eq!(naive.network, "");
    assert!(naive.security.fingerprint.is_none());
    assert!(naive.security.alpn.is_none());
    assert!(naive.security.allow_insecure.is_none());
    assert_eq!(naive.security.stream_security.as_deref(), Some("tls"));
}

#[test]
fn vless_defaults_encryption_and_flow() {
    let mut profile = base(ConfigType::Vless);
    profile.password = "11111111-2222-3333-4444-555555555555".into();
    profile.proto_extra.vless_encryption = Some("   ".into());
    profile.proto_extra.flow = Some("bogus".into());

    let saved = save(profile).expect("vless save");

    assert_eq!(saved.proto_extra.vless_encryption.as_deref(), Some("none"));
    assert_eq!(saved.proto_extra.flow.as_deref(), Some(""));
}

#[test]
fn vmess_rejects_invalid_security_then_missing_password() {
    let mut profile = base(ConfigType::Vmess);
    profile.password = "11111111-2222-3333-4444-555555555555".into();
    profile.proto_extra.vmess_security = Some("bogus".into());
    let error = save(profile).unwrap_err();
    assert_eq!(error.code, codes::FIELD_FORMAT);
    assert_eq!(error.field_path.as_deref(), Some("vmessSecurity"));

    let mut profile = base(ConfigType::Vmess);
    profile.password = "   ".into();
    profile.proto_extra.vmess_security = Some("auto".into());
    let error = save(profile).unwrap_err();
    assert_eq!(error.code, codes::FIELD_REQUIRED);
    assert_eq!(error.field_path.as_deref(), Some("password"));
}

#[test]
fn shadowsocks_rejects_invalid_method_then_missing_password() {
    let mut profile = base(ConfigType::Shadowsocks);
    profile.password = "secret".into();
    profile.proto_extra.ss_method = Some("bogus".into());
    let error = save(profile).unwrap_err();
    assert_eq!(error.code, codes::FIELD_FORMAT);
    assert_eq!(error.field_path.as_deref(), Some("ssMethod"));

    let mut profile = base(ConfigType::Shadowsocks);
    profile.password = String::new();
    profile.proto_extra.ss_method = Some("aes-256-gcm".into());
    let error = save(profile).unwrap_err();
    assert_eq!(error.code, codes::FIELD_REQUIRED);
    assert_eq!(error.field_path.as_deref(), Some("password"));
}

#[test]
fn import_normalizes_but_stays_tolerant_of_missing_password() {
    // FIX-04/PR-09: the import path must persist a node whose credential is
    // empty (honestly flagged invalid) rather than silently dropping it, while
    // still applying the upstream shape normalization.
    let mut profile = base(ConfigType::Trojan);
    profile.index_id = "re-prof-07-import".into();
    profile.remarks = "bare".into();
    profile.address = " 192.0.2.1 ".into();
    profile.port = 443;
    profile.password = String::new();

    let saved = import(profile).expect("import stays tolerant");

    assert_eq!(saved.address, "192.0.2.1");
    assert_eq!(saved.security.stream_security.as_deref(), Some("tls"));
    assert!(saved.password.is_empty(), "tolerant import keeps the node");
}

#[test]
fn common_tail_clears_invalid_security_and_network() {
    let mut profile = base(ConfigType::Vless);
    profile.password = "11111111-2222-3333-4444-555555555555".into();
    profile.network = "tcp".into();
    profile.security.stream_security = Some("TLS".into());

    let saved = save(profile).expect("vless save");

    assert_eq!(saved.network, "raw");
    assert!(saved.security.stream_security.is_none());
}
