//! Xray DNS generation (`V2rayDnsService`).

use serde_json::{json, Map, Value};

use crate::input::SimpleDns;
use crate::util::*;
use crate::xray::routing::final_rule_tags;
use crate::xray::XrayState;
use crate::CodegenError;

/// Embedded `dns_v2ray_normal` fallback.
pub(crate) const DNS_V2RAY_NORMAL_FALLBACK: &str = r#"{
  "hosts": {"dns.google": "8.8.8.8", "proxy.example.com": "127.0.0.1"},
  "servers": [
    {"address": "1.1.1.1", "skipFallback": true, "domains": ["geosite:google"]},
    {"address": "223.5.5.5", "skipFallback": true, "domains": ["geosite:cn"], "expectIPs": ["geoip:cn"]},
    "1.1.1.1", "8.8.8.8", "https://dns.google/dns-query"
  ]
}"#;

/// `ParseUrl` subset (scheme/domain/port/path); shared with the sing-box side.
pub(crate) use crate::util::parse_url;

pub(crate) fn is_domain(value: &str) -> bool {
    !value.is_empty()
        && !is_ip_address(value)
        && value != "localhost"
        && value.contains('.')
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        && !value.starts_with('.')
        && !value.ends_with('.')
}

fn parse_dns_addresses(input: Option<&String>, default_address: &str) -> Vec<String> {
    let list: Vec<String> = string2_list_opt(input)
        .unwrap_or_default()
        .into_iter()
        .map(|addr| {
            if addr.to_lowercase().starts_with("dhcp") {
                "localhost".to_string()
            } else {
                addr
            }
        })
        .collect();
    let mut dedup: Vec<String> = Vec::new();
    for addr in list {
        if !dedup.contains(&addr) {
            dedup.push(addr);
        }
    }
    if dedup.is_empty() {
        vec![default_address.to_string()]
    } else {
        dedup
    }
}

fn create_dns_server(
    dns_address: &str,
    domains: &[String],
    expected_ips: Option<&[String]>,
) -> Value {
    let (domain, scheme, port, _) = parse_url(dns_address);
    let (address, port_final) = if scheme.is_empty() || scheme.to_lowercase().starts_with("udp") {
        (domain.clone(), if port > 0 { Some(port) } else { None })
    } else if scheme.to_lowercase().starts_with("tcp") {
        (
            format!("{scheme}://{domain}"),
            if port > 0 { Some(port) } else { None },
        )
    } else {
        (dns_address.to_string(), None)
    };
    let mut server = obj();
    server.insert("address".into(), json!(address));
    put_opt_i32(&mut server, "port", port_final);
    if !domains.is_empty() {
        server.insert("domains".into(), json!(domains));
    }
    server.insert("skipFallback".into(), json!(true));
    if let Some(ips) = expected_ips.filter(|v| !v.is_empty()) {
        server.insert("expectedIPs".into(), json!(ips));
    }
    Value::Object(server)
}

fn add_dns_servers(
    servers: &mut Vec<Value>,
    direct_tag_index: &mut i32,
    dns_addresses: &[String],
    domains: &[String],
    is_direct: bool,
    expected_ips: Option<&[String]>,
) {
    if domains.is_empty() {
        return;
    }
    for dns_address in dns_addresses {
        let mut server = create_dns_server(dns_address, domains, expected_ips);
        if is_direct {
            if let Some(map) = server.as_object_mut() {
                map.insert(
                    "tag".into(),
                    json!(format!("{DIRECT_DNS_TAG}-{}", *direct_tag_index)),
                );
            }
            *direct_tag_index += 1;
        }
        servers.push(server);
    }
}

fn gen_fake_dns(simple: &SimpleDns) -> Value {
    let fakeip_range = simple
        .fake_ip_range
        .clone()
        .unwrap_or_else(|| FAKE_IP_DEFAULT.into());
    let mut pool_size: i64 = 65535;
    if let Some(net) = parse_net4(&fakeip_range) {
        let total: u128 = 1u128 << (32 - u32::from(net.prefix));
        let total = total.saturating_sub(1);
        if total > 0 {
            pool_size = if total >= i64::MAX as u128 {
                i64::MAX
            } else {
                total as i64
            };
        }
    } else if let Some(net) = parse_net6(&fakeip_range) {
        let total: u128 = 1u128 << (128 - u32::from(net.prefix));
        let total = total.saturating_sub(1);
        if total > 0 {
            pool_size = i64::MAX;
        }
    }
    json!({"ipPool": fakeip_range, "poolSize": pool_size})
}

/// Last-rule check used to select direct DNS as `final`.
fn use_direct_dns(rules: &[crate::input::CodegenRule]) -> bool {
    let Some(last) = rules.last() else {
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
        })
        .unwrap_or(0)
        == 0;
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

pub(crate) fn build_dns(state: &mut XrayState<'_>) -> Result<(), CodegenError> {
    let input = state.input;
    let raw = input.dns.clone().unwrap_or_default();

    if raw.enabled {
        gen_dns_custom(state, &raw);
        if routing_domain_strategy(state) != IP_IF_NON_MATCH {
            return Ok(());
        }
        let mut dns = state
            .config
            .get("dns")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if let Some(map) = dns.as_object_mut() {
            map.insert("tag".into(), json!(DNS_TAG));
        }
        state.config.insert("dns".into(), dns);
        add_rule(
            state,
            json!({"type": "field", "inboundTag": [DNS_TAG], "outboundTag": PROXY_TAG}),
        );
        return Ok(());
    }

    let simple = &raw.simple;
    let routing_rules: Vec<crate::input::CodegenRule> = input
        .routing
        .as_ref()
        .map(|r| r.rule_set.clone())
        .unwrap_or_default();

    // Freedom outbound domain strategy.
    let strategy4_freedom = simple
        .strategy4_freedom
        .clone()
        .unwrap_or_else(|| AS_IS.into());
    if !strategy4_freedom.is_empty() && strategy4_freedom != AS_IS {
        if let Some(outbound) = state
            .outbounds
            .iter_mut()
            .find(|o| o.get("protocol").and_then(Value::as_str) == Some("freedom"))
        {
            set_sockopt_domain_strategy(
                outbound,
                Some(&strategy4_freedom),
                Some(&input.settings.happy_eyeballs4_ray),
            );
        }
    }

    let proxy_protocols = xray_protocol_names();
    let strategy4_proxy = simple
        .strategy4_proxy
        .clone()
        .unwrap_or_else(|| AS_IS.into());
    if !strategy4_proxy.is_empty() && strategy4_proxy != AS_IS {
        for outbound in state.outbounds.iter_mut() {
            let is_proxy = outbound
                .get("protocol")
                .and_then(Value::as_str)
                .is_some_and(|p| proxy_protocols.contains(&p));
            if is_proxy {
                if let Some(map) = outbound.as_object_mut() {
                    map.insert("targetStrategy".into(), json!(strategy4_proxy));
                }
            }
        }
    }
    let strategy4_dial = simple
        .strategy4_proxy_dial
        .clone()
        .unwrap_or_else(|| AS_IS.into());
    if !strategy4_dial.is_empty() && strategy4_dial != AS_IS {
        for outbound in state.outbounds.iter_mut() {
            let is_proxy = outbound
                .get("protocol")
                .and_then(Value::as_str)
                .is_some_and(|p| proxy_protocols.contains(&p));
            if is_proxy {
                set_sockopt_domain_strategy(
                    outbound,
                    Some(&strategy4_dial),
                    Some(&input.settings.happy_eyeballs4_ray),
                );
            }
        }
    }

    let mut dns = obj();
    fill_dns_servers(state, simple, &routing_rules, &mut dns);
    fill_dns_hosts(state, simple, &mut dns);
    dns.insert("serveStale".into(), json!(simple.serve_stale));
    if simple.parallel_query {
        dns.insert("enableParallelQuery".into(), json!(true));
    }
    if simple.block_aaaa_query {
        dns.insert("queryStrategy".into(), json!("UseIPv4"));
    }

    let direct_tags: Vec<String> = dns
        .get("servers")
        .and_then(Value::as_array)
        .map(|servers| {
            servers
                .iter()
                .filter_map(|s| s.get("tag").and_then(Value::as_str))
                .filter(|tag| tag.starts_with(DIRECT_DNS_TAG))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    if !direct_tags.is_empty() {
        add_rule(
            state,
            json!({"type": "field", "inboundTag": direct_tags, "outboundTag": DIRECT_TAG}),
        );
    }

    let routing_value = state
        .config
        .get("routing")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let (final_outbound, final_balancer) = final_rule_tags(&routing_value);
    dns.insert("tag".into(), json!(DNS_TAG));
    let mut dns_rule = obj();
    dns_rule.insert("type".into(), json!("field"));
    dns_rule.insert("inboundTag".into(), json!([DNS_TAG]));
    put_opt_string(&mut dns_rule, "outboundTag", final_outbound.as_ref());
    put_opt_string(&mut dns_rule, "balancerTag", final_balancer.as_ref());
    add_rule(state, Value::Object(dns_rule));

    state.config.insert("dns".into(), Value::Object(dns));
    Ok(())
}

fn routing_domain_strategy(state: &XrayState<'_>) -> String {
    state
        .config
        .get("routing")
        .and_then(|r| r.get("domainStrategy"))
        .and_then(Value::as_str)
        .unwrap_or(AS_IS)
        .to_string()
}

fn xray_protocol_names() -> Vec<&'static str> {
    vec![
        "vmess",
        "vless",
        "shadowsocks",
        "trojan",
        "hysteria",
        "wireguard",
        "socks",
        "http",
    ]
}

fn add_rule(state: &mut XrayState<'_>, rule: Value) {
    if let Some(rules) = state
        .config
        .get_mut("routing")
        .and_then(|r| r.get_mut("rules"))
        .and_then(Value::as_array_mut)
    {
        rules.push(rule);
    }
}

fn set_sockopt_domain_strategy(
    outbound: &mut Value,
    strategy: Option<&str>,
    happy_eyeballs: Option<&crate::input::HappyEyeballs4Ray>,
) {
    let Some(strategy) = strategy.filter(|s| !s.is_empty()) else {
        return;
    };
    let Some(map) = outbound.as_object_mut() else {
        return;
    };
    let stream = map
        .entry("streamSettings")
        .or_insert_with(|| Value::Object(obj()));
    let Some(stream_map) = stream.as_object_mut() else {
        return;
    };
    let sockopt = stream_map
        .entry("sockopt")
        .or_insert_with(|| Value::Object(obj()));
    let Some(sockopt_map) = sockopt.as_object_mut() else {
        return;
    };
    sockopt_map.insert("domainStrategy".into(), json!(strategy));
    if let Some(item) = happy_eyeballs {
        if item != &crate::input::HappyEyeballs4Ray::default() {
            let mut happy = obj();
            put_opt_i32(&mut happy, "tryDelayMs", item.try_delay_ms);
            put_opt_bool(&mut happy, "prioritizeIPv6", item.prioritize_ipv6);
            put_opt_i32(&mut happy, "interleave", item.interleave);
            put_opt_i32(&mut happy, "maxConcurrentTry", item.max_concurrent_try);
            sockopt_map.insert("happyEyeballs".into(), Value::Object(happy));
        }
    }
}

fn gen_dns_custom(state: &mut XrayState<'_>, raw: &crate::input::CodegenDns) {
    let input = state.input;
    let custom_dns = if input.settings.tun.enabled {
        raw.tun.clone().filter(|s| !s.is_empty())
    } else {
        raw.normal.clone().filter(|s| !s.is_empty())
    }
    .or_else(|| input.dns_v2ray_normal.clone())
    .unwrap_or_else(|| DNS_V2RAY_NORMAL_FALLBACK.to_string());

    if let Some(strategy) = raw
        .domain_strategy4_freedom
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        if let Some(outbound) = state
            .outbounds
            .iter_mut()
            .find(|o| o.get("protocol").and_then(Value::as_str) == Some("freedom"))
        {
            set_sockopt_domain_strategy(outbound, Some(strategy), None);
        }
    }

    let mut parsed = serde_json::from_str::<Value>(&custom_dns).unwrap_or_else(|_| {
        let servers: Vec<String> = custom_dns.split(',').map(ToString::to_string).collect();
        json!({"servers": servers})
    });

    if raw.use_system_hosts {
        if let Some(hosts) = parsed.get_mut("hosts").and_then(Value::as_object_mut) {
            for (host, value) in &raw.system_hosts {
                if !hosts.contains_key(host) {
                    hosts.insert(host.clone(), json!(value));
                }
            }
        }
    }
    if let Some(hosts) = parsed.get_mut("hosts").and_then(Value::as_object_mut) {
        let keys: Vec<String> = hosts.keys().cloned().collect();
        for key in keys {
            if let Some(Value::String(ip)) = hosts.get(&key) {
                hosts.insert(key, json!([ip]));
            }
        }
    }

    fill_dns_domains_custom(state, raw, &mut parsed);
    state.config.insert("dns".into(), parsed);
}

fn fill_dns_domains_custom(
    state: &mut XrayState<'_>,
    raw: &crate::input::CodegenDns,
    dns: &mut Value,
) {
    let Some(servers) = dns.get_mut("servers").and_then(Value::as_array_mut) else {
        return;
    };
    if raw.protect_domain_list.is_empty() {
        return;
    }
    let address = raw
        .domain_dns_address
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DOMAIN_PURE_IP_DNS_DEFAULT.into());
    servers.push(json!({
        "address": address,
        "skipFallback": true,
        "domains": raw.protect_domain_list
    }));
    let _ = state;
}

fn fill_dns_servers(
    state: &mut XrayState<'_>,
    simple: &SimpleDns,
    rules: &[crate::input::CodegenRule],
    dns: &mut Map<String, Value>,
) {
    let direct_dns_address =
        parse_dns_addresses(simple.direct_dns.as_ref(), DOMAIN_DIRECT_DNS_DEFAULT);
    let remote_dns_address =
        parse_dns_addresses(simple.remote_dns.as_ref(), DOMAIN_REMOTE_DNS_DEFAULT);
    let bootstrap_dns_address =
        parse_dns_addresses(simple.bootstrap_dns.as_ref(), DOMAIN_PURE_IP_DNS_DEFAULT);

    let mut dns_server_domains: Vec<String> = Vec::new();
    for dns_address in direct_dns_address.iter().chain(remote_dns_address.iter()) {
        let (domain, _, _, _) = parse_url(dns_address);
        if domain == "localhost" {
            continue;
        }
        if is_domain(&domain) {
            let entry = format!("full:{domain}");
            if !dns_server_domains.contains(&entry) {
                dns_server_domains.push(entry);
            }
        }
    }

    let mut expected_ips: Vec<String> = Vec::new();
    let mut region_name = String::new();
    if let Some(ips) = simple
        .direct_expected_ips
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        expected_ips = string2_list(ips);
        if let Some(region) = expected_ips
            .iter()
            .filter(|ip| ip.to_lowercase().starts_with(GEOIP_PREFIX))
            .map(|ip| &ip[GEOIP_PREFIX.len()..])
            .find(|region| !region.is_empty())
        {
            region_name = region.to_string();
        }
    }

    let mut direct_domains: Vec<String> = Vec::new();
    let mut direct_geosites: Vec<String> = Vec::new();
    let mut proxy_domains: Vec<String> = Vec::new();
    let mut proxy_geosites: Vec<String> = Vec::new();
    let mut expected_domains: Vec<String> = Vec::new();

    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if rule.is_routing() {
            continue;
        }
        let Some(domains) = rule.domain.as_ref().filter(|d| !d.is_empty()) else {
            continue;
        };
        for domain in domains {
            if domain.starts_with('#') {
                continue;
            }
            let normalized = domain.replace(ROUTING_RULE_COMMA, ",");
            if rule.outbound_tag == DIRECT_TAG {
                if normalized.starts_with(GEOSITE_PREFIX) || normalized.starts_with("ext:") {
                    let is_expected = !region_name.is_empty()
                        && (normalized.ends_with(&format!("-{region_name}"))
                            || normalized.ends_with(&format!("@{region_name}"))
                            || normalized == format!("{GEOSITE_PREFIX}{region_name}"));
                    if is_expected {
                        expected_domains.push(normalized);
                    } else {
                        direct_geosites.push(normalized);
                    }
                } else {
                    direct_domains.push(normalized);
                }
            } else if rule.outbound_tag != BLOCK_TAG {
                if normalized.starts_with(GEOSITE_PREFIX) || normalized.starts_with("ext:") {
                    proxy_geosites.push(normalized);
                } else {
                    proxy_domains.push(normalized);
                }
            }
        }
    }

    let mut servers: Vec<Value> = dns
        .get("servers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut direct_tag_index = 1;

    if !dns_server_domains.is_empty() {
        add_dns_servers(
            &mut servers,
            &mut direct_tag_index,
            &bootstrap_dns_address,
            &dns_server_domains,
            false,
            None,
        );
    }
    if !state
        .input
        .dns
        .as_ref()
        .map(|d| d.protect_domain_list.clone())
        .unwrap_or_default()
        .is_empty()
    {
        let protect = state
            .input
            .dns
            .as_ref()
            .unwrap()
            .protect_domain_list
            .clone();
        add_dns_servers(
            &mut servers,
            &mut direct_tag_index,
            &direct_dns_address,
            &protect,
            true,
            None,
        );
    }

    if simple.fake_ip {
        let mut fake_match: Vec<String> = proxy_domains.clone();
        fake_match.extend(proxy_geosites.clone());
        if simple.global_fake_ip != Some(false) {
            fake_match.extend(direct_domains.clone());
            fake_match.extend(direct_geosites.clone());
            fake_match.extend(expected_domains.clone());
        }
        if !fake_match.is_empty() {
            state.config.insert("fakedns".into(), gen_fake_dns(simple));
            add_dns_servers(
                &mut servers,
                &mut direct_tag_index,
                &["fakedns".to_string()],
                &fake_match,
                false,
                None,
            );
        }
    }

    add_dns_servers(
        &mut servers,
        &mut direct_tag_index,
        &remote_dns_address,
        &proxy_domains,
        false,
        None,
    );
    add_dns_servers(
        &mut servers,
        &mut direct_tag_index,
        &direct_dns_address,
        &direct_domains,
        true,
        None,
    );
    add_dns_servers(
        &mut servers,
        &mut direct_tag_index,
        &remote_dns_address,
        &proxy_geosites,
        false,
        None,
    );
    add_dns_servers(
        &mut servers,
        &mut direct_tag_index,
        &direct_dns_address,
        &direct_geosites,
        true,
        None,
    );
    add_dns_servers(
        &mut servers,
        &mut direct_tag_index,
        &direct_dns_address,
        &expected_domains,
        true,
        Some(&expected_ips),
    );

    if use_direct_dns(rules) {
        for dns in &direct_dns_address {
            let mut server = create_dns_server(dns, &[], None);
            if let Some(map) = server.as_object_mut() {
                map.insert(
                    "tag".into(),
                    json!(format!("{DIRECT_DNS_TAG}-{direct_tag_index}")),
                );
                map.insert("skipFallback".into(), json!(false));
            }
            direct_tag_index += 1;
            servers.push(server);
        }
    } else {
        for dns_address in &remote_dns_address {
            servers.push(json!(dns_address));
        }
    }

    dns.insert("servers".into(), Value::Array(servers));
}

fn fill_dns_hosts(state: &mut XrayState<'_>, simple: &SimpleDns, dns: &mut Map<String, Value>) {
    let raw = state.input.dns.clone().unwrap_or_default();
    if !simple.add_common_hosts
        && !simple.use_system_hosts
        && simple.hosts.as_deref().unwrap_or("").is_empty()
    {
        return;
    }
    let mut hosts: Map<String, Value> = obj();
    if simple.add_common_hosts {
        for (key, values) in predefined_hosts() {
            hosts.insert(key.to_string(), json!(values));
        }
    }
    if simple.use_system_hosts {
        for (host, value) in &raw.system_hosts {
            if !hosts.contains_key(host) {
                hosts.insert(host.clone(), json!([value]));
            }
        }
    }
    for (key, values) in parse_hosts_to_dictionary(simple.hosts.as_deref()) {
        hosts.insert(key, json!(values));
    }
    dns.insert("hosts".into(), Value::Object(hosts));
}
