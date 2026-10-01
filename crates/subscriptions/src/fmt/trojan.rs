//! `TrojanFmt`.

use domain::{ConfigType, Profile};

use super::base;
use crate::error::SubError;
use crate::util::{url_decode, url_encode, Query};

pub const INSECURE_KEYS: [&str; 2] = ["allowInsecure", "insecure"];

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let url = url::Url::parse(input.trim()).map_err(|e| SubError::InvalidUri(e.to_string()))?;
    let mut item = Profile {
        config_type: ConfigType::Trojan,
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
    if INSECURE_KEYS.iter().any(|key| query.get(key) == Some("1")) {
        base::set_allow_insecure(&mut item, true);
    }
    base::set_optional(&mut item.proto_extra.flow, query.get("flow").unwrap_or(""));
    base::resolve_uri_query(&query, &mut item);
    Ok(item)
}

pub fn emit(item: &Profile) -> Result<String, SubError> {
    let remark = if item.remarks.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(&item.remarks))
    };
    let mut query = Vec::new();
    if base::allow_insecure(item) {
        for key in INSECURE_KEYS {
            query.push((key.into(), "1".into()));
        }
    }
    if let Some(flow) = item.proto_extra.flow.as_deref().filter(|v| !v.is_empty()) {
        query.push(("flow".into(), flow.to_string()));
    }
    base::to_uri_query(item, None, &mut query);
    Ok(base::build_uri(
        base::TROJAN,
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
    fn roundtrip_writes_both_insecure_spellings() {
        let mut item = Profile {
            config_type: ConfigType::Trojan,
            remarks: "trojan demo".into(),
            address: "trojan.example".into(),
            port: 443,
            password: "trojan-pass".into(),
            network: "raw".into(),
            ..Profile::default()
        };
        item.security.stream_security = Some("tls".into());
        item.security.sni = Some("sni.trojan.example".into());
        base::set_allow_insecure(&mut item, true);
        item.proto_extra.flow = Some("xtls-rprx-vision".into());

        let uri = emit(&item).unwrap();
        assert!(uri.contains("allowInsecure=1"));
        assert!(uri.contains("insecure=1"));
        assert!(uri.contains("flow=xtls-rprx-vision"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.password, "trojan-pass");
        assert!(base::allow_insecure(&parsed));
        assert_eq!(parsed.security.sni.as_deref(), Some("sni.trojan.example"));
    }

    #[test]
    fn special_characters_in_password_roundtrip() {
        let mut item = Profile {
            config_type: ConfigType::Trojan,
            remarks: "Trojan — тест 東京 #1".into(),
            address: "t.example".into(),
            port: 443,
            password: "p:a@ss#% +/=".into(),
            ..Profile::default()
        };
        item.security.stream_security = Some(String::new());
        let uri = emit(&item).unwrap();
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.password, "p:a@ss#% +/=");
        assert_eq!(parsed.remarks, "Trojan — тест 東京 #1");
    }
}
