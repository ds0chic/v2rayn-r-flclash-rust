//! `Hysteria2Fmt` plus the `HyRealm` rendezvous encoding.

use domain::{ConfigType, Profile};

use super::base;
use crate::error::SubError;
use crate::util::{bracket_ipv6, url_decode, url_encode, Query};

/// A parsed `realm://` / `realm+http://` rendezvous descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyRealm {
    pub is_http: bool,
    pub token: String,
    pub rendezvous_host: String,
    pub rendezvous_port: i32,
    pub realm_name: String,
    pub stun_list: Vec<String>,
}

impl HyRealm {
    pub fn rendezvous_host_port(&self) -> String {
        if self.rendezvous_port > 0 {
            format!("{}:{}", self.rendezvous_host, self.rendezvous_port)
        } else {
            self.rendezvous_host.clone()
        }
    }

    /// sing-box `realm.server_url` requires an explicit scheme.
    pub fn to_server_url(&self) -> String {
        let scheme = if self.is_http { "http" } else { "https" };
        format!("{scheme}://{}", self.rendezvous_host_port())
    }

    pub fn try_parse(input: &str) -> Option<HyRealm> {
        let is_http = input.starts_with("realm+http://");
        if !is_http && !input.starts_with("realm://") {
            return None;
        }
        let url = url::Url::parse(input).ok()?;

        let mut raw_user = url.username().to_string();
        if let Some(password) = url.password() {
            raw_user.push(':');
            raw_user.push_str(password);
        }

        let default_port = if is_http { 80 } else { 443 };
        let stun_list = url
            .query()
            .map(|query| {
                query
                    .split('&')
                    .filter_map(|pair| pair.split_once('='))
                    .filter(|(key, _)| key.eq_ignore_ascii_case("stun"))
                    .map(|(_, value)| url_decode(value))
                    .collect()
            })
            .unwrap_or_default();

        Some(HyRealm {
            is_http,
            token: url_decode(&raw_user),
            rendezvous_host: base::host_addr(&url),
            rendezvous_port: url.port().map(i32::from).unwrap_or(default_port),
            realm_name: url.path().trim_start_matches('/').to_string(),
            stun_list,
        })
    }

    pub fn to_uri(&self) -> String {
        let prefix = if self.is_http {
            "realm+http://"
        } else {
            "realm://"
        };
        let mut uri = format!(
            "{prefix}{}@{}:{}/{}",
            url_encode(&self.token),
            self.rendezvous_host,
            self.rendezvous_port,
            self.realm_name
        );
        if !self.stun_list.is_empty() {
            let query = self
                .stun_list
                .iter()
                .map(|stun| format!("stun={stun}"))
                .collect::<Vec<_>>()
                .join("&");
            uri.push('?');
            uri.push_str(&query);
        }
        uri
    }

    pub fn to_uri_for_finalmask(&self) -> String {
        let prefix = if self.is_http {
            "realm+http://"
        } else {
            "realm://"
        };
        format!(
            "{prefix}{}@{}/{}",
            url_encode(&self.token),
            self.rendezvous_host_port(),
            url_encode(&self.realm_name)
        )
    }
}

pub fn parse(input: &str) -> Result<Profile, SubError> {
    let url = url::Url::parse(input.trim()).map_err(|e| SubError::InvalidUri(e.to_string()))?;
    let mut item = Profile {
        config_type: ConfigType::Hysteria2,
        ..Profile::default()
    };
    item.address = base::host_addr(&url);
    // `port()` is `None` only when the port was omitted or empty; an explicit
    // `:0` stays 0 and is rejected later by validation.
    item.port = url.port().map(i32::from).unwrap_or(443);
    item.remarks = url_decode(url.fragment().unwrap_or(""));
    let mut raw_user = url.username().to_string();
    if let Some(password) = url.password() {
        raw_user.push(':');
        raw_user.push_str(password);
    }
    item.password = url_decode(&raw_user);

    let query = Query::parse(url.query().unwrap_or(""));
    base::resolve_uri_query(&query, &mut item);
    resolve_hy2_query(&query, &mut item);
    item.network = base::get_network(&item);
    Ok(item)
}

pub fn parse_realm(input: &str) -> Result<Profile, SubError> {
    let realm_str = input
        .strip_prefix("hysteria2+")
        .ok_or_else(|| SubError::Unsupported("hysteria2 realm".into()))?;
    let realm =
        HyRealm::try_parse(realm_str).ok_or_else(|| SubError::InvalidUri("hy2 realm".into()))?;
    let url = url::Url::parse(input.trim()).map_err(|e| SubError::InvalidUri(e.to_string()))?;

    let mut item = Profile {
        config_type: ConfigType::Hysteria2,
        address: realm.rendezvous_host.clone(),
        port: realm.rendezvous_port,
        remarks: url_decode(url.fragment().unwrap_or("")),
        ..Profile::default()
    };

    let query = Query::parse(url.query().unwrap_or(""));
    base::resolve_uri_query(&query, &mut item);
    let auth = query.get("auth").unwrap_or("");
    if auth.is_empty() {
        return Err(SubError::InvalidConfig("hy2 realm auth".into()));
    }
    item.password = auth.to_string();
    resolve_hy2_query(&query, &mut item);
    item.proto_extra.hy2_realm_url = Some(realm.to_uri());
    Ok(item)
}

pub fn emit(item: &Profile) -> Result<String, SubError> {
    if item
        .proto_extra
        .hy2_realm_url
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty())
    {
        return emit_realm(item);
    }
    let remark = if item.remarks.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(&item.remarks))
    };
    let mut query = Vec::new();
    base::to_uri_query_lite(item, &mut query);
    to_hy2_query(item, &mut query);
    Ok(base::build_uri(
        base::HYSTERIA2,
        &item.address,
        item.port,
        &item.password,
        &query,
        &remark,
    ))
}

fn emit_realm(item: &Profile) -> Result<String, SubError> {
    let realm = HyRealm::try_parse(item.proto_extra.hy2_realm_url.as_deref().unwrap_or(""))
        .ok_or_else(|| SubError::InvalidConfig("hy2 realm url".into()))?;
    let remark = if item.remarks.is_empty() {
        String::new()
    } else {
        format!("#{}", url_encode(&item.remarks))
    };
    let mut query = Vec::new();
    base::to_uri_query_lite(item, &mut query);
    to_hy2_query(item, &mut query);
    query.push(("auth".into(), url_encode(&item.password)));

    let mut query_string = String::from("?");
    for (key, value) in &query {
        query_string.push_str(key);
        query_string.push('=');
        query_string.push_str(value);
        query_string.push('&');
    }
    for stun in &realm.stun_list {
        query_string.push_str("stun=");
        query_string.push_str(&url_encode(stun));
        query_string.push('&');
    }
    let query_string = query_string.trim_end_matches('&').to_string();

    // The rendezvous host:port lives in the realm descriptor, so the emitted
    // prologue is `scheme + token@host/realm-name`.
    let url = format!(
        "{}@{}/{}",
        url_encode(&realm.token),
        bracket_ipv6(&realm.rendezvous_host),
        realm.realm_name
    );
    let scheme = if realm.is_http {
        base::HY2_REALM_HTTP
    } else {
        base::HY2_REALM
    };
    Ok(format!("{scheme}{url}{query_string}{remark}"))
}

fn resolve_hy2_query(query: &Query, item: &mut Profile) {
    if query.get("insecure") == Some("1") {
        base::set_allow_insecure(item, true);
    }
    if item.security.cert_sha.as_deref().unwrap_or("").is_empty() {
        let pin = query.get("pinSHA256").unwrap_or("").to_string();
        base::set_optional(&mut item.security.cert_sha, &pin);
        if !pin.is_empty() {
            // Xray-style self-signed links carry a fingerprint but not the
            // insecure flag; keep the connection usable.
            base::set_allow_insecure(item, true);
        }
    }
    base::set_optional(
        &mut item.security.ech_config_list,
        query.get("ech").unwrap_or(""),
    );
    base::set_optional(
        &mut item.proto_extra.ports,
        query.get("mport").unwrap_or(""),
    );
    base::set_optional(
        &mut item.proto_extra.salamander_pass,
        query.get("obfs-password").unwrap_or(""),
    );
    base::set_optional(
        &mut item.proto_extra.gecko_min_packet_size,
        query.get("minPacketSize").unwrap_or(""),
    );
    base::set_optional(
        &mut item.proto_extra.gecko_max_packet_size,
        query.get("maxPacketSize").unwrap_or(""),
    );
    if query.get("obfs") == Some("gecko") {
        if item
            .proto_extra
            .gecko_min_packet_size
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            item.proto_extra.gecko_min_packet_size = Some("512".into());
        }
        if item
            .proto_extra
            .gecko_max_packet_size
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            item.proto_extra.gecko_max_packet_size = Some("1200".into());
        }
    }
}

fn to_hy2_query(item: &Profile, query: &mut Vec<(String, String)>) {
    if base::allow_insecure(item) {
        query.push(("insecure".into(), "1".into()));
    }
    let cert_sha = item.security.cert_sha.clone().unwrap_or_default();
    if !cert_sha.is_empty() && !cert_sha.contains(',') {
        query.push(("pinSHA256".into(), url_encode(&cert_sha)));
    } else if let Some(thumbprint) = item
        .security
        .cert
        .as_deref()
        .and_then(crate::util::leaf_cert_sha256_hex)
    {
        query.push(("pinSHA256".into(), url_encode(&thumbprint)));
    }
    if let Some(ech) = item
        .security
        .ech_config_list
        .as_deref()
        .filter(|v| !v.is_empty())
    {
        query.push(("ech".into(), url_encode(ech)));
    }

    let min = item
        .proto_extra
        .gecko_min_packet_size
        .as_deref()
        .unwrap_or("");
    let max = item
        .proto_extra
        .gecko_max_packet_size
        .as_deref()
        .unwrap_or("");
    let is_gecko = !min.is_empty() || !max.is_empty();
    if let Some(pass) = item
        .proto_extra
        .salamander_pass
        .as_deref()
        .filter(|v| !v.is_empty())
    {
        query.push((
            "obfs".into(),
            if is_gecko { "gecko" } else { "salamander" }.into(),
        ));
        query.push(("obfs-password".into(), url_encode(pass)));
        if is_gecko {
            query.push(("minPacketSize".into(), min.to_string()));
            query.push(("maxPacketSize".into(), max.to_string()));
        }
    }
    if let Some(ports) = item.proto_extra.ports.as_deref().filter(|v| !v.is_empty()) {
        query.push(("mport".into(), url_encode(&ports.replace(':', "-"))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_defaults_to_443_and_explicit_zero_is_kept() {
        let parsed = parse("hysteria2://password@hy2.example/").unwrap();
        assert_eq!(parsed.port, 443);
        let zero = parse("hysteria2://password@hy2.example:0/").unwrap();
        assert_eq!(zero.port, 0);
        let explicit = parse("hysteria2://password@hy2.example:8443/").unwrap();
        assert_eq!(explicit.port, 8443);
    }

    #[test]
    fn obfs_and_port_range_roundtrip() {
        let mut item = Profile {
            config_type: ConfigType::Hysteria2,
            remarks: "hysteria2 demo".into(),
            address: "hy2.example".into(),
            port: 8443,
            password: "demo-user:demo-pass".into(),
            ..Profile::default()
        };
        item.security.sni = Some("sni.hy2.example".into());
        item.security.alpn = Some("h3".into());
        item.security.ech_config_list = Some("AAj+DQAEAAAAAA==".into());
        item.proto_extra.salamander_pass = Some("salamander-pass".into());
        item.proto_extra.ports = Some("5000:6000".into());
        base::set_allow_insecure(&mut item, true);

        let uri = emit(&item).unwrap();
        assert!(uri.contains("insecure=1"));
        assert!(uri.contains("obfs=salamander"));
        assert!(uri.contains("mport=5000-6000"));
        let parsed = parse(&uri).unwrap();
        assert_eq!(parsed.password, item.password);
        assert_eq!(
            parsed.security.ech_config_list,
            item.security.ech_config_list
        );
        assert_eq!(
            parsed.proto_extra.salamander_pass,
            item.proto_extra.salamander_pass
        );
        // The port range is normalized to the URI spelling on import.
        assert_eq!(parsed.proto_extra.ports.as_deref(), Some("5000-6000"));
    }

    #[test]
    fn escaped_query_decodes_exactly_once() {
        let parsed = parse(
            "hysteria2://pw@hy2.example:8443/?ech=AAj%2BDQAEAAAAAA%3D%3D&obfs=salamander&obfs-password=a%20b",
        )
        .unwrap();
        assert_eq!(
            parsed.security.ech_config_list.as_deref(),
            Some("AAj+DQAEAAAAAA==")
        );
        assert_eq!(parsed.proto_extra.salamander_pass.as_deref(), Some("a b"));
    }

    #[test]
    fn realm_roundtrip() {
        let input = "hysteria2+realm://mytoken@rendezvous.example.com/my-cabin?auth=your_password&insecure=1&pinSHA256=deadbeef#remark";
        let parsed = parse_realm(input).unwrap();
        assert_eq!(parsed.password, "your_password");
        assert_eq!(parsed.address, "rendezvous.example.com");
        let realm =
            HyRealm::try_parse(parsed.proto_extra.hy2_realm_url.as_deref().unwrap()).unwrap();
        assert_eq!(realm.token, "mytoken");
        let uri = emit(&parsed).unwrap();
        assert!(uri.contains("hysteria2+realm://mytoken@rendezvous.example.com"));
        assert!(uri.ends_with("#remark"));
    }

    #[test]
    fn hyrealm_parse_and_server_url() {
        let realm = HyRealm::try_parse(
            "realm://public@realm.hy2.io/57f9be7c?stun=example.stun:3478&stun=example2.stun:3478",
        )
        .unwrap();
        assert!(!realm.is_http);
        assert_eq!(realm.token, "public");
        assert_eq!(realm.rendezvous_host, "realm.hy2.io");
        assert_eq!(realm.rendezvous_port, 443);
        assert_eq!(realm.realm_name, "57f9be7c");
        assert_eq!(realm.stun_list.len(), 2);
        assert_eq!(realm.to_server_url(), "https://realm.hy2.io:443");
    }
}
