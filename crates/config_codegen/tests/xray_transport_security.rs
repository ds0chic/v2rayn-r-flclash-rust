mod common;

use common::*;
use config_codegen::generate_xray;
use config_codegen::input::ConfigType;
use serde_json::json;

fn vless_with_network(network: &str) -> config_codegen::input::CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.50", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = network.into();
    p.proto_extra.vless_encryption = Some("none".into());
    p
}

#[test]
fn xray_raw_http_header() {
    let mut p = vless_with_network("raw");
    p.transport_extra.raw_header_type = Some("http".into());
    p.transport_extra.host = Some("raw.example.test".into());
    p.transport_extra.path = Some("/p".into());
    p.stream_security = "tls".into();
    let mut settings = settings();
    settings.core_basic.def_user_agent = Some("chrome".into());
    let mut input = codegen_input(p);
    input.settings = settings;

    let generated = generate_xray(&input).expect("raw");
    let main = &generated.main;
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/network"),
        "raw"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/rawSettings/header/type"),
        "http"
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/rawSettings/header/request/path"
        ),
        json!(["/p"])
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/rawSettings/header/request/headers/Host"
        ),
        json!(["raw.example.test"])
    );
    let ua = json_at(
        main,
        "/outbounds/0/streamSettings/rawSettings/header/request/headers/User-Agent",
    );
    let ua = ua
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        ua.starts_with("Mozilla/5.0"),
        "chrome UA mapping expected, got {ua}"
    );
}

#[test]
fn xray_httpupgrade_ws_xhttp() {
    let mut ws = vless_with_network("ws");
    ws.transport_extra.host = Some("ws.example.test".into());
    ws.transport_extra.path = Some("/ws".into());
    let mut settings = settings();
    settings.core_basic.def_user_agent = Some("curl".into());
    let mut input = codegen_input(ws);
    input.settings = settings;
    let generated = generate_xray(&input).expect("ws");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/streamSettings/network"),
        "ws"
    );
    assert_eq!(
        string_at(
            &generated.main,
            "/outbounds/0/streamSettings/wsSettings/host"
        ),
        "ws.example.test"
    );
    assert_eq!(
        string_at(
            &generated.main,
            "/outbounds/0/streamSettings/wsSettings/path"
        ),
        "/ws"
    );
    assert_eq!(
        string_at(
            &generated.main,
            "/outbounds/0/streamSettings/wsSettings/headers/User-Agent"
        ),
        "curl"
    );

    let mut hu = vless_with_network("httpupgrade");
    hu.transport_extra.host = Some("hu.example.test".into());
    hu.transport_extra.path = Some("/hu".into());
    let generated = generate_xray(&codegen_input(hu)).expect("httpupgrade");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/streamSettings/network"),
        "httpupgrade"
    );
    assert_eq!(
        string_at(
            &generated.main,
            "/outbounds/0/streamSettings/httpupgradeSettings/host"
        ),
        "hu.example.test"
    );

    let mut xhttp = vless_with_network("xhttp");
    xhttp.transport_extra.path = Some("/xh".into());
    xhttp.transport_extra.host = Some("xh.example.test".into());
    xhttp.transport_extra.xhttp_mode = Some("stream-up".into());
    xhttp.transport_extra.xhttp_extra =
        Some(r#"{"downloadSettings":{"sockopt":{"interface":"eth0"}}}"#.into());
    xhttp.mux_enabled = Some(true);
    let generated = generate_xray(&codegen_input(xhttp)).expect("xhttp");
    let main = &generated.main;
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/network"),
        "xhttp"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/xhttpSettings/mode"),
        "stream-up"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/xhttpSettings/path"),
        "/xh"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/xhttpSettings/host"),
        "xh.example.test"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/xhttpSettings/extra/downloadSettings/sockopt/interface"
        ),
        "eth0"
    );
    // xhttp always rewrites mux to disabled.
    assert_eq!(json_at(main, "/outbounds/0/mux/enabled"), json!(false));
}

#[test]
fn xray_kcp_header_and_seed_order() {
    let mut p = vless_with_network("kcp");
    p.transport_extra.kcp_header_type = Some("wechat-video".into());
    p.transport_extra.kcp_seed = Some("seed".into());
    p.transport_extra.kcp_mtu = Some(1350);
    let generated = generate_xray(&codegen_input(p)).expect("kcp");
    let main = &generated.main;
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/network"),
        "kcp"
    );
    assert_eq!(
        json_at(main, "/outbounds/0/streamSettings/kcpSettings/mtu"),
        json!(1350)
    );
    assert_eq!(
        json_at(main, "/outbounds/0/streamSettings/kcpSettings/tti"),
        json!(20)
    );
    // finalmask.udp is reversed: seed mask first, header mask second.
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/finalmask/udp/0/settings/value"
        ),
        "seed"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/finalmask/udp/1/settings/header"
        ),
        "wechat"
    );
}

#[test]
fn xray_grpc_reality() {
    let mut p = vless_with_network("grpc");
    p.stream_security = "reality".into();
    p.public_key = "SYNTHETIC_PUBLIC_KEY".into();
    p.short_id = "abcd".into();
    p.sni = "grpc.example.test".into();
    p.transport_extra.grpc_service_name = Some("svc".into());
    p.transport_extra.grpc_mode = Some("multi".into());
    p.transport_extra.grpc_authority = Some("authority.example.test".into());
    let mut settings = settings();
    settings.grpc.idle_timeout = Some(10);
    settings.grpc.health_check_timeout = Some(5);
    settings.grpc.permit_without_stream = Some(true);
    settings.grpc.initial_windows_size = Some(65535);
    let mut input = codegen_input(p);
    input.settings = settings;

    let generated = generate_xray(&input).expect("grpc reality");
    let main = &generated.main;
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/security"),
        "reality"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/grpcSettings/authority"),
        "authority.example.test"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/grpcSettings/serviceName"),
        "svc"
    );
    assert_eq!(
        json_at(main, "/outbounds/0/streamSettings/grpcSettings/multiMode"),
        json!(true)
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/grpcSettings/idle_timeout"
        ),
        json!(10)
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/grpcSettings/permit_without_stream"
        ),
        json!(true)
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/grpcSettings/initial_windows_size"
        ),
        json!(65535)
    );
}

#[test]
fn xray_tls_certificates_and_ech() {
    let pem = "-----BEGIN CERTIFICATE-----\nMIIBsynthetic\n-----END CERTIFICATE-----";
    let mut p = vless_with_network("raw");
    p.stream_security = "tls".into();
    p.cert = pem.into();
    p.ech_config_list = "AEXD+abc".into();
    p.verify_peer_cert_by_name = "verify.example.test".into();
    let generated = generate_xray(&codegen_input(p)).expect("tls");
    let main = &generated.main;
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/disableSystemRoot"
        ),
        json!(true)
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/certificates/0/certificate"
        ),
        json!([
            "-----BEGIN CERTIFICATE-----",
            "MIIBsynthetic",
            "-----END CERTIFICATE-----"
        ])
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/echConfigList"
        ),
        "AEXD+abc"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/echForceQuery"
        ),
        "full"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/verifyPeerCertByName"
        ),
        "verify.example.test"
    );
    assert!(!main.to_string().contains("allowInsecure"));
    assert!(!main.to_string().contains("insecure"));
}
