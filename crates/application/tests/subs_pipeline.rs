//! T09 update-pipeline tests against a local loopback HTTP server.
//!
//! Ports are chosen from 11808 upwards; the reserved user proxy port 10808 is
//! never touched. Every server is aborted at the end of its test.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use application::{AppEngine, SubItem, SubUpdateOutcome, SubUpdateRequest};
use domain::CancellationToken;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct Server {
    base: String,
    handle: tokio::task::JoinHandle<()>,
}

impl Server {
    fn abort(self) {
        self.handle.abort();
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
}

async fn bind() -> TcpListener {
    for port in 11808..11950u16 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return listener;
        }
    }
    panic!("no free loopback port in 11808..11950");
}

fn http_ok(body: &[u8]) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn http_status(code: u16) -> Vec<u8> {
    format!("HTTP/1.1 {code} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
}

/// A server that returns the queued bodies in order, ignoring the path.
async fn spawn_bodies(bodies: Vec<Vec<u8>>) -> Server {
    let listener = bind().await;
    let port = listener.local_addr().unwrap().port();
    let queue = Arc::new(Mutex::new(VecDeque::from(bodies)));
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let _ = read_request(&mut socket).await;
            let Some(reply) = queue.lock().unwrap().pop_front() else {
                break;
            };
            let _ = socket.write_all(&reply).await;
            let _ = socket.shutdown().await;
        }
    });
    Server {
        base: format!("http://127.0.0.1:{port}"),
        handle,
    }
}

/// A server that captures the request lines and returns a fixed body, so
/// header assertions can be made.
async fn spawn_capturing(body: Vec<u8>) -> (Server, Arc<Mutex<Vec<String>>>) {
    let listener = bind().await;
    let port = listener.local_addr().unwrap().port();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_task = captured.clone();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let request = read_request(&mut socket).await;
            captured_task.lock().unwrap().push(request);
            let _ = socket.write_all(&body).await;
            let _ = socket.shutdown().await;
        }
    });
    (
        Server {
            base: format!("http://127.0.0.1:{port}"),
            handle,
        },
        captured,
    )
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let read = tokio::time::timeout_at(deadline, socket.read(&mut chunk)).await;
        match read {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(n)) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Ok(Err(_)) => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn b64_lines(lines: &[&str]) -> String {
    subscriptions::util::base64_encode(&lines.join("\n"))
}

fn sub_item(url: &str) -> SubItem {
    SubItem {
        id: "sub-1".into(),
        remarks: "test".into(),
        url: url.into(),
        enabled: true,
        ..SubItem::default()
    }
}

#[tokio::test]
async fn successful_update_replaces_subid_nodes() {
    let body = b64_lines(&[
        "vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one",
        "vless://22222222-2222-2222-2222-222222222222@b.example:443?encryption=none#two",
    ]);
    let server = spawn_bodies(vec![http_ok(body.as_bytes())]).await;
    let engine = AppEngine::in_memory();
    engine.save_sub_item(sub_item(&server.url("/s"))).unwrap();

    let report = engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: vec!["sub-1".into()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            1000,
        )
        .await;
    assert_eq!(report.success_count(), 1);
    assert_eq!(engine.profiles_by_subid("sub-1").unwrap().len(), 2);
    // UpdateTime advanced.
    let updated = engine.get_sub_item("sub-1").unwrap().unwrap();
    assert!(updated.update_time > 0);
    server.abort();
}

#[tokio::test]
async fn successful_update_replaces_old_nodes_by_subid() {
    let body = b64_lines(&[
        "vless://33333333-3333-3333-3333-333333333333@c.example:443?encryption=none#new",
    ]);
    let server = spawn_bodies(vec![http_ok(body.as_bytes())]).await;
    let engine = AppEngine::in_memory();
    engine.save_sub_item(sub_item(&server.url("/s"))).unwrap();

    let old = subscriptions::resolve_uri(
        "vless://99999999-9999-9999-9999-999999999999@old.example:443?encryption=none#old",
    )
    .unwrap();
    let mut old = old;
    old.index_id = "old-1".into();
    old.subid = "sub-1".into();
    engine
        .save_profile(old, domain::DesiredRevision::new(engine.desired_revision()))
        .unwrap();

    engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: vec!["sub-1".into()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            1000,
        )
        .await;

    let profiles = engine.profiles_by_subid("sub-1").unwrap();
    assert_eq!(profiles.len(), 1);
    assert!(profiles.iter().all(|p| p.index_id != "old-1"));
    server.abort();
}

#[tokio::test]
async fn http_failure_preserves_existing_nodes() {
    let server = spawn_bodies(vec![http_status(500)]).await;
    let engine = AppEngine::in_memory();
    engine.save_sub_item(sub_item(&server.url("/s"))).unwrap();

    let mut old = subscriptions::resolve_uri(
        "vless://99999999-9999-9999-9999-999999999999@old.example:443?encryption=none#keep",
    )
    .unwrap();
    old.index_id = "old-keep".into();
    old.subid = "sub-1".into();
    engine
        .save_profile(old, domain::DesiredRevision::new(engine.desired_revision()))
        .unwrap();

    let report = engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: vec!["sub-1".into()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            1000,
        )
        .await;
    assert_eq!(report.success_count(), 0);
    assert!(matches!(
        report.entries[0].outcome,
        SubUpdateOutcome::PreservedError { .. }
    ));
    assert_eq!(engine.profiles_by_subid("sub-1").unwrap().len(), 1);
    server.abort();
}

#[tokio::test]
async fn empty_result_preserves_existing_nodes() {
    let server = spawn_bodies(vec![http_ok(b"   \n  ")]).await;
    let engine = AppEngine::in_memory();
    engine.save_sub_item(sub_item(&server.url("/s"))).unwrap();

    let mut old = subscriptions::resolve_uri(
        "vless://99999999-9999-9999-9999-999999999999@old.example:443?encryption=none#keep",
    )
    .unwrap();
    old.index_id = "old-empty".into();
    old.subid = "sub-1".into();
    engine
        .save_profile(old, domain::DesiredRevision::new(engine.desired_revision()))
        .unwrap();

    let report = engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: vec!["sub-1".into()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            1000,
        )
        .await;
    assert_eq!(report.success_count(), 0);
    assert_eq!(engine.profiles_by_subid("sub-1").unwrap().len(), 1);
    server.abort();
}

#[tokio::test]
async fn more_url_merges_in_order() {
    // Three requests: main, more-url-1, more-url-2.
    let server = spawn_bodies(vec![
        http_ok(
            b64_lines(&[
                "vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one",
            ])
            .as_bytes(),
        ),
        http_ok(
            b64_lines(&[
                "vless://22222222-2222-2222-2222-222222222222@b.example:443?encryption=none#two",
            ])
            .as_bytes(),
        ),
        http_ok(
            b64_lines(&[
                "vless://33333333-3333-3333-3333-333333333333@c.example:443?encryption=none#three",
            ])
            .as_bytes(),
        ),
    ])
    .await;
    let engine = AppEngine::in_memory();
    let mut item = sub_item(&server.url("/main"));
    item.more_url = format!("{},{}", server.url("/m1"), server.url("/m2"));
    engine.save_sub_item(item).unwrap();

    let report = engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: vec!["sub-1".into()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            1000,
        )
        .await;
    assert_eq!(report.success_count(), 1);
    assert_eq!(engine.profiles_by_subid("sub-1").unwrap().len(), 3);
    server.abort();
}

#[tokio::test]
async fn request_headers_are_sent() {
    let body = b64_lines(&[
        "vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one",
    ]);
    let (server, captured) = spawn_capturing(http_ok(body.as_bytes())).await;
    let engine = AppEngine::in_memory();
    let mut item = sub_item(&server.url("/s"));
    item.request_headers = Some(r#"{"X-Auth":"secret-token"}"#.into());
    engine.save_sub_item(item).unwrap();

    engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: vec!["sub-1".into()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            1000,
        )
        .await;
    let requests = captured.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|r| r.to_ascii_lowercase().contains("x-auth") && r.contains("secret-token")),
        "custom header not sent: {requests:?}"
    );
    server.abort();
}

#[tokio::test]
async fn filter_drops_non_matching_nodes() {
    let body = b64_lines(&[
        "vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#Tokyo",
        "vless://22222222-2222-2222-2222-222222222222@b.example:443?encryption=none#Frankfurt",
    ]);
    let server = spawn_bodies(vec![http_ok(body.as_bytes())]).await;
    let engine = AppEngine::in_memory();
    let mut item = sub_item(&server.url("/s"));
    item.filter = Some("Tokyo".into());
    engine.save_sub_item(item).unwrap();

    engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: vec!["sub-1".into()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            1000,
        )
        .await;
    let profiles = engine.profiles_by_subid("sub-1").unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].remarks, "Tokyo");
    server.abort();
}

#[tokio::test]
async fn cancellation_marks_entry_cancelled() {
    // A slow body so the cancellation lands during the download.
    let listener = bind().await;
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let _ = read_request(&mut socket).await;
            tokio::time::sleep(Duration::from_secs(5)).await;
            let _ = socket.shutdown().await;
        }
    });
    let server = Server {
        base: format!("http://127.0.0.1:{port}"),
        handle,
    };
    let engine = AppEngine::in_memory();
    engine.save_sub_item(sub_item(&server.url("/s"))).unwrap();

    let token = CancellationToken::new();
    let token_clone = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        token_clone.cancel();
    });
    let report = engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: vec!["sub-1".into()],
                via_proxy: false,
                proxy_url: None,
            },
            &token,
            1000,
        )
        .await;
    assert!(report.cancelled(), "report: {report:?}");
    server.abort();
}

#[tokio::test]
async fn update_all_skips_empty_url_plain_group_without_failing() {
    // FIX-06 / SET-01: a plain group (empty URL) mixed with a real
    // subscription must not fail "update all", must be reported as skipped,
    // and must not replace/own any nodes.
    let body = b64_lines(&[
        "vless://11111111-1111-1111-1111-111111111111@a.example:443?encryption=none#one",
    ]);
    let server = spawn_bodies(vec![http_ok(body.as_bytes())]).await;
    let engine = AppEngine::in_memory();

    let plain = engine
        .save_sub_item(SubItem {
            remarks: "普通分组".into(),
            url: String::new(),
            ..SubItem::default()
        })
        .unwrap();
    assert!(!plain.id.is_empty());
    engine
        .save_sub_item(SubItem {
            remarks: "real".into(),
            url: server.url("/s"),
            ..SubItem::default()
        })
        .unwrap();

    let report = engine
        .refresh_subscriptions(
            SubUpdateRequest {
                sub_ids: Vec::new(),
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            1000,
        )
        .await;

    assert_eq!(report.success_count(), 1);
    let plain_entry = report
        .entries
        .iter()
        .find(|e| e.sub_id == plain.id)
        .expect("plain group entry present");
    assert!(matches!(
        plain_entry.outcome,
        SubUpdateOutcome::Skipped { .. }
    ));
    assert!(engine.profiles_by_subid(&plain.id).unwrap().is_empty());
    assert_eq!(
        engine.get_sub_item(&plain.id).unwrap().unwrap().update_time,
        0
    );
    server.abort();
}

#[tokio::test]
async fn empty_url_plain_group_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    {
        let engine = AppEngine::open(&path).unwrap();
        let saved = engine
            .save_sub_item(SubItem {
                remarks: "仅备注普通分组".into(),
                url: String::new(),
                memo: Some("memo".into()),
                ..SubItem::default()
            })
            .unwrap();
        assert!(!saved.id.is_empty());
    }
    {
        let engine = AppEngine::open(&path).unwrap();
        let items = engine.list_sub_items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].remarks, "仅备注普通分组");
        assert!(items[0].url.is_empty());
    }
}

#[tokio::test]
async fn subitem_crud_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    {
        let engine = AppEngine::open(&path).unwrap();
        let mut item = SubItem {
            remarks: "reopen".into(),
            url: "https://example.com/sub".into(),
            more_url: "https://example.com/more".into(),
            enabled: false,
            user_agent: "ua/1".into(),
            request_headers: Some(r#"{"A":"b"}"#.into()),
            filter: Some("Tokyo".into()),
            auto_update_interval: 30,
            convert_target: Some("clash".into()),
            prev_profile: Some("p".into()),
            next_profile: Some("n".into()),
            pre_socks_port: Some(11808),
            memo: Some("memo".into()),
            custom_core_type: Some(3),
            ..SubItem::default()
        };
        item = engine.save_sub_item(item).unwrap();
        assert!(!item.id.is_empty());
    }
    {
        let engine = AppEngine::open(&path).unwrap();
        let items = engine.list_sub_items().unwrap();
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.remarks, "reopen");
        assert_eq!(item.more_url, "https://example.com/more");
        assert!(!item.enabled);
        assert_eq!(item.user_agent, "ua/1");
        assert_eq!(item.filter.as_deref(), Some("Tokyo"));
        assert_eq!(item.auto_update_interval, 30);
        assert_eq!(item.convert_target.as_deref(), Some("clash"));
        assert_eq!(item.prev_profile.as_deref(), Some("p"));
        assert_eq!(item.next_profile.as_deref(), Some("n"));
        assert_eq!(item.pre_socks_port, Some(11808));
        assert_eq!(item.memo.as_deref(), Some("memo"));
        assert_eq!(item.custom_core_type, Some(3));
    }
}
