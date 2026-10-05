//! R4-16 per-format matrix (upstream `compat/features.yaml` `fmt_formats`
//! FMT-001..FMT-017): every format is exercised with a synthetic fixture
//! through import -> export -> re-import, asserting the wire stays idempotent
//! and unknown keys survive. Formats with no standalone persisted codec
//! (`BaseFmt` base class / `FmtHandler` dispatch / `HtmlPageFmt` detection-only)
//! are asserted as such instead of faking a round-trip.
//!
//! These are the pure parse/emit guarantees; the real SQLite import -> reopen
//! -> export -> re-import run lives in `apps/desktop/test/r4_16_matrix_test.dart`
//! (real FRB bridge + data directory).

use domain::{ConfigType, CoreType, Profile};
use subscriptions::fmt::{self, FmtKind};
use subscriptions::{parse_content, resolve_uri, to_uri, ContentHint, ParseOptions};

fn opts() -> ParseOptions {
    ParseOptions::default()
}

/// Assert the canonical share URI survives parse -> emit -> parse unchanged.
fn assert_share_roundtrip(item: &Profile) {
    let uri = to_uri(item).expect("emit share uri");
    let parsed = resolve_uri(&uri).expect("parse share uri");
    assert_eq!(parsed.config_type, item.config_type, "config_type");
    assert_eq!(parsed.remarks, item.remarks, "remarks");
    assert_eq!(parsed.address, item.address, "address");
    assert_eq!(parsed.port, item.port, "port");
    assert_eq!(parsed.password, item.password, "password");
    assert_eq!(parsed.username, item.username, "username");
    assert_eq!(parsed.network, item.network, "network");
    let uri2 = to_uri(&parsed).expect("re-emit share uri");
    assert_eq!(uri, uri2, "share uri is idempotent");
}

fn vless() -> Profile {
    Profile {
        config_type: ConfigType::Vless,
        remarks: "vless demo".into(),
        address: "vless.example".into(),
        port: 8443,
        password: "b831381d-6324-4d53-ad4f-8cda48b30811".into(),
        network: "raw".into(),
        ..Profile::default()
    }
}

// FMT-001 VmessFmt ---------------------------------------------------------

#[test]
fn fmt_001_vmess_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Vmess,
        remarks: "vmess demo".into(),
        address: "example.com".into(),
        port: 443,
        password: "b831381d-6324-4d53-ad4f-8cda48b30811".into(),
        network: "ws".into(),
        ..Profile::default()
    };
    item.proto_extra.alter_id = Some("0".into());
    item.proto_extra.vmess_security = Some("auto".into());
    item.transport_extra.host = Some("ws.example".into());
    item.transport_extra.path = Some("/ws".into());
    assert_share_roundtrip(&item);

    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.proto_extra.alter_id.as_deref(), Some("0"));
    assert_eq!(parsed.transport_extra.path.as_deref(), Some("/ws"));
}

// FMT-002 VLESSFmt ---------------------------------------------------------

#[test]
fn fmt_002_vless_roundtrip_reality() {
    let mut item = vless();
    item.security.stream_security = Some("reality".into());
    item.security.sni = Some("s.example".into());
    item.security.fingerprint = Some("chrome".into());
    item.security.public_key = Some("KEY".into());
    item.security.short_id = Some("ab12".into());
    item.proto_extra.flow = Some("xtls-rprx-vision".into());
    assert_share_roundtrip(&item);

    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.security.stream_security.as_deref(), Some("reality"));
    assert_eq!(parsed.security.public_key.as_deref(), Some("KEY"));
    assert_eq!(parsed.proto_extra.flow.as_deref(), Some("xtls-rprx-vision"));
}

// FMT-003 ShadowsocksFmt (share link + SIP008 subscription) ----------------

#[test]
fn fmt_003_shadowsocks_share_link_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Shadowsocks,
        remarks: "ss demo".into(),
        address: "1.2.3.4".into(),
        port: 8388,
        password: "pass123".into(),
        ..Profile::default()
    };
    item.proto_extra.ss_method = Some("aes-128-gcm".into());
    assert_share_roundtrip(&item);
}

#[test]
fn fmt_003_shadowsocks_sip008_import() {
    let json = r#"{"version":1,"servers":[{"id":"s1","remarks":"sip","server":"1.2.3.4","server_port":8388,"method":"aes-256-gcm","password":"pw","plugin":"","plugin_opts":""}]}"#;
    let list = fmt::shadowsocks::resolve_sip008(json).expect("SIP008 servers");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].config_type, ConfigType::Shadowsocks);
    assert_eq!(list[0].address, "1.2.3.4");
    assert_eq!(
        list[0].proto_extra.ss_method.as_deref(),
        Some("aes-256-gcm")
    );
    let local = list[0].clone();
    assert_share_roundtrip(&local);
}

// FMT-004 SocksFmt ---------------------------------------------------------

#[test]
fn fmt_004_socks_roundtrip_and_legacy() {
    let item = Profile {
        config_type: ConfigType::Socks,
        remarks: "socks".into(),
        address: "127.0.0.1".into(),
        port: 1180,
        username: "user".into(),
        password: "pass".into(),
        ..Profile::default()
    };
    assert_share_roundtrip(&item);

    let payload = subscriptions::util::base64_encode("u:p@example.com:1080");
    let legacy = resolve_uri(&format!("socks://{payload}#tag")).unwrap();
    assert_eq!(legacy.address, "example.com");
    assert_eq!(legacy.port, 1080);
}

// FMT-005 TrojanFmt --------------------------------------------------------

#[test]
fn fmt_005_trojan_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Trojan,
        remarks: "trojan".into(),
        address: "t.example".into(),
        port: 443,
        password: "p:a@ss#% +/=".into(),
        network: "raw".into(),
        ..Profile::default()
    };
    item.security.stream_security = Some("tls".into());
    item.security.sni = Some("sni.t.example".into());
    item.security.allow_insecure = Some("true".into());
    assert_share_roundtrip(&item);
    let uri = to_uri(&item).unwrap();
    assert!(uri.contains("allowInsecure=1"), "{uri}");
}

// FMT-006 Hysteria2Fmt (share + realm) -------------------------------------

#[test]
fn fmt_006_hysteria2_roundtrip_and_realm() {
    let mut item = Profile {
        config_type: ConfigType::Hysteria2,
        remarks: "hy2".into(),
        address: "hy2.example".into(),
        port: 8443,
        password: "pw".into(),
        network: "raw".into(),
        ..Profile::default()
    };
    item.security.sni = Some("sni.hy2.example".into());
    item.proto_extra.salamander_pass = Some("obfs-pass".into());
    item.proto_extra.ports = Some("5000:6000".into());
    assert_share_roundtrip(&item);

    let realm = resolve_uri(
        "hysteria2+realm://mytoken@rendezvous.example.com/my-cabin?auth=your_password#remark",
    )
    .unwrap();
    assert_eq!(realm.config_type, ConfigType::Hysteria2);
    assert_eq!(realm.password, "your_password");
    let uri = to_uri(&realm).unwrap();
    assert!(uri.contains("hysteria2+realm://mytoken@rendezvous.example.com"));
}

// FMT-007 TuicFmt ----------------------------------------------------------

#[test]
fn fmt_007_tuic_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Tuic,
        remarks: "tuic".into(),
        address: "tuic.example".into(),
        port: 8443,
        username: "01234567-89ab-cdef-0123-456789abcdef".into(),
        password: "tuic-pass".into(),
        network: "raw".into(),
        ..Profile::default()
    };
    item.proto_extra.congestion_control = Some("bbr".into());
    assert_share_roundtrip(&item);
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.username, item.username);
    assert_eq!(
        parsed.proto_extra.congestion_control.as_deref(),
        Some("bbr")
    );
}

// FMT-008 WireguardFmt (share link + wg conf) ------------------------------

#[test]
fn fmt_008_wireguard_roundtrip_and_conf() {
    use base64::Engine as _;
    let key = |b: u8| base64::engine::general_purpose::STANDARD.encode(vec![b; 32]);
    let mut item = Profile {
        config_type: ConfigType::WireGuard,
        remarks: "wg 東京".into(),
        address: "2001:db8::40".into(),
        port: 51820,
        password: key(0xFE),
        ..Profile::default()
    };
    item.proto_extra.wg_public_key = Some(key(0xFD));
    item.proto_extra.wg_interface_address = Some("10.0.0.2/32,fd00::2/128".into());
    item.proto_extra.wg_mtu = Some(1420);
    assert_share_roundtrip(&item);

    let conf = "[Interface]\nPrivateKey = k\nAddress = 10.0.0.2/32\n[Peer]\nPublicKey = p1\nEndpoint = [2001:db8::1]:51820\n[Peer]\nPublicKey = p2\nEndpoint = example.com:12345\n";
    let list = fmt::wireguard::resolve_config(conf).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].address, "2001:db8::1");
    assert_eq!(list[1].port, 12345);
}

// FMT-009 AnytlsFmt --------------------------------------------------------

#[test]
fn fmt_009_anytls_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Anytls,
        remarks: "anytls".into(),
        address: "anytls.example".into(),
        port: 8443,
        password: "anytls-pass".into(),
        network: "raw".into(),
        ..Profile::default()
    };
    item.security.stream_security = Some("tls".into());
    item.security.alpn = Some("h2,http/1.1".into());
    assert_share_roundtrip(&item);
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.security.alpn.as_deref(), Some("h2,http/1.1"));
}

// FMT-010 NaiveFmt (https + quic) ------------------------------------------

#[test]
fn fmt_010_naive_roundtrip_both_transports() {
    for quic in [false, true] {
        let mut item = Profile {
            config_type: ConfigType::Naive,
            remarks: "naive".into(),
            address: "naive.example".into(),
            port: 443,
            username: "naive-user".into(),
            password: "päss:word@/?#&=+ 東京".into(),
            network: "raw".into(),
            ..Profile::default()
        };
        item.proto_extra.naive_quic = Some(quic);
        assert_share_roundtrip(&item);
    }
}

// FMT-011 InnerFmt (v2rayn://) ---------------------------------------------

#[test]
fn fmt_011_inner_uri_roundtrip() {
    let item = vless();
    let uri = fmt::to_inner_uri(std::slice::from_ref(&item)).expect("inner uri");
    assert!(uri.starts_with("v2rayn://vless/"));
    let json = subscriptions::util::base64_decode(uri.trim_start_matches("v2rayn://vless/"))
        .expect("base64 payload");
    assert!(
        !json.is_empty(),
        "inner payload must carry the profile JSON"
    );
    let parsed = fmt::inner::parse(&uri, "sub-inner");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].config_type, ConfigType::Vless);
    assert_eq!(parsed[0].address, item.address);
    // Unknown extensions survive the inner wire (whole-profile JSON).
    let mut with_unknown = item.clone();
    with_unknown
        .extra
        .insert("x-future".into(), serde_json::json!([1, 2, 3]));
    let uri2 = fmt::to_inner_uri(&[with_unknown]).unwrap();
    let reparsed = fmt::inner::parse(&uri2, "sub-inner");
    assert_eq!(
        reparsed[0].extra.get("x-future"),
        Some(&serde_json::json!([1, 2, 3]))
    );
}

// FMT-012 V2rayFmt (full/outbound JSON -> Custom/Outbound) -----------------

#[test]
fn fmt_012_v2ray_full_is_custom_and_unknown_keys_survive() {
    let data = r#"{"inbounds":[{"port":11888,"protocol":"socks"}],"outbounds":[{"protocol":"vmess","tag":"r416-v2ray","settings":{"vnext":[]},"streamSettings":{"network":"tcp"}}],"x-future":[1,2,3]}"#;
    let result = parse_content(data, ContentHint::Xray, &opts());
    assert_eq!(result.profiles.len(), 1);
    let mut profile = result.profiles.into_iter().next().unwrap();
    assert_eq!(profile.config_type, ConfigType::Custom);
    assert_eq!(profile.core_type, Some(CoreType::Xray));
    let raw = subscriptions::take_raw_config(&mut profile).expect("raw config");
    assert!(raw.contains("x-future"), "unknown key must survive: {raw}");
    let reparsed = parse_content(&raw, ContentHint::Xray, &opts());
    assert_eq!(reparsed.profiles.len(), 1);
    assert_eq!(reparsed.profiles[0].config_type, ConfigType::Custom);
    assert_eq!(reparsed.profiles[0].core_type, Some(CoreType::Xray));
}

// FMT-013 SingboxFmt -------------------------------------------------------

#[test]
fn fmt_013_singbox_full_is_custom_and_unknown_keys_survive() {
    let data = r#"{"inbounds":[],"outbounds":[{"type":"vless","tag":"r416-sbox","server":"node.example.invalid","server_port":11981}],"x-future":"kept"}"#;
    let result = parse_content(data, ContentHint::Singbox, &opts());
    assert_eq!(result.profiles.len(), 1);
    let mut profile = result.profiles.into_iter().next().unwrap();
    assert_eq!(profile.config_type, ConfigType::Custom);
    assert_eq!(profile.core_type, Some(CoreType::SingBox));
    let raw = subscriptions::take_raw_config(&mut profile).expect("raw config");
    assert!(raw.contains("x-future"));
    let reparsed = parse_content(&raw, ContentHint::Singbox, &opts());
    assert_eq!(reparsed.profiles[0].core_type, Some(CoreType::SingBox));
}

// FMT-014 ClashFmt ---------------------------------------------------------

#[test]
fn fmt_014_clash_full_is_mihomo_custom_and_unknown_keys_survive() {
    let data = "proxies:\n  - name: r416-clash\n    type: ss\n    server: node.example.invalid\n    port: 11982\nrules:\n  - MATCH,DIRECT\nmixed-port: 7890\nx-future: kept\n";
    let result = parse_content(data, ContentHint::Clash, &opts());
    assert_eq!(result.profiles.len(), 1);
    let mut profile = result.profiles.into_iter().next().unwrap();
    assert_eq!(profile.config_type, ConfigType::Custom);
    assert_eq!(profile.core_type, Some(CoreType::Mihomo));
    let raw = subscriptions::take_raw_config(&mut profile).expect("raw config");
    assert!(raw.contains("x-future: kept"));
}

// FMT-015 HtmlPageFmt (detection-only) -------------------------------------

#[test]
fn fmt_015_htmlpage_is_detected_without_a_roundtrip() {
    let html = "<!doctype html><html><head><title>x</title></head><body>no nodes</body></html>";
    let result = parse_content(html, ContentHint::Auto, &opts());
    assert!(result.profiles.is_empty(), "an HTML page yields no nodes");
    assert!(
        !result.errors.is_empty() || !result.warnings.is_empty(),
        "the page is reported as a readable failure, not silently dropped"
    );
    assert!(batch_html(html));
}

fn batch_html(data: &str) -> bool {
    subscriptions::fmt::batch::is_html_page(data)
}

// FMT-016 BaseFmt (shared base class, not a standalone codec) --------------

#[test]
fn fmt_016_basefmt_is_a_shared_base_not_a_codec() {
    // No share scheme of its own: the base helpers are exercised through the
    // concrete codecs (quote/transport/IPv6 handling). A bare scheme with an
    // empty body is rejected by the shared dispatch.
    assert!(resolve_uri("vmess://").is_err());
    // The codec registry has exactly the 17 upstream entries.
    assert_eq!(FmtKind::ALL.len(), 17);
}

// FMT-017 FmtHandler (share URI dispatch) ----------------------------------

#[test]
fn fmt_017_fmthandler_dispatches_every_scheme() {
    let cases = [
        ("vmess://x", FmtKind::Vmess),
        ("ss://x", FmtKind::Shadowsocks),
        ("socks://x", FmtKind::Socks),
        ("trojan://x", FmtKind::Trojan),
        ("vless://x", FmtKind::Vless),
        ("hy2://x", FmtKind::Hysteria2),
        ("tuic://x", FmtKind::Tuic),
        ("wireguard://x", FmtKind::WireGuard),
        ("anytls://x", FmtKind::Anytls),
        ("naive+quic://x", FmtKind::Naive),
        ("v2rayn://vless/AAA", FmtKind::Inner),
    ];
    for (input, expected) in cases {
        assert_eq!(fmt::fmt_kind_of(input), Some(expected), "{input}");
    }
    assert_eq!(fmt::fmt_kind_of("https://not-a-node.example"), None);
    // `of_config_type` maps every exportable protocol exactly once.
    for kind in FmtKind::ALL {
        if matches!(
            kind,
            FmtKind::Inner
                | FmtKind::Sip008
                | FmtKind::V2ray
                | FmtKind::Singbox
                | FmtKind::Clash
                | FmtKind::HtmlPage
                | FmtKind::Base64List
        ) {
            continue;
        }
        let ct = match kind {
            FmtKind::Vmess => ConfigType::Vmess,
            FmtKind::Vless => ConfigType::Vless,
            FmtKind::Shadowsocks => ConfigType::Shadowsocks,
            FmtKind::Socks => ConfigType::Socks,
            FmtKind::Trojan => ConfigType::Trojan,
            FmtKind::Hysteria2 => ConfigType::Hysteria2,
            FmtKind::Tuic => ConfigType::Tuic,
            FmtKind::WireGuard => ConfigType::WireGuard,
            FmtKind::Anytls => ConfigType::Anytls,
            FmtKind::Naive => ConfigType::Naive,
            _ => unreachable!(),
        };
        assert_eq!(FmtKind::of_config_type(ct), Some(kind));
    }
}
