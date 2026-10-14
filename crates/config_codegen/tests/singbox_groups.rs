mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::input::*;
use serde_json::json;

fn vless(index: &str, remarks: &str, address: &str) -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, address, 443);
    p.index_id = index.into();
    p.remarks = remarks.into();
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p
}

fn wireguard(index: &str, remarks: &str) -> CodegenProfile {
    let mut p = profile(ConfigType::WireGuard, "192.0.2.130", 51820);
    p.index_id = index.into();
    p.remarks = remarks.into();
    p.password = "SYNTHETIC_PRIVATE_KEY".into();
    p.proto_extra.wg_public_key = Some("SYNTHETIC_PUBLIC_KEY".into());
    p
}

#[test]
fn singbox_policy_group_selector() {
    let mut group = profile(ConfigType::PolicyGroup, "", 0);
    group.index_id = "group-1".into();
    group.proto_extra.child_items = Some("c1,c2".into());
    group.proto_extra.multiple_load = Some(MultipleLoad::Fallback);
    let mut input = codegen_input(group);
    input
        .profiles
        .insert("c1".into(), vless("c1", "c1", "192.0.2.120"));
    input
        .profiles
        .insert("c2".into(), vless("c2", "c2", "192.0.2.121"));

    let generated = generate_singbox(&input).expect("group");
    let main = &generated.main;
    assert_eq!(string_at(main, "/outbounds/0/type"), "selector");
    assert_eq!(string_at(main, "/outbounds/0/tag"), "proxy");
    assert_eq!(
        json_at(main, "/outbounds/0/outbounds"),
        json!(["proxy-auto", "proxy-1-c1", "proxy-2-c2"])
    );
    assert_eq!(string_at(main, "/outbounds/1/type"), "urltest");
    assert_eq!(string_at(main, "/outbounds/1/tag"), "proxy-auto");
    assert_eq!(
        json_at(main, "/outbounds/1/outbounds"),
        json!(["proxy-1-c1", "proxy-2-c2"])
    );
    assert_eq!(json_at(main, "/outbounds/1/tolerance"), json!(5000));
    assert_eq!(string_at(main, "/outbounds/2/tag"), "proxy-1-c1");
    assert_eq!(string_at(main, "/outbounds/3/tag"), "proxy-2-c2");
}

#[test]
fn singbox_proxy_chain_detour() {
    let mut chain = profile(ConfigType::ProxyChain, "", 0);
    chain.index_id = "chain-1".into();
    chain.proto_extra.child_items = Some("c1,c2".into());
    let mut input = codegen_input(chain);
    input
        .profiles
        .insert("c1".into(), vless("c1", "c1", "192.0.2.122"));
    input
        .profiles
        .insert("c2".into(), vless("c2", "c2", "192.0.2.123"));

    let generated = generate_singbox(&input).expect("chain");
    let main = &generated.main;
    // Reverse order: c2 becomes proxy (front) and dials through c1.
    assert_eq!(string_at(main, "/outbounds/0/tag"), "proxy");
    assert_eq!(string_at(main, "/outbounds/0/server"), "192.0.2.123");
    assert_eq!(string_at(main, "/outbounds/0/detour"), "chain-proxy-1-c1");
    assert_eq!(string_at(main, "/outbounds/1/tag"), "chain-proxy-1-c1");
    assert_eq!(string_at(main, "/outbounds/1/server"), "192.0.2.122");
    assert!(json_at(main, "/outbounds/1/detour").is_null());
}

#[test]
fn singbox_rule_targets_group_tag() {
    let mut group = profile(ConfigType::PolicyGroup, "", 0);
    group.index_id = "group-1".into();
    group.proto_extra.child_items = Some("c1,c2".into());
    let mut c1 = vless("c1", "c1", "192.0.2.124");
    c1.stream_security = "tls".into();
    let mut c2 = vless("c2", "c2", "192.0.2.125");
    c2.stream_security = "tls".into();
    let routing = CodegenRouting {
        rule_set: vec![CodegenRule {
            enabled: true,
            rule_type: RuleType::Routing,
            outbound_tag: "Group Node".into(),
            domain: Some(vec!["full:group.test".into()]),
            ..Default::default()
        }],
        ..Default::default()
    };
    group.remarks = "Group Node".into();
    let mut second_group = group.clone();
    second_group.index_id = "group-2".into();
    second_group.remarks = "Group Node".into();

    let mut input = codegen_input(group.clone());
    input.profiles.insert("c1".into(), c1);
    input.profiles.insert("c2".into(), c2);
    input.profiles.insert("group-2".into(), second_group);
    input.routing = Some(routing);

    let generated = generate_singbox(&input).expect("rule group");
    let main = &generated.main;
    // remark:"Group Node" resolves to group-1 (first match), outbounds prefixed.
    let rules = main["route"]["rules"].as_array().unwrap();
    let rule = rules
        .iter()
        .find(|r| {
            r.get("domain")
                .and_then(|d| d.as_array())
                .map(|list| list.iter().any(|v| v.as_str() == Some("group.test")))
                .unwrap_or(false)
        })
        .expect("rule");
    assert_eq!(rule["outbound"].as_str(), Some("group-1-proxy-Group Node"));
}

// S3-03: a ProxyChain hop that is a multi-child group fans out the already
// accumulated servers. Endpoints (WireGuard) must be fanned out and rewired
// exactly like outbounds (upstream keeps a single `List<BaseServer4Sbox>`);
// a stale `detour` pointing at the group tag matches no emitted tag.
#[test]
fn singbox_chain_group_fanout_rewires_endpoints() {
    let mut chain = profile(ConfigType::ProxyChain, "", 0);
    chain.index_id = "chain-1".into();
    chain.proto_extra.child_items = Some("grp,wg".into());
    let mut group = profile(ConfigType::PolicyGroup, "", 0);
    group.index_id = "grp".into();
    group.remarks = "grp".into();
    group.proto_extra.child_items = Some("a,b".into());

    let mut input = codegen_input(chain);
    input.profiles.insert("grp".into(), group);
    input
        .profiles
        .insert("a".into(), vless("a", "a", "192.0.2.131"));
    input
        .profiles
        .insert("b".into(), vless("b", "b", "192.0.2.132"));
    input.profiles.insert("wg".into(), wireguard("wg", "wg"));

    let generated = generate_singbox(&input).expect("chain");
    let main = &generated.main;
    let outbounds = main["outbounds"].as_array().unwrap();
    let endpoints = main
        .get("endpoints")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let tags: Vec<&str> = outbounds
        .iter()
        .chain(endpoints.iter())
        .filter_map(|v| v.get("tag").and_then(|t| t.as_str()))
        .collect();
    // Every detour must resolve to an emitted tag (no dangling detour).
    for server in outbounds.iter().chain(endpoints.iter()) {
        if let Some(detour) = server.get("detour").and_then(|d| d.as_str()) {
            assert!(
                tags.contains(&detour),
                "dangling detour {detour:?} on {:?}: {main}",
                server.get("tag")
            );
        }
    }
    // The exit endpoint is fanned out once per chain-start branch.
    assert_eq!(endpoints.len(), 2, "{main}");
    let mut endpoint_detours: Vec<&str> = endpoints
        .iter()
        .filter_map(|e| e.get("detour").and_then(|d| d.as_str()))
        .collect();
    endpoint_detours.sort_unstable();
    assert_eq!(
        endpoint_detours,
        vec!["chain-proxy-1-grp-1-a", "chain-proxy-1-grp-2-b"],
        "{main}"
    );
}
