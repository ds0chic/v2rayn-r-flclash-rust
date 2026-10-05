//! `ShadowsocksFmt` — SIP002 URI, legacy base64 URI and SIP008 server lists.

use std::sync::OnceLock;

use domain::{ConfigType, Profile};
use regex::Regex;

use super::base::{self, DEFAULT_NETWORK, RAW_HEADER_HTTP, STREAM_SECURITY};
use crate::error::SubError;
use crate::util::{base64_decode, base64_encode_nopad, url_decode, url_encode, Query};

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let trimmed = input.trim();
    let mut item = parse_legacy(trimmed);
    if item.is_none() {
        item = parse_sip002(trimmed)?;
    }
    let mut item = item.ok_or_else(|| SubError::InvalidConfig("shadowsocks".into()))?;

    let method = item.proto_extra.ss_method.clone().unwrap_or_default();
    if item.address.is_empty() || item.port == 0 || method.is_empty() || item.password.is_empty() {
        return Err(SubError::InvalidConfig("shadowsocks fields".into()));
    }
    item.config_type = ConfigType::Shadowsocks;
    Ok(item)
}

pub fn emit(item: &Profile) -> Result<String, SubError> {
    let remark = if item.remarks.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(&item.remarks))
    };
    let method = item.proto_extra.ss_method.clone().unwrap_or_default();
    let pw = base64_encode_nopad(&format!("{method}:{}", item.password));
    let transport = &item.transport_extra;

    let mut plugin = String::new();
    let mut plugin_args = String::new();
    if base::get_network(item) == DEFAULT_NETWORK
        && transport.raw_header_type.as_deref() == Some(RAW_HEADER_HTTP)
    {
        plugin = "obfs-local".into();
        plugin_args = format!(
            "obfs=http;obfs-host={};",
            transport.host.as_deref().unwrap_or("")
        );
    } else {
        if base::get_network(item) == "ws" {
            plugin_args.push_str("mode=websocket;");
            plugin_args.push_str(&format!(
                "host={};",
                transport.host.as_deref().unwrap_or("")
            ));
            let path = transport
                .path
                .clone()
                .unwrap_or_default()
                .replace('\\', "\\\\")
                .replace('=', "\\=")
                .replace(',', "\\,");
            plugin_args.push_str(&format!("path={path};"));
        }
        if base::stream_security(item) == STREAM_SECURITY {
            plugin_args.push_str("tls;");
            if let Some(body) = item
                .security
                .cert
                .as_deref()
                .and_then(|cert| crate::util::pem_cert_bodies(cert).into_iter().next())
            {
                let escaped = body.replace('=', "\\=");
                plugin_args.push_str(&format!("certRaw={escaped};"));
            }
        }
        if !plugin_args.is_empty() {
            plugin = "v2ray-plugin".into();
            plugin_args.push_str("mux=0;");
        }
    }

    let mut query = Vec::new();
    if !plugin.is_empty() {
        let mut plugin_str = format!("{plugin};{plugin_args}");
        if let Some(stripped) = plugin_str.strip_suffix(';') {
            plugin_str = stripped.to_string();
        }
        query.push(("plugin".into(), url_encode(&plugin_str)));
    }

    Ok(base::build_uri(
        base::SS,
        &item.address,
        item.port,
        &pw,
        &query,
        &remark,
    ))
}

fn url_finder() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)ss://(?P<base64>[A-Za-z0-9+/\-=_]+)(?:#(?P<tag>\S+))?")
            .expect("url finder regex")
    })
}

fn details_parser() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^((?P<method>.+?):(?P<password>.*)@(?P<hostname>.+?):(?P<port>\d+?))$")
            .expect("details regex")
    })
}

fn parse_legacy(input: &str) -> Option<Profile> {
    let caps = url_finder().captures(input)?;
    let mut item = Profile {
        config_type: ConfigType::Shadowsocks,
        ..Profile::default()
    };
    let base64 = caps.name("base64")?.as_str().trim_end_matches('/');
    if let Some(tag) = caps.name("tag") {
        item.remarks = url_decode(tag.as_str());
    }
    let decoded = base64_decode(base64).ok()?;
    let details = details_parser().captures(&decoded)?;
    item.proto_extra.ss_method = Some(details.name("method")?.as_str().to_string());
    item.password = details.name("password")?.as_str().to_string();
    item.address = details.name("hostname")?.as_str().to_string();
    item.port = details.name("port")?.as_str().parse().ok()?;
    Some(item)
}

fn parse_sip002(input: &str) -> Result<Option<Profile>, SubError> {
    let Ok(url) = url::Url::parse(input) else {
        return Ok(None);
    };
    // Only treat URI-shaped input as SIP002 when it looks like `ss://`.
    if !input
        .get(..base::SS.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(base::SS))
    {
        return Ok(None);
    }

    let mut item = Profile {
        config_type: ConfigType::Shadowsocks,
        remarks: url_decode(url.fragment().unwrap_or("")),
        address: base::host_addr(&url),
        port: url.port().map(i32::from).unwrap_or(0),
        ..Profile::default()
    };

    let mut raw_user = url.username().to_string();
    if let Some(password) = url.password() {
        raw_user.push(':');
        raw_user.push_str(password);
    }
    let raw_user = url_decode(&raw_user);
    let (method, password) = if raw_user.contains(':') {
        let (method, password) = raw_user.split_once(':').unwrap();
        (method.to_string(), url_decode(password))
    } else {
        let decoded = base64_decode(&raw_user)?;
        let (method, password) = decoded
            .split_once(':')
            .ok_or_else(|| SubError::InvalidConfig("ss userinfo".into()))?;
        (method.to_string(), password.to_string())
    };
    item.proto_extra.ss_method = Some(method);
    item.password = password;

    let query = Query::parse(url.query().unwrap_or(""));
    if let Some(plugin_str) = query.get("plugin") {
        parse_plugin(plugin_str, &mut item)?;
    }
    Ok(Some(item))
}

fn parse_plugin(plugin_str: &str, item: &mut Profile) -> Result<(), SubError> {
    let parts: Vec<&str> = plugin_str.split(';').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Err(SubError::InvalidConfig("ss plugin".into()));
    }
    let mut plugin_name = parts[0];
    if plugin_name == "simple-obfs" {
        plugin_name = "obfs-local";
    }

    if plugin_name == "obfs-local" {
        let obfs_mode = parts.iter().find(|p| p.starts_with("obfs="));
        let obfs_host = parts.iter().find(|p| p.starts_with("obfs-host="));
        if let (Some(mode), Some(host)) = (obfs_mode, obfs_host) {
            if mode.contains("obfs=http") && !host.is_empty() {
                item.network = DEFAULT_NETWORK.to_string();
                item.transport_extra.raw_header_type = Some(RAW_HEADER_HTTP.to_string());
                item.transport_extra.host = Some(host.replacen("obfs-host=", "", 1));
            }
        }
    } else if plugin_name == "v2ray-plugin" {
        let mode = parts
            .iter()
            .find(|p| p.starts_with("mode="))
            .copied()
            .unwrap_or("mode=websocket");
        let host = parts.iter().find(|p| p.starts_with("host="));
        let path = parts.iter().find(|p| p.starts_with("path="));
        let has_tls = parts.contains(&"tls");
        let cert_raw = parts.iter().find(|p| p.starts_with("certRaw="));
        let mux = parts.iter().find(|p| p.starts_with("mux="));

        if mode.replacen("mode=", "", 1) == "websocket" {
            item.network = "ws".to_string();
            if let Some(host) = host {
                let ws_host = host.replacen("host=", "", 1);
                item.transport_extra.host = Some(ws_host.clone());
                item.security.sni = Some(ws_host);
            }
            if let Some(path) = path {
                let path_value = path
                    .replacen("path=", "", 1)
                    .replace("\\=", "=")
                    .replace("\\,", ",")
                    .replace("\\\\", "\\");
                item.transport_extra.path = Some(path_value);
            }
        }

        if has_tls {
            item.security.stream_security = Some(STREAM_SECURITY.to_string());
            if let Some(cert_raw) = cert_raw {
                let cert_base64 = cert_raw.replacen("certRaw=", "", 1).replace("\\=", "=");
                item.security.cert = Some(format!(
                    "-----BEGIN CERTIFICATE-----\n{cert_base64}\n-----END CERTIFICATE-----"
                ));
            }
        }

        if let Some(mux) = mux {
            if mux.replacen("mux=", "", 1).parse::<i32>().unwrap_or(0) > 0 {
                return Err(SubError::InvalidConfig("ss plugin mux".into()));
            }
        }
    }
    Ok(())
}

/// SIP008 Shadowsocks JSON server list (`ShadowsocksFmt.ResolveSip008`).
pub fn resolve_sip008(input: &str) -> Option<Vec<Profile>> {
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let servers: Vec<serde_json::Value> = match &value {
        serde_json::Value::Array(items) => items.clone(),
        serde_json::Value::Object(map) => map.get("servers")?.as_array()?.clone(),
        _ => return None,
    };
    if servers.is_empty() {
        return None;
    }
    let mut list = Vec::new();
    for server in servers {
        let Some(obj) = server.as_object() else {
            continue;
        };
        let mut item = Profile {
            config_type: ConfigType::Shadowsocks,
            ..Profile::default()
        };
        item.remarks = str_field(obj, "remarks");
        item.address = str_field(obj, "server");
        item.port = str_field(obj, "server_port").parse().unwrap_or(0);
        item.password = str_field(obj, "password");
        item.proto_extra.ss_method = Some(str_field(obj, "method"));
        list.push(item);
    }
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

fn str_field(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    match obj.get(key) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_and_sip002_roundtrip() {
        let item = Profile {
            config_type: ConfigType::Shadowsocks,
            remarks: "ss demo".into(),
            address: "1.2.3.4".into(),
            port: 8388,
            password: "pass123".into(),
            network: "raw".into(),
            ..Profile::default()
        };
        let mut item = item;
        item.proto_extra.ss_method = Some("aes-128-gcm".into());
        let uri = emit(&item).unwrap();
        assert!(uri.starts_with("ss://"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.address, "1.2.3.4");
        assert_eq!(parsed.port, 8388);
        assert_eq!(parsed.password, "pass123");
        assert_eq!(parsed.proto_extra.ss_method.as_deref(), Some("aes-128-gcm"));
    }

    #[test]
    fn legacy_base64_form_is_parsed() {
        let payload = base64_encode_full("aes-256-gcm:secret@example.com:443");
        let uri = format!("ss://{payload}#node");
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.address, "example.com");
        assert_eq!(parsed.port, 443);
        assert_eq!(parsed.password, "secret");
        assert_eq!(parsed.remarks, "node");
    }

    #[test]
    fn noncanonical_plugin_with_literal_equals_configures_obfs() {
        let uri = "ss://YWVzLTEyOC1nY206cGFzczEyMw==@1.2.3.4:8388/?plugin=obfs-local;obfs=http;obfs-host=example.com#ss";
        let parsed = parse(uri).unwrap();
        assert_eq!(parsed.transport_extra.host.as_deref(), Some("example.com"));
        assert_eq!(
            parsed.transport_extra.raw_header_type.as_deref(),
            Some("http")
        );
    }

    #[test]
    fn missing_required_fields_are_rejected() {
        assert!(parse("ss://@example.com:8388").is_err());
        assert!(parse("ss://YWVzLTEyOC1nY206cGFzczEyMw==@:8388").is_err());
    }

    fn base64_encode_full(value: &str) -> String {
        crate::util::base64_encode(value)
    }
}
