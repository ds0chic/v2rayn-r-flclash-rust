//! sing-box outbound/endpoint/transport/TLS generation (`SingboxOutboundService`).

use serde_json::{json, Map, Value};

use crate::input::{CodegenInput, CodegenProfile, ConfigType, MultipleLoad, TransportExtra};
use crate::util::*;
use crate::{CodegenError, Diagnostic};

/// Generated servers, split by sing-box container.
#[derive(Debug, Default)]
pub(crate) struct BuiltServers {
    pub outbounds: Vec<Value>,
    pub endpoints: Vec<Value>,
    pub custom_tags: Vec<(String, String)>,
}

impl BuiltServers {
    fn push(&mut self, value: Value, is_endpoint: bool, custom: Option<String>, tag: &str) {
        if let Some(index_id) = custom {
            self.custom_tags.push((tag.to_string(), index_id));
        }
        if is_endpoint {
            self.endpoints.push(value);
        } else {
            self.outbounds.push(value);
        }
    }

    pub(crate) fn all_tags(&self) -> Vec<String> {
        self.outbounds
            .iter()
            .chain(self.endpoints.iter())
            .filter_map(|s| {
                s.get("tag")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .collect()
    }
}

pub(crate) fn child_items(node: &CodegenProfile) -> Option<Vec<String>> {
    string2_list_opt(node.proto_extra.child_items.as_ref())
}

/// Diagnostics are collected through a thread-local sink because the pure
/// builder functions only receive `&CodegenInput`. The generator sets a sink
/// around each run.
use std::cell::RefCell;

thread_local! {
    static DIAGNOSTIC_SINK: RefCell<Vec<Diagnostic>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn diagnostic_sink_reset() {
    DIAGNOSTIC_SINK.with(|sink| sink.borrow_mut().clear());
}

pub(crate) fn diagnostic_sink_take() -> Vec<Diagnostic> {
    DIAGNOSTIC_SINK.with(|sink| std::mem::take(&mut *sink.borrow_mut()))
}

fn push_diagnostic(_input: &CodegenInput, diagnostic: Diagnostic) {
    DIAGNOSTIC_SINK.with(|sink| sink.borrow_mut().push(diagnostic));
}

pub(crate) fn resolve_children<'a>(
    input: &'a CodegenInput,
    node: &CodegenProfile,
) -> Result<Vec<&'a CodegenProfile>, CodegenError> {
    let items = child_items(node).ok_or_else(|| {
        CodegenError::missing_required_field(
            "group node has no child items",
            "profile.protoExtra.childItems",
        )
    })?;
    let mut children = Vec::with_capacity(items.len());
    for id in items {
        match input.profiles.get(&id) {
            Some(child) => children.push(child),
            None => {
                return Err(CodegenError::dangling_reference(
                    format!("child item {id} does not resolve to a profile"),
                    "profile.protoExtra.childItems",
                ))
            }
        }
    }
    if children.is_empty() {
        return Err(CodegenError::missing_required_field(
            "group node has no child items",
            "profile.protoExtra.childItems",
        ));
    }
    Ok(children)
}

pub(crate) fn get_network(node: &CodegenProfile) -> String {
    if node.network.is_empty() || !NETWORKS.contains(&node.network.as_str()) {
        DEFAULT_NETWORK.to_string()
    } else {
        node.network.trim().to_string()
    }
}

/// Issue T06b/M-012: mirror `NodeValidator.ValidateSingboxTransport`. The
/// generator silently ignores a non-`raw` transport for protocols that cannot
/// carry it (e.g. TUIC+ws); surface that as a structured warning instead of
/// dropping it without a trace.
pub(crate) fn transport_diagnostic(node: &CodegenProfile) -> Option<Diagnostic> {
    if node.config_type.is_group()
        || node.config_type == ConfigType::Custom
        || node.config_type == ConfigType::Outbound
    {
        return None;
    }
    let network = get_network(node);
    if SINGBOX_REJECTED_NETWORKS.contains(&network.as_str()) {
        // Already rejected by `validate()` with a hard error.
        return None;
    }
    let transport_protocols = [
        ConfigType::Vmess,
        ConfigType::Vless,
        ConfigType::Trojan,
        ConfigType::Shadowsocks,
    ];
    if !transport_protocols.contains(&node.config_type) && network != DEFAULT_NETWORK {
        return Some(Diagnostic::warning(
            "singbox_transport_ignored",
            format!(
                "transport '{network}' is ignored for config type {:?}: sing-box only supports \
                 the raw transport for this protocol",
                node.config_type
            ),
            Some("profile.network"),
        ));
    }
    if node.config_type == ConfigType::Shadowsocks && !["raw", "ws"].contains(&network.as_str()) {
        return Some(Diagnostic::warning(
            "singbox_transport_ignored",
            format!(
                "transport '{network}' is ignored for Shadowsocks: sing-box only supports \
                 raw/ws (plugin based) for it"
            ),
            Some("profile.network"),
        ));
    }
    None
}

pub(crate) fn build_all_proxy_outbounds(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<BuiltServers, CodegenError> {
    if let Some(diagnostic) = transport_diagnostic(node) {
        push_diagnostic(input, diagnostic);
    }
    let mut servers = if node.config_type.is_group() {
        match node.config_type {
            ConfigType::PolicyGroup => build_outbounds_list(input, node, base_tag)?,
            ConfigType::ProxyChain => build_chain_outbounds_list(input, node, base_tag)?,
            _ => unreachable!(),
        }
    } else {
        let (value, is_endpoint, custom, tag) = build_proxy_outbound(input, node, base_tag)?;
        let mut built = BuiltServers::default();
        built.push(value, is_endpoint, custom, &tag);
        built
    };

    let proxy_tags: Vec<String> = servers
        .all_tags()
        .into_iter()
        .filter(|tag| tag.starts_with(base_tag))
        .collect();
    if proxy_tags.len() > 1 {
        let multiple_load = node
            .proto_extra
            .multiple_load
            .unwrap_or(MultipleLoad::LeastPing);
        let (selector, urltest) = build_selector_outbounds(&proxy_tags, base_tag, multiple_load);
        // Upstream inserts `[selector, urltest]` at index 0.
        servers.outbounds.insert(0, urltest);
        servers.outbounds.insert(0, selector);
    }
    Ok(servers)
}

fn build_group_proxy_outbounds(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<BuiltServers, CodegenError> {
    match node.config_type {
        ConfigType::PolicyGroup => build_outbounds_list(input, node, base_tag),
        ConfigType::ProxyChain => build_chain_outbounds_list(input, node, base_tag),
        _ => {
            let (value, is_endpoint, custom, tag) = build_proxy_outbound(input, node, base_tag)?;
            let mut built = BuiltServers::default();
            built.push(value, is_endpoint, custom, &tag);
            Ok(built)
        }
    }
}

fn build_outbounds_list(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<BuiltServers, CodegenError> {
    let children = resolve_children(input, node)?;
    let mut result = BuiltServers::default();
    for (i, child) in children.iter().enumerate() {
        let current_tag = if children.len() == 1 {
            base_tag.to_string()
        } else {
            format!("{base_tag}-{}-{}", i + 1, child.remarks)
        };
        if child.config_type.is_group() {
            let built = build_group_proxy_outbounds(input, child, &current_tag)?;
            result.outbounds.extend(built.outbounds);
            result.endpoints.extend(built.endpoints);
            result.custom_tags.extend(built.custom_tags);
            continue;
        }
        let (mut value, is_endpoint, custom, _) = build_proxy_outbound(input, child, &current_tag)?;
        if let Some(map) = value.as_object_mut() {
            map.insert("tag".into(), json!(current_tag));
        }
        result.push(value, is_endpoint, custom, &current_tag);
    }
    Ok(result)
}

fn build_chain_outbounds_list(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<BuiltServers, CodegenError> {
    let children = resolve_children(input, node)?;
    let nodes_reverse: Vec<&&CodegenProfile> = children.iter().rev().collect();
    let count = nodes_reverse.len();
    let mut result = BuiltServers::default();

    for (i, child) in nodes_reverse.iter().enumerate() {
        let child: &CodegenProfile = child;
        let current_tag = if i == 0 {
            base_tag.to_string()
        } else {
            format!("chain-{base_tag}-{i}-{}", child.remarks)
        };
        let dialer_proxy_tag = if i != count - 1 {
            Some(format!(
                "chain-{base_tag}-{}-{}",
                i + 1,
                nodes_reverse[i + 1].remarks
            ))
        } else {
            None
        };

        if child.config_type.is_group() {
            let mut built = build_group_proxy_outbounds(input, child, &current_tag)?;
            if let Some(detour) = &dialer_proxy_tag {
                for value in built.outbounds.iter_mut().chain(built.endpoints.iter_mut()) {
                    if outbound_detour(value).is_none() {
                        fill_detour(value, detour);
                    }
                }
            }
            if i != 0 {
                let chain_start_tags: Vec<String> = built
                    .all_tags()
                    .into_iter()
                    .filter(|tag| tag.starts_with(&current_tag))
                    .collect();
                match chain_start_tags.len() {
                    0 => {}
                    1 => {
                        let first = chain_start_tags[0].clone();
                        for existed in result
                            .outbounds
                            .iter_mut()
                            .chain(result.endpoints.iter_mut())
                        {
                            if outbound_detour(existed).as_deref() == Some(current_tag.as_str()) {
                                fill_detour(existed, &first);
                            }
                        }
                    }
                    _ => {
                        let existed_clones = result.outbounds.clone();
                        result.outbounds.clear();
                        for (j, chain_start_tag) in chain_start_tags.iter().enumerate() {
                            let mut clone_list = existed_clones.clone();
                            for existed in clone_list.iter_mut() {
                                if let Some(tag) = existed.get("tag").and_then(Value::as_str) {
                                    let clone_tag = format!("{tag}-clone-{}", j + 1);
                                    if let Some(map) = existed.as_object_mut() {
                                        map.insert("tag".into(), json!(clone_tag));
                                    }
                                }
                            }
                            let len = clone_list.len();
                            let clone_tags: Vec<String> = clone_list
                                .iter()
                                .map(|v| {
                                    v.get("tag")
                                        .and_then(Value::as_str)
                                        .unwrap_or("")
                                        .to_string()
                                })
                                .collect();
                            for (k, existed) in clone_list.iter_mut().enumerate() {
                                let previous = outbound_detour(existed);
                                let next_tag = if k + 1 < len {
                                    clone_tags[k + 1].clone()
                                } else {
                                    chain_start_tag.clone()
                                };
                                let target = if previous.as_deref() == Some(current_tag.as_str()) {
                                    chain_start_tag.clone()
                                } else {
                                    next_tag
                                };
                                fill_detour(existed, &target);
                                result.outbounds.push(existed.clone());
                            }
                        }
                    }
                }
            }
            result.outbounds.extend(built.outbounds);
            result.endpoints.extend(built.endpoints);
            result.custom_tags.extend(built.custom_tags);
            continue;
        }

        let (mut value, is_endpoint, custom, _) = build_proxy_outbound(input, child, &current_tag)?;
        if let Some(map) = value.as_object_mut() {
            map.insert("tag".into(), json!(current_tag));
        }
        if let Some(detour) = &dialer_proxy_tag {
            fill_detour(&mut value, detour);
        }
        result.push(value, is_endpoint, custom, &current_tag);
    }
    Ok(result)
}

pub(crate) fn outbound_detour(value: &Value) -> Option<String> {
    value
        .get("detour")?
        .as_str()
        .map(ToString::to_string)
        .filter(|s| !s.is_empty())
}

pub(crate) fn fill_detour(value: &mut Value, detour: &str) {
    if let Some(map) = value.as_object_mut() {
        map.insert("detour".into(), json!(detour));
    }
}

/// Build one proxy server. Returns `(value, is_endpoint, custom_index_id, tag)`.
fn build_proxy_outbound(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<(Value, bool, Option<String>, String), CodegenError> {
    if node.config_type == ConfigType::Outbound {
        let is_endpoint = node.proto_extra.is_singbox_endpoint == Some(true);
        let mut placeholder = if is_endpoint {
            json!({"type": "vless"})
        } else {
            json!({"type": "vless", "server": "", "server_port": 443})
        };
        if let Some(map) = placeholder.as_object_mut() {
            map.insert("tag".into(), json!(base_tag));
        }
        return Ok((
            placeholder,
            is_endpoint,
            Some(node.index_id.clone()),
            base_tag.to_string(),
        ));
    }

    if node.config_type == ConfigType::WireGuard {
        let endpoint = fill_endpoint(node);
        return Ok((endpoint, true, None, base_tag.to_string()));
    }

    let outbound = fill_outbound(input, node)?;
    Ok((outbound, false, None, base_tag.to_string()))
}

fn fill_outbound(input: &CodegenInput, node: &CodegenProfile) -> Result<Value, CodegenError> {
    let protocol = node.config_type.protocol_type().ok_or_else(|| {
        CodegenError::unsupported_combination(
            format!(
                "config type {:?} has no sing-box outbound type",
                node.config_type
            ),
            "profile.configType",
        )
    })?;
    let mut outbound = obj();
    outbound.insert("type".into(), json!(protocol));
    outbound.insert("tag".into(), json!(PROXY_TAG));
    outbound.insert("server".into(), json!(node.address));
    outbound.insert("server_port".into(), json!(node.port));

    let extra = &node.proto_extra;
    let network = get_network(node);

    match node.config_type {
        ConfigType::Vmess => {
            outbound.insert("uuid".into(), json!(node.password));
            outbound.insert(
                "alter_id".into(),
                json!(parse_i32(extra.alter_id.as_deref()).unwrap_or(0)),
            );
            outbound.insert(
                "security".into(),
                json!(valid_vmess_security(extra.vmess_security.as_deref())),
            );
            fill_outbound_mux(input, node, &mut outbound);
            fill_outbound_transport(input, node, &mut outbound);
        }
        ConfigType::Shadowsocks => {
            let method = extra.ss_method.as_deref().unwrap_or("");
            outbound.insert(
                "method".into(),
                json!(if SS_SECURITIES_IN_SINGBOX.contains(&method) {
                    method
                } else {
                    NONE
                }),
            );
            outbound.insert("password".into(), json!(node.password));
            if extra.uot == Some(true) {
                outbound.insert("udp_over_tcp".into(), json!(true));
            }
            fill_shadowsocks_plugin(node, &network, &mut outbound);
            fill_outbound_mux(input, node, &mut outbound);
        }
        ConfigType::Socks => {
            outbound.insert("version".into(), json!("5"));
            if !node.username.is_empty() && !node.password.is_empty() {
                outbound.insert("username".into(), json!(node.username));
                outbound.insert("password".into(), json!(node.password));
            }
        }
        ConfigType::Http => {
            if !node.username.is_empty() && !node.password.is_empty() {
                outbound.insert("username".into(), json!(node.username));
                outbound.insert("password".into(), json!(node.password));
            }
        }
        ConfigType::Vless => {
            outbound.insert("uuid".into(), json!(node.password));
            outbound.insert("packet_encoding".into(), json!("xudp"));
            match extra.flow.as_deref() {
                Some("xtls-rprx-vision" | "xtls-rprx-vision-udp443") => {
                    outbound.insert("flow".into(), json!("xtls-rprx-vision"));
                }
                Some(flow) if !flow.is_empty() => {
                    outbound.insert("flow".into(), json!(flow));
                }
                _ => {}
            }
            fill_outbound_mux(input, node, &mut outbound);
            fill_outbound_transport(input, node, &mut outbound);
        }
        ConfigType::Trojan => {
            outbound.insert("password".into(), json!(node.password));
            fill_outbound_mux(input, node, &mut outbound);
            fill_outbound_transport(input, node, &mut outbound);
        }
        ConfigType::Hysteria2 => {
            outbound.insert("password".into(), json!(node.password));
            fill_hysteria2(input, node, &mut outbound);
        }
        ConfigType::Tuic => {
            outbound.insert("uuid".into(), json!(node.username));
            outbound.insert("password".into(), json!(node.password));
            put_opt_string(
                &mut outbound,
                "congestion_control",
                extra.congestion_control.as_ref(),
            );
        }
        ConfigType::Anytls => {
            outbound.insert("password".into(), json!(node.password));
        }
        ConfigType::Naive => {
            outbound.insert("username".into(), json!(node.username));
            outbound.insert("password".into(), json!(node.password));
            if extra.naive_quic == Some(true) {
                outbound.insert("quic".into(), json!(true));
                put_opt_string(
                    &mut outbound,
                    "quic_congestion_control",
                    extra.congestion_control.as_ref(),
                );
            }
            if let Some(concurrency) = extra.insecure_concurrency.filter(|v| *v > 0) {
                outbound.insert("insecure_concurrency".into(), json!(concurrency));
            }
            if extra.uot == Some(true) {
                outbound.insert("udp_over_tcp".into(), json!(true));
            }
        }
        ConfigType::WireGuard => unreachable!(),
        _ => {
            return Err(CodegenError::unsupported_combination(
                format!(
                    "config type {:?} is not generated by sing-box",
                    node.config_type
                ),
                "profile.configType",
            ))
        }
    }

    fill_outbound_tls(input, node, &mut outbound);
    Ok(Value::Object(outbound))
}

fn valid_vmess_security(value: Option<&str>) -> &str {
    match value {
        Some(v) if VMESS_SECURITIES.contains(&v) => v,
        _ => DEFAULT_SECURITY,
    }
}

fn parse_i32(value: Option<&str>) -> Option<i32> {
    value.and_then(|v| v.trim().parse::<i32>().ok())
}

/// Shadowsocks `plugin`/`plugin_opts` handling.
fn fill_shadowsocks_plugin(
    node: &CodegenProfile,
    network: &str,
    outbound: &mut Map<String, Value>,
) {
    let extra = &node.transport_extra;
    if network == "raw" && extra.raw_header_type.as_deref() == Some(RAW_HEADER_HTTP) {
        outbound.insert("plugin".into(), json!("obfs-local"));
        outbound.insert(
            "plugin_opts".into(),
            json!(format!(
                "obfs=http;obfs-host={};",
                extra.host.clone().unwrap_or_default()
            )),
        );
        return;
    }
    let mut plugin_args = String::new();
    if network == "ws" {
        plugin_args.push_str("mode=websocket;");
        plugin_args.push_str(&format!("host={};", extra.host.clone().unwrap_or_default()));
        let path = extra
            .path
            .clone()
            .unwrap_or_default()
            .replace('\\', "\\\\")
            .replace('=', "\\=")
            .replace(',', "\\,");
        plugin_args.push_str(&format!("path={path};"));
    }
    if node.stream_security == STREAM_SECURITY {
        plugin_args.push_str("tls;");
        let certs = parse_pem_chain(&node.cert);
        if let Some(cert) = certs.first() {
            let base64 = cert
                .replace("-----BEGIN CERTIFICATE-----", "")
                .replace("-----END CERTIFICATE-----", "")
                .replace('\n', "")
                .trim()
                .replace('=', "\\=");
            plugin_args.push_str(&format!("certRaw={base64};"));
        }
    }
    if !plugin_args.is_empty() {
        plugin_args.push_str("mux=0;");
        let plugin_args = plugin_args.trim_end_matches(';').to_string();
        outbound.insert("plugin".into(), json!("v2ray-plugin"));
        outbound.insert("plugin_opts".into(), json!(plugin_args));
    }
}

fn fill_hysteria2(input: &CodegenInput, node: &CodegenProfile, outbound: &mut Map<String, Value>) {
    let extra = &node.proto_extra;
    if let Some(pass) = extra.salamander_pass.as_deref().filter(|s| !s.is_empty()) {
        let is_gecko = extra
            .gecko_min_packet_size
            .as_deref()
            .is_some_and(|s| !s.is_empty())
            || extra
                .gecko_max_packet_size
                .as_deref()
                .is_some_and(|s| !s.is_empty());
        let mut obfs = obj();
        obfs.insert(
            "type".into(),
            json!(if is_gecko { "gecko" } else { "salamander" }),
        );
        obfs.insert("password".into(), json!(pass.trim()));
        if is_gecko {
            obfs.insert(
                "min_packet_size".into(),
                json!(parse_i32(extra.gecko_min_packet_size.as_deref()).unwrap_or(0)),
            );
            obfs.insert(
                "max_packet_size".into(),
                json!(parse_i32(extra.gecko_max_packet_size.as_deref()).unwrap_or(0)),
            );
        }
        outbound.insert("obfs".into(), Value::Object(obfs));
    }

    let up_mbps = match extra.up_mbps {
        Some(v) if v >= 0 => Some(v),
        _ => input.settings.hysteria.up_mbps,
    };
    let down_mbps = match extra.down_mbps {
        Some(v) if v >= 0 => Some(v),
        _ => input.settings.hysteria.down_mbps,
    };
    if up_mbps.unwrap_or(0) > 0 {
        outbound.insert("up_mbps".into(), json!(up_mbps));
    }
    if down_mbps.unwrap_or(0) > 0 {
        outbound.insert("down_mbps".into(), json!(down_mbps));
    }

    let ports = extra.ports.as_deref().filter(|p| !p.is_empty());
    if let Some(ports) = ports {
        if ports.contains(':') || ports.contains('-') || ports.contains(',') {
            outbound.remove("server_port");
            let server_ports: Vec<String> = ports
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let port = p.replace('-', ":");
                    if port.contains(':') {
                        port
                    } else {
                        format!("{port}:{port}")
                    }
                })
                .collect();
            outbound.insert("server_ports".into(), json!(server_ports));
            let mut hop = if input.settings.hysteria.hop_interval >= 5 {
                format!("{}s", input.settings.hysteria.hop_interval)
            } else {
                format!("{HYSTERIA2_DEFAULT_HOP_INT}s")
            };
            if let Some(hop_extra) = extra.hop_interval.as_deref().filter(|s| !s.is_empty()) {
                if let Ok(parsed) = hop_extra.parse::<i32>() {
                    if parsed >= 5 {
                        hop = format!("{parsed}s");
                    }
                } else if hop_extra.contains('-') {
                    let parts: Vec<&str> = hop_extra.split('-').collect();
                    if parts.len() == 2 {
                        if let (Ok(low), Ok(high)) =
                            (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                        {
                            let average = (low + high) / 2;
                            if average >= 5 {
                                hop = format!("{average}s");
                            }
                        }
                    }
                }
            }
            outbound.insert("hop_interval".into(), json!(hop));
        }
    }

    if let Some(realm_url) = extra.hy2_realm_url.as_deref().filter(|s| !s.is_empty()) {
        if let Some(realm) = parse_realm(realm_url) {
            outbound.insert("realm".into(), realm);
            outbound.remove("server");
            outbound.remove("server_port");
            outbound.remove("server_ports");
        }
    }
    let _ = input;
}

/// Best-effort `HyRealm` parse (see contract UNC-X / UNC-S notes).
fn parse_realm(url: &str) -> Option<Value> {
    let rest = url
        .strip_prefix("realm://")
        .or_else(|| url.strip_prefix("hysteria2+realm+http://"))
        .or_else(|| url.strip_prefix("hysteria2+realm://"))?;
    let (token, rest) = match rest.split_once('@') {
        Some((token, rest)) => (Some(token.to_string()), rest),
        None => (None, rest),
    };
    let (authority, tail) = match rest.split_once('/') {
        Some((authority, tail)) => (authority.to_string(), tail.to_string()),
        None => (rest.to_string(), String::new()),
    };
    let (realm_name, query) = match tail.split_once('?') {
        Some((name, query)) => (name.to_string(), query.to_string()),
        None => (tail.to_string(), String::new()),
    };
    let stun_servers: Vec<String> = query
        .split('&')
        .filter_map(|pair| pair.strip_prefix("stun="))
        .map(ToString::to_string)
        .collect();
    let mut realm = obj();
    realm.insert("server_url".into(), json!(format!("https://{authority}")));
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        realm.insert("token".into(), json!(token));
    }
    if !realm_name.is_empty() {
        realm.insert("realm_id".into(), json!(realm_name));
    }
    if !stun_servers.is_empty() {
        realm.insert("stun_servers".into(), json!(stun_servers));
    }
    Some(Value::Object(realm))
}

fn fill_endpoint(node: &CodegenProfile) -> Value {
    let extra = &node.proto_extra;
    let mut endpoint = obj();
    endpoint.insert("type".into(), json!("wireguard"));
    let address = string2_list_opt(extra.wg_interface_address.as_ref())
        .unwrap_or_else(|| vec!["172.16.0.2/32".into()]);
    endpoint.insert("address".into(), json!(address));
    endpoint.insert("private_key".into(), json!(node.password));
    endpoint.insert(
        "mtu".into(),
        json!(extra.wg_mtu.filter(|m| *m > 0).unwrap_or(TUN_MTU_DEFAULT)),
    );
    let mut peer = obj();
    peer.insert("address".into(), json!(node.address));
    peer.insert("port".into(), json!(node.port));
    peer.insert(
        "public_key".into(),
        json!(extra.wg_public_key.clone().unwrap_or_default()),
    );
    put_opt_string(&mut peer, "pre_shared_key", extra.wg_preshared_key.as_ref());
    let reserved: Option<Vec<i64>> = string2_list_opt(extra.wg_reserved.as_ref())
        .map(|list| {
            list.iter()
                .filter_map(|s| parse_i32(Some(s)).map(i64::from))
                .collect()
        })
        .filter(|v: &Vec<i64>| !v.is_empty());
    put_opt_value(&mut peer, "reserved", reserved.map(|v| json!(v)));
    peer.insert("allowed_ips".into(), json!(["0.0.0.0/0", "::/0"]));
    endpoint.insert("peers".into(), json!([peer]));
    Value::Object(endpoint)
}

fn fill_outbound_mux(
    input: &CodegenInput,
    node: &CodegenProfile,
    outbound: &mut Map<String, Value>,
) {
    let mux_enabled = node.mux_enabled.unwrap_or(false);
    let mux = &input.settings.mux4_sbox;
    if mux_enabled && !mux.protocol.is_empty() {
        let mut value = obj();
        value.insert("enabled".into(), json!(true));
        value.insert("protocol".into(), json!(mux.protocol));
        value.insert("max_connections".into(), json!(mux.max_connections));
        put_opt_bool(&mut value, "padding", mux.padding);
        outbound.insert("multiplex".into(), Value::Object(value));
    }
}

fn parse_ech_config(ech_config: &str) -> Option<Value> {
    if ech_config.is_empty() {
        return None;
    }
    if !ech_config.contains("://") {
        return Some(json!({
            "enabled": true,
            "config": [format!("-----BEGIN ECH CONFIGS-----\n{ech_config}\n-----END ECH CONFIGS-----")]
        }));
    }
    let (query_server_name, _ech_dns_server) = match ech_config.find('+') {
        Some(idx) if idx > 0 => (
            Some(ech_config[..idx].to_string()),
            ech_config[idx + 1..].to_string(),
        ),
        _ => (None, ech_config.to_string()),
    };
    let mut ech = obj();
    ech.insert("enabled".into(), json!(true));
    put_opt_string(&mut ech, "query_server_name", query_server_name.as_ref());
    Some(Value::Object(ech))
}

fn fill_outbound_tls(
    input: &CodegenInput,
    node: &CodegenProfile,
    outbound: &mut Map<String, Value>,
) {
    if node.stream_security != STREAM_SECURITY && node.stream_security != STREAM_SECURITY_REALITY {
        return;
    }
    if matches!(
        node.config_type,
        ConfigType::Shadowsocks | ConfigType::Socks | ConfigType::WireGuard
    ) {
        return;
    }
    let server_name = if !node.sni.trim().is_empty() {
        Some(node.sni.trim().to_string())
    } else {
        let host = match get_network(node).as_str() {
            "raw" | "ws" | "httpupgrade" | "xhttp" => node.transport_extra.host.clone(),
            "grpc" => node.transport_extra.grpc_authority.clone(),
            _ => None,
        };
        host.and_then(|h| string2_list(&h).into_iter().next())
    };
    let mut tls = obj();
    tls.insert("enabled".into(), json!(true));
    put_opt_string(&mut tls, "server_name", server_name.as_ref());
    let mut insecure = node.allow_insecure;
    if let Some(alpn) = alpn_list(&node.alpn) {
        tls.insert("alpn".into(), json!(alpn));
    }
    if input.settings.core_basic.enable_fragment {
        tls.insert("fragment".into(), json!(true));
        tls.insert("record_fragment".into(), json!(true));
    }
    let fingerprint = if !node.fingerprint.trim().is_empty() {
        Some(node.fingerprint.trim().to_string())
    } else {
        input
            .settings
            .core_basic
            .def_fingerprint
            .clone()
            .filter(|s| !s.is_empty())
    };
    if let Some(fingerprint) = fingerprint {
        tls.insert(
            "utls".into(),
            json!({"enabled": true, "fingerprint": fingerprint}),
        );
    }
    if node.stream_security == STREAM_SECURITY {
        let certs = parse_pem_chain(&node.cert);
        if !certs.is_empty() {
            tls.insert("certificate".into(), json!(certs));
            insecure = false;
        }
    } else if node.stream_security == STREAM_SECURITY_REALITY {
        let mut reality = obj();
        reality.insert("enabled".into(), json!(true));
        put_opt_str(
            &mut reality,
            "public_key",
            option_non_empty(&node.public_key),
        );
        put_opt_str(&mut reality, "short_id", option_non_empty(&node.short_id));
        tls.insert("reality".into(), Value::Object(reality));
        insecure = false;
    }
    tls.insert("insecure".into(), json!(insecure));
    if let Some(ech) = parse_ech_config(&node.ech_config_list) {
        tls.insert("ech".into(), ech);
    }
    outbound.insert("tls".into(), Value::Object(tls));
}

fn option_non_empty(value: &str) -> Option<&str> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn fill_outbound_transport(
    input: &CodegenInput,
    _node: &CodegenProfile,
    outbound: &mut Map<String, Value>,
) {
    let node = _node;
    let Some(mut transport) = build_transport(input, node) else {
        return;
    };
    if let Some(map) = transport.as_object_mut() {
        if map.get("type").and_then(Value::as_str).is_some() {
            outbound.insert("transport".into(), transport);
        }
    }
}

fn build_transport(input: &CodegenInput, node: &CodegenProfile) -> Option<Value> {
    let extra: &TransportExtra = &node.transport_extra;
    let useragent = input
        .settings
        .core_basic
        .def_user_agent
        .clone()
        .unwrap_or_default();
    let useragent_value = raw_user_agent_value(&useragent);
    let mut transport = obj();
    match get_network(node).as_str() {
        "raw" => {
            if extra.raw_header_type.as_deref() == Some(RAW_HEADER_HTTP) {
                transport.insert("type".into(), json!("http"));
                if let Some(host) = extra.host.as_deref().filter(|h| !h.is_empty()) {
                    transport.insert("host".into(), json!(string2_list(host)));
                }
                if let Some(path) = extra.path.as_deref().filter(|p| !p.is_empty()) {
                    transport.insert("path".into(), json!(path));
                }
                if !useragent_value.is_empty() {
                    transport.insert("headers".into(), json!({"User-Agent": useragent_value}));
                }
            }
        }
        "ws" => {
            transport.insert("type".into(), json!("ws"));
            let mut ws_path = extra.path.clone();
            if let Some(path) = ws_path.as_deref().filter(|p| !p.is_empty()) {
                let (new_path, max_early_data, eh) = parse_ws_path(path);
                if let Some(value) = max_early_data {
                    transport.insert("max_early_data".into(), json!(value));
                    transport.insert(
                        "early_data_header_name".into(),
                        json!("Sec-WebSocket-Protocol"),
                    );
                }
                if let Some(name) = eh {
                    transport.insert("early_data_header_name".into(), json!(name));
                }
                ws_path = new_path;
            }
            if let Some(path) = ws_path.as_deref().filter(|p| !p.is_empty()) {
                transport.insert("path".into(), json!(path));
            }
            let mut headers = obj();
            if let Some(host) = extra.host.as_deref().filter(|h| !h.is_empty()) {
                headers.insert("Host".into(), json!(host));
            }
            if !useragent_value.is_empty() {
                headers.insert("User-Agent".into(), json!(useragent_value));
            }
            if !headers.is_empty() {
                transport.insert("headers".into(), Value::Object(headers));
            }
        }
        "httpupgrade" => {
            transport.insert("type".into(), json!("httpupgrade"));
            if let Some(path) = extra.path.as_deref().filter(|p| !p.is_empty()) {
                transport.insert("path".into(), json!(path));
            }
            if let Some(host) = extra.host.as_deref().filter(|h| !h.is_empty()) {
                transport.insert("host".into(), json!(host));
            }
            if !useragent_value.is_empty() {
                transport.insert("headers".into(), json!({"User-Agent": useragent_value}));
            }
        }
        "grpc" => {
            transport.insert("type".into(), json!("grpc"));
            if let Some(service) = extra.grpc_service_name.as_deref() {
                transport.insert("service_name".into(), json!(service));
            }
            if let Some(idle) = input.settings.grpc.idle_timeout {
                transport.insert("idle_timeout".into(), json!(format!("{idle}s")));
            }
            if let Some(ping) = input.settings.grpc.health_check_timeout {
                transport.insert("ping_timeout".into(), json!(format!("{ping}s")));
            }
            put_opt_bool(
                &mut transport,
                "permit_without_stream",
                input.settings.grpc.permit_without_stream,
            );
        }
        _ => {}
    }
    Some(Value::Object(transport))
}

/// Extract `?ed=N` / `?eh=...` out of a websocket path.
fn parse_ws_path(path: &str) -> (Option<String>, Option<i32>, Option<String>) {
    let mut result = path.to_string();
    let mut max_early_data = None;
    let mut eh = None;
    if let Some(idx) = result.find("?ed=") {
        let rest = &result[idx + 4..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(value) = digits.parse::<i32>() {
            max_early_data = Some(value);
            result.replace_range(idx..idx + 4 + digits.len(), "");
            result = result.replace("?&", "?").trim_end_matches('?').to_string();
        }
    }
    if let Some(idx) = result.find("?eh=") {
        let rest = &result[idx + 4..];
        let value: String = rest.chars().take_while(|c| *c != '&').collect();
        eh = Some(percent_decode(&value));
    }
    let path = if result.is_empty() {
        None
    } else {
        Some(result)
    };
    (path, max_early_data, eh)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &value[i + 1..i + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn build_selector_outbounds(
    proxy_tags: &[String],
    base_tag: &str,
    multiple_load: MultipleLoad,
) -> (Value, Value) {
    let mut urltest = obj();
    urltest.insert("type".into(), json!("urltest"));
    urltest.insert("tag".into(), json!(format!("{base_tag}-auto")));
    urltest.insert("outbounds".into(), json!(proxy_tags));
    urltest.insert("interrupt_exist_connections".into(), json!(false));
    if multiple_load == MultipleLoad::Fallback {
        urltest.insert("tolerance".into(), json!(5000));
    }
    let mut selector = obj();
    selector.insert("type".into(), json!("selector"));
    selector.insert("tag".into(), json!(base_tag));
    let mut selector_outbounds = vec![json!(format!("{base_tag}-auto"))];
    selector_outbounds.extend(proxy_tags.iter().map(|t| json!(t)));
    selector.insert("outbounds".into(), Value::Array(selector_outbounds));
    selector.insert("interrupt_exist_connections".into(), json!(false));
    (Value::Object(selector), Value::Object(urltest))
}
