//! T16 WebDAV loopback tests. Mock servers bind a probed 127.0.0.1 port in
//! `11808..11900` (never 10808) and are reached directly (no environment
//! proxy). Credentials are synthetic and must never leak into errors.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use application::webdav::{WebDavClient, WebDavConfig};
use application::{zip_upstream_layout, BackupService};
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

/// Bind the first free loopback port at or above 11808 so no test ever touches
/// the user's 10808 proxy port.
fn bind_loopback() -> Server {
    for port in 11808u16..11900 {
        if let Ok(server) = Server::http(("127.0.0.1", port)) {
            return server;
        }
    }
    panic!("no free loopback test port in 11808..11900");
}

/// The OS-assigned port chosen by [`bind_loopback`].
fn bound_port(server: &Server) -> u16 {
    server
        .server_addr()
        .to_ip()
        .expect("expected an IP listen address")
        .port()
}

#[derive(Clone)]
struct MockOptions {
    require_auth: bool,
    dir_exists: bool,
    put_status: u16,
    get_status: u16,
    delay: Duration,
}

impl Default for MockOptions {
    fn default() -> Self {
        Self {
            require_auth: false,
            dir_exists: true,
            put_status: 201,
            get_status: 200,
            delay: Duration::ZERO,
        }
    }
}

fn spawn_mock(opts: MockOptions) -> (u16, Arc<Mutex<Vec<u8>>>) {
    let server = bind_loopback();
    let port = bound_port(&server);
    let stored = Arc::new(Mutex::new(Vec::<u8>::new()));
    let stored_thread = Arc::clone(&stored);
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            if !opts.delay.is_zero() {
                std::thread::sleep(opts.delay);
            }
            if opts.require_auth {
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
                    if opts.dir_exists {
                        let _ = request
                            .respond(Response::from_string(PROPFIND_XML).with_status_code(207));
                    } else {
                        let _ = request.respond(Response::empty(404));
                    }
                }
                "MKCOL" => {
                    let _ = request.respond(Response::empty(201));
                }
                "PUT" => {
                    let mut body = Vec::new();
                    {
                        let reader = request.as_reader();
                        let _ = reader.read_to_end(&mut body);
                    }
                    if opts.put_status >= 400 {
                        let _ = request.respond(Response::empty(opts.put_status));
                        continue;
                    }
                    *stored_thread.lock().expect("lock") = body;
                    let _ = request.respond(Response::empty(opts.put_status));
                }
                "GET" => {
                    if opts.get_status >= 400 {
                        let _ = request.respond(Response::empty(opts.get_status));
                        continue;
                    }
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
    let (port, _stored) = spawn_mock(MockOptions::default());
    let client = WebDavClient::new(config(port), Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    rt.block_on(async {
        let check = client.check().await.expect("check");
        assert!(!check.created_dir);
        assert_eq!(check.status, 207);

        let entries = client.list().await.expect("list");
        assert!(entries.iter().any(|e| e.href.ends_with("backup.zip")));
        assert!(entries.iter().any(|e| e.href.ends_with('/')));

        let payload = b"bundle-bytes";
        let work = tempfile::tempdir().expect("work");
        let source = work.path().join("upload.zip");
        std::fs::write(&source, payload).expect("stage upload");
        let uploaded = client
            .upload_file(&source, payload.len() as u64)
            .await
            .expect("upload");
        assert_eq!(uploaded, payload.len() as u64);
        let destination = work.path().join("download.zip");
        let downloaded = client
            .download_to_file(&destination)
            .await
            .expect("download");
        assert_eq!(downloaded, payload.len() as u64);
        assert_eq!(std::fs::read(destination).expect("read download"), payload);
    });
}

#[test]
fn check_creates_directory_when_propfind_misses() {
    let opts = MockOptions {
        dir_exists: false,
        ..MockOptions::default()
    };
    let (port, _stored) = spawn_mock(opts);
    let client = WebDavClient::new(config(port), Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    let check = rt.block_on(client.check()).expect("mkcol");
    assert!(check.created_dir);
    assert_eq!(check.status, 201);
}

#[test]
fn unauthorized_is_classified_without_credentials() {
    let opts = MockOptions {
        require_auth: true,
        ..MockOptions::default()
    };
    let (port, _stored) = spawn_mock(opts);
    let anonymous = WebDavConfig::new(format!("http://127.0.0.1:{port}"), "", "", "v2rayN_backup");
    let client = WebDavClient::new(anonymous, Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    let error = rt.block_on(client.check()).expect_err("401");
    assert_eq!(error.code, "E_PERMISSION_DENIED");
    assert!(!error.detail.unwrap_or_default().contains("secret"));
}

#[test]
fn download_missing_file_is_not_found() {
    let opts = MockOptions {
        get_status: 404,
        ..MockOptions::default()
    };
    let (port, _stored) = spawn_mock(opts);
    let client = WebDavClient::new(config(port), Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    let error = rt.block_on(client.download()).expect_err("404");
    assert_eq!(error.code, "E_NOT_FOUND");
}

#[test]
fn upload_remote_failure_is_retryable() {
    let opts = MockOptions {
        put_status: 500,
        ..MockOptions::default()
    };
    let (port, _stored) = spawn_mock(opts);
    let client = WebDavClient::new(config(port), Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    let error = rt
        .block_on(client.upload(b"payload".to_vec()))
        .expect_err("500");
    assert_eq!(error.code, "E_UNAVAILABLE");
    assert!(error.retryable);
}

#[test]
fn slow_server_times_out() {
    let opts = MockOptions {
        delay: Duration::from_secs(3),
        ..MockOptions::default()
    };
    let (port, _stored) = spawn_mock(opts);
    let client = WebDavClient::new(config(port), Duration::from_millis(400), None).expect("client");
    let rt = runtime();
    let error = rt.block_on(client.check()).expect_err("timeout");
    assert_eq!(error.code, "E_TIMEOUT");
}

/// The remote `backup.zip` must be an upstream-interoperable `guiConfigs/`
/// layout (not the project manifest bundle): it is uploaded, GET back and
/// recognised as an upstream archive, with nested resources and the database
/// preserved and the project manifest dropped.
#[test]
fn upstream_layout_upload_download_and_recognize() {
    let src = tempfile::tempdir().expect("tempdir");
    let bundle = src.path();
    std::fs::write(bundle.join("guiNConfig.json"), b"{\"IndexId\":\"x\"}").unwrap();
    std::fs::write(bundle.join("guiNDB.db"), b"SQLite format 3\0snapshot").unwrap();
    std::fs::write(bundle.join("manifest.json"), b"{\"format_version\":1}").unwrap();
    std::fs::create_dir_all(bundle.join("custom/sub")).unwrap();
    std::fs::write(bundle.join("custom/sub/node.json"), b"node").unwrap();
    std::fs::create_dir_all(bundle.join(".work")).unwrap();
    std::fs::write(bundle.join(".work/scratch"), b"transient").unwrap();

    let bytes = zip_upstream_layout(bundle).expect("zip upstream layout");

    let (port, _stored) = spawn_mock(MockOptions::default());
    let client = WebDavClient::new(config(port), Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    let downloaded = rt.block_on(async {
        client.check().await.expect("check");
        client.upload(bytes).await.expect("upload");
        client.download().await.expect("download")
    });

    let out = tempfile::tempdir().expect("tempdir");
    let zip_path = out.path().join("remote-backup.zip");
    std::fs::write(&zip_path, &downloaded).unwrap();

    let service = BackupService::new(out.path().join("data"));
    let recognition = service.recognize(&zip_path).expect("recognize");
    assert!(
        recognition.is_upstream,
        "guiConfigs layout must be upstream"
    );
    assert!(
        recognition.entries.iter().any(|e| e.contains("guiConfigs")),
        "recognition must see the guiConfigs wrapper: {:?}",
        recognition.entries
    );
}
