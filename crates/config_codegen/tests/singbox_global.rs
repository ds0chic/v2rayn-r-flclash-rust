mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::input::*;
use serde_json::json;

fn vless_base() -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.110", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p
}

#[test]
fn singbox_dns_and_routing_rule_types() {
    let routing = CodegenRouting {
        rule_set: vec![
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Routing,
                outbound_tag: "proxy".into(),
                domain: Some(vec!["full:route.test".into()]),
                ..Default::default()
            },
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Dns,
                outbound_tag: "direct".into(),
                domain: Some(vec!["full:dns.test".into()]),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let dns = CodegenDns {
        simple: SimpleDns {
            direct_dns: Some("223.5.5.5".into()),
            remote_dns: Some("1.1.1.1".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut codegen_input = codegen_input(vless_base());
    codegen_input.routing = Some(routing);
    codegen_input.dns = Some(dns);

    let generated = generate_singbox(&codegen_input).expect("dns rule types");
    let main = &generated.main;
    // DNS rules are skipped by routing generation.
    let route_has = |needle: &str| {
        main["route"]["rules"].as_array().unwrap().iter().any(|r| {
            r.get("domain")
                .and_then(|d| d.as_array())
                .map(|list| list.iter().any(|v| v.as_str() == Some(needle)))
                .unwrap_or(false)
        })
    };
    assert!(route_has("route.test"));
    assert!(!route_has("dns.test"));
    // Routing rules are skipped by DNS generation; the DNS rule uses direct-dns-1.
    let dns_rule = main["dns"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r.get("domain")
                .and_then(|d| d.as_array())
                .map(|list| list.iter().any(|v| v.as_str() == Some("dns.test")))
                .unwrap_or(false)
        })
        .expect("dns rule");
    assert_eq!(dns_rule["server"].as_str(), Some("direct-dns-1"));
}

#[test]
fn singbox_log_and_inbound_ports() {
    let mut settings = settings();
    settings.core_basic.loglevel = "warning".into();
    settings.core_basic.log_enabled = true;
    settings.inbound.second_local_port_enabled = true;
    settings.inbound.allow_lan_conn = true;
    settings.inbound.new_port4_lan = true;
    settings.inbound.user = "lanuser".into();
    settings.inbound.pass = "lanpass".into();
    let mut input = codegen_input(vless_base());
    input.settings = settings;

    let generated = generate_singbox(&input).expect("log");
    let main = &generated.main;
    assert_eq!(string_at(main, "/log/level"), "warn");
    assert_eq!(string_at(main, "/log/output"), "logs/sbox_2026-10-01.txt");
    assert_eq!(json_at(main, "/log/timestamp"), json!(true));

    assert_eq!(string_at(main, "/inbounds/0/type"), "mixed");
    assert_eq!(string_at(main, "/inbounds/0/tag"), "socks");
    assert_eq!(json_at(main, "/inbounds/0/listen_port"), json!(11808));
    assert_eq!(json_at(main, "/inbounds/1/listen_port"), json!(11809));
    assert_eq!(json_at(main, "/inbounds/2/listen_port"), json!(11810));
    assert_eq!(string_at(main, "/inbounds/2/listen"), "0.0.0.0");
    assert_eq!(string_at(main, "/inbounds/2/users/0/username"), "lanuser");
    assert_eq!(string_at(main, "/inbounds/2/users/0/password"), "lanpass");
}

#[test]
fn singbox_tun_inbound_and_rules() {
    let mut settings = settings();
    settings.tun.enabled = true;
    settings.tun.mtu = 1420;
    settings.tun.ipv4_address = Some("172.18.0.1/30".into());
    settings.tun.enable_ipv6_address = true;
    settings.tun.ipv6_address = Some("fc00::172:18:0:1/126".into());
    settings.tun.route_exclude_address = vec!["10.0.0.0/8".into()];
    settings.protect_core_executables = vec!["sing-box.exe".into()];
    settings.has_global_ipv6_address = true;
    let mut input = codegen_input(vless_base());
    input.settings = settings;

    let generated = generate_singbox(&input).expect("tun");
    let main = &generated.main;
    assert_eq!(string_at(main, "/inbounds/1/type"), "tun");
    assert_eq!(string_at(main, "/inbounds/1/interface_name"), "singbox_tun");
    assert_eq!(json_at(main, "/inbounds/1/mtu"), json!(1420));
    assert_eq!(string_at(main, "/inbounds/1/stack"), "gvisor");
    assert_eq!(
        json_at(main, "/inbounds/1/address"),
        json!(["172.18.0.1/30", "fc00::172:18:0:1/126"])
    );
    assert_eq!(json_at(main, "/route/auto_detect_interface"), json!(true));
    assert_eq!(string_at(main, "/route/final"), "proxy");

    let rules = main["route"]["rules"].as_array().unwrap();
    // sample tun rules + single-address reject + direct exe + sniff rules
    assert_eq!(string_at(main, "/route/rules/0/action"), "reject");
    let reject_cidr = rules
        .iter()
        .find(|r| {
            r.get("ip_cidr")
                .and_then(|v| v.as_array())
                .map(|list| list.iter().any(|x| x.as_str() == Some("172.18.0.1/32")))
                .unwrap_or(false)
        })
        .expect("tun address reject");
    assert_eq!(
        reject_cidr["ip_cidr"],
        json!(["172.18.0.1/32", "fc00::172:18:0:1/128"])
    );
    assert!(rules
        .iter()
        .any(|r| r.get("action").and_then(|v| v.as_str()) == Some("sniff")));
    assert!(rules.iter().any(|r| {
        r.get("action").and_then(|v| v.as_str()) == Some("hijack-dns")
            && r.get("mode").and_then(|v| v.as_str()) == Some("or")
    }));
}

#[test]
fn singbox_dns_and_experimental() {
    let dns = CodegenDns {
        simple: SimpleDns {
            direct_dns: Some("223.5.5.5".into()),
            remote_dns: Some("1.1.1.1".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut settings = settings();
    settings.core_basic.enable_cache_file4_sbox = true;
    let mut input = codegen_input(vless_base());
    input.dns = Some(dns);
    input.settings = settings;

    let generated = generate_singbox(&input).expect("dns");
    let main = &generated.main;
    assert_eq!(
        string_at(main, "/experimental/clash_api/external_controller"),
        "127.0.0.1:11810"
    );
    assert_eq!(
        json_at(main, "/experimental/cache_file/enabled"),
        json!(true)
    );
    assert_eq!(
        string_at(main, "/experimental/cache_file/path"),
        "bin/cache.db"
    );
    assert_eq!(
        json_at(main, "/experimental/cache_file/store_fakeip"),
        json!(false)
    );

    assert_eq!(string_at(main, "/dns/final"), "remote-dns-1");
    assert_eq!(
        string_at(main, "/route/default_domain_resolver/server"),
        "direct-dns-1"
    );
    let servers = main["dns"]["servers"].as_array().unwrap();
    let tags: Vec<String> = servers
        .iter()
        .filter_map(|s| {
            s.get("tag")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert_eq!(
        tags,
        vec!["local-local", "remote-dns-1", "direct-dns-1", "hosts-dns"]
    );
    assert_eq!(string_at(main, "/dns/servers/0/server"), "119.29.29.29");
    assert_eq!(string_at(main, "/dns/servers/1/detour"), "proxy");
    assert_eq!(
        json_at(main, "/dns/rules/0/preferred_by"),
        json!("hosts-dns")
    );
}

#[test]
fn singbox_rule_set_conversion() {
    let routing = CodegenRouting {
        rule_set: vec![CodegenRule {
            enabled: true,
            rule_type: RuleType::Routing,
            outbound_tag: "proxy".into(),
            domain: Some(vec!["geosite:google".into()]),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut input = codegen_input(vless_base());
    input.routing = Some(routing.clone());

    let generated = generate_singbox(&input).expect("ruleset");
    let main = &generated.main;
    let route_rule = main["route"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("rule_set").is_some())
        .expect("rule_set rule");
    assert_eq!(route_rule["rule_set"], json!(["geosite-google"]));
    assert_eq!(string_at(main, "/route/rule_set/0/tag"), "geosite-google");
    assert_eq!(string_at(main, "/route/rule_set/0/type"), "remote");
    assert_eq!(string_at(main, "/route/rule_set/0/format"), "binary");
    let url = string_at(main, "/route/rule_set/0/url");
    assert!(url.contains("rule-set-geosite/geosite-google.srs"), "{url}");
    assert_eq!(
        string_at(main, "/http_clients/0/tag"),
        "srs-download-http-client"
    );
    assert_eq!(string_at(main, "/http_clients/0/detour"), "proxy");

    // Local srs presence switches to a local rule_set and drops http_clients.
    let mut settings = settings();
    settings.local_srs_files.insert("geosite-google".into());
    let mut input = codegen_input(vless_base());
    input.routing = Some(routing);
    input.settings = settings;
    let generated = generate_singbox(&input).expect("local srs");
    assert_eq!(
        string_at(&generated.main, "/route/rule_set/0/type"),
        "local"
    );
    assert_eq!(
        string_at(&generated.main, "/route/rule_set/0/path"),
        "bin/srss/geosite-google.srs"
    );
    assert!(json_at(&generated.main, "/http_clients").is_null());
}

#[test]
fn singbox_bind_interface_and_final_fragment() {
    let mut settings = settings();
    settings.core_basic.bind_interface = Some("wg0".into());
    settings.core_basic.send_through = Some("192.0.2.210".into());
    settings.core_basic.enable_final_fragment = true;
    let mut input = codegen_input(vless_base());
    input.settings = settings;

    let generated = generate_singbox(&input).expect("bind");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/bind_interface"), "wg0");
    assert_eq!(
        string_at(main, "/outbounds/0/inet4_bind_address"),
        "192.0.2.210"
    );
    let rules = main["route"]["rules"].as_array().unwrap();
    assert!(rules.iter().any(|r| {
        r.get("action").and_then(|v| v.as_str()) == Some("route-options")
            && r.get("tls_record_fragment").and_then(|v| v.as_bool()) == Some(true)
    }));
    assert_no_live_port(main);
}
