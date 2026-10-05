//! `TuicFmt`.

use domain::{ConfigType, Profile};

use super::base;
use crate::error::SubError;
use crate::util::{url_encode, Query};

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let url = url::Url::parse(input.trim()).map_err(|e| SubError::InvalidUri(e.to_string()))?;
    let mut item = Profile {
        config_type: ConfigType::Tuic,
        ..Profile::default()
    };
    item.address = base::host_addr(&url);
    item.port = url.port().map(i32::from).unwrap_or(0);
    item.remarks = crate::util::url_decode(url.fragment().unwrap_or(""));

    let (username, password) = base::split_user_info(&url);
    item.username = username;
    item.password = password;

    let query = Query::parse(url.query().unwrap_or(""));
    base::resolve_uri_query(&query, &mut item);
    if query.get("allow_insecure") == Some("1") {
        base::set_allow_insecure(&mut item, true);
    }
    base::set_optional(
        &mut item.proto_extra.congestion_control,
        query.get("congestion_control").unwrap_or(""),
    );
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
    base::to_uri_query_lite(item, &mut query);
    if base::allow_insecure(item) {
        query.push(("allow_insecure".into(), "1".into()));
    }
    if let Some(cc) = item
        .proto_extra
        .congestion_control
        .as_deref()
        .filter(|v| !v.is_empty())
    {
        query.push(("congestion_control".into(), cc.to_string()));
    }
    let user_info = format!("{}:{}", item.username, item.password);
    Ok(base::build_uri(
        base::TUIC,
        &item.address,
        item.port,
        &user_info,
        &query,
        &remark,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_userinfo_and_congestion_control() {
        let mut item = Profile {
            config_type: ConfigType::Tuic,
            remarks: "tuic demo".into(),
            address: "tuic.example".into(),
            port: 8443,
            username: "01234567-89ab-cdef-0123-456789abcdef".into(),
            password: "tuic-pass".into(),
            ..Profile::default()
        };
        item.security.sni = Some("sni.tuic.example".into());
        item.security.alpn = Some("h3".into());
        item.proto_extra.congestion_control = Some("bbr".into());
        base::set_allow_insecure(&mut item, true);

        let uri = emit(&item).unwrap();
        assert!(uri.contains("allow_insecure=1"));
        assert!(uri.contains("congestion_control=bbr"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.username, item.username);
        assert_eq!(parsed.password, "tuic-pass");
        assert_eq!(parsed.security.alpn.as_deref(), Some("h3"));
        assert_eq!(
            parsed.proto_extra.congestion_control.as_deref(),
            Some("bbr")
        );
        assert!(base::allow_insecure(&parsed));
    }
}
