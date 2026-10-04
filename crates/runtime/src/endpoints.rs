//! Resolved runtime endpoints for a pass-through full Custom config (RR-07).
//!
//! A `ConfigType::Custom` node is handed to the core verbatim, so the port the
//! plan must wait on and publish is whatever the config actually listens on,
//! not `AppManager.GetLocalPort`. Upstream keeps the proxy inbound and the
//! statistics/API listener distinct; this module reads the same two facts from
//! the emitted JSON (Xray `inbounds`/`metrics.listen`, sing-box
//! `inbounds`/`experimental.clash_api.external_controller`).

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
    /// Whether the config also carries UDP for this inbound.
    pub udp: bool,
}

/// The real endpoints a full Custom config binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomEndpoints {
    /// Client inbounds in config order; the first is the primary endpoint.
    pub inbounds: Vec<ResolvedInbound>,
    /// Xray `metrics.listen` / sing-box `experimental.clash_api` port.
    pub api_port: Option<u16>,
}

impl CustomEndpoints {
    pub fn primary(&self) -> Option<&ResolvedInbound> {
        self.inbounds.first()
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
                udp,
            });
        }
    }
    let api_port = xray_api_port(value);
    if inbounds.is_empty() {
        return Err("custom Xray config has no socks/http inbound".into());
    }
    Ok(CustomEndpoints { inbounds, api_port })
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
            inbounds.push(ResolvedInbound {
                protocol: proto,
                port,
                udp,
            });
        }
    }
    let api_port = value
        .get("experimental")
        .and_then(|experimental| experimental.get("clash_api"))
        .and_then(|clash| clash.get("external_controller"))
        .and_then(Value::as_str)
        .and_then(parse_host_port);
    if inbounds.is_empty() {
        return Err("custom sing-box config has no socks/http inbound".into());
    }
    Ok(CustomEndpoints { inbounds, api_port })
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

fn xray_api_port(value: &Value) -> Option<u16> {
    if let Some(port) = value
        .get("metrics")
        .and_then(|metrics| metrics.get("listen"))
        .and_then(Value::as_str)
        .and_then(parse_host_port)
    {
        return Some(port);
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
                item.get("port")
                    .and_then(Value::as_u64)
                    .and_then(valid_port)
            })
        })
}

fn valid_port(raw: u64) -> Option<u16> {
    u16::try_from(raw).ok().filter(|port| *port > 0)
}

/// Parse a `host:port` or bare `port` string (IPv6 in brackets supported).
pub fn parse_host_port(raw: &str) -> Option<u16> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let port = if let Some(close) = raw.rfind(']') {
        // `[::1]:8080`
        raw.get(close + 1..)
            .and_then(|rest| rest.strip_prefix(':'))?
    } else if raw.matches(':').count() == 1 {
        // `host:port` / `:port`
        raw.rsplit(':').next()?
    } else if raw.contains(':') {
        // Bare IPv6 without brackets is not a host:port form.
        return None;
    } else {
        // Bare numeric port.
        raw
    };
    port.trim().parse::<u16>().ok().filter(|port| *port > 0)
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
        assert_eq!(eps.api_port, Some(12015));
        assert_eq!(ProxyProtocol::Socks.scheme(), "socks5");
    }

    #[test]
    fn xray_custom_socks_only_has_socks_scheme() {
        let body = r#"{"inbounds":[{"port":12020,"protocol":"socks"}]}"#;
        let eps = parse_custom_endpoints(CoreType::Xray, body).expect("parse");
        assert_eq!(eps.primary().unwrap().port, 12020);
        assert_eq!(eps.primary().unwrap().protocol.scheme(), "socks5");
        assert_eq!(eps.api_port, None);
    }

    #[test]
    fn singbox_custom_reads_listen_port_and_clash_api() {
        let body = r#"{
            "inbounds": [{"type": "mixed", "listen_port": 12030}],
            "experimental": {"clash_api": {"external_controller": "127.0.0.1:12031"}}
        }"#;
        let eps = parse_custom_endpoints(CoreType::SingBox, body).expect("parse");
        assert_eq!(eps.primary().unwrap().protocol, ProxyProtocol::Mixed);
        assert_eq!(eps.primary().unwrap().port, 12030);
        assert_eq!(eps.api_port, Some(12031));
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
    }
}
