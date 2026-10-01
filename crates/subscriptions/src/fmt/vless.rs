//! `VLESSFmt`.

use domain::{ConfigType, Profile};

use super::base::{self, NONE};
use crate::error::SubError;
use crate::util::{url_decode, url_encode, Query};

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let url = url::Url::parse(input.trim()).map_err(|e| SubError::InvalidUri(e.to_string()))?;
    let mut item = Profile {
        config_type: ConfigType::Vless,
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
        &mut item.proto_extra.vless_encryption,
        query.get_or("encryption", NONE),
    );
    base::set_optional(&mut item.proto_extra.flow, query.get("flow").unwrap_or(""));
    base::set_optional(
        &mut item.security.stream_security,
        query.get("security").unwrap_or(""),
    );
    base::resolve_uri_query(&query, &mut item);
    item.network = base::get_network(&item);
    Ok(item)
}

pub fn emit(item: &Profile) -> Result<String, SubError> {
    let remark = if item.remarks.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(&item.remarks))
    };
    let mut query = Vec::new();
    let encryption = item
        .proto_extra
        .vless_encryption
        .clone()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| NONE.to_string());
    query.push(("encryption".into(), encryption));
    if let Some(flow) = item.proto_extra.flow.as_deref().filter(|v| !v.is_empty()) {
        query.push(("flow".into(), flow.to_string()));
    }
    base::to_uri_query(item, Some(NONE), &mut query);
    Ok(base::build_uri(
        base::VLESS,
        &item.address,
        item.port,
        &item.password,
        &query,
        &remark,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic_fields_and_encryption_default() {
        let item = Profile {
            config_type: ConfigType::Vless,
            remarks: "vless demo".into(),
            address: "vless.example".into(),
            port: 8443,
            password: "b831381d-6324-4d53-ad4f-8cda48b30811".into(),
            network: "raw".into(),
            ..Profile::default()
        };
        let uri = emit(&item).unwrap();
        assert!(uri.starts_with("vless://"));
        assert!(uri.contains("encryption=none"));
        assert!(uri.contains("security=none"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.address, item.address);
        assert_eq!(parsed.port, item.port);
        assert_eq!(parsed.password, item.password);
        assert_eq!(parsed.proto_extra.vless_encryption.as_deref(), Some("none"));
        assert_eq!(parsed.remarks, item.remarks);
    }

    #[test]
    fn reality_parameters_roundtrip() {
        let uri = "vless://uuid@example.com:443?encryption=none&security=reality&sni=s.example&fp=chrome&pbk=KEY&sid=ab12&type=tcp&flow=xtls-rprx-vision#n";
        let parsed = parse(uri).unwrap();
        assert_eq!(parsed.security.stream_security.as_deref(), Some("reality"));
        assert_eq!(parsed.security.public_key.as_deref(), Some("KEY"));
        assert_eq!(parsed.security.short_id.as_deref(), Some("ab12"));
        assert_eq!(parsed.proto_extra.flow.as_deref(), Some("xtls-rprx-vision"));
    }
}
