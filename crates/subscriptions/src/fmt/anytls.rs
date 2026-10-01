//! `AnytlsFmt`.

use domain::{ConfigType, Profile};

use super::base::{self, NONE};
use crate::error::SubError;
use crate::util::{url_decode, url_encode, Query};

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let url = url::Url::parse(input.trim()).map_err(|e| SubError::InvalidUri(e.to_string()))?;
    let mut item = Profile {
        config_type: ConfigType::Anytls,
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
    base::resolve_uri_query(&query, &mut item);
    if query.get("insecure") == Some("1") {
        base::set_allow_insecure(&mut item, true);
    }
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
    if base::allow_insecure(item) {
        query.push(("insecure".into(), "1".into()));
    }
    base::to_uri_query(item, Some(NONE), &mut query);
    Ok(base::build_uri(
        base::ANYTLS,
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
    fn roundtrip_basic_fields() {
        let mut item = Profile {
            config_type: ConfigType::Anytls,
            remarks: "anytls demo".into(),
            address: "anytls.example".into(),
            port: 8443,
            password: "anytls-pass".into(),
            network: "raw".into(),
            ..Profile::default()
        };
        item.security.stream_security = Some("tls".into());
        item.security.sni = Some("sni.anytls.example".into());
        item.security.alpn = Some("h2,http/1.1".into());
        base::set_allow_insecure(&mut item, true);

        let uri = emit(&item).unwrap();
        assert!(uri.contains("insecure=1"));
        assert!(uri.contains("security=tls"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.address, "anytls.example");
        assert_eq!(parsed.password, "anytls-pass");
        assert_eq!(parsed.security.alpn.as_deref(), Some("h2,http/1.1"));
        assert!(base::allow_insecure(&parsed));
    }
}
