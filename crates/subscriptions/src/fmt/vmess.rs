//! `VmessFmt` — the JSON-over-base64 `vmess://` link and its legacy standard
//! URI form.

use domain::{ConfigType, Profile};
use serde_json::{Map, Value};

use super::base::{self, DEFAULT_SECURITY, NONE, RAW_NETWORK_ALIAS};
use crate::error::SubError;
use crate::util::{base64_decode, base64_encode, url_decode};

/// Parse a `vmess://` share URI (standard URI or base64 JSON form).
pub fn parse(input: &str) -> Result<Profile, SubError> {
    let trimmed = input.trim();
    if trimmed.contains('@') {
        if let Ok(item) = parse_std(trimmed) {
            return Ok(item);
        }
    }
    parse_json(trimmed)
}

pub fn emit(item: &Profile) -> Result<String, SubError> {
    let proto = &item.proto_extra;
    let transport = &item.transport_extra;
    let network = base::get_network(item);

    let type_value = match network.as_str() {
        "raw" => transport.raw_header_type.clone().unwrap_or_default(),
        "kcp" => transport.kcp_header_type.clone().unwrap_or_default(),
        "xhttp" => transport.xhttp_mode.clone().unwrap_or_default(),
        "grpc" => transport.grpc_mode.clone().unwrap_or_default(),
        _ => NONE.to_string(),
    };
    let host = match network.as_str() {
        "raw" | "ws" | "httpupgrade" | "xhttp" => transport.host.clone(),
        "grpc" => transport.grpc_authority.clone(),
        _ => None,
    };
    let path = match network.as_str() {
        "raw" | "ws" | "httpupgrade" | "xhttp" => transport.path.clone(),
        "kcp" => transport.kcp_seed.clone(),
        "grpc" => transport.grpc_service_name.clone(),
        _ => None,
    };

    let aid = proto
        .alter_id
        .as_deref()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);

    let mut obj = Map::new();
    obj.insert("v".into(), Value::String("2".into()));
    obj.insert("ps".into(), Value::String(item.remarks.trim().into()));
    obj.insert("add".into(), Value::String(item.address.clone()));
    obj.insert("port".into(), Value::String(item.port.to_string()));
    obj.insert("id".into(), Value::String(item.password.clone()));
    obj.insert("aid".into(), Value::String(aid.to_string()));
    obj.insert(
        "scy".into(),
        Value::String(proto.vmess_security.clone().unwrap_or_default()),
    );
    obj.insert(
        "net".into(),
        Value::String(if network == base::DEFAULT_NETWORK {
            RAW_NETWORK_ALIAS.into()
        } else {
            network
        }),
    );
    obj.insert("type".into(), Value::String(type_value));
    obj.insert("host".into(), Value::String(host.unwrap_or_default()));
    obj.insert("path".into(), Value::String(path.unwrap_or_default()));
    obj.insert(
        "tls".into(),
        Value::String(base::stream_security(item).to_string()),
    );
    obj.insert(
        "sni".into(),
        Value::String(item.security.sni.clone().unwrap_or_default()),
    );
    obj.insert(
        "alpn".into(),
        Value::String(item.security.alpn.clone().unwrap_or_default()),
    );
    obj.insert(
        "fp".into(),
        Value::String(item.security.fingerprint.clone().unwrap_or_default()),
    );
    obj.insert(
        "insecure".into(),
        Value::String(if base::allow_insecure(item) { "1" } else { "0" }.into()),
    );
    obj.insert(
        "vcn".into(),
        Value::String(
            item.security
                .verify_peer_cert_by_name
                .clone()
                .unwrap_or_default(),
        ),
    );
    obj.insert(
        "pcs".into(),
        Value::String(item.security.cert_sha.clone().unwrap_or_default()),
    );

    let json = Value::Object(obj).to_string();
    Ok(format!("{}{}", base::VMESS, base64_encode(&json)))
}

fn parse_std(input: &str) -> Result<Profile, SubError> {
    let url = url::Url::parse(input).map_err(|e| SubError::InvalidUri(e.to_string()))?;
    let mut item = Profile {
        config_type: ConfigType::Vmess,
        ..Profile::default()
    };
    item.address = base::host_addr(&url);
    item.port = url.port().map(i32::from).unwrap_or(0);
    item.remarks = url_decode(url.fragment().unwrap_or(""));
    let mut raw_user = url.username().to_string();
    if let Some(password) = url.password() {
        raw_user.push(':');
        raw_user.push_str(password);
    }
    item.password = url_decode(&raw_user);
    item.proto_extra.vmess_security = Some(DEFAULT_SECURITY.to_string());

    let query = crate::util::Query::parse(url.query().unwrap_or(""));
    base::resolve_uri_query(&query, &mut item);
    item.config_type = ConfigType::Vmess;
    Ok(item)
}

fn parse_json(input: &str) -> Result<Profile, SubError> {
    let trimmed = input.trim();
    let body = trimmed
        .strip_prefix(base::VMESS)
        .ok_or_else(|| SubError::Unsupported("vmess scheme".into()))?;
    let decoded = base64_decode(body)?;
    let value: Value =
        serde_json::from_str(&decoded).map_err(|_| SubError::Decode("vmess json".into()))?;
    let obj = value
        .as_object()
        .ok_or_else(|| SubError::Decode("vmess object".into()))?;

    let mut item = Profile {
        config_type: ConfigType::Vmess,
        ..Profile::default()
    };

    item.remarks = get_string(obj, "ps");
    item.address = get_string(obj, "add");
    item.port = get_i32(obj, "port");
    item.password = get_string(obj, "id");
    item.proto_extra.alter_id = Some(get_i64(obj, "aid").to_string());
    let scy = get_string(obj, "scy");
    item.proto_extra.vmess_security = Some(if scy.is_empty() {
        DEFAULT_SECURITY.to_string()
    } else {
        scy
    });

    let net = get_string(obj, "net");
    if !net.is_empty() {
        item.network = if net == RAW_NETWORK_ALIAS {
            base::DEFAULT_NETWORK.to_string()
        } else {
            net
        };
    } else {
        item.network = base::DEFAULT_NETWORK.to_string();
    }

    let type_value = get_string(obj, "type");
    let transport = &mut item.transport_extra;
    // Upstream seeds `RawHeaderType = none` for every network, then overrides it
    // from the `type` field only on the matching transport branch.
    transport.raw_header_type = Some(NONE.to_string());
    if !type_value.is_empty() {
        match item.network.as_str() {
            "raw" => transport.raw_header_type = Some(type_value.clone()),
            "kcp" => transport.kcp_header_type = Some(type_value),
            "xhttp" => transport.xhttp_mode = Some(type_value),
            "grpc" => transport.grpc_mode = Some(type_value),
            _ => {}
        }
    }

    let host = get_string(obj, "host");
    let path = get_string(obj, "path");
    match item.network.as_str() {
        "raw" | "ws" | "httpupgrade" | "xhttp" => {
            base::set_optional(&mut transport.host, &host);
            base::set_optional(&mut transport.path, &path);
        }
        "kcp" => {
            base::set_optional(&mut transport.kcp_seed, &path);
        }
        "grpc" => {
            base::set_optional(&mut transport.grpc_authority, &host);
            base::set_optional(&mut transport.grpc_service_name, &path);
        }
        _ => {}
    }

    item.security.stream_security = Some(get_string(obj, "tls"));
    item.security.sni = Some(get_string(obj, "sni"));
    item.security.alpn = Some(get_string(obj, "alpn"));
    item.security.fingerprint = Some(get_string(obj, "fp"));
    base::set_allow_insecure(&mut item, get_string(obj, "insecure") == "1");
    item.security.verify_peer_cert_by_name = Some(get_string(obj, "vcn"));
    item.security.cert_sha = Some(get_string(obj, "pcs"));
    Ok(item)
}

fn get_string(obj: &Map<String, Value>, key: &str) -> String {
    match obj.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn get_i64(obj: &Map<String, Value>, key: &str) -> i64 {
    get_string(obj, key).parse().unwrap_or(0)
}

fn get_i32(obj: &Map<String, Value>, key: &str) -> i32 {
    get_i64(obj, key) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip_preserves_core_fields() {
        let mut item = Profile {
            config_type: ConfigType::Vmess,
            remarks: "vmess demo".into(),
            address: "example.com".into(),
            port: 443,
            password: "b831381d-6324-4d53-ad4f-8cda48b30811".into(),
            network: "raw".into(),
            ..Profile::default()
        };
        item.proto_extra.alter_id = Some("0".into());
        item.proto_extra.vmess_security = Some(DEFAULT_SECURITY.into());
        item.transport_extra.raw_header_type = Some(NONE.into());

        let uri = emit(&item).unwrap();
        assert!(uri.starts_with("vmess://"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.address, item.address);
        assert_eq!(parsed.port, item.port);
        assert_eq!(parsed.password, item.password);
        assert_eq!(parsed.proto_extra.alter_id.as_deref(), Some("0"));
        assert_eq!(parsed.remarks, item.remarks);
    }

    #[test]
    fn std_uri_with_at_is_parsed_as_standard_form() {
        let parsed = parse("vmess://id@example.com:443/?type=ws&path=/x#node").unwrap();
        assert_eq!(parsed.address, "example.com");
        assert_eq!(parsed.password, "id");
        assert_eq!(parsed.network, "ws");
        assert_eq!(parsed.transport_extra.path.as_deref(), Some("/x"));
        assert_eq!(parsed.remarks, "node");
    }

    #[test]
    fn invalid_json_base64_is_rejected() {
        assert!(parse("vmess://not-base64-@@@").is_err());
    }
}
