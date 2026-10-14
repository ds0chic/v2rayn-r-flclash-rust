//! Xray inbound generation (`V2rayInboundService`).

use serde_json::{json, Value};

use crate::input::{CodegenInput, Platform};
use crate::util::*;

/// `EInboundProtocol` offsets.
const SOCKS: i32 = 0;
const SOCKS2: i32 = 1;
const SOCKS3: i32 = 2;

fn build_inbound(input: &CodegenInput, tag: &str, offset: i32) -> Value {
    let inbound = &input.settings.inbound;
    let core_basic = &input.settings.core_basic;
    let _ = core_basic;
    let kernel_protocol = if inbound.protocol.trim().is_empty() {
        "mixed"
    } else {
        inbound.protocol.trim()
    };
    let mut sniffing = obj();
    sniffing.insert("enabled".into(), json!(inbound.sniffing_enabled));
    if !inbound.dest_override.is_empty() {
        sniffing.insert("destOverride".into(), json!(inbound.dest_override));
    }
    let mut dest_override = inbound.dest_override.clone();
    if input
        .dns
        .as_ref()
        .map(|d| d.simple.fake_ip)
        .unwrap_or(false)
        && !dest_override.iter().any(|d| d == "fakedns")
    {
        dest_override.push("fakedns".into());
    }
    if !dest_override.is_empty() {
        sniffing.insert("destOverride".into(), json!(dest_override));
    }
    sniffing.insert("routeOnly".into(), json!(inbound.route_only));

    let mut settings = obj();
    settings.insert("auth".into(), json!("noauth"));
    settings.insert("udp".into(), json!(inbound.udp_enabled));
    settings.insert("allowTransparent".into(), json!(false));

    let mut value = obj();
    value.insert("tag".into(), json!(tag));
    value.insert("port".into(), json!(inbound.local_port + offset));
    value.insert("protocol".into(), json!(kernel_protocol));
    value.insert("listen".into(), json!(LOOPBACK));
    value.insert("settings".into(), Value::Object(settings));
    value.insert("sniffing".into(), Value::Object(sniffing));
    Value::Object(value)
}

fn build_tun_inbound(input: &CodegenInput) -> Value {
    let tun = &input.settings.tun;
    let mut settings = obj();
    let name = tun.name.clone().unwrap_or_else(|| {
        if input.settings.platform == Platform::MacOS {
            "utun0".into()
        } else {
            "xray_tun".into()
        }
    });
    settings.insert("name".into(), json!(name));
    let mtu = if tun.mtu <= 0 {
        TUN_MTU_DEFAULT
    } else {
        tun.mtu
    };
    settings.insert("MTU".into(), json!(mtu));
    let address = tun
        .ipv4_address
        .clone()
        .unwrap_or_else(|| TUN_IPV4_DEFAULT.into());
    let mut gateway = vec![address];
    if tun.enable_ipv6_address {
        gateway.push(
            tun.ipv6_address
                .clone()
                .unwrap_or_else(|| TUN_IPV6_DEFAULT.into()),
        );
    }
    settings.insert("gateway".into(), json!(gateway));
    settings.insert("dns".into(), json!(["1.1.1.1", "8.8.8.8"]));
    let route_table = if tun.route_exclude_address.is_empty() {
        if input.settings.has_global_ipv6_address {
            vec!["0.0.0.0/0".to_string(), "::/0".to_string()]
        } else {
            vec!["0.0.0.0/0".to_string()]
        }
    } else {
        tun_route_table(
            &tun.route_exclude_address,
            input.settings.has_global_ipv6_address,
        )
    };
    settings.insert("autoSystemRoutingTable".into(), json!(route_table));
    if let Some(bind) = input
        .settings
        .core_basic
        .bind_interface
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        settings.insert("autoOutboundsInterface".into(), json!(bind));
    }

    let mut sniffing = obj();
    sniffing.insert(
        "enabled".into(),
        json!(input.settings.inbound.sniffing_enabled),
    );
    let mut dest_override = input.settings.inbound.dest_override.clone();
    if input
        .dns
        .as_ref()
        .map(|dns| dns.simple.fake_ip)
        .unwrap_or(false)
        && !dest_override.iter().any(|value| value == "fakedns")
    {
        dest_override.push("fakedns".into());
    }
    if !dest_override.is_empty() {
        sniffing.insert("destOverride".into(), json!(dest_override));
    }
    sniffing.insert("routeOnly".into(), json!(true));

    let mut value = obj();
    value.insert("tag".into(), json!("tun"));
    value.insert("protocol".into(), json!("tun"));
    value.insert("settings".into(), Value::Object(settings));
    value.insert("sniffing".into(), Value::Object(sniffing));
    Value::Object(value)
}

/// `GenInbounds`; returns the rebuilt inbounds array.
pub(crate) fn build_inbounds(input: &CodegenInput) -> Vec<Value> {
    let inbound = &input.settings.inbound;
    let mut result: Vec<Value> = Vec::new();
    let is_using_local_mixed_port =
        input.profile.address == LOOPBACK && input.profile.port == inbound.local_port;
    let use_tun = input.settings.tun.enabled;

    if !use_tun || !is_using_local_mixed_port {
        let mut first = build_inbound(input, "socks", SOCKS);
        if inbound.second_local_port_enabled {
            result.push(build_inbound(input, "socks2", SOCKS2));
        }
        if inbound.allow_lan_conn {
            if inbound.new_port4_lan {
                let mut lan = build_inbound(input, "socks3", SOCKS3);
                if let Some(map) = lan.as_object_mut() {
                    map.insert("listen".into(), json!("0.0.0.0"));
                    if !inbound.user.is_empty() && !inbound.pass.is_empty() {
                        if let Some(settings) =
                            map.get_mut("settings").and_then(Value::as_object_mut)
                        {
                            settings.insert("auth".into(), json!("password"));
                            settings.insert(
                                "accounts".into(),
                                json!([{ "user": inbound.user, "pass": inbound.pass }]),
                            );
                        }
                    }
                }
                result.push(lan);
            } else if let Some(map) = first.as_object_mut() {
                map.insert("listen".into(), json!("0.0.0.0"));
            }
        }
        result.insert(0, first);
    }

    if use_tun {
        result.push(build_tun_inbound(input));
    }
    result
}
