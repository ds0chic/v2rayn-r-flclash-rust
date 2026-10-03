//! T15a Clash API client tests against a loopback mock controller.
//!
//! The mock binds `127.0.0.1:11818` (>= 11808, never the user's 10808) and
//! records each request so the select/delay/connection calls can be asserted.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use application::ClashApiService;
use tiny_http::{Response, Server};

const MOCK_PORT: u16 = 11818;

struct MockController {
    base: String,
    stop: Arc<AtomicBool>,
    seen: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl MockController {
    fn start() -> Self {
        let server = Server::http(("127.0.0.1", MOCK_PORT)).expect("bind mock clash controller");
        let base = format!("http://127.0.0.1:{MOCK_PORT}");
        let stop = Arc::new(AtomicBool::new(false));
        let seen: Arc<Mutex<Vec<(String, String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let worker_stop = Arc::clone(&stop);
        let worker_seen = Arc::clone(&seen);
        std::thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                let Ok(Some(mut request)) = server.recv_timeout(Duration::from_millis(100)) else {
                    continue;
                };
                let method = request.method().to_string();
                let url = request.url().to_string();
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                worker_seen
                    .lock()
                    .unwrap()
                    .push((method.clone(), url.clone(), body));

                let (status, payload) = route(&method, &url);
                let response = Response::from_string(payload).with_status_code(status);
                let _ = request.respond(response);
            }
        });
        Self { base, stop, seen }
    }

    fn requests(&self) -> Vec<(String, String, String)> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for MockController {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn route(method: &str, url: &str) -> (u16, String) {
    match (method, url) {
        ("GET", "/proxies") => (
            200,
            r#"{"proxies":{"GROUP":{"name":"GROUP","type":"Selector","now":"A","all":["A","B"]},"A":{"name":"A","type":"Shadowsocks","history":[{"delay":42}]}}}"#
                .to_string(),
        ),
        ("GET", "/providers/proxies") => (200, r#"{"providers":{}}"#.to_string()),
        ("PUT", url) if url.starts_with("/proxies/") => (204, String::new()),
        ("GET", url) if url.contains("/delay?") => (200, r#"{"delay":123}"#.to_string()),
        ("GET", "/connections") => (
            200,
            r#"{"downloadTotal":10,"uploadTotal":20,"connections":[{"id":"c1","upload":1,"download":2,"metadata":{"host":"example.com","network":"tcp","type":"HTTP","processPath":"C:/app.exe"},"start":"2026-01-01T00:00:00Z","chains":["A"]}]}"#
                .to_string(),
        ),
        ("DELETE", "/connections/") => (204, String::new()),
        ("DELETE", _) => (204, String::new()),
        _ => (404, String::new()),
    }
}

#[tokio::test]
async fn clash_api_service_reads_selects_and_closes() {
    let mock = MockController::start();
    let service = ClashApiService::from_client(
        core_adapters::clash_api::ClashApiClient::from_base(
            mock.base.clone(),
            None,
            Duration::from_secs(5),
        )
        .unwrap(),
        Duration::from_secs(2),
    );

    // Read: proxies merge and delay.
    let proxies = service.proxies().await.unwrap();
    assert!(proxies.proxies.contains_key("GROUP"));
    let delay = service.proxy_delay("A").await;
    assert_eq!(delay, 123);

    // Group delay probes both children (`GROUP` only lists A/B; B has no
    // provider entry so it falls back to the per-proxy delay endpoint, which
    // the mock answers for any `/proxies/*/delay?`).
    let group = service.group_delay("GROUP").await.unwrap();
    assert_eq!(group.len(), 2);
    assert!(group.iter().all(|(_, d)| *d == 123));

    // Select: PUT with the JSON body.
    service.select("GROUP", "B").await.unwrap();

    // Connections + close.
    let conns = service.connections().await.unwrap();
    assert_eq!(conns.connections.as_ref().unwrap().len(), 1);
    assert_eq!(
        conns.connections.as_ref().unwrap()[0].id.as_deref(),
        Some("c1")
    );
    service.close_connection("c1").await.unwrap();
    service.close_all().await.unwrap();

    let seen = mock.requests();
    let select = seen
        .iter()
        .find(|(m, u, _)| m == "PUT" && u == "/proxies/GROUP")
        .expect("PUT select");
    assert!(select.2.contains("\"name\":\"B\""));
    assert!(seen
        .iter()
        .any(|(m, u, _)| m == "DELETE" && u == "/connections/c1"));
    // Upstream `ClashConnectionClose(all: true)` passes an empty id, producing
    // `/connections/` (trailing slash).
    assert!(seen
        .iter()
        .any(|(m, u, _)| m == "DELETE" && u == "/connections/"));
}

#[test]
fn mock_port_is_never_the_live_proxy() {
    // Runtime check keeps the invariant auditable without tripping
    // `assertions_on_constants`.
    let port = std::hint::black_box(MOCK_PORT);
    assert!(port >= 11808);
    assert_ne!(port, 10808);
}
