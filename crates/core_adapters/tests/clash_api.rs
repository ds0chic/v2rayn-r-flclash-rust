mod common;

use std::time::Duration;

use common::{pick_spare_port, MockHttp, MockRequest, MockResponse};
use core_adapters::clash_api::{ClashApiClient, ClashError};
use core_adapters::error::HttpError;

fn client(port: u16) -> ClashApiClient {
    ClashApiClient::new(port, None, Duration::from_secs(2)).unwrap()
}

fn client_with_secret(port: u16, secret: &str) -> ClashApiClient {
    ClashApiClient::new(port, Some(secret.to_string()), Duration::from_secs(2)).unwrap()
}

const PROXIES: &str = r#"{"proxies":{"GLOBAL":{"type":"Selector","now":"node-a","all":["node-a","node-b"]},"node-a":{"type":"Shadowsocks","udp":true,"history":[{"time":"t","delay":12}]}}}"#;
const PROVIDERS: &str = r#"{"providers":{"sub":{"name":"sub","type":"Proxy","vehicleType":"HTTP","proxies":[{"name":"node-b","type":"Trojan","udp":false},{"name":"node-a","type":"Shadowsocks"}]}}}"#;

fn routing(request: &MockRequest) -> MockResponse {
    let url = request.url.as_str();
    if url == "/proxies" {
        MockResponse::json(PROXIES)
    } else if url == "/providers/proxies" {
        MockResponse::json(PROVIDERS)
    } else if url == "/connections" {
        MockResponse::json(CONNECTIONS)
    } else if url == "/connections/" && request.method == "DELETE" {
        MockResponse::json("").status(204)
    } else if url.starts_with("/proxies/") && url.contains("/delay") {
        MockResponse::json(r#"{"delay":123}"#)
    } else if url.starts_with("/providers/proxies/") && url.contains("/healthcheck") {
        MockResponse::json(r#"{"delay":77}"#)
    } else if url.starts_with("/connections/") && request.method == "DELETE" {
        MockResponse::json("").status(204)
    } else if url.starts_with("/proxies/node") {
        MockResponse::json(r#"{"name":"node one","type":"Shadowsocks","udp":true,"delay":5}"#)
    } else if url == "/configs" {
        MockResponse::json(r#"{"mode":"rule","mode-list":["rule","global"]}"#)
    } else {
        MockResponse::json("{}").status(404)
    }
}

const CONNECTIONS: &str = r#"{"downloadTotal":100,"uploadTotal":50,"connections":[{"id":"abc","upload":10,"download":20,"start":"2026-01-01T00:00:00Z","chains":["proxy"],"rule":"rule-1","rulePayload":"payload","metadata":{"network":"tcp","type":"http","sourceIP":"127.0.0.1","sourcePort":"1","destinationIP":"1.2.3.4","destinationPort":"443","host":"example.com","uid":1000,"processPath":"C:/x.exe"}}]}"#;

#[tokio::test]
async fn merges_proxies_and_providers() {
    let mock = MockHttp::spawn(routing);
    let item = client(mock.port).get_proxies().await.unwrap();
    assert!(item.proxies.contains_key("GLOBAL"));
    assert!(item.proxies.contains_key("node-a"));
    assert!(item.proxies.contains_key("node-b"));
    assert_eq!(
        item.provider_index_map.get("node-b"),
        Some(&"sub".to_string())
    );
    assert!(!item.provider_index_map.contains_key("node-a"));
}

#[tokio::test]
async fn tolerates_provider_endpoint_failure() {
    let mock = MockHttp::spawn(|request| {
        if request.url == "/proxies" {
            MockResponse::json(PROXIES)
        } else {
            MockResponse::json("boom").status(500)
        }
    });
    let item = client(mock.port).get_proxies().await.unwrap();
    assert!(item.proxies.contains_key("node-a"));
}

#[tokio::test]
async fn fully_unreachable_proxies_surface_the_error() {
    let mock = MockHttp::spawn(|_| MockResponse::json("boom").status(500));
    let err = client(mock.port).get_proxies().await.unwrap_err();
    assert!(matches!(err, ClashError::Http(HttpError::Status(500))));
}

#[tokio::test]
async fn get_proxy_encodes_path_segment() {
    let mock = MockHttp::spawn(|request| {
        assert_eq!(request.url, "/proxies/node%20one");
        routing(request)
    });
    let proxy = client(mock.port).get_proxy("node one").await.unwrap();
    assert_eq!(proxy.name.as_deref(), Some("node one"));
    assert!(proxy.udp);
}

#[tokio::test]
async fn proxy_delay_parses_number() {
    let mock = MockHttp::spawn(|request| {
        assert!(request.url.starts_with("/proxies/node/delay?"));
        assert!(request.url.contains("timeout=10000"));
        routing(request)
    });
    let delay = client(mock.port)
        .get_proxy_delay("node", 10_000, "https://www.google.com/generate_204")
        .await;
    assert_eq!(delay, 123);
}

#[tokio::test]
async fn proxy_delay_returns_minus_one_on_non_number() {
    let mock = MockHttp::spawn(|_| MockResponse::json(r#"{"message":"timeout"}"#));
    let delay = client(mock.port)
        .get_proxy_delay("node", 10_000, "https://x")
        .await;
    assert_eq!(delay, -1);
}

#[tokio::test]
async fn proxy_delay_returns_minus_one_on_transport_error() {
    let port = pick_spare_port();
    let delay = client(port)
        .get_proxy_delay("node", 10_000, "https://x")
        .await;
    assert_eq!(delay, -1);
}

#[tokio::test]
async fn provider_delay_hits_healthcheck() {
    let mock = MockHttp::spawn(|request| {
        assert!(request
            .url
            .starts_with("/providers/proxies/prov/node/healthcheck?"));
        routing(request)
    });
    let delay = client(mock.port)
        .try_get_provider_proxy_delay("prov", "node", 10_000, "https://x")
        .await
        .unwrap();
    assert_eq!(delay, 77);
}

#[tokio::test]
async fn parses_connections() {
    let mock = MockHttp::spawn(routing);
    let connections = client(mock.port).get_connections().await.unwrap();
    assert_eq!(connections.upload_total, 50);
    assert_eq!(connections.download_total, 100);
    let item = &connections.connections.unwrap()[0];
    assert_eq!(item.id.as_deref(), Some("abc"));
    assert_eq!(
        item.metadata.as_ref().unwrap().host.as_deref(),
        Some("example.com")
    );
}

#[tokio::test]
async fn closes_one_connection() {
    let mock = MockHttp::spawn(|request| {
        assert_eq!(request.method, "DELETE");
        assert_eq!(request.url, "/connections/abc");
        routing(request)
    });
    client(mock.port).close_connection("abc").await.unwrap();
}

#[tokio::test]
async fn closes_all_connections() {
    let mock = MockHttp::spawn(|request| {
        assert_eq!(request.method, "DELETE");
        assert_eq!(request.url, "/connections/");
        routing(request)
    });
    client(mock.port).close_all_connections().await.unwrap();
}

#[tokio::test]
async fn missing_secret_is_unauthorized() {
    let mock = MockHttp::spawn(|request| {
        if request.header("Authorization").is_some() {
            MockResponse::json(PROXIES)
        } else {
            MockResponse::json("{}").status(401)
        }
    });
    let err = client(mock.port).get_proxies().await.unwrap_err();
    assert!(matches!(err, ClashError::Http(HttpError::Unauthorized)));
}

#[tokio::test]
async fn secret_is_sent_as_bearer_token() {
    let mock = MockHttp::spawn(|request| {
        assert_eq!(request.header("Authorization"), Some("Bearer s3cr3t"));
        routing(request)
    });
    let item = client_with_secret(mock.port, "s3cr3t")
        .get_proxies()
        .await
        .unwrap();
    assert!(!item.is_empty());
}

#[tokio::test]
async fn timeout_is_classified() {
    let mock = MockHttp::spawn(|_| {
        std::thread::sleep(Duration::from_millis(500));
        MockResponse::json("{}")
    });
    let client = ClashApiClient::new(mock.port, None, Duration::from_millis(100)).unwrap();
    let err = client.get_connections().await.unwrap_err();
    assert!(matches!(err, ClashError::Http(HttpError::Timeout)));
}

#[tokio::test]
async fn connection_refused_is_classified() {
    let port = pick_spare_port();
    // Loopback refusals on this host can take ~2s (SYN retry before RST), so
    // the timeout must be comfortably longer than that to observe the connect
    // error rather than a timeout.
    let client = ClashApiClient::new(port, None, Duration::from_secs(5)).unwrap();
    let err = client.get_connections().await.unwrap_err();
    assert!(
        matches!(err, ClashError::Http(HttpError::Connect(_))),
        "got {err:?}"
    );
}

#[tokio::test]
async fn malformed_json_is_decode_error() {
    let mock = MockHttp::spawn(|_| MockResponse::json("{ not json"));
    let err = client(mock.port).get_connections().await.unwrap_err();
    assert!(matches!(err, ClashError::Http(HttpError::Decode(_))));
}

#[tokio::test]
async fn get_config_returns_raw_json() {
    let mock = MockHttp::spawn(routing);
    let config = client(mock.port).get_config().await.unwrap();
    assert_eq!(config["mode"], "rule");
    assert_eq!(config["mode-list"][1], "global");
}

#[tokio::test]
async fn get_mode_reads_config() {
    let mock = MockHttp::spawn(routing);
    assert_eq!(
        client(mock.port).get_mode().await.unwrap().as_deref(),
        Some("rule")
    );
}

#[tokio::test]
async fn get_modes_prefers_mode_list() {
    let mock = MockHttp::spawn(routing);
    let modes = client(mock.port).get_modes().await.unwrap();
    assert_eq!(modes, vec!["rule", "global"]);
}

#[tokio::test]
async fn update_mode_patches_config_header() {
    let mock = MockHttp::spawn(|request| {
        assert_eq!(request.method, "PATCH");
        assert_eq!(request.url, "/configs");
        assert_eq!(request.header("mode"), Some("global"));
        MockResponse::json("").status(204)
    });
    client(mock.port).update_mode("global").await.unwrap();
}

#[tokio::test]
async fn retry_returns_first_non_empty_after_failures() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_handler = Arc::clone(&calls);
    let mock = MockHttp::spawn(move |request| {
        let n = calls_for_handler.fetch_add(1, Ordering::SeqCst);
        // Fail the whole first attempt (both endpoints) so the retry path runs.
        if n < 2 {
            MockResponse::json("boom").status(500)
        } else {
            routing(request)
        }
    });
    let item = client(mock.port)
        .get_proxies_with_retry(3, Duration::from_millis(10))
        .await
        .unwrap();
    assert!(item.proxies.contains_key("GLOBAL"));
    assert!(calls.load(Ordering::SeqCst) >= 4);
}

#[tokio::test]
async fn retry_gives_up_after_attempts() {
    let mock = MockHttp::spawn(|_| MockResponse::json("boom").status(500));
    let err = client(mock.port)
        .get_proxies_with_retry(2, Duration::from_millis(10))
        .await
        .unwrap_err();
    assert!(matches!(err, ClashError::Http(HttpError::Status(500))));
}
