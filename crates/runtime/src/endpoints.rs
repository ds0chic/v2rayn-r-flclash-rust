//! Resolved runtime endpoints for a pass-through full Custom config (RR-07).
//!
//! A `ConfigType::Custom` node is handed to the core verbatim, so the port the
//! plan must wait on and publish is whatever the config actually listens on,
//! not `AppManager.GetLocalPort`. Upstream keeps the proxy inbound and the
//! statistics/API listener distinct; this module reads the same facts from the
//! emitted JSON (Xray `inbounds`/`metrics.listen`, sing-box
//! `inbounds`/`experimental.clash_api`).
//!
//! R3-07 also resolves the facts the monitor/Clash path needs: the inbound
//! `listen` address, whether the proxy inbound requires authentication, and the
//! API kind/listen address/secret. Secrets are returned as data and must never
//! be logged.

use domain::CoreType;
use serde_json::Value;

/// A client-facing proxy inbound protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyProtocol {
    Http,
    Socks,
    Mixed,
}

impl ProxyProtocol {
    /// The URL scheme a local proxy endpoint uses for this protocol.
    pub fn scheme(self) -> &'static str {
        match self {
            ProxyProtocol::Socks => "socks5",
            ProxyProtocol::Http | ProxyProtocol::Mixed => "http",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ProxyProtocol::Http => "http",
            ProxyProtocol::Socks => "socks",
            ProxyProtocol::Mixed => "mixed",
        }
    }
}

/// One resolved client inbound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedInbound {
    pub protocol: ProxyProtocol,
    pub port: u16,
    /// Resolved `listen` address (`0.0.0.0` when the config omits it).
    pub listen: String,
    /// Whether the config also carries UDP for this inbound.
    pub udp: bool,
    /// Whether the inbound requires proxy authentication (accounts/users).
    pub authentication: bool,
}

/// The kind of statistics/API listener the config exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKind {
    /// sing-box `experimental.clash_api`.
    ClashApi,
    /// Xray `metrics` / `api` dokodemo-door.
    XrayStats,
}

/// A resolved statistics/API listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomApi {
    pub kind: ApiKind,
    pub listen: String,
    pub port: u16,
    /// Clash `secret`, when configured. Never logged.
    pub secret: Option<String>,
}

/// The real endpoints a full Custom config binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomEndpoints {
    /// Client inbounds in config order; the first is the primary endpoint.
    pub inbounds: Vec<ResolvedInbound>,
    /// Statistics/API listener, when the config declares one.
    pub api: Option<CustomApi>,
}

impl CustomEndpoints {
    pub fn primary(&self) -> Option<&ResolvedInbound> {
        self.inbounds.first()
    }

    /// The API port, when present. Kept for callers that only need the port.
    pub fn api_port(&self) -> Option<u16> {
        self.api.as_ref().map(|api| api.port)
    }
}

/// Parse a full pass-through config. `Err` carries a human-readable detail and
/// callers must fail the plan rather than pretend the core is ready.
pub fn parse_custom_endpoints(core: CoreType, body: &str) -> Result<CustomEndpoints, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| format!("custom config is not valid JSON: {error}"))?;
    match core {
        CoreType::SingBox => parse_singbox(&value),
        _ => parse_xray(&value),
    }
}

fn parse_xray(value: &Value) -> Result<CustomEndpoints, String> {
    let mut inbounds = Vec::new();
    if let Some(list) = value.get("inbounds").and_then(Value::as_array) {
        for item in list {
            let protocol = item.get("protocol").and_then(Value::as_str).unwrap_or("");
            let Some(proto) = xray_proxy_protocol(protocol) else {
                continue;
            };
            let Some(port) = item
                .get("port")
                .and_then(Value::as_u64)
                .and_then(valid_port)
            else {
                continue;
            };
            let udp = item.get("network").and_then(Value::as_str) == Some("udp");
            inbounds.push(ResolvedInbound {
                protocol: proto,
                port,
                listen: listen_address(item.get("listen")),
                udp,
                authentication: xray_inbound_authenticated(item),
            });
        }
    }
    let api = xray_api(value);
    if inbounds.is_empty() {
        return Err("custom Xray config has no socks/http inbound".into());
    }
    Ok(CustomEndpoints { inbounds, api })
}

fn parse_singbox(value: &Value) -> Result<CustomEndpoints, String> {
    let mut inbounds = Vec::new();
    if let Some(list) = value.get("inbounds").and_then(Value::as_array) {
        for item in list {
            let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
            let Some(proto) = singbox_proxy_protocol(kind) else {
                continue;
            };
            let Some(port) = item
                .get("listen_port")
                .and_then(Value::as_u64)
                .and_then(valid_port)
            else {
                continue;
            };
            let udp = item.get("udp").and_then(Value::as_bool).unwrap_or(false);
            let authentication = item
                .get("users")
                .and_then(Value::as_array)
                .map(|users| !users.is_empty())
                .unwrap_or(false);
            inbounds.push(ResolvedInbound {
                protocol: proto,
                port,
                listen: listen_address(item.get("listen")),
                udp,
                authentication,
            });
        }
    }
    let api = value
        .get("experimental")
        .and_then(|experimental| experimental.get("clash_api"))
        .and_then(|clash| {
            let raw = clash.get("external_controller").and_then(Value::as_str)?;
            let (listen, port) = split_host_port(raw)?;
            let secret = clash
                .get("secret")
                .and_then(Value::as_str)
                .filter(|secret| !secret.is_empty())
                .map(str::to_string);
            Some(CustomApi {
                kind: ApiKind::ClashApi,
                listen,
                port,
                secret,
            })
        });
    if inbounds.is_empty() {
        return Err("custom sing-box config has no socks/http inbound".into());
    }
    Ok(CustomEndpoints { inbounds, api })
}

fn xray_proxy_protocol(protocol: &str) -> Option<ProxyProtocol> {
    match protocol {
        "socks" => Some(ProxyProtocol::Socks),
        "http" => Some(ProxyProtocol::Http),
        "mixed" => Some(ProxyProtocol::Mixed),
        _ => None,
    }
}

fn singbox_proxy_protocol(kind: &str) -> Option<ProxyProtocol> {
    match kind {
        "socks" => Some(ProxyProtocol::Socks),
        "http" => Some(ProxyProtocol::Http),
        "mixed" => Some(ProxyProtocol::Mixed),
        _ => None,
    }
}

/// Whether an Xray socks/http inbound carries accounts (proxy authentication).
fn xray_inbound_authenticated(item: &Value) -> bool {
    let settings = item.get("settings");
    let accounts = settings
        .and_then(|settings| settings.get("accounts"))
        .and_then(Value::as_array)
        .map(|accounts| !accounts.is_empty())
        .unwrap_or(false);
    if accounts {
        return true;
    }
    settings
        .and_then(|settings| settings.get("auth"))
        .and_then(Value::as_str)
        .map(|auth| auth.eq_ignore_ascii_case("password"))
        .unwrap_or(false)
}

fn xray_api(value: &Value) -> Option<CustomApi> {
    if let Some(raw) = value
        .get("metrics")
        .and_then(|metrics| metrics.get("listen"))
        .and_then(Value::as_str)
        .and_then(split_host_port)
    {
        return Some(CustomApi {
            kind: ApiKind::XrayStats,
            listen: raw.0,
            port: raw.1,
            secret: None,
        });
    }
    // Fall back to an explicit `api` dokodemo-door inbound.
    value
        .get("inbounds")
        .and_then(Value::as_array)
        .and_then(|list| {
            list.iter().find_map(|item| {
                let protocol = item.get("protocol").and_then(Value::as_str).unwrap_or("");
                let tag = item.get("tag").and_then(Value::as_str).unwrap_or("");
                if protocol != "dokodemo-door" || tag != "api" {
                    return None;
                }
                let port = item
                    .get("port")
                    .and_then(Value::as_u64)
                    .and_then(valid_port)?;
                Some(CustomApi {
                    kind: ApiKind::XrayStats,
                    listen: listen_address(item.get("listen")),
                    port,
                    secret: None,
                })
            })
        })
}

fn listen_address(raw: Option<&Value>) -> String {
    raw.and_then(Value::as_str)
        .map(str::trim)
        .filter(|listen| !listen.is_empty())
        .unwrap_or("0.0.0.0")
        .to_string()
}

fn valid_port(raw: u64) -> Option<u16> {
    u16::try_from(raw).ok().filter(|port| *port > 0)
}

/// Parse a `host:port` or bare `port` string (IPv6 in brackets supported),
/// returning the resolved listen host and port.
pub fn split_host_port(raw: &str) -> Option<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(close) = raw.rfind(']') {
        // `[::1]:8080`
        let host = raw.get(..=close)?.to_string();
        let port = raw.get(close + 1..)?.strip_prefix(':')?;
        return Some((host, port.trim().parse::<u16>().ok().filter(|p| *p > 0)?));
    }
    if raw.matches(':').count() == 1 {
        let (host, port) = raw.rsplit_once(':')?;
        let listen = if host.trim().is_empty() {
            "0.0.0.0".to_string()
        } else {
            host.trim().to_string()
        };
        return Some((listen, port.trim().parse::<u16>().ok().filter(|p| *p > 0)?));
    }
    if raw.contains(':') {
        // Bare IPv6 without brackets is not a host:port form.
        return None;
    }
    // Bare numeric port.
    Some((
        "0.0.0.0".to_string(),
        raw.parse::<u16>().ok().filter(|port| *port > 0)?,
    ))
}

/// Parse just the port from a `host:port` / bare `port` string.
pub fn parse_host_port(raw: &str) -> Option<u16> {
    split_host_port(raw).map(|(_, port)| port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xray_custom_reads_real_inbound_and_metrics() {
        let body = r#"{
            "inbounds": [
                {"port": 12010, "listen": "127.0.0.1", "protocol": "socks"},
                {"port": 12011, "protocol": "http"}
            ],
            "metrics": {"listen": "127.0.0.1:12015"},
            "outbounds": [{"protocol": "freedom"}]
        }"#;
        let eps = parse_custom_endpoints(CoreType::Xray, body).expect("parse");
        assert_eq!(eps.inbounds.len(), 2);
        assert_eq!(eps.primary().unwrap().protocol, ProxyProtocol::Socks);
        assert_eq!(eps.primary().unwrap().port, 12010);
        assert_eq!(eps.primary().unwrap().listen, "127.0.0.1");
        assert!(!eps.primary().unwrap().authentication);
        let api = eps.api.as_ref().expect("api");
        assert_eq!(api.kind, ApiKind::XrayStats);
        assert_eq!(api.listen, "127.0.0.1");
        assert_eq!(api.port, 12015);
        assert_eq!(eps.api_port(), Some(12015));
        assert_eq!(ProxyProtocol::Socks.scheme(), "socks5");
    }

    #[test]
    fn xray_custom_socks_only_has_socks_scheme() {
        let body = r#"{"inbounds":[{"port":12020,"protocol":"socks"}]}"#;
        let eps = parse_custom_endpoints(CoreType::Xray, body).expect("parse");
        assert_eq!(eps.primary().unwrap().port, 12020);
        assert_eq!(eps.primary().unwrap().listen, "0.0.0.0");
        assert_eq!(eps.primary().unwrap().protocol.scheme(), "socks5");
        assert!(eps.api.is_none());
    }

    #[test]
    fn xray_inbound_auth_is_detected() {
        let body = r#"{"inbounds":[{"port":12021,"protocol":"http","settings":{"accounts":[{"user":"u","pass":"p"}]}}]}"#;
        let eps = parse_custom_endpoints(CoreType::Xray, body).expect("parse");
        assert!(eps.primary().unwrap().authentication);
    }

    #[test]
    fn singbox_custom_reads_listen_port_and_clash_api() {
        let body = r#"{
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": 12030}],
            "experimental": {"clash_api": {"external_controller": "127.0.0.1:12031", "secret": "synthetic-secret"}}
        }"#;
        let eps = parse_custom_endpoints(CoreType::SingBox, body).expect("parse");
        assert_eq!(eps.primary().unwrap().protocol, ProxyProtocol::Mixed);
        assert_eq!(eps.primary().unwrap().port, 12030);
        assert_eq!(eps.primary().unwrap().listen, "127.0.0.1");
        let api = eps.api.as_ref().expect("api");
        assert_eq!(api.kind, ApiKind::ClashApi);
        assert_eq!(api.listen, "127.0.0.1");
        assert_eq!(api.port, 12031);
        assert_eq!(api.secret.as_deref(), Some("synthetic-secret"));
    }

    #[test]
    fn custom_without_proxy_inbound_is_an_error() {
        let body = r#"{"inbounds":[{"port":12040,"protocol":"dokodemo-door"}],"outbounds":[]}"#;
        let error = parse_custom_endpoints(CoreType::Xray, body).unwrap_err();
        assert!(
            error.contains("no socks/http inbound"),
            "unexpected: {error}"
        );
    }

    #[test]
    fn invalid_json_is_an_error() {
        let error = parse_custom_endpoints(CoreType::Xray, "{not json").unwrap_err();
        assert!(error.contains("not valid JSON"), "unexpected: {error}");
    }

    #[test]
    fn host_port_parsing_handles_brackets_and_bare() {
        assert_eq!(parse_host_port("127.0.0.1:12050"), Some(12050));
        assert_eq!(parse_host_port(":12051"), Some(12051));
        assert_eq!(parse_host_port("12052"), Some(12052));
        assert_eq!(parse_host_port("[::1]:12053"), Some(12053));
        assert_eq!(parse_host_port("localhost:0"), None);
        assert_eq!(parse_host_port(""), None);
        assert_eq!(split_host_port(":12051").unwrap().0, "0.0.0.0");
        assert_eq!(split_host_port("[::1]:12053").unwrap().0, "[::1]");
    }
}
