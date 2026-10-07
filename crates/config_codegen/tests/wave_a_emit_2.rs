//! Wave A emit-2: FLD-CFG-120..180 leaves lacking codegen emit coverage.
//!
//! Complements (never edits) the existing files:
//! - `xray_protocols.rs:66` (120 default-8 on-path only),
//! - `xray_global.rs:140` (150 NOT covered: only sets max_split),
//! - `xray_routing_dns.rs:15/247` (161 fake_ip on-path; 177 on-path only),
//! - `fixtures_samples.rs:95` (161 singbox fakeip filter only),
//! - `sp24_happy_fragment.rs` (G-01 gate matrix + 150..153 wire; per-leaf
//!   singles, freedom-ON and negatives live here),
//! - application `t18_settings_chain.rs:76-77` (120/123 plan-level only),
//! - application `t11_routing_dns.rs:239` (159 template-level only).
//!
//! G-01 (176..180 gate) and G-02 (150/153 TryParseMaxSplit unify) production
//! fixes already exist in-tree (`xray/dns.rs:383` gate,
//! `application/src/settings.rs:93/110` validator); this file only asserts the
//! codegen-visible behavior. No production code is touched here.
//!
//! Synthetic fixtures only; ports >= 11808; never 10808.

mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::generate_xray;
use config_codegen::input::*;
use serde_json::json;

fn vless_tls(octet: u8) -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, &format!("192.0.2.{octet}"), 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = "raw".into();
    p.stream_security = "tls".into();
    p
}

/// Save/reopen equivalent at the codegen layer: serialize the input and read
/// it back, then regenerate. Proves the emitted effect survives a roundtrip.
fn reopen(input: &CodegenInput) -> CodegenInput {
    let text = serde_json::to_string(input).expect("serialize input");
    assert!(!text.contains("10808"), "input must not reference 10808");
    serde_json::from_str(&text).expect("reopen input")
}

fn dns_input(octet: u8, simple: SimpleDns, happy: HappyEyeballs4Ray) -> CodegenInput {
    let mut input = codegen_input(vless_tls(octet));
    input.dns = Some(CodegenDns {
        simple,
        ..Default::default()
    });
    input.settings.happy_eyeballs4_ray = happy;
    input
}

fn dns_rule(outbound: &str, domains: &[&str]) -> CodegenRule {
    CodegenRule {
        enabled: true,
        rule_type: RuleType::Dns,
        outbound_tag: outbound.into(),
        domain: Some(domains.iter().map(ToString::to_string).collect()),
        ..Default::default()
    }
}

fn freedom(main: &serde_json::Value) -> &serde_json::Value {
    main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o.get("protocol").and_then(|v| v.as_str()) == Some("freedom"))
        .expect("freedom outbound")
}

fn proxy_mux(main: &serde_json::Value) -> serde_json::Value {
    json_at(main, "/outbounds/0/mux")
}

fn singbox_multiplex(main: &serde_json::Value) -> Option<serde_json::Value> {
    main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|o| o.get("multiplex").cloned())
}

// FLD-CFG-120: Mux4RayItem.Concurrency. Existing xray_protocols.rs:66 covers
// default 8 on-path; here: custom value + mux-off negative + reopen.
#[test]
fn fld120_mux_concurrency_custom_and_off() {
    let mut s = settings();
    s.mux4_ray.concurrency = 4;
    let mut p = vless_tls(120);
    p.mux_enabled = Some(true);
    let mut input = codegen_input(p);
    input.settings = s;
    let main = generate_xray(&input).expect("mux").main;
    let mux = proxy_mux(&main);
    assert_eq!(mux["enabled"], json!(true));
    assert_eq!(mux["concurrency"], json!(4));
    assert!(mux.get("xudpConcurrency").is_none(), "{mux}");
    assert_no_live_port(&main);

    let main2 = generate_xray(&reopen(&input)).expect("reopen").main;
    assert_eq!(json_at(&main2, "/outbounds/0/mux/concurrency"), json!(4));

    let mut p = vless_tls(120);
    p.mux_enabled = Some(false);
    let main = generate_xray(&codegen_input(p)).expect("off").main;
    let mux = proxy_mux(&main);
    assert_eq!(mux["enabled"], json!(false));
    assert_eq!(mux["concurrency"], json!(-1));
}

// FLD-CFG-121: Mux4RayItem.XudpConcurrency. The xudp branch emits only when
// the TCP mux is off but UDP mux is on (vless with flow set); else absent.
#[test]
fn fld121_xudp_concurrency_emit_and_reopen() {
    let mut s = settings();
    s.mux4_ray.xudp_concurrency = 9;
    let mut p = vless_tls(121);
    p.proto_extra.flow = Some("xtls-rprx-vision".into());
    p.mux_enabled = Some(true);
    let mut input = codegen_input(p);
    input.settings = s;
    let main = generate_xray(&input).expect("xudp").main;
    let mux = proxy_mux(&main);
    assert_eq!(mux["enabled"], json!(true));
    assert_eq!(mux["xudpConcurrency"], json!(9));
    assert_no_live_port(&main);

    let main2 = generate_xray(&reopen(&input)).expect("reopen").main;
    assert_eq!(
        json_at(&main2, "/outbounds/0/mux/xudpConcurrency"),
        json!(9)
    );

    // Negative: mux off -> disabled base object, no xudp keys.
    let mut p = vless_tls(121);
    p.proto_extra.flow = Some("xtls-rprx-vision".into());
    p.mux_enabled = Some(false);
    let main = generate_xray(&codegen_input(p)).expect("off").main;
    let mux = proxy_mux(&main);
    assert_eq!(mux["enabled"], json!(false));
    assert!(mux.get("xudpConcurrency").is_none(), "{mux}");
    assert!(mux.get("xudpProxyUDP443").is_none(), "{mux}");
}

// FLD-CFG-122: Mux4RayItem.XudpProxyUDP443 ("reject" default, frozen enum).
#[test]
fn fld122_xudp_proxy_udp443_emit_and_reopen() {
    let mut s = settings();
    s.mux4_ray.xudp_proxy_udp443 = "skip".into();
    let mut p = vless_tls(122);
    p.proto_extra.flow = Some("xtls-rprx-vision".into());
    p.mux_enabled = Some(true);
    let mut input = codegen_input(p);
    input.settings = s;
    let main = generate_xray(&input).expect("xudp443").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/mux/xudpProxyUDP443"),
        json!("skip")
    );
    assert_no_live_port(&main);

    let main2 = generate_xray(&reopen(&input)).expect("reopen").main;
    assert_eq!(
        json_at(&main2, "/outbounds/0/mux/xudpProxyUDP443"),
        json!("skip")
    );

    // Negative: TCP-mux path (no flow) carries no xudp443 key.
    let mut p = vless_tls(122);
    p.mux_enabled = Some(true);
    let main = generate_xray(&codegen_input(p)).expect("tcp path").main;
    assert!(proxy_mux(&main).get("xudpProxyUDP443").is_none());
}

// FLD-CFG-123: Mux4SboxItem.Protocol. Application t18 covers plan-level;
// here: sing-box multiplex emit + negatives + reopen.
#[test]
fn fld123_mux_sbox_protocol_emit_and_reopen() {
    let mut s = settings();
    s.mux4_sbox.protocol = "smux".into();
    let mut p = vless_tls(123);
    p.mux_enabled = Some(true);
    let mut input = codegen_input(p);
    input.settings = s;
    let main = generate_singbox(&input).expect("mux").main;
    let mux = singbox_multiplex(&main).expect("multiplex");
    assert_eq!(mux["enabled"], json!(true));
    assert_eq!(mux["protocol"], json!("smux"));
    assert_no_live_port(&main);

    let main2 = generate_singbox(&reopen(&input)).expect("reopen").main;
    assert_eq!(
        singbox_multiplex(&main2).expect("multiplex")["protocol"],
        json!("smux")
    );

    // Negative: mux off -> no multiplex block at all.
    let mut p = vless_tls(123);
    p.mux_enabled = Some(false);
    let mut input = codegen_input(p);
    input.settings.mux4_sbox.protocol = "smux".into();
    let main = generate_singbox(&input).expect("off").main;
    assert!(singbox_multiplex(&main).is_none());

    // Negative: empty protocol -> no multiplex even when enabled.
    let mut p = vless_tls(123);
    p.mux_enabled = Some(true);
    let main = generate_singbox(&codegen_input(p)).expect("empty").main;
    assert!(singbox_multiplex(&main).is_none());
}

// FLD-CFG-125: Mux4SboxItem.Padding (nullable; None omits the key).
#[test]
fn fld125_mux_sbox_padding_emit_and_reopen() {
    let mut s = settings();
    s.mux4_sbox.protocol = "h2mux".into();
    s.mux4_sbox.padding = Some(true);
    let mut p = vless_tls(125);
    p.mux_enabled = Some(true);
    let mut input = codegen_input(p);
    input.settings = s;
    let main = generate_singbox(&input).expect("padding").main;
    let mux = singbox_multiplex(&main).expect("multiplex");
    assert_eq!(mux["padding"], json!(true));
    assert_no_live_port(&main);

    let main2 = generate_singbox(&reopen(&input)).expect("reopen").main;
    assert_eq!(
        singbox_multiplex(&main2).expect("multiplex")["padding"],
        json!(true)
    );

    // Negative: None -> key omitted (block still present for protocol).
    let mut s = settings();
    s.mux4_sbox.protocol = "h2mux".into();
    let mut p = vless_tls(125);
    p.mux_enabled = Some(true);
    let mut input = codegen_input(p);
    input.settings = s;
    let main = generate_singbox(&input).expect("none").main;
    let mux = singbox_multiplex(&main).expect("multiplex");
    assert!(mux.get("padding").is_none(), "{mux}");
}

// FLD-CFG-150: Fragment4RayItem.Packets. xray_global.rs:140 only sets
// max_split, so the packets leaf itself is asserted here.
#[test]
fn fld150_fragment_packets_leaf_and_reopen() {
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    s.fragment4_ray.packets = Some("synthetic-packets".into());
    let mut input = codegen_input(vless_tls(150));
    input.settings = s;
    let main = generate_xray(&input).expect("fragment").main;
    let base = "/outbounds/0/streamSettings/finalmask/tcp/0/settings";
    assert_eq!(
        string_at(&main, &format!("{base}/packets")),
        "synthetic-packets"
    );
    assert_no_live_port(&main);

    let main2 = generate_xray(&reopen(&input)).expect("reopen").main;
    assert_eq!(
        string_at(&main2, &format!("{base}/packets")),
        "synthetic-packets"
    );

    // Negative: unset -> frozen fallback "tlshello".
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    let mut input = codegen_input(vless_tls(150));
    input.settings = s;
    let main = generate_xray(&input).expect("default").main;
    assert_eq!(string_at(&main, &format!("{base}/packets")), "tlshello");
}

// FLD-CFG-151: Fragment4RayItem.Lengths (wire lengths + first-value length).
#[test]
fn fld151_fragment_lengths_leaf_and_reopen() {
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    s.fragment4_ray.lengths = vec!["20-40".into(), "60-80".into()];
    let mut input = codegen_input(vless_tls(151));
    input.settings = s;
    let main = generate_xray(&input).expect("fragment").main;
    let base = "/outbounds/0/streamSettings/finalmask/tcp/0/settings";
    assert_eq!(
        json_at(&main, &format!("{base}/lengths")),
        json!(["20-40", "60-80"])
    );
    assert_eq!(string_at(&main, &format!("{base}/length")), "20-40");
    assert_no_live_port(&main);

    let main2 = generate_xray(&reopen(&input)).expect("reopen").main;
    assert_eq!(
        json_at(&main2, &format!("{base}/lengths")),
        json!(["20-40", "60-80"])
    );

    // Negative: empty -> frozen fallback ["50-100"].
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    let mut input = codegen_input(vless_tls(151));
    input.settings = s;
    let main = generate_xray(&input).expect("default").main;
    assert_eq!(
        json_at(&main, &format!("{base}/lengths")),
        json!(["50-100"])
    );
}

// FLD-CFG-152: Fragment4RayItem.Delays (wire delays + first-value delay).
#[test]
fn fld152_fragment_delays_leaf_and_reopen() {
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    s.fragment4_ray.delays = vec!["1-5".into(), "6-9".into()];
    let mut input = codegen_input(vless_tls(152));
    input.settings = s;
    let main = generate_xray(&input).expect("fragment").main;
    let base = "/outbounds/0/streamSettings/finalmask/tcp/0/settings";
    assert_eq!(
        json_at(&main, &format!("{base}/delays")),
        json!(["1-5", "6-9"])
    );
    assert_eq!(string_at(&main, &format!("{base}/delay")), "1-5");
    assert_no_live_port(&main);

    let main2 = generate_xray(&reopen(&input)).expect("reopen").main;
    assert_eq!(
        json_at(&main2, &format!("{base}/delays")),
        json!(["1-5", "6-9"])
    );

    // Negative: empty -> frozen fallback ["10-20"].
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    let mut input = codegen_input(vless_tls(152));
    input.settings = s;
    let main = generate_xray(&input).expect("default").main;
    assert_eq!(json_at(&main, &format!("{base}/delays")), json!(["10-20"]));
}

// G-02 (FLD-CFG-150/153): MaxSplit parse parity at the wire. The validator
// unify (accept "1-3", keep raw) lives in application/src/settings.rs and is
// covered there; here the wire must take the first int for every accepted
// form, and the raw text must survive a reopen unchanged.
#[test]
fn fld150_153_maxsplit_parse_parity_and_reopen() {
    for (raw, wire) in [
        ("1-3", 1),
        ("0", 0),
        ("10000", 10000),
        ("0-10000", 0),
        (" 3 ", 3),
        ("abc", 0),
    ] {
        let mut s = settings();
        s.core_basic.enable_fragment = true;
        s.fragment4_ray.max_split = Some(raw.into());
        let mut input = codegen_input(vless_tls(153));
        input.settings = s;
        let main = generate_xray(&reopen(&input)).expect("fragment").main;
        assert_eq!(
            json_at(
                &main,
                "/outbounds/0/streamSettings/finalmask/tcp/0/settings/maxSplit"
            ),
            json!(wire),
            "raw={raw:?}"
        );
        assert_no_live_port(&main);
    }
    // Raw range text is preserved verbatim through a reopen (never stored
    // truncated to the wired first int).
    let mut s = settings();
    s.fragment4_ray.max_split = Some("1-3".into());
    let mut input = codegen_input(vless_tls(153));
    input.settings = s;
    let back = reopen(&input);
    assert_eq!(
        back.settings.fragment4_ray.max_split.as_deref(),
        Some("1-3")
    );
}

// FLD-CFG-154: legacy Length -> Lengths migration target. The backfill itself
// is domain-owned (domain/src/settings.rs:1114-1120, tested with
// `cargo test -p domain`); here the migrated value must reach the wire.
#[test]
fn fld154_legacy_length_target_reaches_wire() {
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    s.fragment4_ray.lengths = vec!["77-88".into()];
    let mut input = codegen_input(vless_tls(154));
    input.settings = s;
    let main = generate_xray(&reopen(&input)).expect("fragment").main;
    let base = "/outbounds/0/streamSettings/finalmask/tcp/0/settings";
    assert_eq!(json_at(&main, &format!("{base}/lengths")), json!(["77-88"]));
    assert_eq!(string_at(&main, &format!("{base}/length")), "77-88");
    assert_no_live_port(&main);
}

// FLD-CFG-155: legacy Interval -> Delays migration target (same split as 154).
#[test]
fn fld155_legacy_interval_target_reaches_wire() {
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    s.fragment4_ray.delays = vec!["9-19".into()];
    let mut input = codegen_input(vless_tls(155));
    input.settings = s;
    let main = generate_xray(&reopen(&input)).expect("fragment").main;
    let base = "/outbounds/0/streamSettings/finalmask/tcp/0/settings";
    assert_eq!(json_at(&main, &format!("{base}/delays")), json!(["9-19"]));
    assert_eq!(string_at(&main, &format!("{base}/delay")), "9-19");
    assert_no_live_port(&main);
}

// FLD-CFG-159 complement: SimpleDNSItem.UseSystemHosts codegen emit.
// t11_routing_dns.rs:239 covers the template level; here the hosts merge.
#[test]
fn fld159_use_system_hosts_emit() {
    let simple = SimpleDns {
        use_system_hosts: true,
        ..Default::default()
    };
    let mut input = dns_input(159, simple, HappyEyeballs4Ray::default());
    input
        .dns
        .as_mut()
        .unwrap()
        .system_hosts
        .insert("sys.example.test".into(), "10.9.9.9".into());
    let input = reopen(&input);
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/dns/hosts/sys.example.test"),
        json!(["10.9.9.9"])
    );
    let main = generate_singbox(&input).expect("sbox").main;
    let predefined = json_at(&main, "/dns/servers/3/predefined");
    assert_eq!(predefined["sys.example.test"], json!(["10.9.9.9"]));
    assert_no_live_port(&main);

    // Negative: switch off -> system host appears in neither core.
    let input = dns_input(159, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/dns/hosts").is_null());
}

// FLD-CFG-160: SimpleDNSItem.AddCommonHosts (default true upstream).
#[test]
fn fld160_add_common_hosts_emit_and_reopen() {
    let simple = SimpleDns {
        add_common_hosts: true,
        ..Default::default()
    };
    let input = reopen(&dns_input(160, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    let hosts = json_at(&main, "/dns/hosts");
    assert_eq!(
        hosts["dns.google"],
        json!([
            "8.8.8.8",
            "8.8.4.4",
            "2001:4860:4860::8888",
            "2001:4860:4860::8844"
        ])
    );
    let main = generate_singbox(&input).expect("sbox").main;
    let predefined = json_at(&main, "/dns/servers/3/predefined");
    assert_eq!(
        predefined["dns.google"],
        json!([
            "8.8.8.8",
            "8.8.4.4",
            "2001:4860:4860::8888",
            "2001:4860:4860::8844"
        ])
    );
    assert_no_live_port(&main);

    // Negative: off + nothing else -> no hosts merge in either core.
    let input = dns_input(160, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/dns/hosts").is_null());
    let main = generate_singbox(&input).expect("sbox").main;
    assert_eq!(json_at(&main, "/dns/servers/3/predefined"), json!({}));
}

// FLD-CFG-161 complement: FakeIP master switch off -> no fakeip objects.
// (On-path is covered by xray_routing_dns.rs:15 + fixtures_samples.rs:95.)
#[test]
fn fld161_fakeip_off_absent_both_cores() {
    let mut input = dns_input(161, SimpleDns::default(), HappyEyeballs4Ray::default());
    input.routing = Some(CodegenRouting {
        rule_set: vec![dns_rule("proxy", &["full:px.test"])],
        ..Default::default()
    });
    let main = generate_xray(&input).expect("xray").main;
    assert!(main.get("fakedns").is_none());
    let main = generate_singbox(&input).expect("sbox").main;
    let tags: Vec<String> = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s.get("tag").and_then(|v| v.as_str()).map(str::to_string))
        .collect();
    assert!(!tags.contains(&"fake-dns".to_string()), "{tags:?}");
    assert_no_live_port(&main);
}

// FLD-CFG-162: SimpleDNSItem.GlobalFakeIp branch switch.
#[test]
fn fld162_global_fakeip_branch_and_reopen() {
    let routing = CodegenRouting {
        rule_set: vec![
            dns_rule("proxy", &["full:px.test"]),
            dns_rule("direct", &["full:dx.test"]),
        ],
        ..Default::default()
    };
    let base_simple = SimpleDns {
        fake_ip: true,
        ..Default::default()
    };

    // Global (default): fakedns server covers proxy AND direct domains.
    let mut input = dns_input(162, base_simple.clone(), HappyEyeballs4Ray::default());
    input.routing = Some(routing.clone());
    let input = reopen(&input);
    let main = generate_xray(&input).expect("xray").main;
    let fake_server = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("address").and_then(|v| v.as_str()) == Some("fakedns"))
        .expect("fakedns server");
    let domains = fake_server["domains"].as_array().unwrap();
    assert!(domains.iter().any(|d| d.as_str() == Some("full:px.test")));
    assert!(domains.iter().any(|d| d.as_str() == Some("full:dx.test")));
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(
        main["dns"]["rules"].as_array().unwrap().iter().any(|r| {
            r.get("server").and_then(|v| v.as_str()) == Some("fake-dns")
                && r.get("type").and_then(|v| v.as_str()) == Some("logical")
        }),
        "global logical fake rule"
    );
    assert_no_live_port(&main);

    // Non-global: direct domains excluded from the xray fakedns match.
    let simple = SimpleDns {
        global_fake_ip: Some(false),
        ..base_simple.clone()
    };
    let mut input = dns_input(162, simple, HappyEyeballs4Ray::default());
    input.routing = Some(routing.clone());
    let main = generate_xray(&input).expect("xray").main;
    let fake_server = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("address").and_then(|v| v.as_str()) == Some("fakedns"))
        .expect("fakedns server");
    let domains = fake_server["domains"].as_array().unwrap();
    assert!(domains.iter().any(|d| d.as_str() == Some("full:px.test")));
    assert!(!domains.iter().any(|d| d.as_str() == Some("full:dx.test")));
    // singbox: no global logical rule; per-rule fake entries instead.
    let main = generate_singbox(&input).expect("sbox").main;
    let rules = main["dns"]["rules"].as_array().unwrap();
    assert!(
        !rules
            .iter()
            .any(|r| r.get("type").and_then(|v| v.as_str()) == Some("logical")),
        "no global logical rule"
    );
    assert!(
        rules
            .iter()
            .any(|r| r.get("server").and_then(|v| v.as_str()) == Some("fake-dns")),
        "per-rule fake entries"
    );

    // Non-global with direct-only match -> xray emits no fakedns at all.
    let mut input = dns_input(
        162,
        SimpleDns {
            fake_ip: true,
            global_fake_ip: Some(false),
            ..Default::default()
        },
        HappyEyeballs4Ray::default(),
    );
    input.routing = Some(CodegenRouting {
        rule_set: vec![dns_rule("direct", &["full:dx.test"])],
        ..Default::default()
    });
    let main = generate_xray(&input).expect("xray").main;
    assert!(main.get("fakedns").is_none());
}

// FLD-CFG-163: SimpleDNSItem.FakeIPRange pool CIDR.
#[test]
fn fld163_fakeip_range_pool_and_reopen() {
    let routing = CodegenRouting {
        rule_set: vec![dns_rule("proxy", &["full:px.test"])],
        ..Default::default()
    };
    let simple = SimpleDns {
        fake_ip: true,
        fake_ip_range: Some("10.9.0.0/16".into()),
        ..Default::default()
    };
    let mut input = dns_input(163, simple, HappyEyeballs4Ray::default());
    input.routing = Some(routing);
    let input = reopen(&input);
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/fakedns/ipPool"), "10.9.0.0/16");
    assert_eq!(json_at(&main, "/fakedns/poolSize"), json!(65535));
    let main = generate_singbox(&input).expect("sbox").main;
    let fake = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("fake-dns"))
        .expect("fake-dns server");
    assert_eq!(fake["inet4_range"].as_str(), Some("10.9.0.0/16"));
    assert_no_live_port(&main);

    // Negative: unset -> frozen default pool in both cores.
    let simple = SimpleDns {
        fake_ip: true,
        ..Default::default()
    };
    let mut input = dns_input(163, simple, HappyEyeballs4Ray::default());
    input.routing = Some(CodegenRouting {
        rule_set: vec![dns_rule("proxy", &["full:px.test"])],
        ..Default::default()
    });
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/fakedns/ipPool"), "198.18.0.0/15");
    let main = generate_singbox(&input).expect("sbox").main;
    let fake = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("fake-dns"))
        .expect("fake-dns server");
    assert_eq!(fake["inet4_range"].as_str(), Some("198.18.0.0/15"));
}

// FLD-CFG-164: SimpleDNSItem.BlockBindingQuery (sing-box-only leaf; xray has
// no counterpart branch, so xray must simply generate cleanly).
#[test]
fn fld164_block_binding_query_emit() {
    let simple = SimpleDns {
        block_binding_query: true,
        ..Default::default()
    };
    let input = reopen(&dns_input(164, simple, HappyEyeballs4Ray::default()));
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(
        main["dns"]["rules"].as_array().unwrap().iter().any(|r| {
            r.get("query_type") == Some(&json!([64, 65]))
                && r.get("action").and_then(|v| v.as_str()) == Some("predefined")
        }),
        "binding-query filter rule"
    );
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/dns/tag"), "dns-module");
    assert_no_live_port(&main);

    let input = dns_input(164, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(
        !main["dns"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| { r.get("query_type") == Some(&json!([64, 65])) }),
        "no binding-query rule when off"
    );
}

// FLD-CFG-165: SimpleDNSItem.BlockAAAAQuery (both cores).
#[test]
fn fld165_block_aaaa_query_emit_and_reopen() {
    let simple = SimpleDns {
        block_aaaa_query: true,
        ..Default::default()
    };
    let input = reopen(&dns_input(165, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(string_at(&main, "/dns/queryStrategy"), "UseIPv4");
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(
        main["dns"]["rules"].as_array().unwrap().iter().any(|r| {
            r.get("query_type") == Some(&json!([28]))
                && r.get("action").and_then(|v| v.as_str()) == Some("predefined")
        }),
        "aaaa filter rule"
    );
    assert_no_live_port(&main);

    let input = dns_input(165, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/dns/queryStrategy").is_null());
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(
        !main["dns"]["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| { r.get("query_type") == Some(&json!([28])) }),
        "no aaaa rule when off"
    );
}

// FLD-CFG-166: SimpleDNSItem.DirectDNS value reach (xray tagged server +
// sing-box direct-dns-1), with frozen default fallback.
#[test]
fn fld166_direct_dns_value_and_reopen() {
    let simple = SimpleDns {
        direct_dns: Some("9.9.9.9".into()),
        ..Default::default()
    };
    let mut input = dns_input(166, simple, HappyEyeballs4Ray::default());
    input.routing = Some(CodegenRouting {
        rule_set: vec![CodegenRule {
            enabled: true,
            rule_type: RuleType::Routing,
            outbound_tag: "direct".into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    let input = reopen(&input);
    let main = generate_xray(&input).expect("xray").main;
    let direct = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("direct-dns-1"))
        .expect("direct-dns server");
    assert_eq!(direct["address"].as_str(), Some("9.9.9.9"));
    let main = generate_singbox(&input).expect("sbox").main;
    let direct = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("direct-dns-1"))
        .expect("direct-dns server");
    assert_eq!(direct["server"].as_str(), Some("9.9.9.9"));
    assert_no_live_port(&main);

    // Negative: unset -> frozen default 119.29.29.29 in both cores.
    let input = dns_input(166, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_singbox(&input).expect("sbox").main;
    let direct = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("direct-dns-1"))
        .expect("direct-dns server");
    assert_eq!(direct["server"].as_str(), Some("119.29.29.29"));
}

// FLD-CFG-167: SimpleDNSItem.RemoteDNS value reach + default DoH fallback.
#[test]
fn fld167_remote_dns_value_and_reopen() {
    let simple = SimpleDns {
        remote_dns: Some("8.8.8.8".into()),
        ..Default::default()
    };
    let input = reopen(&dns_input(167, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    assert!(
        main["dns"]["servers"].as_array().unwrap().iter().any(|s| {
            s.as_str() == Some("8.8.8.8")
                || s.get("address").and_then(|v| v.as_str()) == Some("8.8.8.8")
        }),
        "remote server present"
    );
    let main = generate_singbox(&input).expect("sbox").main;
    let remote = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("remote-dns-1"))
        .expect("remote-dns server");
    assert_eq!(remote["server"].as_str(), Some("8.8.8.8"));
    assert_eq!(remote["detour"].as_str(), Some("proxy"));
    assert_no_live_port(&main);

    // Negative: unset -> frozen cloudflare DoH default in both cores.
    let input = dns_input(167, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_xray(&input).expect("xray").main;
    assert!(
        main["dns"]["servers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s.as_str() == Some("https://cloudflare-dns.com/dns-query")),
        "default doh"
    );
    let main = generate_singbox(&input).expect("sbox").main;
    let remote = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("remote-dns-1"))
        .expect("remote-dns server");
    assert_eq!(remote["server"].as_str(), Some("cloudflare-dns.com"));
}

// FLD-CFG-168: SimpleDNSItem.BootstrapDNS (sing-box local-local + xray
// bootstrap server for domain-form DoH remotes).
#[test]
fn fld168_bootstrap_dns_value_and_reopen() {
    let simple = SimpleDns {
        bootstrap_dns: Some("9.9.9.9".into()),
        remote_dns: Some("https://dns.example.test/dns-query".into()),
        ..Default::default()
    };
    let input = reopen(&dns_input(168, simple, HappyEyeballs4Ray::default()));
    let main = generate_singbox(&input).expect("sbox").main;
    let local = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("local-local"))
        .expect("local-local server");
    assert_eq!(local["server"].as_str(), Some("9.9.9.9"));
    let main = generate_xray(&input).expect("xray").main;
    assert!(
        main["dns"]["servers"].as_array().unwrap().iter().any(|s| {
            s.get("address").and_then(|v| v.as_str()) == Some("9.9.9.9")
                && s.get("domains")
                    .map(|d| d.to_string().contains("dns.example.test"))
                    .unwrap_or(false)
        }),
        "bootstrap server for domain-form remote"
    );
    assert_no_live_port(&main);

    // Negative: unset -> frozen default 119.29.29.29 (sing-box local-local).
    let input = dns_input(168, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_singbox(&input).expect("sbox").main;
    let local = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("local-local"))
        .expect("local-local server");
    assert_eq!(local["server"].as_str(), Some("119.29.29.29"));
}

// FLD-CFG-169: SimpleDNSItem.Strategy4Freedom -> freedom sockopt.
#[test]
fn fld169_strategy4_freedom_emit_and_reopen() {
    let simple = SimpleDns {
        strategy4_freedom: Some("UseIP".into()),
        ..Default::default()
    };
    let input = reopen(&dns_input(169, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        freedom(&main)
            .pointer("/streamSettings/sockopt/domainStrategy")
            .cloned(),
        Some(json!("UseIP"))
    );
    // No happy block without the master switch (G-01 gate holds here too).
    assert!(freedom(&main)
        .pointer("/streamSettings/sockopt/happyEyeballs")
        .is_none());
    assert_no_live_port(&main);

    for strategy in [None, Some("AsIs".to_string())] {
        let simple = SimpleDns {
            strategy4_freedom: strategy,
            ..Default::default()
        };
        let input = dns_input(169, simple, HappyEyeballs4Ray::default());
        let main = generate_xray(&input).expect("xray").main;
        assert!(
            freedom(&main)
                .pointer("/streamSettings/sockopt/domainStrategy")
                .is_none(),
            "no strategy emitted"
        );
    }
}

// FLD-CFG-170: SimpleDNSItem.Strategy4Proxy -> proxy targetStrategy.
#[test]
fn fld170_strategy4_proxy_emit_and_reopen() {
    let simple = SimpleDns {
        strategy4_proxy: Some("UseIP".into()),
        ..Default::default()
    };
    let input = reopen(&dns_input(170, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/targetStrategy"),
        json!("UseIP")
    );
    assert_no_live_port(&main);

    for strategy in [None, Some("AsIs".to_string())] {
        let simple = SimpleDns {
            strategy4_proxy: strategy,
            ..Default::default()
        };
        let input = dns_input(170, simple, HappyEyeballs4Ray::default());
        let main = generate_xray(&input).expect("xray").main;
        assert!(json_at(&main, "/outbounds/0/targetStrategy").is_null());
    }
}

// FLD-CFG-171: SimpleDNSItem.Strategy4ProxyDial (xray dial sockopt +
// sing-box default_domain_resolver strategy).
#[test]
fn fld171_strategy4_proxy_dial_emit_and_reopen() {
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        ..Default::default()
    };
    let input = reopen(&dns_input(171, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/sockopt/domainStrategy"),
        json!("UseIP")
    );
    let main = generate_singbox(&input).expect("sbox").main;
    assert_eq!(
        json_at(&main, "/route/default_domain_resolver/strategy"),
        json!("prefer_ipv4")
    );
    assert_no_live_port(&main);

    let input = dns_input(171, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/outbounds/0/streamSettings/sockopt/domainStrategy").is_null());
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(json_at(&main, "/route/default_domain_resolver/strategy").is_null());
}

// FLD-CFG-172: SimpleDNSItem.ServeStale (xray always-written key; sing-box
// optimistic flag only when on).
#[test]
fn fld172_serve_stale_emit_and_reopen() {
    let simple = SimpleDns {
        serve_stale: true,
        ..Default::default()
    };
    let input = reopen(&dns_input(172, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/dns/serveStale"), json!(true));
    let main = generate_singbox(&input).expect("sbox").main;
    assert_eq!(json_at(&main, "/dns/optimistic"), json!(true));
    assert_no_live_port(&main);

    let input = dns_input(172, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/dns/serveStale"), json!(false));
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(json_at(&main, "/dns/optimistic").is_null());
}

// FLD-CFG-173: SimpleDNSItem.ParallelQuery (xray flag + sing-box race fan-out
// across the two synthetic remote servers).
#[test]
fn fld173_parallel_query_emit_and_reopen() {
    let simple = SimpleDns {
        parallel_query: true,
        remote_dns: Some("1.1.1.1,8.8.8.8".into()),
        ..Default::default()
    };
    let input = reopen(&dns_input(173, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(json_at(&main, "/dns/enableParallelQuery"), json!(true));
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(
        main["dns"]["rules"].as_array().unwrap().iter().any(|r| {
            r.get("action").and_then(|v| v.as_str()) == Some("respond")
                && r.get("race").and_then(|v| v.as_bool()) == Some(true)
        }),
        "race fan-out"
    );
    assert_no_live_port(&main);

    let simple = SimpleDns {
        remote_dns: Some("1.1.1.1,8.8.8.8".into()),
        ..Default::default()
    };
    let input = dns_input(173, simple, HappyEyeballs4Ray::default());
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/dns/enableParallelQuery").is_null());
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(
        !main["dns"]["rules"].to_string().contains("\"race\""),
        "no race when off"
    );
}

// FLD-CFG-174: SimpleDNSItem.Hosts custom text merge (both cores).
#[test]
fn fld174_custom_hosts_merge_and_reopen() {
    let simple = SimpleDns {
        hosts: Some("example.test 93.184.216.34".into()),
        ..Default::default()
    };
    let input = reopen(&dns_input(174, simple, HappyEyeballs4Ray::default()));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/dns/hosts/example.test"),
        json!(["93.184.216.34"])
    );
    let main = generate_singbox(&input).expect("sbox").main;
    let predefined = json_at(&main, "/dns/servers/3/predefined");
    assert_eq!(predefined["example.test"], json!(["93.184.216.34"]));
    assert_no_live_port(&main);

    // Negative: empty -> merge skipped in both cores.
    let input = dns_input(174, SimpleDns::default(), HappyEyeballs4Ray::default());
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/dns/hosts").is_null());
    let main = generate_singbox(&input).expect("sbox").main;
    assert_eq!(json_at(&main, "/dns/servers/3/predefined"), json!({}));
}

// FLD-CFG-175: SimpleDNSItem.DirectExpectedIPs (xray expectedIPs +
// sing-box geoip on the matching expected-domain rule).
#[test]
fn fld175_direct_expected_ips_emit_and_reopen() {
    let routing = CodegenRouting {
        rule_set: vec![dns_rule("direct", &["geosite:cn"])],
        ..Default::default()
    };
    let simple = SimpleDns {
        direct_expected_ips: Some("geoip:cn".into()),
        ..Default::default()
    };
    let mut input = dns_input(175, simple, HappyEyeballs4Ray::default());
    input.routing = Some(routing);
    let input = reopen(&input);
    let main = generate_xray(&input).expect("xray").main;
    assert!(
        main["dns"]["servers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| { s.get("expectedIPs") == Some(&json!(["geoip:cn"])) }),
        "expectedIPs server"
    );
    let main = generate_singbox(&input).expect("sbox").main;
    // sing-box converts geoip into an srs rule_set ref (ruleset.rs): the
    // expected-IP effect surfaces as rule_set ["geoip-cn"] on the respond
    // rule plus a matching route.rule_set entry.
    assert!(
        main["dns"]["rules"].as_array().unwrap().iter().any(|r| {
            r.get("action").and_then(|v| v.as_str()) == Some("respond")
                && r.get("rule_set") == Some(&json!(["geoip-cn"]))
        }),
        "geoip respond rule"
    );
    assert!(
        main["route"]["rule_set"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| { s.get("tag").and_then(|v| v.as_str()) == Some("geoip-cn") }),
        "geoip-cn rule_set entry"
    );
    assert_no_live_port(&main);

    // Negative: unset -> no expectedIPs / geoip anywhere.
    let mut input = dns_input(175, SimpleDns::default(), HappyEyeballs4Ray::default());
    input.routing = Some(CodegenRouting {
        rule_set: vec![dns_rule("direct", &["geosite:cn"])],
        ..Default::default()
    });
    let main = generate_xray(&input).expect("xray").main;
    assert!(!main["dns"]["servers"].to_string().contains("expectedIPs"));
    let main = generate_singbox(&input).expect("sbox").main;
    assert!(!main.to_string().contains("geoip-cn"));
}

// G-01 (FLD-CFG-176): gate ON at the freedom site emits the block.
// (OFF both sites is covered by sp24_happy_off_*; this is the ON complement.)
#[test]
fn fld176_happy_gate_on_freedom_emits_block() {
    let simple = SimpleDns {
        strategy4_freedom: Some("UseIP".into()),
        enable_happy_eyeballs: true,
        ..Default::default()
    };
    let happy = HappyEyeballs4Ray {
        try_delay_ms: Some(300),
        prioritize_ipv6: Some(true),
        interleave: Some(2),
        max_concurrent_try: Some(3),
    };
    let input = reopen(&dns_input(176, simple, happy));
    let main = generate_xray(&input).expect("xray").main;
    let block = freedom(&main).pointer("/streamSettings/sockopt/happyEyeballs");
    assert_eq!(
        block.cloned(),
        Some(json!({
            "tryDelayMs": 300,
            "prioritizeIPv6": true,
            "interleave": 2,
            "maxConcurrentTry": 3
        })),
        "freedom block"
    );
    assert_no_live_port(&main);
}

// G-01 (FLD-CFG-177): TryDelayMs single-param truth table. On-path is covered
// by xray_routing_dns.rs:247 + sp24; here: isolated param + OFF gate.
#[test]
fn fld177_try_delay_single_param_gate() {
    let happy = HappyEyeballs4Ray {
        try_delay_ms: Some(300),
        ..Default::default()
    };
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        enable_happy_eyeballs: true,
        ..Default::default()
    };
    let input = reopen(&dns_input(177, simple, happy));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs"),
        json!({"tryDelayMs": 300})
    );
    assert_no_live_port(&main);

    let happy = HappyEyeballs4Ray {
        try_delay_ms: Some(300),
        ..Default::default()
    };
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        enable_happy_eyeballs: false,
        ..Default::default()
    };
    let input = dns_input(177, simple, happy);
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/sockopt/domainStrategy"),
        json!("UseIP")
    );
    assert!(json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs").is_null());
}

// G-01 (FLD-CFG-178): PrioritizeIPv6 single-param truth table.
#[test]
fn fld178_prioritize_ipv6_single_param_gate() {
    let happy = HappyEyeballs4Ray {
        prioritize_ipv6: Some(true),
        ..Default::default()
    };
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        enable_happy_eyeballs: true,
        ..Default::default()
    };
    let input = reopen(&dns_input(178, simple, happy));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs"),
        json!({"prioritizeIPv6": true})
    );
    assert_no_live_port(&main);

    let happy = HappyEyeballs4Ray {
        prioritize_ipv6: Some(true),
        ..Default::default()
    };
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        enable_happy_eyeballs: false,
        ..Default::default()
    };
    let input = dns_input(178, simple, happy);
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs").is_null());
}

// G-01 (FLD-CFG-179): Interleave single-param truth table.
#[test]
fn fld179_interleave_single_param_gate() {
    let happy = HappyEyeballs4Ray {
        interleave: Some(2),
        ..Default::default()
    };
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        enable_happy_eyeballs: true,
        ..Default::default()
    };
    let input = reopen(&dns_input(179, simple, happy));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs"),
        json!({"interleave": 2})
    );
    assert_no_live_port(&main);

    let happy = HappyEyeballs4Ray {
        interleave: Some(2),
        ..Default::default()
    };
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        enable_happy_eyeballs: false,
        ..Default::default()
    };
    let input = dns_input(179, simple, happy);
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs").is_null());
}

// G-01 (FLD-CFG-180): MaxConcurrentTry single-param truth table.
#[test]
fn fld180_max_concurrent_try_single_param_gate() {
    let happy = HappyEyeballs4Ray {
        max_concurrent_try: Some(3),
        ..Default::default()
    };
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        enable_happy_eyeballs: true,
        ..Default::default()
    };
    let input = reopen(&dns_input(180, simple, happy));
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs"),
        json!({"maxConcurrentTry": 3})
    );
    assert_no_live_port(&main);

    let happy = HappyEyeballs4Ray {
        max_concurrent_try: Some(3),
        ..Default::default()
    };
    let simple = SimpleDns {
        strategy4_proxy_dial: Some("UseIP".into()),
        enable_happy_eyeballs: false,
        ..Default::default()
    };
    let input = dns_input(180, simple, happy);
    let main = generate_xray(&input).expect("xray").main;
    assert!(json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs").is_null());
}
