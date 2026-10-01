mod common;

use common::*;
use config_codegen::generate_xray;
use config_codegen::input::ConfigType;
use serde_json::json;

#[test]
fn xray_vmess_ws_tls() {
    let mut p = profile(ConfigType::Vmess, "192.0.2.10", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.proto_extra.alter_id = Some("4".into());
    p.proto_extra.vmess_security = Some("chacha20-poly1305".into());
    p.network = "ws".into();
    p.transport_extra.host = Some("ws.test".into());
    p.transport_extra.path = Some("/p".into());
    p.stream_security = "tls".into();
    p.cert_sha = "aabbcc".into();
    p.alpn = "h2".into();
    p.sni = "ws.test".into();
    p.mux_enabled = Some(true);

    let generated = generate_xray(&codegen_input(p)).expect("generate");
    let main = &generated.main;

    assert_eq!(string_at(main, "/outbounds/0/protocol"), "vmess");
    assert_eq!(
        string_at(main, "/outbounds/0/settings/id"),
        "11111111-2222-3333-4444-555555555555"
    );
    assert_eq!(json_at(main, "/outbounds/0/settings/alterId"), json!(4));
    assert_eq!(
        string_at(main, "/outbounds/0/settings/security"),
        "chacha20-poly1305"
    );
    assert_eq!(string_at(main, "/outbounds/0/settings/email"), "t@t.tt");
    assert_eq!(string_at(main, "/outbounds/0/streamSettings/network"), "ws");
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/wsSettings/host"),
        "ws.test"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/wsSettings/path"),
        "/p"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/security"),
        "tls"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/pinnedPeerCertSha256"
        ),
        "aabbcc"
    );
    assert_eq!(
        json_at(main, "/outbounds/0/streamSettings/tlsSettings/alpn"),
        json!(["h2"])
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/tlsSettings/serverName"),
        "ws.test"
    );
    assert_eq!(json_at(main, "/outbounds/0/mux/enabled"), json!(true));
    assert_eq!(json_at(main, "/outbounds/0/mux/concurrency"), json!(8));
    assert!(!main.to_string().contains("allowInsecure"));
    assert_no_live_port(main);
}

#[test]
fn xray_vless_reality_raw() {
    let mut p = profile(ConfigType::Vless, "192.0.2.10", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = "raw".into();
    p.stream_security = "reality".into();
    p.public_key = "SYNTHETIC_PUBLIC_KEY".into();
    p.short_id = "0a1b2c".into();
    p.sni = "example.test".into();
    p.fingerprint = "chrome".into();
    p.proto_extra.vless_encryption = Some("none".into());
    p.proto_extra.flow = Some(String::new());

    let generated = generate_xray(&codegen_input(p)).expect("generate");
    let main = &generated.main;

    assert_eq!(string_at(main, "/outbounds/0/protocol"), "vless");
    assert_eq!(string_at(main, "/outbounds/0/settings/encryption"), "none");
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/network"),
        "raw"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/security"),
        "reality"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/realitySettings/publicKey"
        ),
        "SYNTHETIC_PUBLIC_KEY"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/realitySettings/shortId"),
        "0a1b2c"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/realitySettings/serverName"
        ),
        "example.test"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/realitySettings/fingerprint"
        ),
        "chrome"
    );
    assert_eq!(
        json_at(main, "/outbounds/0/streamSettings/realitySettings/show"),
        json!(false)
    );
    assert!(!main.to_string().contains("allowInsecure"));
}

#[test]
fn xray_shadowsocks_socks_http() {
    let mut ss = profile(ConfigType::Shadowsocks, "192.0.2.11", 8388);
    ss.password = "synthetic-pass".into();
    ss.proto_extra.ss_method = Some("aes-256-gcm".into());
    ss.proto_extra.uot = Some(true);
    let generated = generate_xray(&codegen_input(ss)).expect("ss");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/protocol"),
        "shadowsocks"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/settings/method"),
        "aes-256-gcm"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/settings/password"),
        "synthetic-pass"
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/settings/uot"),
        json!(true)
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/settings/ota"),
        json!(false)
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/settings/level"),
        json!(1)
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/mux/enabled"),
        json!(false)
    );

    let mut socks = profile(ConfigType::Socks, "192.0.2.12", 11820);
    socks.username = "user1".into();
    socks.password = "pass1".into();
    let generated = generate_xray(&codegen_input(socks)).expect("socks");
    assert_eq!(string_at(&generated.main, "/outbounds/0/protocol"), "socks");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/settings/user"),
        "user1"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/settings/pass"),
        "pass1"
    );

    let mut http = profile(ConfigType::Http, "192.0.2.13", 11821);
    http.username = "user2".into();
    http.password = "pass2".into();
    http.proto_extra.http_headers = Some(r#"{"X-Test": "1"}"#.into());
    let generated = generate_xray(&codegen_input(http)).expect("http");
    assert_eq!(string_at(&generated.main, "/outbounds/0/protocol"), "http");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/settings/headers/X-Test"),
        "1"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/settings/user"),
        "user2"
    );
}

#[test]
fn xray_trojan_and_hysteria2() {
    let mut trojan = profile(ConfigType::Trojan, "192.0.2.14", 443);
    trojan.password = "trojan-pass".into();
    let generated = generate_xray(&codegen_input(trojan)).expect("trojan");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/protocol"),
        "trojan"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/settings/password"),
        "trojan-pass"
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/settings/ota"),
        json!(false)
    );

    let mut hy2 = profile(ConfigType::Hysteria2, "192.0.2.15", 443);
    hy2.password = "hy2-pass".into();
    hy2.proto_extra.ports = Some("20000-30000,40000".into());
    hy2.proto_extra.up_mbps = Some(50);
    hy2.proto_extra.down_mbps = Some(0);
    hy2.proto_extra.salamander_pass = Some("pw".into());
    let generated = generate_xray(&codegen_input(hy2)).expect("hy2");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/protocol"), "hysteria");
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/network"),
        "hysteria"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/hysteriaSettings/auth"),
        "hy2-pass"
    );
    assert_eq!(
        json_at(main, "/outbounds/0/streamSettings/hysteriaSettings/version"),
        json!(2)
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/finalmask/quicParams/udpHop/ports"
        ),
        "20000-30000,40000"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/finalmask/quicParams/congestion"
        ),
        "brutal"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/finalmask/quicParams/brutalUp"
        ),
        "50mbps"
    );
    assert!(json_at(
        main,
        "/outbounds/0/streamSettings/finalmask/quicParams/brutalDown"
    )
    .is_null());
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/finalmask/udp/0/type"),
        "salamander"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/finalmask/udp/0/settings/password"
        ),
        "pw"
    );
}

#[test]
fn xray_wireguard() {
    let mut wg = profile(ConfigType::WireGuard, "192.0.2.10", 443);
    wg.password = "synthetic-private-key".into();
    wg.proto_extra.wg_public_key = Some("synthetic-public-key".into());
    wg.proto_extra.wg_preshared_key = Some("synthetic-psk".into());
    wg.proto_extra.wg_interface_address = Some("10.0.0.2/32,fd00::2/128".into());
    wg.proto_extra.wg_reserved = Some("1,2".into());
    wg.proto_extra.wg_mtu = Some(1420);
    wg.proto_extra.wg_dns = Some("1.1.1.1,8.8.8.8".into());

    let generated = generate_xray(&codegen_input(wg)).expect("wg");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/protocol"), "wireguard");
    assert_eq!(
        json_at(main, "/outbounds/0/settings/address"),
        json!(["10.0.0.2/32", "fd00::2/128"])
    );
    assert_eq!(
        string_at(main, "/outbounds/0/settings/secretKey"),
        "synthetic-private-key"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/settings/peers/0/endpoint"),
        "192.0.2.10:443"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/settings/peers/0/publicKey"),
        "synthetic-public-key"
    );
    assert_eq!(json_at(main, "/outbounds/0/settings/mtu"), json!(1420));
    assert_eq!(
        json_at(main, "/outbounds/0/settings/reserved"),
        json!([1, 2])
    );
    assert_eq!(
        json_at(main, "/outbounds/0/settings/remoteDNS"),
        json!(["1.1.1.1", "8.8.8.8"])
    );
    assert!(json_at(main, "/outbounds/0/mux").is_null());
}
