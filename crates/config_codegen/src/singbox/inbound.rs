//! sing-box inbound generation (`SingboxInboundService`).

use serde_json::{json, Value};

use crate::input::{CodegenInput, Platform};
use crate::util::*;

const SOCKS: i32 = 0;
const SOCKS2: i32 = 1;
const SOCKS3: i32 = 2;

fn build_inbound(input: &CodegenInput, tag: &str, offset: i32) -> Value {
    let inbound = &input.settings.inbound;
    let kernel_protocol = if inbound.protocol.trim().is_empty() {
        "mixed"
    } else {
        inbound.protocol.trim()
    };
    json!({
        "type": kernel_protocol,
        "tag": tag,
        "listen": LOOPBACK,
        "listen_port": inbound.local_port + offset
    })
}

fn build_tun_inbound(input: &CodegenInput) -> Value {
    let tun = &input.settings.tun;
    let name = tun.name.clone().unwrap_or_else(|| {
        if input.settings.platform == Platform::MacOS {
            "utun0".into()
        } else {
            "singbox_tun".into()
        }
    });
    let mtu = if tun.mtu <= 0 {
        TUN_MTU_DEFAULT
    } else {
        tun.mtu
    };
    let stack = tun
        .stack
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "gvisor".into());
    let address = tun
        .ipv4_address
        .clone()
        .unwrap_or_else(|| TUN_IPV4_DEFAULT.into());
    let mut address_list = vec![address];
    if tun.enable_ipv6_address {
        address_list.push(
            tun.ipv6_address
                .clone()
                .unwrap_or_else(|| TUN_IPV6_DEFAULT.into()),
        );
    }
    let mut inbound = obj();
    inbound.insert("type".into(), json!("tun"));
    inbound.insert("tag".into(), json!("tun"));
    inbound.insert("interface_name".into(), json!(name));
    inbound.insert("address".into(), json!(address_list));
    inbound.insert("mtu".into(), json!(mtu));
    inbound.insert("auto_route".into(), json!(tun.auto_route));
    inbound.insert("strict_route".into(), json!(tun.strict_route));
    inbound.insert("stack".into(), json!(stack));
    if !tun.route_exclude_address.is_empty() {
        inbound.insert(
            "route_exclude_address".into(),
            json!(tun.route_exclude_address),
        );
    }
    Value::Object(inbound)
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
                        map.insert(
                            "users".into(),
                            json!([{"username": inbound.user, "password": inbound.pass}]),
                        );
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
