mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::input::ConfigType;
use serde_json::json;

#[test]
fn singbox_vmess_vless_ss() {
    let mut vmess = profile(ConfigType::Vmess, "192.0.2.20", 443);
    vmess.password = "11111111-2222-3333-4444-555555555555".into();
    vmess.proto_extra.alter_id = Some("2".into());
    vmess.proto_extra.vmess_security = Some("auto".into());
    let generated = generate_singbox(&codegen_input(vmess)).expect("vmess");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/type"), "vmess");
    assert_eq!(string_at(main, "/outbounds/0/server"), "192.0.2.20");
    assert_eq!(json_at(main, "/outbounds/0/server_port"), json!(443));
    assert_eq!(
        string_at(main, "/outbounds/0/uuid"),
        "11111111-2222-3333-4444-555555555555"
    );
    assert_eq!(json_at(main, "/outbounds/0/alter_id"), json!(2));
    assert_eq!(string_at(main, "/outbounds/0/security"), "auto");

    let mut vless = profile(ConfigType::Vless, "192.0.2.21", 443);
    vless.password = "11111111-2222-3333-4444-555555555555".into();
    vless.proto_extra.flow = Some("xtls-rprx-vision-udp443".into());
    let generated = generate_singbox(&codegen_input(vless)).expect("vless");
    assert_eq!(string_at(&generated.main, "/outbounds/0/type"), "vless");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/packet_encoding"),
        "xudp"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/flow"),
        "xtls-rprx-vision"
    );

    let mut ss = profile(ConfigType::Shadowsocks, "192.0.2.22", 8388);
    ss.password = "synthetic-pass".into();
    ss.proto_extra.ss_method = Some("2022-blake3-aes-256-gcm".into());
    ss.proto_extra.uot = Some(true);
    let generated = generate_singbox(&codegen_input(ss)).expect("ss");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/type"),
        "shadowsocks"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/method"),
        "2022-blake3-aes-256-gcm"
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/udp_over_tcp"),
        json!(true)
    );
}

#[test]
fn singbox_socks_http_trojan() {
    let mut socks = profile(ConfigType::Socks, "192.0.2.23", 11830);
    socks.username = "user".into();
    socks.password = "pass".into();
    let generated = generate_singbox(&codegen_input(socks)).expect("socks");
    assert_eq!(string_at(&generated.main, "/outbounds/0/type"), "socks");
    assert_eq!(string_at(&generated.main, "/outbounds/0/version"), "5");
    assert_eq!(string_at(&generated.main, "/outbounds/0/username"), "user");

    let mut http = profile(ConfigType::Http, "192.0.2.24", 11831);
    http.username = "user".into();
    http.password = "pass".into();
    let generated = generate_singbox(&codegen_input(http)).expect("http");
    assert_eq!(string_at(&generated.main, "/outbounds/0/type"), "http");
    assert_eq!(string_at(&generated.main, "/outbounds/0/password"), "pass");

    let mut trojan = profile(ConfigType::Trojan, "192.0.2.25", 443);
    trojan.password = "trojan-pass".into();
    let generated = generate_singbox(&codegen_input(trojan)).expect("trojan");
    assert_eq!(string_at(&generated.main, "/outbounds/0/type"), "trojan");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/password"),
        "trojan-pass"
    );
}

#[test]
fn singbox_tuic_anytls_naive() {
    let mut tuic = profile(ConfigType::Tuic, "192.0.2.30", 443);
    tuic.username = "11111111-2222-3333-4444-555555555555".into();
    tuic.password = "tuic-pass".into();
    tuic.proto_extra.congestion_control = Some("bbr".into());
    let generated = generate_singbox(&codegen_input(tuic)).expect("tuic");
    assert_eq!(string_at(&generated.main, "/outbounds/0/type"), "tuic");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/uuid"),
        "11111111-2222-3333-4444-555555555555"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/password"),
        "tuic-pass"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/congestion_control"),
        "bbr"
    );

    let mut anytls = profile(ConfigType::Anytls, "192.0.2.31", 443);
    anytls.password = "anytls-pass".into();
    let generated = generate_singbox(&codegen_input(anytls)).expect("anytls");
    assert_eq!(string_at(&generated.main, "/outbounds/0/type"), "anytls");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/password"),
        "anytls-pass"
    );

    let mut naive = profile(ConfigType::Naive, "192.0.2.32", 443);
    naive.username = "user".into();
    naive.password = "pass".into();
    naive.proto_extra.naive_quic = Some(true);
    naive.proto_extra.congestion_control = Some("bbr".into());
    naive.proto_extra.insecure_concurrency = Some(4);
    let generated = generate_singbox(&codegen_input(naive)).expect("naive");
    assert_eq!(string_at(&generated.main, "/outbounds/0/type"), "naive");
    assert_eq!(json_at(&generated.main, "/outbounds/0/quic"), json!(true));
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/quic_congestion_control"),
        "bbr"
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/insecure_concurrency"),
        json!(4)
    );
}

#[test]
fn singbox_hysteria2_ports_and_obfs() {
    let mut hy2 = profile(ConfigType::Hysteria2, "192.0.2.33", 443);
    hy2.password = "hy2-pass".into();
    hy2.proto_extra.ports = Some("20000-30000,40000".into());
    hy2.proto_extra.up_mbps = Some(50);
    hy2.proto_extra.salamander_pass = Some("pw".into());
    let generated = generate_singbox(&codegen_input(hy2)).expect("hy2");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/type"), "hysteria2");
    assert!(json_at(main, "/outbounds/0/server_port").is_null());
    assert_eq!(
        json_at(main, "/outbounds/0/server_ports"),
        json!(["20000:30000", "40000:40000"])
    );
    assert_eq!(string_at(main, "/outbounds/0/hop_interval"), "30s");
    assert_eq!(string_at(main, "/outbounds/0/obfs/type"), "salamander");
    assert_eq!(string_at(main, "/outbounds/0/obfs/password"), "pw");
    assert_eq!(json_at(main, "/outbounds/0/up_mbps"), json!(50));
    assert!(json_at(main, "/outbounds/0/down_mbps").is_null());
}

#[test]
fn singbox_wireguard_endpoint() {
    let mut wg = profile(ConfigType::WireGuard, "192.0.2.40", 51820);
    wg.password = "synthetic-private-key".into();
    wg.proto_extra.wg_public_key = Some("synthetic-public-key".into());
    wg.proto_extra.wg_interface_address = Some("10.0.0.2/32".into());
    wg.proto_extra.wg_mtu = Some(1420);
    wg.proto_extra.wg_reserved = Some("1,2".into());
    let generated = generate_singbox(&codegen_input(wg)).expect("wg");
    let main = &generated.main;
    assert_eq!(string_at(main, "/endpoints/0/type"), "wireguard");
    assert_eq!(
        string_at(main, "/endpoints/0/private_key"),
        "synthetic-private-key"
    );
    assert_eq!(json_at(main, "/endpoints/0/mtu"), json!(1420));
    assert_eq!(
        json_at(main, "/endpoints/0/address"),
        json!(["10.0.0.2/32"])
    );
    assert_eq!(
        string_at(main, "/endpoints/0/peers/0/address"),
        "192.0.2.40"
    );
    assert_eq!(json_at(main, "/endpoints/0/peers/0/port"), json!(51820));
    assert_eq!(
        json_at(main, "/endpoints/0/peers/0/reserved"),
        json!([1, 2])
    );
    assert_eq!(
        json_at(main, "/endpoints/0/peers/0/allowed_ips"),
        json!(["0.0.0.0/0", "::/0"])
    );
    let outbounds = json_at(main, "/outbounds");
    let has_wg = outbounds
        .as_array()
        .map(|list| {
            list.iter()
                .any(|o| o.get("type").and_then(|v| v.as_str()) == Some("wireguard"))
        })
        .unwrap_or(false);
    assert!(!has_wg, "wireguard must be an endpoint, not an outbound");
    assert_no_live_port(main);
}
