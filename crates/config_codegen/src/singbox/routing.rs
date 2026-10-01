//! sing-box routing generation (`SingboxRoutingService`).

use serde_json::{json, Map, Value};

use crate::input::{CodegenProfile, ConfigType};
use crate::singbox::outbound::build_all_proxy_outbounds;
use crate::singbox::SboxState;
use crate::util::*;
use crate::{CodegenError, Diagnostic};

pub(crate) fn build_routing(state: &mut SboxState<'_>) -> Result<(), CodegenError> {
    let input = state.input;
    if state.config.get("route").is_none() {
        state.config.insert("route".into(), json!({"rules": []}));
    }
    set_route(state, "final", json!(PROXY_TAG));

    let simple = input
        .dns
        .as_ref()
        .map(|d| &d.simple)
        .cloned()
        .unwrap_or_default();
    let mut default_resolver_tag = SINGBOX_DIRECT_DNS_TAG.to_string();
    let mut dial_strategy = domain_strategy4_sbox(simple.strategy4_proxy_dial.as_deref());
    let raw_dns_enabled = input.dns.as_ref().map(|d| d.enabled).unwrap_or(false);
    if raw_dns_enabled {
        default_resolver_tag = SINGBOX_LOCAL_DNS_TAG.to_string();
        dial_strategy = input
            .dns
            .as_ref()
            .and_then(|d| d.domain_strategy4_freedom.clone())
            .filter(|s| !s.is_empty());
    } else if let Some(strategy) = simple
        .strategy4_freedom
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        let direct_strategy = domain_strategy4_sbox(Some(strategy));
        if let Some(outbound) = state
            .outbounds
            .iter_mut()
            .find(|o| o.get("tag").and_then(Value::as_str) == Some(DIRECT_TAG))
        {
            if let Some(map) = outbound.as_object_mut() {
                let mut resolver = obj();
                resolver.insert("server".into(), json!(default_resolver_tag));
                put_opt_string(&mut resolver, "strategy", direct_strategy.as_ref());
                map.insert("domain_resolver".into(), Value::Object(resolver));
            }
        }
    }
    let mut resolver = obj();
    resolver.insert("server".into(), json!(default_resolver_tag));
    put_opt_string(&mut resolver, "strategy", dial_strategy.as_ref());
    set_route(state, "default_domain_resolver", Value::Object(resolver));

    if input.settings.tun.enabled {
        set_route(state, "auto_detect_interface", json!(true));
        let tun_rules = input
            .tun_singbox_rules
            .clone()
            .unwrap_or_else(sample_tun_singbox_rules);
        if let Value::Array(list) = tun_rules {
            push_rules(state, list);
        }
        let tun_addresses: Vec<String> = state
            .config
            .get("inbounds")
            .and_then(Value::as_array)
            .and_then(|list| {
                list.iter()
                    .find(|i| i.get("type").and_then(Value::as_str) == Some("tun"))
            })
            .and_then(|tun| tun.get("address"))
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|v| v.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if !tun_addresses.is_empty() {
            let addresses: Vec<String> = tun_addresses
                .iter()
                .map(|a| to_single_address_prefix(a))
                .collect();
            push_rules(
                state,
                vec![json!({"ip_cidr": addresses, "action": "reject", "method": "drop"})],
            );
        }
        let direct_exe = &input.settings.protect_core_executables;
        if !direct_exe.is_empty() {
            push_rules(
                state,
                vec![
                    json!({"port": [53], "action": "hijack-dns", "process_path": direct_exe}),
                    json!({"outbound": DIRECT_TAG, "process_path": direct_exe}),
                ],
            );
        }
        let mut icmp = input.settings.tun.icmp_routing.clone().unwrap_or_default();
        if !TUN_ICMP_ROUTING_POLICIES.contains(&icmp.as_str()) {
            icmp = TUN_ICMP_ROUTING_POLICIES[0].to_string();
        }
        if icmp == "direct" {
            push_rules(
                state,
                vec![json!({"network": ["icmp"], "outbound": DIRECT_TAG})],
            );
        } else if icmp != "rule" {
            let method = match icmp.as_str() {
                "unreachable" => "default",
                "drop" => "drop",
                _ => "reply",
            };
            push_rules(
                state,
                vec![json!({"network": ["icmp"], "action": "reject", "method": method})],
            );
        }
    }

    if input.settings.inbound.sniffing_enabled {
        push_rules(state, vec![json!({"action": "sniff"})]);
        push_rules(
            state,
            vec![json!({
                "type": "logical",
                "mode": "or",
                "action": "hijack-dns",
                "rules": [{"port": [53]}, {"protocol": ["dns"]}]
            })],
        );
        if input.settings.core_basic.enable_final_fragment {
            push_rules(
                state,
                vec![
                    json!({"protocol": ["tls"], "action": "route-options", "tls_record_fragment": true}),
                ],
            );
        }
    } else {
        push_rules(state, vec![json!({"port": [53], "action": "hijack-dns"})]);
        if input.settings.core_basic.enable_final_fragment {
            push_rules(
                state,
                vec![json!({"action": "route-options", "tls_record_fragment": true})],
            );
        }
    }

    // hosts resolve rule
    let mut hosts_domains: Vec<String> = Vec::new();
    if !raw_dns_enabled {
        for (host, _) in parse_hosts_to_dictionary(simple.hosts.as_deref()) {
            hosts_domains.push(host);
        }
        if simple.use_system_hosts {
            if let Some(dns) = &input.dns {
                hosts_domains.extend(dns.system_hosts.keys().cloned());
            }
        }
    }
    if !hosts_domains.is_empty() {
        let mut resolve_rule = obj();
        resolve_rule.insert("action".into(), json!("resolve"));
        let mut counter = 0;
        for host in &hosts_domains {
            let mut domain_rule = obj();
            if !parse_v2_domain(host, &mut domain_rule) {
                continue;
            }
            let has_keyword = domain_rule
                .get("domain_keyword")
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty());
            if has_keyword && !host.contains(':') {
                let keyword = domain_rule.remove("domain_keyword").unwrap_or(Value::Null);
                domain_rule.insert("domain".into(), keyword);
            }
            for key in [
                "domain",
                "domain_keyword",
                "domain_suffix",
                "domain_regex",
                "geosite",
            ] {
                let values = domain_rule.get(key).and_then(Value::as_array).cloned();
                if let Some(values) = values.filter(|v| !v.is_empty()) {
                    let target = resolve_rule.entry(key).or_insert_with(|| json!([]));
                    if let Some(list) = target.as_array_mut() {
                        list.extend(values);
                    }
                    counter += 1;
                    break;
                }
            }
        }
        if counter > 0 {
            push_rules(state, vec![Value::Object(resolve_rule)]);
        }
    }

    push_rules(
        state,
        vec![
            json!({"outbound": DIRECT_TAG, "clash_mode": "Direct"}),
            json!({"outbound": PROXY_TAG, "clash_mode": "Global"}),
        ],
    );

    let mut domain_strategy = input
        .settings
        .routing_basic
        .domain_strategy4_singbox
        .clone()
        .filter(|s| !s.is_empty());
    let routing = input.routing.clone().unwrap_or_default();
    if let Some(override_strategy) = routing
        .domain_strategy4_singbox
        .clone()
        .filter(|s| !s.is_empty())
    {
        domain_strategy = Some(override_strategy);
    }
    let mut resolve_rule = obj();
    resolve_rule.insert("action".into(), json!("resolve"));
    put_opt_string(&mut resolve_rule, "strategy", domain_strategy.as_ref());
    let resolve_rule = Value::Object(resolve_rule);
    if input.settings.routing_basic.domain_strategy == IP_ON_DEMAND {
        push_rules(state, vec![resolve_rule.clone()]);
    }

    let mut ip_rules: Vec<crate::input::CodegenRule> = Vec::new();
    // `ERuleMode.Global` / `Direct` bypass user rules (F-ROUTING-001). The
    // split position is recorded so the override can drop exactly the user
    // rules while keeping the sniff / hijack-dns / clash_mode base.
    let user_rule_start = state
        .config
        .get("route")
        .and_then(|r| r.get("rules"))
        .and_then(Value::as_array)
        .map(|list| list.len())
        .unwrap_or(0);
    let bypass_user_rules = matches!(input.rule_mode.as_deref(), Some("Global") | Some("Direct"));
    if !bypass_user_rules {
        for rule in &routing.rule_set {
            if !rule.enabled || rule.is_dns() {
                continue;
            }
            gen_user_rule(state, rule)?;
            if rule.ip.as_ref().is_some_and(|v| !v.is_empty()) {
                ip_rules.push(rule.clone());
            }
        }
    }
    apply_rule_mode_override(state, user_rule_start);
    if input.settings.routing_basic.domain_strategy == IP_IF_NON_MATCH {
        push_rules(state, vec![resolve_rule]);
        for rule in &ip_rules {
            gen_user_rule(state, rule)?;
        }
    }
    Ok(())
}

fn set_route(state: &mut SboxState<'_>, key: &str, value: Value) {
    if let Some(route) = state.config.get_mut("route").and_then(Value::as_object_mut) {
        route.insert(key.into(), value);
    }
}

/// `ERuleMode` override (F-ROUTING-001). Drops the user rules emitted above
/// and leaves a single catch-all; also points `final` at the mode outbound.
fn apply_rule_mode_override(state: &mut SboxState<'_>, user_rule_start: usize) {
    let mode = state.input.rule_mode.as_deref().unwrap_or("Rule");
    if mode != "Global" && mode != "Direct" {
        return;
    }
    let outbound = if mode == "Global" {
        PROXY_TAG
    } else {
        DIRECT_TAG
    };
    if let Some(list) = state
        .config
        .get_mut("route")
        .and_then(|r| r.get_mut("rules"))
        .and_then(Value::as_array_mut)
    {
        list.truncate(user_rule_start);
        list.push(json!({"outbound": outbound}));
    }
    set_route(state, "final", json!(outbound));
    state.diagnostics.push(Diagnostic::info(
        "rule_mode_override",
        format!("rule mode '{mode}' replaced the user routing rules"),
    ));
}

pub(crate) fn push_rules(state: &mut SboxState<'_>, rules: Vec<Value>) {
    if let Some(list) = state
        .config
        .get_mut("route")
        .and_then(|r| r.get_mut("rules"))
        .and_then(Value::as_array_mut)
    {
        list.extend(rules);
    }
}

pub(crate) fn gen_user_rule(
    state: &mut SboxState<'_>,
    rule: &crate::input::CodegenRule,
) -> Result<(), CodegenError> {
    let outbound = gen_user_rule_outbound(state, &rule.outbound_tag)?;
    let mut base = obj();
    if outbound == BLOCK_TAG {
        base.insert("action".into(), json!("reject"));
    } else {
        base.insert("outbound".into(), json!(outbound));
    }
    if let Some(port) = rule.port.as_deref().filter(|p| !p.is_empty()) {
        let ranges: Vec<String> = port
            .split(',')
            .filter(|p| p.contains('-'))
            .map(|p| p.replace('-', ":"))
            .collect();
        let ports: Vec<i64> = port
            .split(',')
            .filter(|p| !p.contains('-'))
            .filter_map(|p| p.trim().parse::<i64>().ok())
            .collect();
        if !ranges.is_empty() {
            base.insert("port_range".into(), json!(ranges));
        }
        if !ports.is_empty() {
            base.insert("port".into(), json!(ports));
        }
    }
    if let Some(network) = rule.network.as_deref().filter(|n| !n.is_empty()) {
        base.insert("network".into(), json!(string2_list(network)));
    }
    if let Some(protocol) = rule.protocol.as_ref().filter(|v| !v.is_empty()) {
        base.insert("protocol".into(), json!(protocol));
    }
    if let Some(inbound) = rule.inbound_tag.as_ref() {
        base.insert("inbound".into(), json!(inbound));
    }
    let base = Value::Object(base);

    let mut has_domain_ip = false;
    if let Some(domains) = rule.domain.as_ref().filter(|v| !v.is_empty()) {
        let mut rule1 = base.clone();
        let mut count = 0;
        for domain in domains {
            if let Some(map) = rule1.as_object_mut() {
                if parse_v2_domain(domain, map) {
                    count += 1;
                }
            }
        }
        if count > 0 {
            push_rules(state, vec![rule1]);
            has_domain_ip = true;
        }
    }

    if let Some(ips) = rule.ip.as_ref().filter(|v| !v.is_empty()) {
        let mut rule2 = base.clone();
        let mut count = 0;
        let negative: Vec<&String> = ips.iter().filter(|ip| ip.starts_with('!')).collect();
        if !negative.is_empty() {
            let positive: Vec<&String> = ips.iter().filter(|ip| !ip.starts_with('!')).collect();
            let mut positive_rule = rule2.clone();
            if let Some(map) = positive_rule.as_object_mut() {
                map.remove("outbound");
                map.remove("action");
                for ip in &positive {
                    if parse_v2_address(ip, map) {
                        count += 1;
                    }
                }
            }
            let mut negative_rule = obj();
            for ip in &negative {
                let ip = ip[1..].trim();
                if parse_v2_address(ip, &mut negative_rule) {
                    count += 1;
                }
            }
            negative_rule.insert("invert".into(), json!(true));
            let outbound = rule2.get("outbound").cloned();
            let action = rule2.get("action").cloned();
            let mut logical = obj();
            if let Some(outbound) = outbound {
                logical.insert("outbound".into(), outbound);
            }
            if let Some(action) = action {
                logical.insert("action".into(), action);
            }
            logical.insert("type".into(), json!("logical"));
            logical.insert("mode".into(), json!("or"));
            logical.insert("rules".into(), json!([positive_rule, negative_rule]));
            rule2 = Value::Object(logical);
        } else if let Some(map) = rule2.as_object_mut() {
            for ip in ips {
                if parse_v2_address(ip, map) {
                    count += 1;
                }
            }
        }
        if count > 0 {
            push_rules(state, vec![rule2]);
            has_domain_ip = true;
        }
    }

    if let Some(processes) = rule.process.as_ref().filter(|v| !v.is_empty()) {
        let mut rule_name = base.clone();
        let mut rule_path = base.clone();
        if let Some(map) = rule_name.as_object_mut() {
            map.insert("process_name".into(), json!([]));
        }
        if let Some(map) = rule_path.as_object_mut() {
            map.insert("process_path".into(), json!([]));
        }
        for process in processes {
            if process == "self/" || process == "xray/" {
                if let Some(list) = rule_name
                    .get_mut("process_name")
                    .and_then(Value::as_array_mut)
                {
                    list.push(json!(exe_name("sing-box", state.input.settings.platform)));
                }
                continue;
            }
            if process.contains('/') || process.contains('\\') {
                let path = if state.input.settings.platform == crate::input::Platform::Windows {
                    process.replace('/', "\\")
                } else {
                    process.clone()
                };
                if let Some(list) = rule_path
                    .get_mut("process_path")
                    .and_then(Value::as_array_mut)
                {
                    list.push(json!(path));
                }
                continue;
            }
            if let Some(list) = rule_name
                .get_mut("process_name")
                .and_then(Value::as_array_mut)
            {
                list.push(json!(exe_name(process, state.input.settings.platform)));
            }
        }
        let name_empty = rule_name
            .get("process_name")
            .and_then(Value::as_array)
            .is_some_and(|a| a.is_empty());
        let path_empty = rule_path
            .get("process_path")
            .and_then(Value::as_array)
            .is_some_and(|a| a.is_empty());
        if !name_empty {
            push_rules(state, vec![rule_name]);
            has_domain_ip = true;
        }
        if !path_empty {
            push_rules(state, vec![rule_path]);
            has_domain_ip = true;
        }
    }

    if !has_domain_ip {
        let plain_has_content = base.get("port").is_some()
            || base.get("port_range").is_some()
            || base.get("protocol").is_some()
            || base.get("inbound").is_some()
            || base.get("network").is_some();
        if plain_has_content {
            push_rules(state, vec![base]);
        }
    }
    Ok(())
}

fn gen_user_rule_outbound(
    state: &mut SboxState<'_>,
    outbound_tag: &str,
) -> Result<String, CodegenError> {
    let tag = if outbound_tag.is_empty() {
        PROXY_TAG
    } else {
        outbound_tag
    };
    if [PROXY_TAG, DIRECT_TAG, BLOCK_TAG].contains(&tag) {
        return Ok(tag.to_string());
    }
    let node: Option<CodegenProfile> = state
        .input
        .profiles
        .values()
        .find(|p| p.remarks == tag)
        .cloned();
    let Some(node) = node else {
        warn_dangling(state, tag, "no profile matches the remarks");
        return Ok(PROXY_TAG.to_string());
    };
    if !singbox_supported_type(node.config_type)
        && !node.config_type.is_group()
        && node.config_type != ConfigType::Outbound
    {
        warn_dangling(
            state,
            tag,
            &format!(
                "config type {:?} is not in Global.SingboxSupportConfigType",
                node.config_type
            ),
        );
        return Ok(PROXY_TAG.to_string());
    }
    let generated_tag = format!("{}-{PROXY_TAG}-{}", node.index_id, node.remarks);
    let exists = state
        .outbounds
        .iter()
        .chain(state.endpoints.iter())
        .any(|s| {
            s.get("tag")
                .and_then(Value::as_str)
                .is_some_and(|existing| existing.starts_with(&generated_tag))
        });
    if exists {
        return Ok(generated_tag);
    }
    let built = build_all_proxy_outbounds(state.input, &node, &generated_tag)?;
    state.custom_tags.extend(built.custom_tags);
    state.outbounds.extend(built.outbounds);
    state.endpoints.extend(built.endpoints);
    Ok(generated_tag)
}

fn singbox_supported_type(config_type: ConfigType) -> bool {
    matches!(
        config_type,
        ConfigType::Vmess
            | ConfigType::Vless
            | ConfigType::Shadowsocks
            | ConfigType::Trojan
            | ConfigType::Hysteria2
            | ConfigType::Tuic
            | ConfigType::Anytls
            | ConfigType::Naive
            | ConfigType::WireGuard
            | ConfigType::Socks
            | ConfigType::Http
    )
}

/// Issue T06b/ISSUE-04: an unresolved routing reference still falls back to
/// `Global.ProxyTag` (upstream behavior) but must not do so silently.
fn warn_dangling(state: &mut SboxState<'_>, tag: &str, reason: &str) {
    state.diagnostics.push(Diagnostic::warning(
        "routing_dangling_reference",
        format!(
            "routing rule references outbound '{tag}' that cannot be resolved ({reason}); \
             falling back to '{PROXY_TAG}'"
        ),
        Some("routing.ruleSet[].outboundTag"),
    ));
}

pub(crate) fn parse_v2_domain(domain: &str, rule: &mut Map<String, Value>) -> bool {
    if domain.starts_with('#') || domain.starts_with("ext:") || domain.starts_with("ext-domain:") {
        return false;
    }
    let (key, value) = if let Some(stripped) = domain.strip_prefix(GEOSITE_PREFIX) {
        ("geosite", stripped.to_string())
    } else if let Some(stripped) = domain.strip_prefix("regexp:") {
        ("domain_regex", stripped.replace(ROUTING_RULE_COMMA, ","))
    } else if let Some(stripped) = domain.strip_prefix("domain:") {
        ("domain_suffix", stripped.to_string())
    } else if let Some(stripped) = domain.strip_prefix("full:") {
        ("domain", stripped.to_string())
    } else if let Some(stripped) = domain.strip_prefix("keyword:") {
        ("domain_keyword", stripped.to_string())
    } else if let Some(stripped) = domain.strip_prefix("dotless:") {
        ("domain_keyword", stripped.to_string())
    } else {
        ("domain_keyword", domain.to_string())
    };
    let target = rule.entry(key).or_insert_with(|| json!([]));
    if let Some(list) = target.as_array_mut() {
        list.push(json!(value));
    }
    true
}

pub(crate) fn parse_v2_address(address: &str, rule: &mut Map<String, Value>) -> bool {
    if address.starts_with("ext:") || address.starts_with("ext-ip:") {
        return false;
    }
    if address == format!("{GEOIP_PREFIX}private") {
        rule.insert("ip_is_private".into(), json!(true));
        return true;
    }
    if let Some(stripped) = address.strip_prefix(GEOIP_PREFIX) {
        let target = rule.entry("geoip").or_insert_with(|| json!([]));
        if let Some(list) = target.as_array_mut() {
            list.push(json!(stripped));
        }
        return true;
    }
    let target = rule.entry("ip_cidr").or_insert_with(|| json!([]));
    if let Some(list) = target.as_array_mut() {
        list.push(json!(address));
    }
    true
}
