//! sing-box DNS generation (`SingboxDnsService`).

use serde_json::{json, Value};

use crate::input::{CodegenInput, CodegenRule};
use crate::singbox::routing::parse_v2_domain;
use crate::singbox::SboxState;
use crate::util::*;
use crate::CodegenError;

/// Embedded `dns_singbox_normal`.
pub(crate) const DNS_SINGBOX_NORMAL_FALLBACK: &str = r#"{
  "servers": [
    {"tag": "remote", "type": "tcp", "server": "8.8.8.8", "detour": "proxy"},
    {"tag": "local", "type": "udp", "server": "223.5.5.5"}
  ],
  "rules": [
    {"rule_set": ["geosite-google"], "server": "remote"},
    {"rule_set": ["geosite-cn"], "server": "local"}
  ],
  "final": "remote",
  "strategy": "prefer_ipv4"
}"#;

/// Embedded `tun_singbox_dns`.
pub(crate) const TUN_SINGBOX_DNS_FALLBACK: &str = r#"{
  "servers": [
    {"tag": "remote", "type": "tcp", "server": "8.8.8.8", "detour": "proxy"},
    {"tag": "local", "type": "udp", "server": "223.5.5.5"}
  ],
  "rules": [
    {"rule_set": ["geosite-google"], "server": "remote"},
    {"rule_set": ["geosite-cn"], "server": "local"}
  ],
  "final": "remote",
  "strategy": "prefer_ipv4"
}"#;

pub(crate) fn build_dns(state: &mut SboxState<'_>) -> Result<(), CodegenError> {
    let input = state.input;
    let raw = input.dns.clone().unwrap_or_default();
    if raw.enabled {
        gen_dns_custom(state, &raw);
        return Ok(());
    }
    gen_dns_servers(state);
    gen_dns_rules(state);
    let simple = &raw.simple;
    if let Some(dns) = state.config.get_mut("dns").and_then(Value::as_object_mut) {
        if simple.serve_stale {
            dns.insert("optimistic".into(), json!(true));
        }
        let final_tag = if use_direct_dns(input) {
            SINGBOX_DIRECT_DNS_TAG
        } else {
            SINGBOX_REMOTE_DNS_TAG
        };
        dns.insert("final".into(), json!(final_tag));
    }
    Ok(())
}

fn gen_dns_servers(state: &mut SboxState<'_>) {
    let input = state.input;
    let simple = input
        .dns
        .as_ref()
        .map(|d| &d.simple)
        .cloned()
        .unwrap_or_default();
    let mut servers: Vec<Value> = Vec::new();

    // Bootstrap server (`local-local`).
    let bootstrap_address = simple
        .bootstrap_dns
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DOMAIN_PURE_IP_DNS_DEFAULT.into());
    let mut final_dns =
        parse_dns_address(&bootstrap_address).unwrap_or_else(|| json!({"type": "udp"}));
    if let Some(map) = final_dns.as_object_mut() {
        map.insert("tag".into(), json!(SINGBOX_LOCAL_DNS_TAG));
    }

    let mut direct_dns_list = parse_dns_addresses(
        simple
            .direct_dns
            .as_deref()
            .unwrap_or(DOMAIN_DIRECT_DNS_DEFAULT),
    );
    for (i, server) in direct_dns_list.iter_mut().enumerate() {
        if let Some(map) = server.as_object_mut() {
            map.insert(
                "tag".into(),
                json!(format!("{SINGBOX_DIRECT_DNS_TAG_PREFIX}{}", i + 1)),
            );
            map.insert("domain_resolver".into(), json!(SINGBOX_LOCAL_DNS_TAG));
        }
    }

    let mut remote_dns_list = parse_dns_addresses(
        simple
            .remote_dns
            .as_deref()
            .unwrap_or(DOMAIN_REMOTE_DNS_DEFAULT),
    );
    for (i, server) in remote_dns_list.iter_mut().enumerate() {
        if let Some(map) = server.as_object_mut() {
            map.insert(
                "tag".into(),
                json!(format!("{SINGBOX_REMOTE_DNS_TAG_PREFIX}{}", i + 1)),
            );
            map.insert("detour".into(), json!(PROXY_TAG));
            map.insert("domain_resolver".into(), json!(SINGBOX_LOCAL_DNS_TAG));
        }
    }

    let mut hosts_dns = json!({"tag": SINGBOX_HOSTS_DNS_TAG, "type": "hosts", "predefined": {}});
    if let Some(predefined) = hosts_dns
        .get_mut("predefined")
        .and_then(Value::as_object_mut)
    {
        if simple.add_common_hosts {
            for (key, values) in predefined_hosts() {
                predefined.insert(key.to_string(), json!(values));
            }
        }
        if simple.use_system_hosts {
            if let Some(dns) = &input.dns {
                for (host, value) in &dns.system_hosts {
                    if !predefined.contains_key(host) {
                        predefined.insert(host.clone(), json!([value]));
                    }
                }
            }
        }
        for (key, values) in parse_hosts_to_dictionary(simple.hosts.as_deref()) {
            let mut test_rule = obj();
            if !parse_v2_domain(&key, &mut test_rule) {
                continue;
            }
            if test_rule
                .get("domain_keyword")
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty())
                && !key.contains(':')
            {
                let keyword = test_rule.remove("domain_keyword").unwrap_or(Value::Null);
                test_rule.insert("domain".into(), keyword);
            }
            let domains = test_rule
                .get("domain")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if domains.len() == 1 {
                let full = domains[0].as_str().unwrap_or("");
                let ips: Vec<String> = values
                    .iter()
                    .filter(|v| is_ip_address(v))
                    .cloned()
                    .collect();
                predefined.insert(full.to_string(), json!(ips));
            }
        }
    }

    // Resolve server hostnames through the hosts server.
    let predefined_pairs: Vec<(String, Value)> = hosts_dns
        .get("predefined")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    for (host, _) in &predefined_pairs {
        if final_dns.get("server").and_then(Value::as_str) == Some(host.as_str()) {
            if let Some(map) = final_dns.as_object_mut() {
                map.insert("domain_resolver".into(), json!(SINGBOX_HOSTS_DNS_TAG));
            }
        }
        for server in direct_dns_list.iter_mut().chain(remote_dns_list.iter_mut()) {
            if server.get("server").and_then(Value::as_str) == Some(host.as_str()) {
                if let Some(map) = server.as_object_mut() {
                    map.insert("domain_resolver".into(), json!(SINGBOX_HOSTS_DNS_TAG));
                }
            }
        }
    }

    servers.push(final_dns);
    servers.extend(remote_dns_list);
    servers.extend(direct_dns_list);
    servers.push(hosts_dns);

    if simple.fake_ip {
        let fakeip_range = simple
            .fake_ip_range
            .clone()
            .unwrap_or_else(|| FAKE_IP_DEFAULT.into());
        servers.push(json!({
            "tag": SINGBOX_FAKE_DNS_TAG,
            "type": "fakeip",
            "inet4_range": fakeip_range
        }));
    }

    let dns = state.config.entry("dns").or_insert_with(|| json!({}));
    if let Some(map) = dns.as_object_mut() {
        map.insert("servers".into(), Value::Array(servers));
    }
}

fn gen_dns_rules(state: &mut SboxState<'_>) {
    let input = state.input;
    let simple = input
        .dns
        .as_ref()
        .map(|d| &d.simple)
        .cloned()
        .unwrap_or_default();
    let mut rules: Vec<Value> = Vec::new();
    rules.push(json!({"preferred_by": SINGBOX_HOSTS_DNS_TAG, "server": SINGBOX_HOSTS_DNS_TAG}));

    if let Some(dns) = &input.dns {
        if !dns.protect_domain_list.is_empty() {
            rules
                .push(json!({"server": SINGBOX_DIRECT_DNS_TAG, "domain": dns.protect_domain_list}));
        }
    }

    rules.push(json!({"server": SINGBOX_REMOTE_DNS_TAG, "clash_mode": "Global"}));
    rules.push(json!({"server": SINGBOX_DIRECT_DNS_TAG, "clash_mode": "Direct"}));

    for (key, values) in parse_hosts_to_dictionary(simple.hosts.as_deref()) {
        let Some(predefined) = values.first() else {
            continue;
        };
        if predefined.is_empty() {
            continue;
        }
        let mut rule = obj();
        rule.insert("query_type".into(), json!([1, 5, 28]));
        rule.insert("action".into(), json!("predefined"));
        rule.insert("rcode".into(), json!("NOERROR"));
        if !parse_v2_domain(&key, &mut rule) {
            continue;
        }
        if rule
            .get("domain_keyword")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
            && !key.contains(':')
        {
            let keyword = rule.remove("domain_keyword").unwrap_or(Value::Null);
            rule.insert("domain".into(), keyword);
        }
        if let Some(rcode) = predefined
            .strip_prefix('#')
            .and_then(|r| r.parse::<i32>().ok())
        {
            let name = match rcode {
                0 => "NOERROR",
                1 => "FORMERR",
                2 => "SERVFAIL",
                3 => "NXDOMAIN",
                4 => "NOTIMP",
                5 => "REFUSED",
                _ => "NOERROR",
            };
            rule.insert("rcode".into(), json!(name));
        } else if is_domain_name(predefined) {
            rule.insert(
                "answer".into(),
                json!([format!("*. IN CNAME {predefined}.")]),
            );
        } else if is_ip_address(predefined)
            && rule
                .get("domain")
                .and_then(Value::as_array)
                .map(|a| a.is_empty())
                .unwrap_or(true)
        {
            if is_valid_ipv6(predefined) {
                rule.insert("answer".into(), json!([format!("*. IN AAAA {predefined}")]));
            } else {
                rule.insert("answer".into(), json!([format!("*. IN A {predefined}")]));
            }
        } else {
            continue;
        }
        rules.push(Value::Object(rule));
    }

    if simple.block_binding_query {
        rules.push(json!({"query_type": [64, 65], "action": "predefined", "rcode": "NOERROR"}));
    }

    if simple.fake_ip && simple.global_fake_ip != Some(false) {
        let mut filter = input
            .singbox_fakeip_filter
            .clone()
            .unwrap_or_else(sample_fakeip_filter);
        if let Some(map) = filter.as_object_mut() {
            map.insert("invert".into(), json!(true));
        }
        rules.push(json!({
            "server": SINGBOX_FAKE_DNS_TAG,
            "type": "logical",
            "mode": "and",
            "rewrite_ttl": 1,
            "rules": [{"query_type": [1, 28]}, filter]
        }));
    }

    if simple.block_aaaa_query {
        rules.push(json!({"query_type": [28], "action": "predefined", "rcode": "NOERROR"}));
    }

    let routing = input.routing.clone().unwrap_or_default();
    let direct_dns_list = dns_server_tags(state, SINGBOX_DIRECT_DNS_TAG_PREFIX);
    let remote_dns_list = dns_server_tags(state, SINGBOX_REMOTE_DNS_TAG_PREFIX);

    let mut expected_ip_cidr: Vec<String> = Vec::new();
    let mut expected_regions: Vec<String> = Vec::new();
    let mut region_name = String::new();
    if let Some(ips) = simple
        .direct_expected_ips
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        for ip in string2_list(ips) {
            if let Some(region) = ip.strip_prefix(GEOIP_PREFIX) {
                if !region.is_empty() {
                    expected_regions.push(region.to_string());
                    region_name = region.to_string();
                }
            } else {
                expected_ip_cidr.push(ip);
            }
        }
    }

    for item in &routing.rule_set {
        if !item.enabled || item.domain.as_ref().map(|d| d.is_empty()).unwrap_or(true) {
            continue;
        }
        if item.is_routing() {
            continue;
        }
        let mut rule = obj();
        let valid: i32 = item
            .domain
            .as_ref()
            .map(|domains| {
                domains
                    .iter()
                    .filter(|d| parse_v2_domain(d, &mut rule))
                    .count() as i32
            })
            .unwrap_or(0);
        if valid <= 0 {
            continue;
        }
        if item.outbound_tag == DIRECT_TAG {
            if !expected_regions.is_empty()
                && rule
                    .get("geosite")
                    .and_then(Value::as_array)
                    .is_some_and(|a| !a.is_empty())
                && !region_name.is_empty()
            {
                let all_geosites = rule
                    .get("geosite")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let region_geosites: Vec<String> = all_geosites
                    .iter()
                    .filter_map(|g| g.as_str())
                    .filter(|g| {
                        g.ends_with(&format!("-{region_name}"))
                            || g.ends_with(&format!("@{region_name}"))
                            || *g == region_name
                    })
                    .map(ToString::to_string)
                    .collect();
                if !region_geosites.is_empty() {
                    if let Some(list) = rule.get_mut("geosite").and_then(Value::as_array_mut) {
                        list.retain(|g| {
                            !region_geosites
                                .iter()
                                .any(|r| Some(r.as_str()) == g.as_str())
                        });
                    }
                    let mut expected_rule = Value::Object(rule.clone());
                    if let Some(map) = expected_rule.as_object_mut() {
                        map.insert("geosite".into(), json!(region_geosites));
                    }
                    let expected_rule_list = build_multi_rules(
                        &expected_rule,
                        item,
                        &direct_dns_list,
                        simple.parallel_query,
                    );
                    for mut response in expected_rule_list
                        .iter()
                        .filter(|r| r.get("action").and_then(Value::as_str) == Some("respond"))
                        .cloned()
                    {
                        if let Some(map) = response.as_object_mut() {
                            if !expected_regions.is_empty() {
                                map.insert("geoip".into(), json!(expected_regions));
                            }
                            if !expected_ip_cidr.is_empty() {
                                map.insert("ip_cidr".into(), json!(expected_ip_cidr));
                            }
                        }
                        rules.push(response);
                    }
                    let fallback = build_multi_rules(
                        &expected_rule,
                        item,
                        &remote_dns_list,
                        simple.parallel_query,
                    );
                    rules.extend(fallback);
                }
                let has_remaining = rule
                    .get("geosite")
                    .and_then(Value::as_array)
                    .is_some_and(|a| !a.is_empty())
                    || rule
                        .get("domain")
                        .and_then(Value::as_array)
                        .is_some_and(|a| !a.is_empty())
                    || rule
                        .get("domain_keyword")
                        .and_then(Value::as_array)
                        .is_some_and(|a| !a.is_empty())
                    || rule
                        .get("domain_regex")
                        .and_then(Value::as_array)
                        .is_some_and(|a| !a.is_empty())
                    || rule
                        .get("domain_suffix")
                        .and_then(Value::as_array)
                        .is_some_and(|a| !a.is_empty());
                if has_remaining {
                    add_rules(
                        state,
                        &mut rules,
                        &Value::Object(rule.clone()),
                        item,
                        &direct_dns_list,
                        simple.parallel_query,
                    );
                }
            } else {
                add_rules(
                    state,
                    &mut rules,
                    &Value::Object(rule.clone()),
                    item,
                    &direct_dns_list,
                    simple.parallel_query,
                );
            }
        } else if item.outbound_tag == BLOCK_TAG {
            rule.insert("action".into(), json!("predefined"));
            rule.insert("rcode".into(), json!("NXDOMAIN"));
            rules.push(Value::Object(rule));
        } else {
            if simple.fake_ip && simple.global_fake_ip == Some(false) {
                let mut fake = Value::Object(rule.clone());
                if let Some(map) = fake.as_object_mut() {
                    map.insert("server".into(), json!(SINGBOX_FAKE_DNS_TAG));
                    map.insert("query_type".into(), json!([1, 28]));
                    map.insert("rewrite_ttl".into(), json!(1));
                }
                rules.push(fake);
            }
            add_rules(
                state,
                &mut rules,
                &Value::Object(rule.clone()),
                item,
                &remote_dns_list,
                simple.parallel_query,
            );
        }
    }

    let use_direct = use_direct_dns(input);
    if !use_direct && simple.fake_ip && simple.global_fake_ip == Some(false) {
        rules
            .push(json!({"server": SINGBOX_FAKE_DNS_TAG, "query_type": [1, 28], "rewrite_ttl": 1}));
    }
    let final_item = CodegenRule {
        id: "final".into(),
        ..Default::default()
    };
    let final_dns_list = if use_direct {
        &direct_dns_list
    } else {
        &remote_dns_list
    };
    add_rules(
        state,
        &mut rules,
        &json!({}),
        &final_item,
        final_dns_list,
        simple.parallel_query,
    );

    let dns = state.config.entry("dns").or_insert_with(|| json!({}));
    if let Some(map) = dns.as_object_mut() {
        map.insert("rules".into(), Value::Array(rules));
    }
}

fn dns_server_tags(state: &SboxState<'_>, prefix: &str) -> Vec<String> {
    state
        .config
        .get("dns")
        .and_then(|d| d.get("servers"))
        .and_then(Value::as_array)
        .map(|servers| {
            servers
                .iter()
                .filter_map(|s| s.get("tag").and_then(Value::as_str))
                .filter(|tag| tag.starts_with(prefix))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn build_multi_rules(
    rule: &Value,
    item: &CodegenRule,
    dns_list: &[String],
    parallel: bool,
) -> Vec<Value> {
    let mut evaluate_rules: Vec<Value> = Vec::new();
    let mut response_rules: Vec<Value> = Vec::new();
    for server_tag in dns_list {
        let mut evaluate = rule.clone();
        let evaluate_tag = format!(
            "{server_tag}-{}",
            if item.id.is_empty() {
                "final"
            } else {
                &item.id
            }
        );
        if let Some(map) = evaluate.as_object_mut() {
            map.insert("action".into(), json!("evaluate"));
            map.insert("tag".into(), json!(evaluate_tag));
            map.insert("server".into(), json!(server_tag));
        }
        evaluate_rules.push(evaluate);
        let mut response = json!({
            "match_response": evaluate_tag,
            "action": "respond"
        });
        if parallel {
            response
                .as_object_mut()
                .map(|m| m.insert("race".into(), json!(true)));
        }
        response_rules.push(response);
    }
    evaluate_rules.extend(response_rules);
    evaluate_rules
}

fn add_rules(
    _state: &mut SboxState<'_>,
    rules: &mut Vec<Value>,
    rule: &Value,
    item: &CodegenRule,
    dns_list: &[String],
    parallel: bool,
) {
    if dns_list.len() == 1 {
        let mut single = rule.clone();
        if let Some(map) = single.as_object_mut() {
            map.insert("server".into(), json!(dns_list[0]));
        }
        rules.push(single);
    } else {
        rules.extend(build_multi_rules(rule, item, dns_list, parallel));
    }
}

fn is_domain_name(value: &str) -> bool {
    !value.is_empty()
        && !is_ip_address(value)
        && value != "localhost"
        && value.contains('.')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

fn parse_dns_addresses(addresses: &str) -> Vec<Value> {
    let mut result = Vec::new();
    for address in string2_list(addresses) {
        if let Some(server) = parse_dns_address(&address) {
            result.push(server);
        }
    }
    result
}

pub(crate) fn parse_dns_address(address: &str) -> Option<Value> {
    let first = string2_list(address).into_iter().next()?;
    if first.is_empty() {
        return None;
    }
    if first == "local" || first == "localhost" {
        return Some(json!({"type": "local"}));
    }
    let (domain, scheme, port, path) = parse_url(&first);
    if scheme.eq_ignore_ascii_case("dhcp") {
        let mut server = obj();
        server.insert("type".into(), json!("dhcp"));
        if !domain.is_empty() && domain != "auto" {
            server.insert("server".into(), json!(domain));
        }
        return Some(Value::Object(server));
    }
    let dns_type = if scheme.is_empty() {
        "udp".to_string()
    } else {
        scheme.replace("+local", "").to_lowercase()
    };
    let mut server = obj();
    server.insert("type".into(), json!(dns_type));
    server.insert("server".into(), json!(domain));
    if port != 0 {
        server.insert("server_port".into(), json!(port));
    }
    if matches!(dns_type.as_str(), "https" | "h3") && !path.is_empty() && path != "/" {
        server.insert("path".into(), json!(path));
    }
    Some(Value::Object(server))
}

fn use_direct_dns(_input: &CodegenInput) -> bool {
    let routing = _input.routing.clone().unwrap_or_default();
    let Some(last) = routing.rule_set.last() else {
        return false;
    };
    if !last.enabled || last.outbound_tag != DIRECT_TAG {
        return false;
    }
    let no_domain = last
        .domain
        .as_ref()
        .map(|d| {
            d.iter()
                .filter(|x| !x.starts_with('#') && !x.is_empty())
                .count()
                == 0
        })
        .unwrap_or(true);
    let no_process = last.process.as_ref().map(|p| p.is_empty()).unwrap_or(true);
    let is_any_ip = last
        .ip
        .as_ref()
        .map(|ips| ips.is_empty() || ips.iter().any(|ip| ip == "0.0.0.0/0"))
        .unwrap_or(true);
    let is_any_port = last
        .port
        .as_deref()
        .map(|p| p.is_empty() || p == "0-65535")
        .unwrap_or(true);
    let is_any_network = last
        .network
        .as_deref()
        .map(|n| n.is_empty() || n == "tcp,udp")
        .unwrap_or(true);
    no_domain && no_process && is_any_ip && is_any_port && is_any_network
}

fn gen_dns_custom(state: &mut SboxState<'_>, raw: &crate::input::CodegenDns) {
    let input = state.input;
    let text = if input.settings.tun.enabled {
        raw.tun
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| TUN_SINGBOX_DNS_FALLBACK.to_string())
    } else {
        raw.normal
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DNS_SINGBOX_NORMAL_FALLBACK.to_string())
    };
    let Ok(mut dns) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if dns.is_null() {
        return;
    }
    // Protect custom DNS.
    let Some(dns_map) = dns.as_object_mut() else {
        return;
    };
    if dns_map.get("servers").is_none() {
        dns_map.insert("servers".into(), json!([]));
    }
    if dns_map.get("rules").is_none() {
        dns_map.insert("rules".into(), json!([]));
    }
    let remote_tag = dns_map
        .get("servers")
        .and_then(Value::as_array)
        .and_then(|servers| {
            servers
                .iter()
                .find(|s| s.get("detour").and_then(Value::as_str) == Some(PROXY_TAG))
        })
        .and_then(|s| s.get("tag").and_then(Value::as_str))
        .unwrap_or("remote")
        .to_string();
    let address = raw
        .domain_dns_address
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DOMAIN_PURE_IP_DNS_DEFAULT.into());
    let mut local = parse_dns_address(&address).unwrap_or_else(|| json!({"type": "udp"}));
    if let Some(map) = local.as_object_mut() {
        map.insert("tag".into(), json!(SINGBOX_LOCAL_DNS_TAG));
    }
    if let Some(servers) = dns_map.get_mut("servers").and_then(Value::as_array_mut) {
        servers.push(local);
    }
    if let Some(rules) = dns_map.get_mut("rules").and_then(Value::as_array_mut) {
        if let Some(protect) = &input.dns {
            if !protect.protect_domain_list.is_empty() {
                rules.insert(
                    0,
                    json!({"server": SINGBOX_LOCAL_DNS_TAG, "domain": protect.protect_domain_list}),
                );
            }
        }
        rules.insert(
            0,
            json!({"server": SINGBOX_LOCAL_DNS_TAG, "clash_mode": "Direct"}),
        );
        rules.insert(0, json!({"server": remote_tag, "clash_mode": "Global"}));
    }
    state.config.insert("dns".into(), dns);
}
