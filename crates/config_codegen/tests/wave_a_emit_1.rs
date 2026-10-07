//! Wave A emit cases (SP-23 FLD-CFG-026..051) with no prior emit test,
//! plus negative/reopen assertions missing from existing coverage.
//!
//! Synthetic fixtures only; ports >= 11808, never 10808.

mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::generate_xray;
use config_codegen::input::*;
use serde_json::json;

fn vless(password: bool) -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.90", 443);
    if password {
        p.password = "11111111-2222-3333-4444-555555555555".into();
    }
    p
}

fn kcp_profile() -> CodegenProfile {
    let mut p = vless(true);
    p.network = "kcp".into();
    p
}

fn reality_profile() -> CodegenProfile {
    let mut p = vless(true);
    p.network = "raw".into();
    p.stream_security = "reality".into();
    p.public_key = "SYNTHETIC_PUBLIC_KEY".into();
    p.short_id = "abcd".into();
    p.sni = "reality.test".into();
    p
}

fn ws_profile() -> CodegenProfile {
    let mut p = vless(true);
    p.network = "ws".into();
    p.transport_extra.host = Some("ws.example.test".into());
    p.transport_extra.path = Some("/ws".into());
    p
}

/// Reopen proxy at this layer: settings survive a serialize/deserialize cycle.
fn reopen(settings: &CodegenSettings) -> CodegenSettings {
    let v = serde_json::to_value(settings).expect("serialize settings");
    serde_json::from_value(v).expect("reopen settings")
}

// FLD-CFG-026: LogEnabled off omits core log files; on writes them.
#[test]
fn wave_a_026_log_enabled_gates_log_files() {
    let mut s = settings();
    s.core_basic.log_enabled = false;
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/log/access").is_null(), "{main}");
    assert!(json_at(&main, "/log/error").is_null(), "{main}");
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(json_at(&main, "/log/output").is_null(), "{main}");
    assert_no_live_port(&main);

    let mut s = settings();
    s.core_basic.log_enabled = true;
    assert!(reopen(&s).core_basic.log_enabled);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert!(string_at(&main, "/log/access").contains("Vaccess_"));
    assert!(string_at(&main, "/log/error").contains("Verror_"));
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(string_at(&main, "/log/output").contains("sbox_"));
    assert_no_live_port(&main);
}

// FLD-CFG-027: Loglevel reaches both cores; warning maps to warn on sing-box.
#[test]
fn wave_a_027_loglevel_reaches_both_cores() {
    let mut s = settings();
    s.core_basic.loglevel = "debug".into();
    assert_eq!(reopen(&s).core_basic.loglevel, "debug");
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/log/loglevel"), "debug");
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(string_at(&main, "/log/level"), "debug");
    assert_no_live_port(&main);

    let input = codegen_input(vless(true));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/log/loglevel"), "warning");
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(string_at(&main, "/log/level"), "warn");
}

// FLD-CFG-028: default fingerprint stamps Reality when node is empty;
// explicit node value wins; empty default stamps nothing.
#[test]
fn wave_a_028_def_fingerprint_default_and_override() {
    let mut s = settings();
    s.core_basic.def_fingerprint = Some("chrome".into());
    assert_eq!(
        reopen(&s).core_basic.def_fingerprint.as_deref(),
        Some("chrome")
    );
    let mut input = codegen_input(reality_profile());
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        string_at(
            &main,
            "/outbounds/0/streamSettings/realitySettings/fingerprint"
        ),
        "chrome"
    );
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(
        string_at(&main, "/outbounds/0/tls/utls/fingerprint"),
        "chrome"
    );
    assert_no_live_port(&main);

    // Explicit node fingerprint wins over the default.
    let mut s = settings();
    s.core_basic.def_fingerprint = Some("chrome".into());
    let mut p = reality_profile();
    p.fingerprint = "firefox".into();
    let mut input = codegen_input(p);
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        string_at(
            &main,
            "/outbounds/0/streamSettings/realitySettings/fingerprint"
        ),
        "firefox"
    );
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(
        string_at(&main, "/outbounds/0/tls/utls/fingerprint"),
        "firefox"
    );

    // Empty default stamps nothing.
    let input = codegen_input(reality_profile());
    let main = generate_xray(&input).expect("xray").main;
    assert!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/realitySettings/fingerprint"
        )
        .is_null(),
        "{main}"
    );
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(json_at(&main, "/outbounds/0/tls/utls").is_null(), "{main}");
}

// FLD-CFG-029 negative: empty default injects no User-Agent (xray + sing-box).
#[test]
fn wave_a_029_empty_user_agent_injects_nothing() {
    let input = codegen_input(ws_profile());
    assert!(reopen(&input.settings).core_basic.def_user_agent.is_none());
    let main = generate_xray(&input).expect("xray").main;
    assert!(
        json_at(&main, "/outbounds/0/streamSettings/wsSettings/headers").is_null(),
        "{main}"
    );
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(
        json_at(&main, "/outbounds/0/transport/headers/User-Agent").is_null(),
        "{main}"
    );
    assert_no_live_port(&main);
}

// FLD-CFG-030: SendThrough reaches outbounds; None emits nothing.
#[test]
fn wave_a_030_send_through_effect_and_empty() {
    let mut s = settings();
    s.core_basic.send_through = Some("192.0.2.200".into());
    assert_eq!(
        reopen(&s).core_basic.send_through.as_deref(),
        Some("192.0.2.200")
    );
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/outbounds/0/sendThrough"), "192.0.2.200");
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(
        string_at(&main, "/outbounds/0/inet4_bind_address"),
        "192.0.2.200"
    );
    assert_no_live_port(&main);

    let input = codegen_input(vless(true));
    let main = generate_xray(&input).expect("xray").main;
    assert!(
        json_at(&main, "/outbounds/0/sendThrough").is_null(),
        "{main}"
    );
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(
        json_at(&main, "/outbounds/0/inet4_bind_address").is_null(),
        "{main}"
    );
}

// FLD-CFG-031: BindInterface reaches outbounds; None emits nothing.
#[test]
fn wave_a_031_bind_interface_effect_and_empty() {
    let mut s = settings();
    s.core_basic.bind_interface = Some("wg0".into());
    assert_eq!(reopen(&s).core_basic.bind_interface.as_deref(), Some("wg0"));
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        string_at(&main, "/outbounds/0/streamSettings/sockopt/interface"),
        "wg0"
    );
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(string_at(&main, "/outbounds/0/bind_interface"), "wg0");
    assert_no_live_port(&main);

    let input = codegen_input(vless(true));
    let main = generate_xray(&input).expect("xray").main;
    assert!(
        json_at(&main, "/outbounds/0/streamSettings/sockopt/interface").is_null(),
        "{main}"
    );
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(
        json_at(&main, "/outbounds/0/bind_interface").is_null(),
        "{main}"
    );
}

// FLD-CFG-032: fragment off emits no mask; on + reopen keeps the switch.
#[test]
fn wave_a_032_fragment_switch_and_reopen() {
    let mut s = settings();
    s.core_basic.enable_fragment = false;
    s.core_basic.enable_final_fragment = false;
    assert!(!reopen(&s).core_basic.enable_fragment);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert!(!main.to_string().contains("finalmask"), "{main}");

    let mut s = settings();
    s.core_basic.enable_fragment = true;
    assert!(reopen(&s).core_basic.enable_fragment);
}

// FLD-CFG-033: sing-box final-fragment rule follows the switch both ways.
#[test]
fn wave_a_033_singbox_final_fragment_rule_both_ways() {
    let mut s = settings();
    s.core_basic.enable_final_fragment = true;
    assert!(reopen(&s).core_basic.enable_final_fragment);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(
        main["route"]["rules"].as_array().unwrap().iter().any(|r| {
            r.get("action").and_then(|v| v.as_str()) == Some("route-options")
                && r.get("tls_record_fragment").and_then(|v| v.as_bool()) == Some(true)
        }),
        "{main}"
    );
    assert_no_live_port(&main);

    let mut s = settings();
    s.core_basic.enable_final_fragment = false;
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(
        !main["route"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| { r.get("tls_record_fragment").and_then(|v| v.as_bool()) == Some(true) }),
        "{main}"
    );
}

// FLD-CFG-034 negative: cache file off emits no cache_file block.
#[test]
fn wave_a_034_cache_file_off_emits_nothing() {
    let mut s = settings();
    s.core_basic.enable_cache_file4_sbox = false;
    assert!(!reopen(&s).core_basic.enable_cache_file4_sbox);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(
        json_at(&main, "/experimental/cache_file").is_null(),
        "{main}"
    );
    assert_no_live_port(&main);

    let mut s = settings();
    s.core_basic.enable_cache_file4_sbox = true;
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(
        json_at(&main, "/experimental/cache_file/enabled"),
        json!(true)
    );
}

// FLD-CFG-035: custom local port reaches both cores; reopen keeps it.
#[test]
fn wave_a_035_local_port_reaches_both_cores() {
    let mut s = settings();
    s.inbound.local_port = 11821;
    assert_eq!(reopen(&s).inbound.local_port, 11821);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/inbounds/0/port"), json!(11821));
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(json_at(&main, "/inbounds/0/listen_port"), json!(11821));
    assert_no_live_port(&main);
}

// FLD-CFG-037: UdpEnabled reaches xray settings.udp; reopen keeps it.
// (sing-box inbound carries no udp flag: N/A there.)
#[test]
fn wave_a_037_udp_enabled_reaches_xray() {
    for enabled in [true, false] {
        let mut s = settings();
        s.inbound.udp_enabled = enabled;
        assert_eq!(reopen(&s).inbound.udp_enabled, enabled);
        let mut input = codegen_input(vless(true));
        input.settings = s;
        let main = generate_xray(&input).expect("xray").main;
        assert_eq!(
            json_at(&main, "/inbounds/0/settings/udp"),
            json!(enabled),
            "{main}"
        );
        assert_no_live_port(&main);
    }
}

// FLD-CFG-038: sniffing switch reaches xray block and sing-box sniff rule.
#[test]
fn wave_a_038_sniffing_enabled_both_cores() {
    let mut s = settings();
    s.inbound.sniffing_enabled = false;
    assert!(!reopen(&s).inbound.sniffing_enabled);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/inbounds/0/sniffing/enabled"), json!(false));
    // Stored values are kept even when the switch is off.
    assert_eq!(
        json_at(&main, "/inbounds/0/sniffing/destOverride"),
        json!(["http", "tls"])
    );
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(
        !main["route"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.get("action").and_then(|v| v.as_str()) == Some("sniff")),
        "{main}"
    );

    let mut s = settings();
    s.inbound.sniffing_enabled = true;
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/inbounds/0/sniffing/enabled"), json!(true));
    let main = generate_singbox(&input).expect("singbox").main;
    assert!(
        main["route"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.get("action").and_then(|v| v.as_str()) == Some("sniff")),
        "{main}"
    );
}

// FLD-CFG-039: custom DestOverride list lands verbatim, order preserved.
#[test]
fn wave_a_039_dest_override_list_reaches_xray() {
    let mut s = settings();
    s.inbound.dest_override = vec!["tls".into()];
    assert_eq!(reopen(&s).inbound.dest_override, vec!["tls".to_string()]);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/inbounds/0/sniffing/destOverride"),
        json!(["tls"]),
        "{main}"
    );
    assert_no_live_port(&main);

    let input = codegen_input(vless(true));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/inbounds/0/sniffing/destOverride"),
        json!(["http", "tls"]),
        "{main}"
    );
}

// FLD-CFG-040: RouteOnly reaches xray sniffing.routeOnly; reopen keeps it.
#[test]
fn wave_a_040_route_only_reaches_xray() {
    for enabled in [true, false] {
        let mut s = settings();
        s.inbound.route_only = enabled;
        assert_eq!(reopen(&s).inbound.route_only, enabled);
        let mut input = codegen_input(vless(true));
        input.settings = s;
        let main = generate_xray(&input).expect("xray").main;
        assert_eq!(
            json_at(&main, "/inbounds/0/sniffing/routeOnly"),
            json!(enabled),
            "{main}"
        );
        assert_no_live_port(&main);
    }
}

// FLD-CFG-041: AllowLANConn switches loopback vs all-interface listen.
#[test]
fn wave_a_041_allow_lan_conn_listen_both_cores() {
    let mut s = settings();
    s.inbound.allow_lan_conn = true;
    assert!(reopen(&s).inbound.allow_lan_conn);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/inbounds/1/listen"), "0.0.0.0", "{main}");
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(string_at(&main, "/inbounds/1/listen"), "0.0.0.0", "{main}");
    assert_no_live_port(&main);

    let mut s = settings();
    s.inbound.allow_lan_conn = false;
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/inbounds").as_array().unwrap().len(), 1);
    assert_eq!(string_at(&main, "/inbounds/0/listen"), "127.0.0.1");
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(json_at(&main, "/inbounds").as_array().unwrap().len(), 1);
}

// FLD-CFG-042 negative: NewPort4LAN off keeps a single loopback listener.
#[test]
fn wave_a_042_new_port4_lan_off_keeps_single_listener() {
    let mut s = settings();
    s.inbound.allow_lan_conn = true;
    s.inbound.new_port4_lan = false;
    assert!(!reopen(&s).inbound.new_port4_lan);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/inbounds").as_array().unwrap().len(), 1);
    assert_eq!(string_at(&main, "/inbounds/0/listen"), "0.0.0.0", "{main}");
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(json_at(&main, "/inbounds").as_array().unwrap().len(), 1);
    assert_eq!(string_at(&main, "/inbounds/0/listen"), "0.0.0.0", "{main}");
    assert_no_live_port(&main);
}

// FLD-CFG-045 negative: second port off keeps a single inbound.
#[test]
fn wave_a_045_second_port_off_keeps_single_inbound() {
    let mut s = settings();
    s.inbound.second_local_port_enabled = false;
    assert!(!reopen(&s).inbound.second_local_port_enabled);
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/inbounds").as_array().unwrap().len(), 1);
    assert_no_live_port(&main);
    let main = generate_singbox(&input).expect("singbox").main;
    assert_eq!(json_at(&main, "/inbounds").as_array().unwrap().len(), 1);
    assert_no_live_port(&main);

    let mut s = settings();
    s.inbound.second_local_port_enabled = true;
    let mut input = codegen_input(vless(true));
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/inbounds/1/tag"), "socks2");
    assert_eq!(json_at(&main, "/inbounds/1/port"), json!(11809));
}

// FLD-CFG-046 + 047: custom KCP mtu/tti reach kcpSettings; reopen keeps them.
#[test]
fn wave_a_046_047_kcp_mtu_tti_reach_emit() {
    let mut s = settings();
    s.kcp.mtu = 1400;
    s.kcp.tti = 40;
    let rt = reopen(&s);
    assert_eq!(rt.kcp.mtu, 1400);
    assert_eq!(rt.kcp.tti, 40);
    let mut input = codegen_input(kcp_profile());
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/kcpSettings/mtu"),
        json!(1400),
        "{main}"
    );
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/kcpSettings/tti"),
        json!(40),
        "{main}"
    );
    assert_no_live_port(&main);
}

// FLD-CFG-048 + 049: KCP capacities reach kcpSettings without swapping direction.
#[test]
fn wave_a_048_049_kcp_capacities_reach_emit() {
    let mut s = settings();
    s.kcp.uplink_capacity = 15;
    s.kcp.downlink_capacity = 90;
    let rt = reopen(&s);
    assert_eq!(rt.kcp.uplink_capacity, 15);
    assert_eq!(rt.kcp.downlink_capacity, 90);
    let mut input = codegen_input(kcp_profile());
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/kcpSettings/uplinkCapacity"
        ),
        json!(15),
        "{main}"
    );
    assert_eq!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/kcpSettings/downlinkCapacity"
        ),
        json!(90),
        "{main}"
    );
    assert_no_live_port(&main);
}

// FLD-CFG-050 + 051: KCP window fields reach kcpSettings; reopen keeps them.
// (<=0 clamp/backfill lives in domain/application projection, not this crate.)
#[test]
fn wave_a_050_051_kcp_window_reaches_emit() {
    let mut s = settings();
    s.kcp.cwnd_multiplier = 4;
    s.kcp.max_sending_window = 1048576;
    let rt = reopen(&s);
    assert_eq!(rt.kcp.cwnd_multiplier, 4);
    assert_eq!(rt.kcp.max_sending_window, 1048576);
    let mut input = codegen_input(kcp_profile());
    input.settings = s;
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/kcpSettings/cwndMultiplier"
        ),
        json!(4),
        "{main}"
    );
    assert_eq!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/kcpSettings/maxSendingWindow"
        ),
        json!(1048576),
        "{main}"
    );
    assert_no_live_port(&main);
}
