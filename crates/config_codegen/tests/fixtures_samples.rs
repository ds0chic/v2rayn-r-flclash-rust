mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::generate_xray;
use config_codegen::input::*;
use serde_json::Value;

fn vless_base() -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.160", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p
}

#[test]
fn xray_raw_dns_from_fixture() {
    let raw = read_sample("dns_v2ray_normal");
    let mut input = codegen_input(vless_base());
    input.dns = Some(CodegenDns {
        enabled: true,
        normal: Some(raw),
        ..Default::default()
    });

    let generated = generate_xray(&input).expect("raw dns");
    let main = &generated.main;
    assert_eq!(string_at(main, "/dns/tag"), "dns-module");
    assert_eq!(json_at(main, "/dns/servers").as_array().unwrap().len(), 5);
    assert_eq!(string_at(main, "/dns/servers/0/address"), "1.1.1.1");
    // string hosts are converted to arrays (GenDnsCustom).
    assert_eq!(
        json_at(main, "/dns/hosts/dns.google"),
        serde_json::json!(["8.8.8.8"])
    );
    let rules = main["routing"]["rules"].as_array().unwrap();
    assert!(rules.iter().any(|r| {
        r.get("inboundTag")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().any(|x| x.as_str() == Some("dns-module")))
            .unwrap_or(false)
    }));
}

#[test]
fn singbox_raw_dns_from_fixture() {
    let raw = read_sample("dns_singbox_normal");
    let mut input = codegen_input(vless_base());
    input.dns = Some(CodegenDns {
        enabled: true,
        normal: Some(raw),
        ..Default::default()
    });

    let generated = generate_singbox(&input).expect("raw dns");
    let main = &generated.main;
    assert_eq!(string_at(main, "/dns/final"), "remote");
    let tags: Vec<String> = main["dns"]["servers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| {
            s.get("tag")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(tags, vec!["remote", "local", "local-local"]);
    let rules = main["dns"]["rules"].as_array().unwrap();
    assert_eq!(rules[0]["clash_mode"].as_str(), Some("Global"));
    assert_eq!(rules[1]["clash_mode"].as_str(), Some("Direct"));
    assert_eq!(
        json_at(main, "/dns/rules/2/rule_set"),
        serde_json::json!(["geosite-google"])
    );
    // geosite rule sets are collected into route.rule_set + http_clients.
    let route_sets: Vec<String> = main["route"]["rule_set"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| {
            s.get("tag")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert!(route_sets.contains(&"geosite-google".to_string()));
    assert!(route_sets.contains(&"geosite-cn".to_string()));
    assert_eq!(
        string_at(main, "/http_clients/0/tag"),
        "srs-download-http-client"
    );
}

#[test]
fn singbox_fakeip_filter_from_fixture() {
    let filter_text = read_sample("singbox_fakeip_filter");
    let filter: Value = serde_json::from_str(&filter_text).expect("filter json");
    let mut input = codegen_input(vless_base());
    input.singbox_fakeip_filter = Some(filter);
    input.dns = Some(CodegenDns {
        simple: SimpleDns {
            fake_ip: true,
            ..Default::default()
        },
        ..Default::default()
    });

    let generated = generate_singbox(&input).expect("fakeip");
    let main = &generated.main;
    let rules = main["dns"]["rules"].as_array().unwrap();
    let fake_rule = rules
        .iter()
        .find(|r| r.get("server").and_then(Value::as_str) == Some("fake-dns"))
        .expect("fake-dns rule");
    assert_eq!(fake_rule["type"].as_str(), Some("logical"));
    assert_eq!(fake_rule["rules"][1]["invert"].as_bool(), Some(true));
    assert!(fake_rule["rules"][1]["domain_keyword"].is_array());
    let servers = main["dns"]["servers"].as_array().unwrap();
    assert!(servers
        .iter()
        .any(|s| s.get("type").and_then(Value::as_str) == Some("fakeip")));
}

#[test]
fn xray_tun_rules_from_fixture() {
    let tun_rules: Value = serde_json::from_str(&read_sample("SampleTunRules")).expect("tun rules");
    let mut settings = settings();
    settings.tun.enabled = true;
    let mut input = codegen_input(vless_base());
    input.tun_rules = Some(tun_rules);
    input.settings = settings;

    let generated = generate_xray(&input).expect("tun");
    let main = &generated.main;
    assert_eq!(string_at(main, "/routing/rules/0/network"), "udp");
    assert_eq!(string_at(main, "/routing/rules/0/outboundTag"), "block");
    assert_eq!(
        json_at(main, "/routing/rules/1/ip"),
        serde_json::json!(["224.0.0.0/3", "ff00::/8"])
    );
}

#[test]
fn singbox_tun_matches_fixture_shape() {
    // The fixture is the upstream sample; the generated TUN must match it for
    // the same TunModeItem values (line by line against tun_singbox_inbound).
    let mut settings = settings();
    settings.tun.enabled = true;
    settings.tun.mtu = 9000;
    settings.tun.ipv4_address = Some("172.18.0.1/30".into());
    settings.tun.enable_ipv6_address = true;
    settings.tun.ipv6_address = Some("fdfe:dcba:9876::1/126".into());
    settings.tun.stack = Some("system".into());
    let mut input = codegen_input(vless_base());
    input.settings = settings;

    let generated = generate_singbox(&input).expect("tun");
    let main = &generated.main;
    assert_eq!(string_at(main, "/inbounds/1/type"), "tun");
    assert_eq!(string_at(main, "/inbounds/1/tag"), "tun");
    assert_eq!(string_at(main, "/inbounds/1/interface_name"), "singbox_tun");
    assert_eq!(json_at(main, "/inbounds/1/mtu"), 9000);
    assert_eq!(
        json_at(main, "/inbounds/1/address"),
        serde_json::json!(["172.18.0.1/30", "fdfe:dcba:9876::1/126"])
    );
    assert_eq!(json_at(main, "/inbounds/1/auto_route"), true);
    assert_eq!(json_at(main, "/inbounds/1/strict_route"), false);
    assert_eq!(string_at(main, "/inbounds/1/stack"), "system");
    assert!(read_sample("tun_singbox_inbound").contains("singbox_tun"));
}
