mod common;

use common::*;
use config_codegen::generate_xray;
use config_codegen::input::*;
use serde_json::json;

fn vless_base() -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.80", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p
}

#[test]
fn xray_simple_dns_fakeip_and_rule_types() {
    let routing = CodegenRouting {
        rule_set: vec![
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Routing,
                outbound_tag: "direct".into(),
                domain: Some(vec!["full:direct.example.test".into()]),
                ..Default::default()
            },
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Routing,
                outbound_tag: "proxy".into(),
                domain: Some(vec!["geosite:google".into()]),
                ..Default::default()
            },
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Dns,
                outbound_tag: "direct".into(),
                domain: Some(vec!["full:dns.example.test".into()]),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let dns = CodegenDns {
        simple: SimpleDns {
            direct_dns: Some("223.5.5.5".into()),
            remote_dns: Some("1.1.1.1".into()),
            fake_ip: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut input = codegen_input(vless_base());
    input.routing = Some(routing);
    input.dns = Some(dns);
    input.settings.tun.enabled = true;

    let generated = generate_xray(&input).expect("dns");
    let main = &generated.main;

    // fakedns is a top-level member (UNC-X-003).
    assert_eq!(string_at(main, "/fakedns/ipPool"), "198.18.0.0/15");
    assert!(json_at(main, "/fakedns/poolSize").as_i64().unwrap() > 0);

    let servers = main["dns"]["servers"].as_array().unwrap();
    assert!(servers
        .iter()
        .any(|s| s.get("address").and_then(|v| v.as_str()) == Some("fakedns")));
    let direct_server = servers
        .iter()
        .find(|s| s.get("tag").and_then(|v| v.as_str()) == Some("direct-dns-1"))
        .expect("direct-dns server");
    let domains: Vec<String> = direct_server["domains"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect();
    // DNS uses RuleType==DNS entries only; the Routing rule is skipped here.
    assert!(domains.contains(&"full:dns.example.test".to_string()));
    assert!(!domains.contains(&"full:direct.example.test".to_string()));
    assert_eq!(string_at(main, "/dns/tag"), "dns-module");
    let tun = main["inbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|inbound| inbound.get("tag").and_then(|v| v.as_str()) == Some("tun"))
        .expect("TUN inbound");
    assert!(tun["sniffing"]["destOverride"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str() == Some("fakedns")));

    // RuleType==DNS is skipped by routing, RuleType==Routing is skipped by DNS.
    let rules = main["routing"]["rules"].as_array().unwrap();
    let has_domain = |needle: &str| {
        rules.iter().any(|r| {
            r.get("domain")
                .and_then(|d| d.as_array())
                .map(|list| list.iter().any(|v| v.as_str() == Some(needle)))
                .unwrap_or(false)
        })
    };
    assert!(has_domain("full:direct.example.test"));
    assert!(!has_domain("full:dns.example.test"));
    assert!(rules.iter().any(|r| {
        r.get("inboundTag")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().any(|x| x.as_str() == Some("direct-dns-1")))
            .unwrap_or(false)
            && r.get("outboundTag").and_then(|v| v.as_str()) == Some("direct")
    }));
    assert!(rules.iter().any(|r| {
        r.get("inboundTag")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().any(|x| x.as_str() == Some("dns-module")))
            .unwrap_or(false)
            && r.get("outboundTag").and_then(|v| v.as_str()) == Some("proxy")
    }));
    assert_eq!(string_at(main, "/routing/domainStrategy"), "IPIfNonMatch");
}

#[test]
fn xray_routing_protocol_condition_is_written() {
    // ISSUE-03: the `protocol` filter must reach the generated rule JSON.
    let routing = CodegenRouting {
        rule_set: vec![CodegenRule {
            enabled: true,
            rule_type: RuleType::Routing,
            outbound_tag: "block".into(),
            protocol: Some(vec!["bittorrent".into()]),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut input = codegen_input(vless_base());
    input.routing = Some(routing);

    let generated = generate_xray(&input).expect("protocol rule");
    let rules = generated.main["routing"]["rules"].as_array().unwrap();
    let rule = rules
        .iter()
        .find(|r| r.get("protocol").is_some())
        .expect("protocol rule present");
    assert_eq!(json_at(rule, "/protocol"), json!(["bittorrent"]));
    assert_eq!(string_at(rule, "/outboundTag"), "block");
    assert_eq!(string_at(rule, "/type"), "field");
}

#[test]
fn xray_routing_dangling_reference_warns() {
    // ISSUE-04: unresolved remarks still fall back to `proxy` (upstream
    // behavior) but must emit a structured warning, not stay silent.
    let routing = CodegenRouting {
        rule_set: vec![CodegenRule {
            enabled: true,
            rule_type: RuleType::Routing,
            outbound_tag: "Missing Node".into(),
            domain: Some(vec!["full:missing.test".into()]),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut input = codegen_input(vless_base());
    input.routing = Some(routing);

    let generated = generate_xray(&input).expect("routing");
    let rules = generated.main["routing"]["rules"].as_array().unwrap();
    let rule = rules
        .iter()
        .find(|r| {
            r.get("domain")
                .and_then(|d| d.as_array())
                .map(|list| list.iter().any(|v| v.as_str() == Some("full:missing.test")))
                .unwrap_or(false)
        })
        .expect("fallback rule");
    assert_eq!(string_at(rule, "/outboundTag"), "proxy");
    let warning = generated
        .diagnostics
        .iter()
        .find(|d| d.code == "routing_dangling_reference")
        .expect("dangling warning");
    assert!(
        warning.message.contains("Missing Node"),
        "{}",
        warning.message
    );
}

#[test]
fn xray_routing_remark_resolution_and_fallback() {
    let mut other = profile(ConfigType::Vless, "192.0.2.81", 443);
    other.index_id = "node-2".into();
    other.remarks = "Node B".into();
    other.password = "11111111-2222-3333-4444-555555555555".into();

    let routing = CodegenRouting {
        rule_set: vec![
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Routing,
                outbound_tag: "Node B".into(),
                domain: Some(vec!["full:xb.test".into()]),
                ..Default::default()
            },
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Routing,
                outbound_tag: "Missing Node".into(),
                domain: Some(vec!["full:missing.test".into()]),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut input = codegen_input(vless_base());
    input.profiles.insert(other.index_id.clone(), other);
    input.routing = Some(routing);

    let generated = generate_xray(&input).expect("routing");
    let main = &generated.main;
    let rules = main["routing"]["rules"].as_array().unwrap();
    let by_domain = |domain: &str| {
        rules
            .iter()
            .find(|r| {
                r.get("domain")
                    .and_then(|d| d.as_array())
                    .map(|list| list.iter().any(|v| v.as_str() == Some(domain)))
                    .unwrap_or(false)
            })
            .expect("rule")
    };
    assert_eq!(
        string_at(by_domain("full:xb.test"), "/outboundTag"),
        "node-2-proxy-Node B"
    );
    assert_eq!(
        string_at(by_domain("full:missing.test"), "/outboundTag"),
        "proxy"
    );
    let tags: Vec<String> = main["outbounds"]
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
        tags.contains(&"node-2-proxy-Node B".to_string()),
        "{tags:?}"
    );
}

#[test]
fn xray_proxy_chain_and_happy_eyeballs() {
    let mut chain = profile(ConfigType::ProxyChain, "", 0);
    chain.index_id = "chain-1".into();
    chain.proto_extra.child_items = Some("c1,c2".into());
    let mut c1 = profile(ConfigType::Vless, "192.0.2.84", 443);
    c1.index_id = "c1".into();
    c1.remarks = "c1".into();
    c1.password = "11111111-2222-3333-4444-555555555555".into();
    let mut c2 = profile(ConfigType::Vless, "192.0.2.85", 443);
    c2.index_id = "c2".into();
    c2.remarks = "c2".into();
    c2.password = "11111111-2222-3333-4444-555555555555".into();

    let dns = CodegenDns {
        simple: SimpleDns {
            strategy4_proxy_dial: Some("UseIP".into()),
            enable_happy_eyeballs: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut settings = settings();
    settings.happy_eyeballs4_ray.try_delay_ms = Some(300);
    settings.happy_eyeballs4_ray.prioritize_ipv6 = Some(true);
    let mut codegen_input = codegen_input(chain);
    codegen_input.profiles.insert(c1.index_id.clone(), c1);
    codegen_input.profiles.insert(c2.index_id.clone(), c2);
    codegen_input.dns = Some(dns);
    codegen_input.settings = settings;

    let generated = generate_xray(&codegen_input).expect("chain");
    let main = &generated.main;
    // Chain order: c2 -> c1 (reverse).
    assert_eq!(string_at(main, "/outbounds/0/tag"), "proxy");
    assert_eq!(
        string_at(main, "/outbounds/0/settings/address"),
        "192.0.2.85"
    );
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/sockopt/dialerProxy"),
        "chain-proxy-1-c1"
    );
    assert_eq!(string_at(main, "/outbounds/1/tag"), "chain-proxy-1-c1");
    assert_eq!(
        string_at(main, "/outbounds/1/settings/address"),
        "192.0.2.84"
    );
    // Dial strategy + happy eyeballs injected on the proxy outbounds.
    assert_eq!(
        string_at(main, "/outbounds/0/streamSettings/sockopt/domainStrategy"),
        "UseIP"
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/sockopt/happyEyeballs/tryDelayMs"
        ),
        json!(300)
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/sockopt/happyEyeballs/prioritizeIPv6"
        ),
        json!(true)
    );
}

#[test]
fn xray_policy_group_balancer() {
    let mut group = profile(ConfigType::PolicyGroup, "", 0);
    group.index_id = "group-1".into();
    group.proto_extra.child_items = Some("c1,c2".into());
    group.proto_extra.multiple_load = Some(MultipleLoad::LeastPing);
    let mut c1 = profile(ConfigType::Vless, "192.0.2.82", 443);
    c1.index_id = "c1".into();
    c1.remarks = "c1".into();
    c1.password = "11111111-2222-3333-4444-555555555555".into();
    let mut c2 = profile(ConfigType::Vless, "192.0.2.83", 443);
    c2.index_id = "c2".into();
    c2.remarks = "c2".into();
    c2.password = "11111111-2222-3333-4444-555555555555".into();

    let routing = CodegenRouting {
        rule_set: vec![CodegenRule {
            enabled: true,
            rule_type: RuleType::Routing,
            outbound_tag: "proxy".into(),
            domain: Some(vec!["full:bal.test".into()]),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut input = codegen_input(group);
    input.profiles.insert(c1.index_id.clone(), c1);
    input.profiles.insert(c2.index_id.clone(), c2);
    input.routing = Some(routing);

    let generated = generate_xray(&input).expect("group");
    let main = &generated.main;
    let tags: Vec<String> = main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| {
            o.get("tag")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })
        .collect();
    assert!(tags.contains(&"proxy-1-c1".to_string()), "{tags:?}");
    assert!(tags.contains(&"proxy-2-c2".to_string()), "{tags:?}");
    assert_eq!(
        json_at(main, "/observatory/subjectSelector"),
        json!(["proxy"])
    );
    assert_eq!(
        string_at(main, "/observatory/probeUrl"),
        "https://example.com/ping"
    );
    assert_eq!(
        string_at(main, "/routing/balancers/0/tag"),
        "proxy-balancer"
    );
    assert_eq!(
        string_at(main, "/routing/balancers/0/strategy/type"),
        "leastPing"
    );
    // user rule referencing "proxy" is rewritten and the final rule appended.
    let rules = main["routing"]["rules"].as_array().unwrap();
    assert!(rules
        .iter()
        .any(|r| r.get("balancerTag").and_then(|v| v.as_str()) == Some("proxy-balancer")));
    let last = rules.last().unwrap();
    assert_eq!(last["balancerTag"].as_str(), Some("proxy-balancer"));
    assert_eq!(json_at(last, "/ip"), json!(["0.0.0.0/0", "::/0"]));
}
