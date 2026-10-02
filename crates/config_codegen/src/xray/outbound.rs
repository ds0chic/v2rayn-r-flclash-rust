//! Xray outbound, transport and TLS generation (`V2rayOutboundService`).

use std::cell::RefCell;

use serde_json::{json, Map, Value};

use crate::input::{CodegenInput, CodegenProfile, ConfigType, MultipleLoad, ProtocolExtra};
use crate::util::*;
use crate::xray::XrayState;
use crate::{CodegenError, Diagnostic};

/// Generated proxy outbounds plus the custom-outbound tag map.
pub(crate) struct BuiltOutbounds {
    pub outbounds: Vec<Value>,
    pub custom_tags: Vec<(String, String)>,
}

// Diagnostics emitted by the pure transport builders are collected through a
// thread-local sink (mirrors the sing-box generator) and drained by
// `xray::config::build` into `XrayState::diagnostics`.
thread_local! {
    static DIAGNOSTIC_SINK: RefCell<Vec<Diagnostic>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn diagnostic_sink_reset() {
    DIAGNOSTIC_SINK.with(|sink| sink.borrow_mut().clear());
}

pub(crate) fn diagnostic_sink_take() -> Vec<Diagnostic> {
    DIAGNOSTIC_SINK.with(|sink| std::mem::take(&mut *sink.borrow_mut()))
}

fn push_diagnostic(diagnostic: Diagnostic) {
    DIAGNOSTIC_SINK.with(|sink| sink.borrow_mut().push(diagnostic));
}

pub(crate) fn child_items(node: &CodegenProfile) -> Option<Vec<String>> {
    string2_list_opt(node.proto_extra.child_items.as_ref())
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

pub(crate) fn build_all_proxy_outbounds(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<BuiltOutbounds, CodegenError> {
    if node.config_type.is_group() {
        build_group_proxy_outbounds(input, node, base_tag)
    } else {
        let (outbound, custom) = build_proxy_outbound(input, node, base_tag)?;
        Ok(BuiltOutbounds {
            outbounds: vec![outbound],
            custom_tags: custom
                .map(|id| (base_tag.to_string(), id))
                .into_iter()
                .collect(),
        })
    }
}

fn build_group_proxy_outbounds(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<BuiltOutbounds, CodegenError> {
    match node.config_type {
        ConfigType::PolicyGroup => build_outbounds_list(input, node, base_tag),
        ConfigType::ProxyChain => build_chain_outbounds_list(input, node, base_tag),
        _ => {
            let (outbound, custom) = build_proxy_outbound(input, node, base_tag)?;
            Ok(BuiltOutbounds {
                outbounds: vec![outbound],
                custom_tags: custom
                    .map(|id| (base_tag.to_string(), id))
                    .into_iter()
                    .collect(),
            })
        }
    }
}

fn build_outbounds_list(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<BuiltOutbounds, CodegenError> {
    let children = resolve_children(input, node)?;
    let mut result: Vec<Value> = Vec::new();
    let mut custom_tags: Vec<(String, String)> = Vec::new();
    for (i, child) in children.iter().enumerate() {
        let current_tag = if children.len() == 1 {
            base_tag.to_string()
        } else {
            format!("{base_tag}-{}-{}", i + 1, child.remarks)
        };
        if child.config_type.is_group() {
            let built = build_group_proxy_outbounds(input, child, &current_tag)?;
            result.extend(built.outbounds);
            custom_tags.extend(built.custom_tags);
            continue;
        }
        let (mut outbound, custom) = build_proxy_outbound(input, child, &current_tag)?;
        set_tag(&mut outbound, &current_tag);
        if let Some(index_id) = custom {
            custom_tags.push((current_tag.clone(), index_id));
        }
        result.push(outbound);
    }
    Ok(BuiltOutbounds {
        outbounds: result,
        custom_tags,
    })
}

fn build_chain_outbounds_list(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<BuiltOutbounds, CodegenError> {
    let children = resolve_children(input, node)?;
    let nodes_reverse: Vec<&&CodegenProfile> = children.iter().rev().collect();
    let count = nodes_reverse.len();
    let mut result: Vec<Value> = Vec::new();
    let mut custom_tags: Vec<(String, String)> = Vec::new();

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
            if let Some(dialer) = &dialer_proxy_tag {
                for outbound in built.outbounds.iter_mut() {
                    if outbound_dialer_proxy(outbound).is_none() {
                        fill_dialer_proxy(outbound, dialer);
                    }
                }
            }
            if i != 0 {
                let chain_start_tags: Vec<String> = built
                    .outbounds
                    .iter()
                    .filter_map(|o| o.get("tag").and_then(Value::as_str))
                    .filter(|tag| tag.starts_with(&current_tag))
                    .map(ToString::to_string)
                    .collect();
                match chain_start_tags.len() {
                    0 => {}
                    1 => {
                        let first = chain_start_tags[0].clone();
                        for existed in result.iter_mut() {
                            if outbound_dialer_proxy(existed).as_deref()
                                == Some(current_tag.as_str())
                            {
                                fill_dialer_proxy(existed, &first);
                            }
                        }
                    }
                    _ => {
                        let existed_clones = result.clone();
                        result.clear();
                        for (j, chain_start_tag) in chain_start_tags.iter().enumerate() {
                            let mut clone_list = existed_clones.clone();
                            for existed in clone_list.iter_mut() {
                                if let Some(tag) = existed.get("tag").and_then(Value::as_str) {
                                    let clone_tag = format!("{tag}-clone-{}", j + 1);
                                    set_tag(existed, &clone_tag);
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
                                let previous = outbound_dialer_proxy(existed);
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
                                fill_dialer_proxy(existed, &target);
                                result.push(existed.clone());
                            }
                        }
                    }
                }
            }
            custom_tags.extend(built.custom_tags);
            result.extend(built.outbounds);
            continue;
        }

        let (mut outbound, custom) = build_proxy_outbound(input, child, &current_tag)?;
        set_tag(&mut outbound, &current_tag);
        if let Some(dialer) = &dialer_proxy_tag {
            fill_dialer_proxy(&mut outbound, dialer);
        }
        if let Some(index_id) = custom {
            custom_tags.push((current_tag.clone(), index_id));
        }
        result.push(outbound);
    }
    Ok(BuiltOutbounds {
        outbounds: result,
        custom_tags,
    })
}

fn set_tag(outbound: &mut Value, tag: &str) {
    if let Some(map) = outbound.as_object_mut() {
        map.insert("tag".into(), Value::String(tag.to_string()));
    }
}

pub(crate) fn outbound_dialer_proxy(outbound: &Value) -> Option<String> {
    outbound
        .get("streamSettings")?
        .get("sockopt")?
        .get("dialerProxy")?
        .as_str()
        .map(ToString::to_string)
        .filter(|s| !s.is_empty())
}

pub(crate) fn fill_dialer_proxy(outbound: &mut Value, dialer_proxy_tag: &str) {
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
    if let Some(sockopt_map) = sockopt.as_object_mut() {
        sockopt_map.insert(
            "dialerProxy".into(),
            Value::String(dialer_proxy_tag.to_string()),
        );
    }
    // xhttp downloadSettings.sockopt.dialerProxy
    if let Some(extra) = stream_map
        .get_mut("xhttpSettings")
        .and_then(|s| s.get_mut("extra"))
        .and_then(Value::as_object_mut)
    {
        if let Some(download) = extra
            .get_mut("downloadSettings")
            .and_then(Value::as_object_mut)
        {
            let sockopt = download
                .entry("sockopt")
                .or_insert_with(|| Value::Object(obj()));
            if let Some(sockopt_map) = sockopt.as_object_mut() {
                sockopt_map.insert(
                    "dialerProxy".into(),
                    Value::String(dialer_proxy_tag.to_string()),
                );
            }
        }
    }
}

/// Build one proxy outbound. Returns `(value, custom_index_id)`.
fn build_proxy_outbound(
    input: &CodegenInput,
    node: &CodegenProfile,
    base_tag: &str,
) -> Result<(Value, Option<String>), CodegenError> {
    if node.config_type == ConfigType::Outbound {
        let mut placeholder = obj();
        put_str(&mut placeholder, "tag", base_tag);
        put_str(&mut placeholder, "protocol", "vmess");
        put(
            &mut placeholder,
            "settings",
            json!({"address": "invalid.invalid", "port": 0}),
        );
        put(
            &mut placeholder,
            "streamSettings",
            json!({"network": "tcp"}),
        );
        put(&mut placeholder, "mux", json!({"enabled": false}));
        return Ok((Value::Object(placeholder), Some(node.index_id.clone())));
    }

    let mut settings = obj();
    let mut mux: Option<Value> = None;
    let protocol: String;

    match node.config_type {
        ConfigType::Vmess => {
            settings.insert("address".into(), json!(node.address));
            settings.insert("port".into(), json!(node.port));
            settings.insert("id".into(), json!(node.password));
            settings.insert(
                "alterId".into(),
                json!(parse_i32(node.proto_extra.alter_id.as_deref()).unwrap_or(0)),
            );
            settings.insert("email".into(), json!(USER_EMAIL));
            settings.insert(
                "security".into(),
                json!(valid_vmess_security(
                    node.proto_extra.vmess_security.as_deref()
                )),
            );
            mux = Some(fill_outbound_mux(
                node.mux_enabled.unwrap_or(false),
                node.mux_enabled.unwrap_or(false),
                &input.settings.mux4_ray,
            ));
            protocol = "vmess".into();
        }
        ConfigType::Shadowsocks => {
            settings.insert("address".into(), json!(node.address));
            settings.insert("port".into(), json!(node.port));
            settings.insert("password".into(), json!(node.password));
            let method = node.proto_extra.ss_method.as_deref().unwrap_or("");
            settings.insert(
                "method".into(),
                json!(if SS_SECURITIES_IN_XRAY.contains(&method) {
                    method
                } else {
                    NONE
                }),
            );
            if node.proto_extra.uot == Some(true) {
                settings.insert("uot".into(), json!(true));
            }
            settings.insert("ota".into(), json!(false));
            settings.insert("level".into(), json!(1));
            mux = Some(fill_outbound_mux(false, false, &input.settings.mux4_ray));
            protocol = "shadowsocks".into();
        }
        ConfigType::Socks => {
            settings.insert("address".into(), json!(node.address));
            settings.insert("port".into(), json!(node.port));
            if !node.username.is_empty() && !node.password.is_empty() {
                settings.insert("user".into(), json!(node.username));
                settings.insert("pass".into(), json!(node.password));
                settings.insert("level".into(), json!(1));
                settings.insert("email".into(), json!(USER_EMAIL));
            }
            mux = Some(fill_outbound_mux(false, false, &input.settings.mux4_ray));
            protocol = "socks".into();
        }
        ConfigType::Http => {
            settings.insert("address".into(), json!(node.address));
            settings.insert("port".into(), json!(node.port));
            if let Some(headers) = node
                .proto_extra
                .http_headers
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                let parsed = parse_json_text(headers);
                put_opt_value(&mut settings, "headers", parsed);
            }
            if !node.username.is_empty() && !node.password.is_empty() {
                settings.insert("user".into(), json!(node.username));
                settings.insert("pass".into(), json!(node.password));
                settings.insert("level".into(), json!(1));
                settings.insert("email".into(), json!(USER_EMAIL));
            }
            mux = Some(fill_outbound_mux(false, false, &input.settings.mux4_ray));
            protocol = "http".into();
        }
        ConfigType::Vless => {
            settings.insert("address".into(), json!(node.address));
            settings.insert("port".into(), json!(node.port));
            settings.insert("id".into(), json!(node.password));
            settings.insert("email".into(), json!(USER_EMAIL));
            settings.insert(
                "encryption".into(),
                json!(node
                    .proto_extra
                    .vless_encryption
                    .clone()
                    .unwrap_or_default()),
            );
            let flow = node.proto_extra.flow.as_deref().unwrap_or("");
            if flow.is_empty() {
                mux = Some(fill_outbound_mux(
                    node.mux_enabled.unwrap_or(false),
                    node.mux_enabled.unwrap_or(false),
                    &input.settings.mux4_ray,
                ));
            } else {
                settings.insert("flow".into(), json!(flow));
                mux = Some(fill_outbound_mux(
                    false,
                    node.mux_enabled.unwrap_or(false),
                    &input.settings.mux4_ray,
                ));
            }
            protocol = "vless".into();
        }
        ConfigType::Trojan => {
            settings.insert("address".into(), json!(node.address));
            settings.insert("port".into(), json!(node.port));
            settings.insert("password".into(), json!(node.password));
            settings.insert("ota".into(), json!(false));
            settings.insert("level".into(), json!(1));
            mux = Some(fill_outbound_mux(false, false, &input.settings.mux4_ray));
            protocol = "trojan".into();
        }
        ConfigType::Hysteria2 => {
            settings.insert("address".into(), json!(node.address));
            settings.insert("port".into(), json!(node.port));
            settings.insert("version".into(), json!(2));
            protocol = "hysteria".into();
        }
        ConfigType::WireGuard => {
            let address = if is_valid_ipv6(&node.address) {
                format!("[{}]", node.address)
            } else {
                node.address.clone()
            };
            let mut peer = obj();
            put_str(
                &mut peer,
                "publicKey",
                node.proto_extra.wg_public_key.as_deref().unwrap_or(""),
            );
            peer.insert("endpoint".into(), json!(format!("{address}:{}", node.port)));
            put_opt_string(
                &mut peer,
                "preSharedKey",
                node.proto_extra.wg_preshared_key.as_ref(),
            );
            let mut wg_settings = obj();
            let interface = string2_list_opt(node.proto_extra.wg_interface_address.as_ref())
                .unwrap_or_else(|| vec!["172.16.0.2/32".into()]);
            wg_settings.insert("address".into(), json!(interface));
            wg_settings.insert("secretKey".into(), json!(node.password));
            let reserved: Option<Vec<i64>> =
                string2_list_opt(node.proto_extra.wg_reserved.as_ref())
                    .map(|list| {
                        list.iter()
                            .filter_map(|s| parse_i32(Some(s)).map(i64::from))
                            .collect()
                    })
                    .filter(|v: &Vec<i64>| !v.is_empty());
            put_opt_value(&mut wg_settings, "reserved", reserved.map(|v| json!(v)));
            let mtu = node
                .proto_extra
                .wg_mtu
                .filter(|m| *m > 0)
                .unwrap_or(TUN_MTU_DEFAULT);
            wg_settings.insert("mtu".into(), json!(mtu));
            if let Some(dns) = string2_list_opt(node.proto_extra.wg_dns.as_ref()) {
                wg_settings.insert("remoteDNS".into(), json!(dns));
            }
            wg_settings.insert("peers".into(), json!([peer]));
            settings = wg_settings;
            protocol = "wireguard".into();
        }
        _ => {
            return Err(CodegenError::unsupported_combination(
                format!(
                    "config type {:?} is not generated by Xray",
                    node.config_type
                ),
                "profile.configType",
            ))
        }
    }

    let mut outbound = obj();
    put_str(&mut outbound, "tag", base_tag);
    put_str(&mut outbound, "protocol", &protocol);
    put(&mut outbound, "settings", Value::Object(settings));
    let stream = fill_bound_stream_settings(node, &input.settings)?;
    put(&mut outbound, "streamSettings", Value::Object(stream));
    // xhttp rewrites mux to disabled (`FillBoundStreamSettings` xhttp case).
    if get_network(node) == "xhttp" && node.config_type != ConfigType::Hysteria2 {
        mux = Some(fill_outbound_mux(false, false, &input.settings.mux4_ray));
    }
    if let Some(mux) = mux {
        outbound.insert("mux".into(), mux);
    }
    Ok((Value::Object(outbound), None))
}

pub(crate) fn fill_outbound_mux(
    enabled_tcp: bool,
    enabled_udp: bool,
    mux4_ray: &crate::input::Mux4Ray,
) -> Value {
    let mut mux = obj();
    mux.insert("enabled".into(), json!(false));
    mux.insert("concurrency".into(), json!(-1));
    if enabled_tcp {
        mux.insert("enabled".into(), json!(true));
        mux.insert("concurrency".into(), json!(mux4_ray.concurrency));
    } else if enabled_udp {
        mux.insert("enabled".into(), json!(true));
        mux.insert("xudpConcurrency".into(), json!(mux4_ray.xudp_concurrency));
        mux.insert("xudpProxyUDP443".into(), json!(mux4_ray.xudp_proxy_udp443));
    }
    Value::Object(mux)
}

fn parse_i32(value: Option<&str>) -> Option<i32> {
    value.and_then(|v| v.trim().parse::<i32>().ok())
}

fn valid_vmess_security(value: Option<&str>) -> &str {
    match value {
        Some(v) if VMESS_SECURITIES.contains(&v) => v,
        _ => DEFAULT_SECURITY,
    }
}

fn parse_json_text(text: &str) -> Option<Value> {
    serde_json::from_str::<Value>(text).ok()
}

/// `FillBoundStreamSettings` transport + security body.
pub(crate) fn fill_bound_stream_settings(
    node: &CodegenProfile,
    settings: &crate::input::CodegenSettings,
) -> Result<Map<String, Value>, CodegenError> {
    let network = if node.config_type == ConfigType::Hysteria2 {
        "hysteria".to_string()
    } else {
        get_network(node)
    };
    let mut stream = obj();
    stream.insert("network".into(), json!(network));

    let transport = &node.transport_extra;
    let (mut host, mut path, mut header_type, mut xhttp_extra, mut kcp_seed) = (
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );
    let mut kcp_mtu = 0;
    match network.as_str() {
        "raw" => {
            host = trim_opt(transport.host.as_ref());
            path = trim_opt(transport.path.as_ref());
            header_type = trim_opt(transport.raw_header_type.as_ref());
        }
        "kcp" => {
            kcp_seed = trim_opt(transport.kcp_seed.as_ref());
            header_type = trim_opt(transport.kcp_header_type.as_ref());
            kcp_mtu = transport
                .kcp_mtu
                .filter(|v| *v > 0)
                .unwrap_or(settings.kcp.mtu);
        }
        "ws" => {
            host = trim_opt(transport.host.as_ref());
            path = trim_opt(transport.path.as_ref());
        }
        "httpupgrade" => {
            host = trim_opt(transport.host.as_ref());
            path = trim_opt(transport.path.as_ref());
        }
        "xhttp" => {
            host = trim_opt(transport.host.as_ref());
            path = trim_opt(transport.path.as_ref());
            header_type = trim_opt(transport.xhttp_mode.as_ref());
            xhttp_extra = trim_opt(transport.xhttp_extra.as_ref());
        }
        "grpc" => {
            host = trim_opt(transport.grpc_authority.as_ref());
            path = trim_opt(transport.grpc_service_name.as_ref());
            header_type = trim_opt(transport.grpc_mode.as_ref());
        }
        _ => {}
    }

    let sni = node.sni.trim();
    let useragent = settings
        .core_basic
        .def_user_agent
        .clone()
        .unwrap_or_default();

    if node.stream_security == STREAM_SECURITY {
        stream.insert("security".into(), json!(STREAM_SECURITY));
        let mut tls = obj();
        if let Some(alpn) = alpn_list(&node.alpn) {
            tls.insert("alpn".into(), json!(alpn));
        }
        let fingerprint = if node.fingerprint.trim().is_empty() {
            settings.core_basic.def_fingerprint.clone()
        } else {
            Some(node.fingerprint.clone())
        };
        put_opt_string(&mut tls, "fingerprint", fingerprint.as_ref());
        put_opt_str(
            &mut tls,
            "echConfigList",
            option_non_empty(&node.ech_config_list),
        );
        put_opt_str(
            &mut tls,
            "verifyPeerCertByName",
            option_non_empty(&node.verify_peer_cert_by_name),
        );
        if !sni.is_empty() {
            tls.insert("serverName".into(), json!(sni));
        } else if !host.is_empty() {
            if let Some(first) = first_host(&host) {
                tls.insert("serverName".into(), json!(first));
            }
        }
        if !node.ech_config_list.is_empty() {
            tls.insert("echForceQuery".into(), json!("full"));
        }
        let certs = parse_pem_chain(&node.cert);
        if !certs.is_empty() {
            let list: Vec<Value> = certs
                .iter()
                .map(|cert| {
                    json!({
                        "certificate": cert.lines().map(ToString::to_string).collect::<Vec<_>>(),
                        "usage": "verify"
                    })
                })
                .collect();
            tls.insert("certificates".into(), Value::Array(list));
            tls.insert("disableSystemRoot".into(), json!(true));
        } else if !node.cert_sha.is_empty() {
            tls.insert("pinnedPeerCertSha256".into(), json!(node.cert_sha));
        }
        stream.insert("tlsSettings".into(), Value::Object(tls));
    }

    if node.stream_security == STREAM_SECURITY_REALITY {
        stream.insert("security".into(), json!(STREAM_SECURITY_REALITY));
        let mut reality = obj();
        let fingerprint = if node.fingerprint.trim().is_empty() {
            settings.core_basic.def_fingerprint.clone()
        } else {
            Some(node.fingerprint.clone())
        };
        put_opt_string(&mut reality, "fingerprint", fingerprint.as_ref());
        put_opt_str(&mut reality, "serverName", option_non_empty(sni));
        put_opt_str(
            &mut reality,
            "publicKey",
            option_non_empty(&node.public_key),
        );
        put_opt_str(&mut reality, "shortId", option_non_empty(&node.short_id));
        put_opt_str(&mut reality, "spiderX", option_non_empty(&node.spider_x));
        put_opt_str(
            &mut reality,
            "mldsa65Verify",
            option_non_empty(&node.mldsa65_verify),
        );
        reality.insert("show".into(), json!(false));
        stream.insert("realitySettings".into(), Value::Object(reality));
    }

    match network.as_str() {
        "kcp" => {
            let mut kcp = obj();
            kcp.insert("mtu".into(), json!(kcp_mtu));
            kcp.insert("tti".into(), json!(settings.kcp.tti));
            kcp.insert("uplinkCapacity".into(), json!(settings.kcp.uplink_capacity));
            kcp.insert(
                "downlinkCapacity".into(),
                json!(settings.kcp.downlink_capacity),
            );
            kcp.insert("cwndMultiplier".into(), json!(settings.kcp.cwnd_multiplier));
            kcp.insert(
                "maxSendingWindow".into(),
                json!(settings.kcp.max_sending_window),
            );
            stream.insert("kcpSettings".into(), Value::Object(kcp));

            let mut udp: Vec<Value> = Vec::new();
            if let Some(finalmask_type) = KCP_HEADER_FINALMASK_MAP
                .iter()
                .find(|(k, _)| *k == header_type)
                .map(|(_, v)| *v)
            {
                udp.push(json!({"type": finalmask_type}));
            }
            if kcp_seed.is_empty() {
                udp.push(json!({"type": "mkcp-original"}));
            } else {
                udp.push(json!({"type": "mkcp-aes128gcm", "settings": {"password": kcp_seed}}));
            }
            udp.reverse();
            stream.insert("finalmask".into(), json!({"udp": udp}));
            push_diagnostic(Diagnostic::warning(
                "xray_kcp_finalmask_translated",
                "locked Xray 26.3.27 does not register the frozen upstream KCP finalmask id \
                 `mkcp-legacy`; emitted the semantically equivalent legacy masks \
                 (`mkcp-original` / `mkcp-aes128gcm` / `header-*`) per Xray MkcpLegacy.Build \
                 (commit aba22722)",
                Some("streamSettings.finalmask.udp"),
            ));
        }
        "ws" => {
            let mut ws = obj();
            put_opt_str(&mut ws, "host", option_non_empty(&host));
            put_opt_str(&mut ws, "path", option_non_empty(&path));
            if !useragent.is_empty() {
                ws.insert("headers".into(), json!({"User-Agent": useragent}));
            }
            stream.insert("wsSettings".into(), Value::Object(ws));
        }
        "httpupgrade" => {
            let mut hu = obj();
            put_opt_str(&mut hu, "host", option_non_empty(&host));
            put_opt_str(&mut hu, "path", option_non_empty(&path));
            if !useragent.is_empty() {
                hu.insert("headers".into(), json!({"User-Agent": useragent}));
            }
            stream.insert("httpupgradeSettings".into(), Value::Object(hu));
        }
        "xhttp" => {
            let mut xhttp = obj();
            put_opt_str(&mut xhttp, "path", option_non_empty(&path));
            put_opt_str(&mut xhttp, "host", option_non_empty(&host));
            if !header_type.is_empty() && XHTTP_MODES.contains(&header_type.as_str()) {
                xhttp.insert("mode".into(), json!(header_type));
            }
            if !xhttp_extra.is_empty() {
                if let Some(extra) = parse_json_text(&xhttp_extra) {
                    xhttp.insert("extra".into(), extra);
                }
            }
            stream.insert("xhttpSettings".into(), Value::Object(xhttp));
        }
        "grpc" => {
            let mut grpc = obj();
            put_opt_str(&mut grpc, "authority", option_non_empty(&host));
            grpc.insert("serviceName".into(), json!(path));
            grpc.insert("multiMode".into(), json!(header_type == GRPC_MULTI_MODE));
            put_opt_i32(&mut grpc, "idle_timeout", settings.grpc.idle_timeout);
            put_opt_i32(
                &mut grpc,
                "health_check_timeout",
                settings.grpc.health_check_timeout,
            );
            put_opt_bool(
                &mut grpc,
                "permit_without_stream",
                settings.grpc.permit_without_stream,
            );
            put_opt_i32(
                &mut grpc,
                "initial_windows_size",
                settings.grpc.initial_windows_size,
            );
            put_opt_str(&mut grpc, "user_agent", option_non_empty(&useragent));
            stream.insert("grpcSettings".into(), Value::Object(grpc));
        }
        "hysteria" => {
            fill_hysteria(node, settings, &mut stream);
        }
        _ => {
            if header_type == RAW_HEADER_HTTP {
                let hosts: Vec<String> = host.split(',').map(ToString::to_string).collect();
                let host_replacement = format!("\"{}\"", hosts.join("\",\""));
                let ua_value = raw_user_agent_value(&useragent);
                let path_value = if path.is_empty() {
                    "/".to_string()
                } else {
                    path.clone()
                };
                let paths: Vec<String> = path_value.split(',').map(ToString::to_string).collect();
                let path_replacement = format!("\"{}\"", paths.join("\",\""));
                let request = SAMPLE_HTTP_REQUEST
                    .replace("$requestHost$", &host_replacement)
                    .replace("$requestUserAgent$", &format!("\"{ua_value}\""))
                    .replace("$requestPath$", &path_replacement);
                if let Ok(request_value) = serde_json::from_str::<Value>(&request) {
                    stream.insert(
                        "rawSettings".into(),
                        json!({"header": {"type": "http", "request": request_value}}),
                    );
                }
            }
        }
    }

    if let Some(finalmask) = &node.finalmask {
        stream.insert("finalmask".into(), finalmask.clone());
    }
    Ok(stream)
}

fn fill_hysteria(
    node: &CodegenProfile,
    settings: &crate::input::CodegenSettings,
    stream: &mut Map<String, Value>,
) {
    let extra: &ProtocolExtra = &node.proto_extra;
    let ports = extra.ports.as_deref().unwrap_or("");
    let up_mbps = match extra.up_mbps {
        Some(v) if v >= 0 => Some(v),
        _ => settings.hysteria.up_mbps,
    };
    let down_mbps = match extra.down_mbps {
        Some(v) if v >= 0 => Some(v),
        _ => settings.hysteria.down_mbps,
    };
    let hop_interval = match extra.hop_interval.as_deref() {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => {
            if settings.hysteria.hop_interval >= 5 {
                settings.hysteria.hop_interval.to_string()
            } else {
                HYSTERIA2_DEFAULT_HOP_INT.to_string()
            }
        }
    };

    let mut quic_params = obj();
    if !ports.is_empty() && (ports.contains(':') || ports.contains('-') || ports.contains(',')) {
        let udp_hop = json!({"ports": ports.replace(':', "-"), "interval": hop_interval});
        quic_params.insert("udpHop".into(), udp_hop);
    }
    if up_mbps.unwrap_or(0) > 0 || down_mbps.unwrap_or(0) > 0 {
        quic_params.insert("congestion".into(), json!("brutal"));
        if let Some(up) = up_mbps.filter(|v| *v > 0) {
            quic_params.insert("brutalUp".into(), json!(format!("{up}mbps")));
        }
        if let Some(down) = down_mbps.filter(|v| *v > 0) {
            quic_params.insert("brutalDown".into(), json!(format!("{down}mbps")));
        }
    } else {
        quic_params.insert("congestion".into(), json!("bbr"));
    }

    let mut udp: Vec<Value> = Vec::new();
    if let Some(realm_url) = extra.hy2_realm_url.as_deref().filter(|s| !s.is_empty()) {
        udp.push(json!({"type": "realm", "settings": {"url": realm_url}}));
    }
    if let Some(pass) = extra.salamander_pass.as_deref().filter(|s| !s.is_empty()) {
        let is_gecko = extra
            .gecko_min_packet_size
            .as_deref()
            .is_some_and(|s| !s.is_empty())
            || extra
                .gecko_max_packet_size
                .as_deref()
                .is_some_and(|s| !s.is_empty());
        let mut mask_settings = obj();
        mask_settings.insert("password".into(), json!(pass.trim()));
        if is_gecko {
            mask_settings.insert(
                "packetSize".into(),
                json!(format!(
                    "{}-{}",
                    extra.gecko_min_packet_size.as_deref().unwrap_or(""),
                    extra.gecko_max_packet_size.as_deref().unwrap_or("")
                )),
            );
        }
        udp.push(json!({"type": "salamander", "settings": mask_settings}));
    }
    udp.reverse();
    let mut finalmask = obj();
    if !udp.is_empty() {
        finalmask.insert("udp".into(), Value::Array(udp));
    }
    finalmask.insert("quicParams".into(), Value::Object(quic_params));
    stream.insert(
        "hysteriaSettings".into(),
        json!({"version": 2, "auth": node.password}),
    );
    stream.insert("finalmask".into(), Value::Object(finalmask));
}

pub(crate) fn trim_opt(value: Option<&String>) -> String {
    value.map(|v| v.trim().to_string()).unwrap_or_default()
}

pub(crate) fn option_non_empty(value: &str) -> Option<&str> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Upstream `ProfileItem.GetNetwork()`.
pub(crate) fn get_network(node: &CodegenProfile) -> String {
    if node.network.is_empty() || !NETWORKS.contains(&node.network.as_str()) {
        DEFAULT_NETWORK.to_string()
    } else {
        node.network.trim().to_string()
    }
}

/// `GenObservatory` for Xray.
pub(crate) fn gen_observatory(
    multiple_load: MultipleLoad,
    base_tag: &str,
    state: &mut XrayState<'_>,
) {
    let mut subject_selectors: Vec<String> = Vec::new();
    if let Some(burst) = state.config.get("burstObservatory") {
        if let Some(list) = burst.get("subjectSelector").and_then(Value::as_array) {
            subject_selectors.extend(
                list.iter()
                    .filter_map(|v| v.as_str().map(ToString::to_string)),
            );
        }
    }
    if let Some(obs) = state.config.get("observatory") {
        if let Some(list) = obs.get("subjectSelector").and_then(Value::as_array) {
            subject_selectors.extend(
                list.iter()
                    .filter_map(|v| v.as_str().map(ToString::to_string)),
            );
        }
    }
    if subject_selectors
        .iter()
        .any(|s| base_tag.starts_with(s.as_str()))
    {
        return;
    }
    if let Some(matched) = subject_selectors.iter().find(|s| s.starts_with(base_tag)) {
        let matched = matched.clone();
        reorder_subject(state, "burstObservatory", &matched);
        reorder_subject(state, "observatory", &matched);
        return;
    }
    match multiple_load {
        MultipleLoad::LeastLoad | MultipleLoad::Fallback => {
            let entry = if state.config.get("burstObservatory").is_none() {
                json!({
                    "subjectSelector": [base_tag],
                    "pingConfig": {
                        "destination": state.input.settings.speed_ping_test_url.clone().unwrap_or_default(),
                        "interval": "5m",
                        "timeout": "30s",
                        "sampling": 2
                    }
                })
            } else {
                json!([base_tag])
            };
            if state.config.get("burstObservatory").is_none() {
                state.config.insert("burstObservatory".into(), entry);
            } else if let Some(list) = state
                .config
                .get_mut("burstObservatory")
                .and_then(|v| v.get_mut("subjectSelector"))
                .and_then(Value::as_array_mut)
            {
                list.push(json!(base_tag));
            }
        }
        _ => {
            if state.config.get("observatory").is_none() {
                state.config.insert(
                    "observatory".into(),
                    json!({
                        "subjectSelector": [base_tag],
                        "probeUrl": state.input.settings.speed_ping_test_url.clone().unwrap_or_default(),
                        "probeInterval": "3m",
                        "enableConcurrency": true
                    }),
                );
            } else if let Some(list) = state
                .config
                .get_mut("observatory")
                .and_then(|v| v.get_mut("subjectSelector"))
                .and_then(Value::as_array_mut)
            {
                list.push(json!(base_tag));
            }
        }
    }
}

fn reorder_subject(state: &mut XrayState<'_>, key: &str, tag: &str) {
    if let Some(list) = state
        .config
        .get_mut(key)
        .and_then(|v| v.get_mut("subjectSelector"))
        .and_then(Value::as_array_mut)
    {
        if let Some(pos) = list.iter().position(|v| v.as_str() == Some(tag)) {
            let value = list.remove(pos);
            list.insert(0, value);
        }
    }
}

/// `GenBalancer`.
pub(crate) fn gen_balancer(multiple_load: MultipleLoad, selector: &str, state: &mut XrayState<'_>) {
    let strategy_type = match multiple_load {
        MultipleLoad::Random => "random",
        MultipleLoad::RoundRobin => "roundRobin",
        MultipleLoad::LeastPing => "leastPing",
        MultipleLoad::LeastLoad | MultipleLoad::Fallback => "leastLoad",
    };
    let mut balancer = obj();
    balancer.insert("selector".into(), json!([selector]));
    if strategy_type == "leastLoad" {
        let mut settings = obj();
        settings.insert("expected".into(), json!(1));
        if multiple_load == MultipleLoad::Fallback {
            settings.insert("tolerance".into(), json!(0.2));
            settings.insert("maxRTT".into(), json!("5000ms"));
        }
        balancer.insert(
            "strategy".into(),
            json!({"type": strategy_type, "settings": settings}),
        );
    } else {
        balancer.insert("strategy".into(), json!({"type": strategy_type}));
    }
    balancer.insert(
        "tag".into(),
        json!(format!("{selector}{BALANCER_TAG_SUFFIX}")),
    );
    if state
        .config
        .get("routing")
        .and_then(|r| r.get("balancers"))
        .is_none()
    {
        if let Some(routing) = state
            .config
            .get_mut("routing")
            .and_then(Value::as_object_mut)
        {
            routing.insert("balancers".into(), json!([]));
        }
    }
    if let Some(list) = state
        .config
        .get_mut("routing")
        .and_then(|r| r.get_mut("balancers"))
        .and_then(Value::as_array_mut)
    {
        list.push(Value::Object(balancer));
    }
}
