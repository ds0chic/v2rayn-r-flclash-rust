//! `WireguardFmt` — `wireguard://` share URI and WireGuard `.conf` parser.

use domain::{ConfigType, Profile};

use super::base;
use crate::error::SubError;
use crate::util::{normalize_json_compact, url_decode, url_encode, Query};

pub const DEFAULT_ENDPOINT_PORT: i32 = 2408;

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let url = url::Url::parse(input.trim()).map_err(|e| SubError::InvalidUri(e.to_string()))?;
    let mut item = Profile {
        config_type: ConfigType::WireGuard,
        ..Profile::default()
    };
    item.address = url.host_str().unwrap_or_default().to_string();
    item.port = url.port().map(i32::from).unwrap_or(0);
    item.remarks = url_decode(url.fragment().unwrap_or(""));
    let mut raw_user = url.username().to_string();
    if let Some(password) = url.password() {
        raw_user.push(':');
        raw_user.push_str(password);
    }
    item.password = url_decode(&raw_user);

    let query = Query::parse(url.query().unwrap_or(""));
    base::set_optional(
        &mut item.finalmask,
        &query
            .get("fm")
            .map(|v| crate::util::normalize_json_pretty(v).unwrap_or_else(|| v.to_string()))
            .unwrap_or_default(),
    );
    item.proto_extra.wg_public_key = non_empty(query.get("publickey"));
    item.proto_extra.wg_preshared_key = non_empty(query.get("presharedkey"));
    item.proto_extra.wg_reserved = non_empty(query.get("reserved"));
    item.proto_extra.wg_interface_address = non_empty(query.get("address"));
    item.proto_extra.wg_mtu = query.get("mtu").and_then(|v| v.parse::<i32>().ok());
    item.proto_extra.wg_dns = non_empty(query.get("dns"));
    Ok(item)
}

pub fn emit(item: &Profile) -> Result<String, SubError> {
    let remark = if item.remarks.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(&item.remarks))
    };
    let proto = &item.proto_extra;
    let mut query = Vec::new();
    push_query(&mut query, "publickey", proto.wg_public_key.as_deref());
    push_query(
        &mut query,
        "presharedkey",
        proto.wg_preshared_key.as_deref(),
    );
    push_query(&mut query, "reserved", proto.wg_reserved.as_deref());
    push_query(&mut query, "address", proto.wg_interface_address.as_deref());
    if let Some(mtu) = proto.wg_mtu.filter(|m| *m > 0) {
        query.push(("mtu".into(), mtu.to_string()));
    }
    push_query(&mut query, "dns", proto.wg_dns.as_deref());
    if let Some(fm) = item.finalmask.as_deref().filter(|v| !v.is_empty()) {
        let encoded = normalize_json_compact(fm).unwrap_or_else(|| fm.to_string());
        query.push(("fm".into(), url_encode(&encoded)));
    }
    Ok(base::build_uri(
        base::WIREGUARD,
        &item.address,
        item.port,
        &item.password,
        &query,
        &remark,
    ))
}

fn push_query(query: &mut Vec<(String, String)>, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|v| !v.is_empty()) {
        query.push((key.to_string(), url_encode(value)));
    }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value.filter(|v| !v.is_empty()).map(str::to_string)
}

/// Parse a WireGuard `.conf` into one profile per `[Peer]` that has an
/// `Endpoint` (`WireguardFmt.ResolveConfig`).
pub fn resolve_config(input: &str) -> Option<Vec<Profile>> {
    let mut interface: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut peers: Vec<std::collections::HashMap<String, String>> = Vec::new();
    let mut current: Option<usize> = None;

    for line in crate::util::split_lines(input) {
        if line.is_empty() {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("[Interface]") {
            current = None;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("[Peer]") {
            peers.push(std::collections::HashMap::new());
            current = Some(peers.len() - 1);
            continue;
        }
        if trimmed.starts_with('[') || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some(index) = line.find('=') else {
            continue;
        };
        if index == 0 {
            continue;
        }
        let key = line[..index].trim().to_string();
        let mut value = line[index + 1..].trim().to_string();
        if let Some(comment) = value.find(['#', ';']) {
            value = value[..comment].trim_end().to_string();
        }
        match current {
            None => {
                interface.insert(key, value);
            }
            Some(peer) => {
                peers[peer].insert(key, value);
            }
        }
    }

    let private_key = interface.get("PrivateKey").cloned()?;
    if private_key.is_empty() {
        return None;
    }
    let mtu = interface
        .get("MTU")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    let interface_address = interface.get("Address").cloned().unwrap_or_default();
    let dns = interface.get("DNS").cloned().unwrap_or_default();

    let mut list = Vec::new();
    for (index, peer) in peers.iter().enumerate() {
        let Some(endpoint) = peer.get("Endpoint").filter(|v| !v.is_empty()) else {
            continue;
        };
        let Some((address, port)) = try_parse_endpoint(endpoint) else {
            continue;
        };
        let mut item = Profile {
            config_type: ConfigType::WireGuard,
            remarks: format!("WireGuard Peer {}", index + 1),
            address,
            port,
            password: private_key.clone(),
            ..Profile::default()
        };
        item.proto_extra.wg_public_key = non_empty(peer.get("PublicKey").map(String::as_str));
        item.proto_extra.wg_preshared_key = non_empty(peer.get("PresharedKey").map(String::as_str));
        item.proto_extra.wg_interface_address = Some(interface_address.clone());
        item.proto_extra.wg_reserved = non_empty(peer.get("Reserved").map(String::as_str));
        item.proto_extra.wg_mtu = if mtu > 0 { Some(mtu) } else { None };
        item.proto_extra.wg_dns = Some(dns.clone());
        list.push(item);
    }
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

fn try_parse_endpoint(endpoint: &str) -> Option<(String, i32)> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('[') {
        let close = rest.find(']')?;
        let address = rest[..close].trim().to_string();
        if address.is_empty() {
            return None;
        }
        let after = &rest[close + 1..];
        let port = after
            .strip_prefix(':')
            .and_then(|p| p.trim().parse::<i32>().ok())
            .filter(|p| (1..=65535).contains(p))
            .unwrap_or(DEFAULT_ENDPOINT_PORT);
        return Some((address, port));
    }
    match trimmed.rfind(':') {
        Some(index) if index > 0 => {
            let address = trimmed[..index].trim().to_string();
            let port_text = trimmed[index + 1..].trim();
            match port_text.parse::<i32>() {
                Ok(port) if (1..=65535).contains(&port) => Some((address, port)),
                _ => Some((trimmed.to_string(), DEFAULT_ENDPOINT_PORT)),
            }
        }
        _ => Some((trimmed.to_string(), DEFAULT_ENDPOINT_PORT)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(vec![byte; 32])
    }

    #[test]
    fn uri_roundtrip_with_ipv6_and_escaped_keys() {
        let mut item = Profile {
            config_type: ConfigType::WireGuard,
            remarks: "WireGuard — тест 東京 #1".into(),
            address: "2001:db8::40".into(),
            port: 51820,
            password: key(0xFE),
            ..Profile::default()
        };
        item.proto_extra.wg_public_key = Some(key(0xFD));
        item.proto_extra.wg_preshared_key = Some(key(0xFC));
        item.proto_extra.wg_reserved = Some("1,2,255".into());
        item.proto_extra.wg_interface_address = Some("10.0.0.2/32,fd00::2/128".into());
        item.proto_extra.wg_mtu = Some(1420);

        let uri = emit(&item).unwrap();
        assert!(uri.contains(url_encode(&item.password).as_str()));
        assert!(uri.contains("@[2001:db8::40]:51820"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.password, item.password);
        assert_eq!(
            parsed.proto_extra.wg_public_key,
            item.proto_extra.wg_public_key
        );
        assert_eq!(parsed.proto_extra.wg_mtu, Some(1420));
        assert_eq!(parsed.remarks, item.remarks);
    }

    #[test]
    fn conf_parses_peers_and_strips_inline_comments() {
        let config = "[Interface]\nPrivateKey = interface-private-key\nAddress = 10.0.0.2/32, fd00::2/128 ; inline comment\nMTU = 1420\nDNS = 2001::db8::53, 1.2.3.4, 2001:db8::54\n\n[Peer]\nPublicKey = peer-public-key\nPresharedKey = peer-preshared-key\nReserved = 1, 2, 3 # inline comment\nEndpoint = [2001:db8::1]:51820 # inline comment\n\n[Peer]\nPublicKey = peer-public-key-2\nEndpoint = example.com:12345\n";
        let list = resolve_config(config).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].address, "2001:db8::1");
        assert_eq!(list[0].port, 51820);
        assert_eq!(list[0].password, "interface-private-key");
        assert_eq!(list[0].proto_extra.wg_reserved.as_deref(), Some("1, 2, 3"));
        assert_eq!(
            list[0].proto_extra.wg_interface_address.as_deref(),
            Some("10.0.0.2/32, fd00::2/128")
        );
        assert_eq!(list[0].proto_extra.wg_mtu, Some(1420));
        assert_eq!(list[1].address, "example.com");
        assert_eq!(list[1].port, 12345);
    }

    #[test]
    fn conf_without_private_key_is_rejected() {
        let config = "[Interface]\nAddress = 10.0.0.2/32\n[Peer]\nEndpoint = example.com:1\n";
        assert!(resolve_config(config).is_none());
    }
}
