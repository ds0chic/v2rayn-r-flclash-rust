//! `NaiveFmt` — `naive://`, `naive+https://`, `naive+quic://`.

use domain::{ConfigType, Profile};

use super::base::{self, NONE};
use crate::error::SubError;
use crate::util::{bracket_ipv6, url_decode, url_encode, Query};

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let url = url::Url::parse(input.trim()).map_err(|e| SubError::InvalidUri(e.to_string()))?;
    let mut item = Profile {
        config_type: ConfigType::Naive,
        ..Profile::default()
    };
    item.address = url.host_str().unwrap_or_default().to_string();
    item.port = url.port().map(i32::from).unwrap_or(0);
    item.remarks = url_decode(url.fragment().unwrap_or(""));
    if url.scheme().contains("quic") {
        item.proto_extra.naive_quic = Some(true);
    }

    let mut raw_user = url.username().to_string();
    if let Some(password) = url.password() {
        raw_user.push(':');
        raw_user.push_str(password);
    }
    let raw_user = url_decode(&raw_user);
    if let Some((user, pass)) = raw_user.split_once(':') {
        item.username = user.to_string();
        item.password = pass.to_string();
    } else {
        item.password = raw_user;
    }

    let query = Query::parse(url.query().unwrap_or(""));
    base::resolve_uri_query(&query, &mut item);
    let concurrency = query
        .get("insecure-concurrency")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    if concurrency > 0 {
        item.proto_extra.insecure_concurrency = Some(concurrency);
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
    let user_info = if item.username.is_empty() {
        url_encode(&item.password)
    } else {
        format!(
            "{}:{}",
            url_encode(&item.username),
            url_encode(&item.password)
        )
    };
    let mut query = Vec::new();
    base::to_uri_query(item, Some(NONE), &mut query);
    if let Some(concurrency) = item.proto_extra.insecure_concurrency.filter(|v| *v > 0) {
        query.push(("insecure-concurrency".into(), concurrency.to_string()));
    }
    let query_string = if query.is_empty() {
        String::new()
    } else {
        let joined = query
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("?{joined}")
    };
    let url = format!(
        "{user_info}@{}:{}{query_string}{remark}",
        bracket_ipv6(&item.address),
        item.port
    );
    let scheme = if item.proto_extra.naive_quic == Some(true) {
        base::NAIVE_QUIC
    } else {
        base::NAIVE_HTTPS
    };
    Ok(format!("{scheme}{url}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive(quic: bool) -> Profile {
        let mut item = Profile {
            config_type: ConfigType::Naive,
            remarks: if quic {
                "naive quic demo".into()
            } else {
                "naive https demo".into()
            },
            address: "naive.example".into(),
            port: 443,
            username: "naive-user".into(),
            password: "päss:word@/?#&=+ 東京".into(),
            network: "raw".into(),
            ..Profile::default()
        };
        item.proto_extra.naive_quic = Some(quic);
        item.proto_extra.insecure_concurrency = Some(4);
        item
    }

    #[test]
    fn https_roundtrip_credentials_and_concurrency() {
        let item = naive(false);
        let uri = emit(&item).unwrap();
        assert!(uri.starts_with("naive+https://"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.username, item.username);
        assert_eq!(parsed.password, item.password);
        assert_eq!(parsed.proto_extra.insecure_concurrency, Some(4));
        assert_ne!(parsed.proto_extra.naive_quic, Some(true));
    }

    #[test]
    fn quic_scheme_roundtrip() {
        let item = naive(true);
        let uri = emit(&item).unwrap();
        assert!(uri.starts_with("naive+quic://"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.proto_extra.naive_quic, Some(true));
        assert_eq!(parsed.password, item.password);
    }
}
