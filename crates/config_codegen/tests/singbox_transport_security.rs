mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::input::ConfigType;
use serde_json::json;

fn vless_with_network(network: &str) -> config_codegen::input::CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.60", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = network.into();
    p
}

#[test]
fn singbox_ws_early_data() {
    let mut p = vless_with_network("ws");
    p.transport_extra.host = Some("ws.test".into());
    p.transport_extra.path = Some("/p?ed=2048".into());
    let mut settings = settings();
    settings.core_basic.def_user_agent = Some("chrome".into());
    let mut input = codegen_input(p);
    input.settings = settings;

    let generated = generate_singbox(&input).expect("ws");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/transport/type"), "ws");
    assert_eq!(string_at(main, "/outbounds/0/transport/path"), "/p");
    assert_eq!(
        json_at(main, "/outbounds/0/transport/max_early_data"),
        json!(2048)
    );
    assert_eq!(
        string_at(main, "/outbounds/0/transport/early_data_header_name"),
        "Sec-WebSocket-Protocol"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/transport/headers/Host"),
        "ws.test"
    );
    assert!(string_at(main, "/outbounds/0/transport/headers/User-Agent").starts_with("Mozilla/5.0"));
}

#[test]
fn singbox_raw_http_httpupgrade_grpc() {
    let mut raw = profile(ConfigType::Vmess, "192.0.2.61", 443);
    raw.password = "11111111-2222-3333-4444-555555555555".into();
    raw.network = "raw".into();
    raw.transport_extra.raw_header_type = Some("http".into());
    raw.transport_extra.host = Some("raw.test".into());
    raw.transport_extra.path = Some("/r".into());
    let generated = generate_singbox(&codegen_input(raw)).expect("raw");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/type"),
        "http"
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/transport/host"),
        json!(["raw.test"])
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/path"),
        "/r"
    );

    let mut hu = vless_with_network("httpupgrade");
    hu.transport_extra.host = Some("hu.test".into());
    hu.transport_extra.path = Some("/hu".into());
    let generated = generate_singbox(&codegen_input(hu)).expect("hu");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/type"),
        "httpupgrade"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/path"),
        "/hu"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/host"),
        "hu.test"
    );

    let mut grpc = vless_with_network("grpc");
    grpc.transport_extra.grpc_service_name = Some("svc".into());
    let mut settings = settings();
    settings.grpc.idle_timeout = Some(10);
    settings.grpc.health_check_timeout = Some(5);
    settings.grpc.permit_without_stream = Some(true);
    let mut input = codegen_input(grpc);
    input.settings = settings;
    let generated = generate_singbox(&input).expect("grpc");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/type"),
        "grpc"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/service_name"),
        "svc"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/idle_timeout"),
        "10s"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/transport/ping_timeout"),
        "5s"
    );
    assert_eq!(
        json_at(
            &generated.main,
            "/outbounds/0/transport/permit_without_stream"
        ),
        json!(true)
    );
}

#[test]
fn singbox_shadowsocks_plugin() {
    let mut obfs = profile(ConfigType::Shadowsocks, "192.0.2.62", 8388);
    obfs.password = "synthetic-pass".into();
    obfs.proto_extra.ss_method = Some("aes-256-gcm".into());
    obfs.network = "raw".into();
    obfs.transport_extra.raw_header_type = Some("http".into());
    obfs.transport_extra.host = Some("obfs.test".into());
    let generated = generate_singbox(&codegen_input(obfs)).expect("obfs");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/plugin"),
        "obfs-local"
    );
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/plugin_opts"),
        "obfs=http;obfs-host=obfs.test;"
    );

    let mut v2 = profile(ConfigType::Shadowsocks, "192.0.2.63", 8389);
    v2.password = "synthetic-pass".into();
    v2.proto_extra.ss_method = Some("aes-256-gcm".into());
    v2.network = "ws".into();
    v2.transport_extra.host = Some("plugin.test".into());
    v2.transport_extra.path = Some("/wp".into());
    v2.stream_security = "tls".into();
    let generated = generate_singbox(&codegen_input(v2)).expect("v2ray-plugin");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/plugin"),
        "v2ray-plugin"
    );
    let opts = string_at(&generated.main, "/outbounds/0/plugin_opts");
    assert!(opts.contains("mode=websocket;"), "{opts}");
    assert!(opts.contains("host=plugin.test;"), "{opts}");
    assert!(opts.contains("path=/wp;"), "{opts}");
    assert!(opts.contains("tls;"), "{opts}");
    assert!(opts.ends_with("mux=0"), "{opts}");
    assert!(json_at(&generated.main, "/outbounds/0/tls").is_null());
}

#[test]
fn singbox_tls_reality_fragment_ech() {
    let mut p = vless_with_network("raw");
    p.stream_security = "reality".into();
    p.public_key = "SYNTHETIC_PUBLIC_KEY".into();
    p.short_id = "abcd".into();
    p.sni = "reality.test".into();
    p.fingerprint = "firefox".into();
    p.alpn = "h2,http/1.1".into();
    let mut settings = settings();
    settings.core_basic.enable_fragment = true;
    let mut input = codegen_input(p);
    input.settings = settings;

    let generated = generate_singbox(&input).expect("reality");
    let main = &generated.main;
    assert_eq!(json_at(main, "/outbounds/0/tls/enabled"), json!(true));
    assert_eq!(
        string_at(main, "/outbounds/0/tls/server_name"),
        "reality.test"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/tls/utls/fingerprint"),
        "firefox"
    );
    assert_eq!(
        json_at(main, "/outbounds/0/tls/alpn"),
        json!(["h2", "http/1.1"])
    );
    assert_eq!(
        json_at(main, "/outbounds/0/tls/reality/enabled"),
        json!(true)
    );
    assert_eq!(
        string_at(main, "/outbounds/0/tls/reality/public_key"),
        "SYNTHETIC_PUBLIC_KEY"
    );
    assert_eq!(string_at(main, "/outbounds/0/tls/reality/short_id"), "abcd");
    assert_eq!(json_at(main, "/outbounds/0/tls/insecure"), json!(false));
    assert_eq!(json_at(main, "/outbounds/0/tls/fragment"), json!(true));
    assert_eq!(
        json_at(main, "/outbounds/0/tls/record_fragment"),
        json!(true)
    );

    let mut ech = vless_with_network("raw");
    ech.stream_security = "tls".into();
    ech.ech_config_list = "AEXD+abc".into();
    let generated = generate_singbox(&codegen_input(ech)).expect("ech");
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/tls/ech/enabled"),
        json!(true)
    );
    assert_eq!(
        json_at(&generated.main, "/outbounds/0/tls/ech/config"),
        json!(["-----BEGIN ECH CONFIGS-----\nAEXD+abc\n-----END ECH CONFIGS-----"])
    );
}
