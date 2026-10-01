//! T16 WebDAV loopback tests. The mock server binds 127.0.0.1 on a port in
//! `11808..13000` and is reached directly (no environment proxy).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use application::webdav::{WebDavClient, WebDavConfig};
use tiny_http::{Response, Server};

const PROPFIND_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/v2rayN_backup/</D:href>
    <D:propstat><D:prop><D:getcontentlength>0</D:getcontentlength></D:prop></D:propstat>
  </D:response>
  <D:response>
    <D:href>/v2rayN_backup/backup.zip</D:href>
    <D:propstat><D:prop><D:getcontentlength>3</D:getcontentlength><D:getlastmodified>Fri, 02 Oct 2026 00:00:00 GMT</D:getlastmodified></D:prop></D:propstat>
  </D:response>
</D:multistatus>"#;

/// The OS-assigned port (always in the ephemeral range, never 10808).
fn bound_port(server: &Server) -> u16 {
    server
        .server_addr()
        .to_ip()
        .expect("expected an IP listen address")
        .port()
}

fn spawn_mock(require_auth: bool, delay: Duration) -> (u16, Arc<Mutex<Vec<u8>>>) {
    let server = Server::http("127.0.0.1:0").expect("bind mock");
    let port = bound_port(&server);
    let stored = Arc::new(Mutex::new(Vec::<u8>::new()));
    let stored_thread = Arc::clone(&stored);
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
            if require_auth {
                let authorized = request.headers().iter().any(|header| {
                    header.field.equiv("Authorization")
                        && header.value.as_str().starts_with("Basic ")
                });
                if !authorized {
                    let _ = request.respond(Response::empty(401));
                    continue;
                }
            }
            let method = request.method().as_str().to_string();
            let url = request.url().to_string();
            match method.as_str() {
                "PROPFIND" => {
                    let _ =
                        request.respond(Response::from_string(PROPFIND_XML).with_status_code(207));
                }
                "MKCOL" => {
                    let _ = request.respond(Response::empty(201));
                }
                "PUT" => {
                    let mut body = Vec::new();
                    {
                        let mut reader = request.as_reader();
                        let _ = std::io::Read::read_to_end(&mut reader, &mut body);
                    }
                    *stored_thread.lock().expect("lock") = body;
                    let _ = request.respond(Response::empty(201));
                }
                "GET" => {
                    let data = stored_thread.lock().expect("lock").clone();
                    if url.ends_with("backup.zip") && !data.is_empty() {
                        let _ = request.respond(Response::from_data(data).with_status_code(200));
                    } else {
                        let _ = request.respond(Response::empty(404));
                    }
                }
                _ => {
                    let _ = request.respond(Response::empty(405));
                }
            }
        }
    });
    (port, stored)
}

fn config(port: u16) -> WebDavConfig {
    WebDavConfig::new(
        format!("http://127.0.0.1:{port}"),
        "user",
        "secret-value-not-logged",
        "v2rayN_backup",
    )
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

#[test]
fn check_list_upload_download_roundtrip() {
    let (port, _stored) = spawn_mock(false, Duration::ZERO);
    let client = WebDavClient::new(config(port), Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    rt.block_on(async {
        let check = client.check().await.expect("check");
        assert!(!check.created_dir);
        assert_eq!(check.status, 207);

        let entries = client.list().await.expect("list");
        assert!(entries.iter().any(|e| e.href.ends_with("backup.zip")));
        assert!(entries.iter().any(|e| e.href.ends_with('/')));

        let payload = b"bundle-bytes".to_vec();
        let uploaded = client.upload(payload.clone()).await.expect("upload");
        assert_eq!(uploaded, payload.len() as u64);
        let downloaded = client.download().await.expect("download");
        assert_eq!(downloaded, payload);
    });
}

#[test]
fn unauthorized_is_classified_without_credentials() {
    let (port, _stored) = spawn_mock(true, Duration::ZERO);
    let anonymous = WebDavConfig::new(format!("http://127.0.0.1:{port}"), "", "", "v2rayN_backup");
    let client = WebDavClient::new(anonymous, Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    let error = rt.block_on(client.check()).expect_err("401");
    assert_eq!(error.code, "E_PERMISSION_DENIED");
    assert!(!error.detail.unwrap_or_default().contains("secret"));
}

#[test]
fn slow_server_times_out() {
    let (port, _stored) = spawn_mock(false, Duration::from_secs(3));
    let client = WebDavClient::new(config(port), Duration::from_millis(400), None).expect("client");
    let rt = runtime();
    let error = rt.block_on(client.check()).expect_err("timeout");
    assert_eq!(error.code, "E_TIMEOUT");
}
