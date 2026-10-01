mod common;

use common::*;
use config_codegen::generate_xray;
use config_codegen::input::ConfigType;
use serde_json::json;

#[test]
fn xray_inbound_ports_lan_log_stat() {
    let p = profile(ConfigType::Vless, "192.0.2.70", 443);
    let mut p = p;
    p.password = "11111111-2222-3333-4444-555555555555".into();
    let mut settings = settings();
    settings.inbound.second_local_port_enabled = true;
    settings.inbound.allow_lan_conn = true;
    settings.inbound.new_port4_lan = true;
    settings.inbound.user = "lanuser".into();
    settings.inbound.pass = "lanpass".into();
    settings.core_basic.log_enabled = true;
    settings.core_basic.loglevel = "info".into();
    settings.gui.enable_statistics = true;
    let mut input = codegen_input(p);
    input.settings = settings;

    let generated = generate_xray(&input).expect("global");
    let main = &generated.main;

    // ports: local_port + EInboundProtocol offset
    assert_eq!(json_at(main, "/inbounds/0/port"), json!(11808));
    assert_eq!(string_at(main, "/inbounds/0/tag"), "socks");
    assert_eq!(json_at(main, "/inbounds/1/port"), json!(11809));
    assert_eq!(string_at(main, "/inbounds/1/tag"), "socks2");
    assert_eq!(json_at(main, "/inbounds/2/port"), json!(11810));
    assert_eq!(string_at(main, "/inbounds/2/tag"), "socks3");
    assert_eq!(string_at(main, "/inbounds/2/listen"), "0.0.0.0");
    assert_eq!(string_at(main, "/inbounds/2/settings/auth"), "password");
    assert_eq!(
        string_at(main, "/inbounds/2/settings/accounts/0/user"),
        "lanuser"
    );
    assert_eq!(
        string_at(main, "/inbounds/2/settings/accounts/0/pass"),
        "lanpass"
    );

    assert_eq!(string_at(main, "/log/loglevel"), "info");
    assert_eq!(
        string_at(main, "/log/access"),
        "logs/Vaccess_2026-10-01.txt"
    );
    assert_eq!(string_at(main, "/log/error"), "logs/Verror_2026-10-01.txt");

    assert_eq!(json_at(main, "/stats"), json!({}));
    assert_eq!(string_at(main, "/metrics/listen"), "127.0.0.1:11809");
    assert_eq!(
        json_at(main, "/policy/system/statsOutboundUplink"),
        json!(true)
    );
    assert_eq!(
        json_at(main, "/policy/system/statsOutboundDownlink"),
        json!(true)
    );
}

#[test]
fn xray_tun_inbound_and_rules() {
    let mut p = profile(ConfigType::Vless, "192.0.2.71", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    let mut settings = settings();
    settings.tun.enabled = true;
    settings.tun.mtu = 1400;
    settings.tun.ipv4_address = Some("172.18.0.1/30".into());
    settings.tun.enable_ipv6_address = true;
    settings.tun.ipv6_address = Some("fc00::172:18:0:1/126".into());
    settings.tun.route_exclude_address = vec!["10.0.0.0/8".into()];
    settings.core_basic.bind_interface = Some("wg0".into());
    settings.protect_core_executables = vec!["xray/".into()];
    settings.has_global_ipv6_address = true;
    let mut input = codegen_input(p);
    input.settings = settings;

    let generated = generate_xray(&input).expect("tun");
    let main = &generated.main;
    assert_eq!(json_at(main, "/inbounds").as_array().unwrap().len(), 2);
    assert_eq!(string_at(main, "/inbounds/0/tag"), "socks");
    assert_eq!(string_at(main, "/inbounds/1/tag"), "tun");
    assert_eq!(string_at(main, "/inbounds/1/settings/name"), "xray_tun");
    assert_eq!(json_at(main, "/inbounds/1/settings/MTU"), json!(1400));
    assert_eq!(
        json_at(main, "/inbounds/1/settings/gateway"),
        json!(["172.18.0.1/30", "fc00::172:18:0:1/126"])
    );
    assert_eq!(
        string_at(main, "/inbounds/1/settings/autoOutboundsInterface"),
        "wg0"
    );
    assert_eq!(json_at(main, "/inbounds/1/sniffing/routeOnly"), json!(true));
    let route_table = json_at(main, "/inbounds/1/settings/autoSystemRoutingTable");
    let route_text = route_table.to_string();
    assert!(route_text.contains("0.0.0.0/5"), "{route_text}");
    assert!(route_text.contains("::/0"), "{route_text}");
    assert!(!route_text.contains("10.0.0.0/8"), "{route_text}");

    // TUN rules are prepended, then direct exe rules, then the tun DNS rule.
    assert_eq!(string_at(main, "/routing/rules/0/outboundTag"), "block");
    assert_eq!(string_at(main, "/routing/rules/1/outboundTag"), "block");
    assert_eq!(json_at(main, "/routing/rules/2/process"), json!(["xray/"]));
    assert_eq!(string_at(main, "/routing/rules/2/outboundTag"), "dns");
    assert_eq!(string_at(main, "/routing/rules/3/outboundTag"), "direct");
    assert_eq!(json_at(main, "/routing/rules/4/inboundTag"), json!(["tun"]));
    assert_eq!(string_at(main, "/routing/rules/4/port"), "53");
    assert_eq!(string_at(main, "/routing/rules/4/outboundTag"), "dns");
    // dns outbound appended for TUN
    let outbound_tags: Vec<String> = main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| {
            o.get("tag")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert!(
        outbound_tags.contains(&"dns".to_string()),
        "{outbound_tags:?}"
    );
}

#[test]
fn xray_fragment_passes() {
    let mut p = profile(ConfigType::Vless, "192.0.2.72", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = "raw".into();
    p.stream_security = "tls".into();
    let mut settings = settings();
    settings.core_basic.enable_fragment = true;
    settings.core_basic.enable_final_fragment = true;
    settings.fragment4_ray.packets = Some("tlshello".into());
    settings.fragment4_ray.max_split = Some("0".into());
    let mut input = codegen_input(p);
    input.settings = settings;

    let generated = generate_xray(&input).expect("fragment");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/tag"), "proxy");
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/sockopt/dialerProxy"),
        "fragment-proxy"
    );
    assert_eq!(string_at(main, "/outbounds/1/tag"), "fragment-proxy");
    assert_eq!(string_at(main, "/outbounds/1/protocol"), "vless");
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/finalmask/tcp/0/type"),
        "fragment"
    );
    assert_eq!(
        string_at(
            main,
            "/outbounds/0/streamSettings/finalmask/tcp/0/settings/packets"
        ),
        "tlshello"
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/finalmask/tcp/0/settings/lengths"
        ),
        json!(["50-100"])
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/finalmask/tcp/0/settings/delays"
        ),
        json!(["10-20"])
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/finalmask/tcp/0/settings/maxSplit"
        ),
        json!(0)
    );
}

#[test]
fn xray_bind_interface_and_send_through() {
    let mut p = profile(ConfigType::Vless, "192.0.2.73", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = "raw".into();
    p.stream_security = "tls".into();
    let mut settings = settings();
    settings.core_basic.bind_interface = Some("wg0".into());
    settings.core_basic.send_through = Some("192.0.2.200".into());
    let mut input = codegen_input(p);
    input.settings = settings;

    let generated = generate_xray(&input).expect("bind");
    let main = &generated.main;
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/sockopt/interface"),
        "wg0"
    );
    assert_eq!(string_at(main, "/outbounds/0/sendThrough"), "192.0.2.200");
    // freedom/blackhole are not bound
    let freedom_index = main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .position(|o| o.get("protocol").and_then(|v| v.as_str()) == Some("freedom"))
        .unwrap();
    assert!(json_at(main, &format!("/outbounds/{freedom_index}/sendThrough")).is_null());
}
