//! PAC server tests: all traffic stays on loopback and every server is
//! stopped (Drop guarantees it) before the test ends.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use platform::{PacConfig, PacServer, PacSource, DEFAULT_PAC_PORT_BASE};

fn free_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral");
    listener.local_addr().expect("addr").port()
}

fn http_get(port: u16, path: &str) -> (String, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    write!(stream, "GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").expect("write");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("read");
    match raw.split_once("\r\n\r\n") {
        Some((head, body)) => (head.to_string(), body.to_string()),
        None => (raw, String::new()),
    }
}

fn temp_path(suffix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("t13-pac-{}-{nanos}-{suffix}", std::process::id()))
}

#[test]
fn auto_port_is_at_least_the_base() {
    let mut server = PacServer::new(PacConfig::default()).expect("config");
    let port = server
        .start(PacSource::Inline(
            "function FindProxyForURL(){}".to_string(),
        ))
        .expect("start");
    assert!(port >= DEFAULT_PAC_PORT_BASE, "port {port} below base");
    assert!(server.is_running());
    server.stop().expect("stop");
    assert!(!server.is_running());
}

#[test]
fn serves_pac_on_loopback() {
    let port = free_port();
    let mut server = PacServer::new(PacConfig::with_port(port)).expect("config");
    server
        .start(PacSource::Inline(
            "function FindProxyForURL(){return 'DIRECT';}".to_string(),
        ))
        .expect("start");

    let (head, body) = http_get(port, "/pac");
    assert!(head.starts_with("HTTP/1.0 200 OK"), "head: {head}");
    assert!(head.contains("application/x-ns-proxy-autoconfig"));
    assert!(body.contains("FindProxyForURL"));

    server.stop().expect("stop");
}

#[test]
fn returns_404_for_unknown_path() {
    let port = free_port();
    let mut server = PacServer::new(PacConfig::with_port(port)).expect("config");
    server
        .start(PacSource::Inline("pac".to_string()))
        .expect("start");

    let (head, _body) = http_get(port, "/not-pac");
    assert!(head.starts_with("HTTP/1.0 404"), "head: {head}");

    server.stop().expect("stop");
}

#[test]
fn port_in_use_is_reported() {
    let holder = TcpListener::bind(("127.0.0.1", 0)).expect("hold");
    let port = holder.local_addr().expect("addr").port();

    let mut server = PacServer::new(PacConfig::with_port(port)).expect("config");
    let err = server
        .start(PacSource::Inline("pac".to_string()))
        .expect_err("must fail");
    assert!(matches!(err, platform::PlatformError::PortInUse(_)));
    assert!(!server.is_running());
    drop(holder);
}

#[test]
fn repeated_start_is_idempotent() {
    let mut server = PacServer::new(PacConfig::with_port(free_port())).expect("config");
    let first = server
        .start(PacSource::Inline("A".to_string()))
        .expect("start 1");
    assert!(server.is_running());

    let second = server
        .start(PacSource::Inline("B".to_string()))
        .expect("start 2");
    assert_eq!(first, second, "port must not change on repeated start");

    let (_, body) = http_get(first, "/pac");
    assert_eq!(body, "B");

    server.stop().expect("stop");
}

#[test]
fn refresh_renders_the_updated_proxy_rule() {
    let config = PacConfig {
        proxy_rule: Some("PROXY 127.0.0.1:11809;DIRECT;".to_string()),
        ..PacConfig::default()
    };
    let mut server = PacServer::new(config).expect("config");
    let script = || PacSource::Inline("var p = '__PROXY__';".to_string());
    let port = server.start(script()).expect("start");

    server.set_proxy_rule(Some("PROXY 127.0.0.1:11819;DIRECT;".to_string()));
    assert_eq!(server.start(script()).expect("refresh"), port);

    let (_, body) = http_get(port, "/pac");
    assert!(body.contains("PROXY 127.0.0.1:11819;DIRECT;"), "{body}");
    server.stop().expect("stop");
}

#[test]
fn stop_then_restart_rebinds() {
    let port = free_port();
    let mut server = PacServer::new(PacConfig::with_port(port)).expect("config");
    server
        .start(PacSource::Inline("one".to_string()))
        .expect("start");
    server.stop().expect("stop");
    assert!(!server.is_running());

    server
        .start(PacSource::Inline("two".to_string()))
        .expect("restart");
    let (_, body) = http_get(port, "/pac");
    assert_eq!(body, "two");
    server.stop().expect("stop");
}

#[test]
fn file_source_with_placeholder_substitution() {
    let path = temp_path("custom.pac");
    std::fs::write(&path, "var p = '__PROXY__';").expect("write pac");

    let config = PacConfig {
        proxy_rule: Some("PROXY 127.0.0.1:11809;DIRECT;".to_string()),
        ..PacConfig::with_port(free_port())
    };
    let mut server = PacServer::new(config).expect("config");
    let port = server.start(PacSource::File(path.clone())).expect("start");

    let (_, body) = http_get(port, "/pac");
    assert!(body.contains("PROXY 127.0.0.1:11809;DIRECT;"));
    assert!(!body.contains("__PROXY__"));

    server.stop().expect("stop");
    let _ = std::fs::remove_file(path);
}

#[test]
fn missing_file_source_is_not_found() {
    let mut server = PacServer::new(PacConfig::with_port(free_port())).expect("config");
    let err = server
        .start(PacSource::File(temp_path("missing.pac")))
        .expect_err("must fail");
    assert!(matches!(err, platform::PlatformError::NotFound(_)));
}

#[test]
fn non_loopback_host_is_rejected() {
    let config = PacConfig {
        host: "0.0.0.0".to_string(),
        ..PacConfig::default()
    };
    assert!(matches!(
        PacServer::new(config),
        Err(platform::PlatformError::Invalid(_))
    ));
}

#[test]
fn query_string_is_ignored_for_routing() {
    let port = free_port();
    let mut server = PacServer::new(PacConfig::with_port(port)).expect("config");
    server
        .start(PacSource::Inline("pac-body".to_string()))
        .expect("start");
    let (head, body) = http_get(port, "/pac?t=123");
    assert!(head.starts_with("HTTP/1.0 200 OK"));
    assert_eq!(body, "pac-body");
    server.stop().expect("stop");
}
