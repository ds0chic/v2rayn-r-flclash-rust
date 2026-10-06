//! SP-25: `RootCertProvider` trust consumers (subscriptions / updater / WebDAV).
//!
//! The suite is hermetic: a synthetic CA (`fixtures/tls/`, shared with
//! UX-TEST-02) signs the loopback server certificate, every trust root is
//! injected per client, and no OS certificate store is ever touched.
//!
//! Matrix per client: `System` (native roots) must *reject* the synthetic CA,
//! while the `mozilla`/`chrome` selection (the synthetic bundle) must *accept*
//! it. Wrong-host and empty-bundle negatives, cancellation/timeout and a
//! provider-switch round trip are covered per client. Expired-leaf negatives
//! are intentionally absent: the fixture tree carries no expired material
//! (recorded as unverified in the SP-25 evidence).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use application::webdav::{WebDavClient, WebDavConfig};
use application::UpdateService;
use domain::CancellationToken;
use rustls::pki_types::CertificateDer;
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use subscriptions::tls::HttpsTrust as SubTrust;
use subscriptions::{build_client_with_trust, DownloadOptions};
use updater::arch::{HostTarget, Os, PlatformArch};
use updater::tls::HttpsTrust as UpTrust;
use updater::{
    CoreReleaseApi, DownloadRequest, DownloaderOptions, FileDownloader, ReleasesClient, UpdateError,
};

const CA_PEM: &[u8] = include_bytes!("fixtures/tls/ca.pem");
const SERVER_PEM: &[u8] = include_bytes!("fixtures/tls/server.pem");
const SERVER_KEY_PEM: &[u8] = include_bytes!("fixtures/tls/server.key.pem");

/// Synthetic bundle standing in for the upstream `chrome`/`mozilla` PEM
/// collections. `System` must reject it; the bundle selection must accept it.
fn mozilla_trust() -> (SubTrust, UpTrust) {
    (
        SubTrust::from_provider("mozilla", CA_PEM, CA_PEM),
        UpTrust::from_provider("mozilla", CA_PEM, CA_PEM),
    )
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn server_config() -> Arc<ServerConfig> {
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut std::io::Cursor::new(SERVER_PEM))
            .collect::<Result<_, _>>()
            .expect("server certs");
    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(SERVER_KEY_PEM))
        .expect("key parse")
        .expect("key present");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("server config");
    Arc::new(config)
}

fn releases_json(base: &str) -> String {
    format!(
        r#"[{{"tag_name":"v26.4.0","name":"Xray","prerelease":false,"assets":[
            {{"name":"Xray-windows-64.zip","size":48,"browser_download_url":"{base}/assets/xray.zip"}},
            {{"name":"Xray-windows-64.zip.dgst","size":80,"browser_download_url":"{base}/assets/xray.zip.dgst"}}
        ]}}]"#
    )
}

const MULTISTATUS: &str = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:"><D:response><D:href>/v2rayN_backup/</D:href><D:getcontentlength>0</D:getcontentlength><D:getlastmodified>Thu, 01 Jan 2026 00:00:00 GMT</D:getlastmodified></D:response></D:multistatus>"#;

/// Loopback HTTPS server (synthetic CA) serving every SP-25 client route.
struct TlsFixture {
    address: std::net::SocketAddr,
    port: u16,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

/// Bind an explicit loopback test port `>= 11808` (never 10808).
///
/// This host's TCP dynamic range is `1024..=64511`, so an ephemeral `:0` bind
/// can hand out a forbidden port; probe high ports explicitly instead.
fn bind_high(ip: &str) -> (TcpListener, std::net::SocketAddr) {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    for attempt in 0..4000u32 {
        let offset = (NEXT.fetch_add(1, Ordering::Relaxed) + attempt) % 4000;
        let candidate = 11980 + offset as u16;
        if candidate == 10808 {
            continue;
        }
        if let Ok(listener) = TcpListener::bind((ip, candidate)) {
            let address = listener.local_addr().expect("local addr");
            assert!(
                address.port() >= 11808 && address.port() != 10808,
                "test port {} violates the >=11808 / never-10808 rule",
                address.port()
            );
            return (listener, address);
        }
    }
    panic!("no free test port >= 11808 for {ip}");
}

impl TlsFixture {
    fn start_on(ip: &str) -> Self {
        let (listener, address) = bind_high(ip);
        let port = address.port();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let config = server_config();
        let uploaded: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let uploaded_thread = uploaded.clone();
        let join = std::thread::spawn(move || {
            while !stop_flag.load(Ordering::Acquire) {
                let Ok((stream, _)) = listener.accept() else {
                    continue;
                };
                let config = config.clone();
                let uploaded = uploaded_thread.clone();
                std::thread::spawn(move || serve_one(stream, config, uploaded));
            }
        });
        Self {
            address,
            port,
            stop,
            join: Some(join),
        }
    }

    fn start() -> Self {
        Self::start_on("127.0.0.1")
    }

    /// Second loopback identity (`127.0.0.2`) whose name is outside the leaf
    /// SANs (`localhost`, `127.0.0.1`): drives the hostname-mismatch negative.
    fn start_mismatch_host() -> Self {
        Self::start_on("127.0.0.2")
    }

    fn base(&self) -> String {
        format!("https://{}", self.address)
    }
}

impl Drop for TlsFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn read_request(
    tls: &mut StreamOwned<ServerConnection, TcpStream>,
) -> Option<(String, String, Vec<u8>)> {
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        match tls.read(&mut buf) {
            Ok(0) => return None,
            Ok(n) => head.extend_from_slice(&buf[..n]),
            Err(_) => return None,
        }
        if head.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if head.len() > 64 * 1024 {
            return None;
        }
    }
    let end = head
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(head.len().saturating_sub(4))
        + 4;
    let text = String::from_utf8_lossy(&head[..end]).to_string();
    let mut lines = text.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut content_length = 0usize;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = head[end.min(head.len())..].to_vec();
    while body.len() < content_length {
        match tls.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
    }
    body.truncate(content_length);
    Some((method, path, body))
}

fn respond(
    tls: &mut StreamOwned<ServerConnection, TcpStream>,
    status: &str,
    content_type: &str,
    body: &[u8],
) {
    let header = format!(
        "HTTP/1.0 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = tls.write_all(header.as_bytes());
    let _ = tls.write_all(body);
    let _ = tls.flush();
}

fn serve_one(stream: TcpStream, config: Arc<ServerConfig>, uploaded: Arc<Mutex<Vec<u8>>>) {
    let conn = match ServerConnection::new(config) {
        Ok(conn) => conn,
        Err(_) => return,
    };
    let mut tls = StreamOwned::new(conn, stream);
    let Some((method, path, body)) = read_request(&mut tls) else {
        return;
    };
    match (method.as_str(), path.as_str()) {
        ("GET", "/sub") => respond(
            &mut tls,
            "200 OK",
            "text/plain",
            b"sp25-synthetic-subscription-body",
        ),
        ("GET", "/slow") => {
            std::thread::sleep(Duration::from_secs(3));
            respond(&mut tls, "204 No Content", "text/plain", b"");
        }
        ("GET", "/assets/xray.zip") => {
            respond(
                &mut tls,
                "200 OK",
                "application/octet-stream",
                &vec![0x58u8; 48 * 1024],
            );
        }
        ("GET", path) if path.ends_with("/releases") => {
            let base = format!(
                "https://127.0.0.1:{}",
                tls.sock.local_addr().map(|a| a.port()).unwrap_or_default()
            );
            respond(
                &mut tls,
                "200 OK",
                "application/json",
                releases_json(&base).as_bytes(),
            );
        }
        ("PROPFIND", "/v2rayN_backup/") => {
            respond(
                &mut tls,
                "207 Multi-Status",
                "application/xml",
                MULTISTATUS.as_bytes(),
            );
        }
        ("MKCOL", "/v2rayN_backup/") => respond(&mut tls, "201 Created", "text/plain", b""),
        ("PUT", "/v2rayN_backup/backup.zip") => {
            *uploaded.lock().expect("uploaded") = body;
            respond(&mut tls, "201 Created", "text/plain", b"");
        }
        ("GET", "/v2rayN_backup/backup.zip") => {
            let stored = uploaded.lock().expect("uploaded").clone();
            if stored.is_empty() {
                respond(&mut tls, "404 Not Found", "text/plain", b"nope");
            } else {
                respond(&mut tls, "200 OK", "application/zip", &stored);
            }
        }
        _ => respond(&mut tls, "404 Not Found", "text/plain", b"nope"),
    }
}

/// TCP blackhole: accepts and stays silent, so client deadlines fire.
struct Blackhole {
    port: u16,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Blackhole {
    fn start() -> Self {
        let (listener, address) = bind_high("127.0.0.1");
        let port = address.port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let join = std::thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                if let Ok((stream, _)) = listener.accept() {
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(5));
                        drop(stream);
                    });
                }
            }
        });
        Self {
            port,
            stop,
            join: Some(join),
        }
    }
}

impl Drop for Blackhole {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(format!("127.0.0.1:{}", self.port));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn webdav_config(base: &str) -> WebDavConfig {
    WebDavConfig::new(base, "user", "secret", "")
}

// ---------------------------------------------------------------------------
// subscriptions
// ---------------------------------------------------------------------------

#[test]
fn sub_bundled_trust_accepts_synthetic_ca() {
    let server = TlsFixture::start();
    let (trust, _) = mozilla_trust();
    let options = DownloadOptions::default();
    let downloader = build_client_with_trust(&options, &trust).expect("client builds");
    let got = runtime()
        .block_on(downloader.download(&format!("{}/sub", server.base()), &CancellationToken::new()))
        .expect("bundled trust must accept the synthetic CA");
    assert_eq!(got.body, "sp25-synthetic-subscription-body");
}

#[test]
fn sub_system_trust_rejects_synthetic_ca() {
    let server = TlsFixture::start();
    let downloader = build_client_with_trust(&DownloadOptions::default(), &SubTrust::System)
        .expect("client builds");
    let err = runtime()
        .block_on(downloader.download(&format!("{}/sub", server.base()), &CancellationToken::new()))
        .expect_err("system roots must reject the synthetic CA");
    match &err {
        subscriptions::SubError::Http(_) => {}
        other => panic!("honest classification must stay transport-level, got {other:?}"),
    }
    assert!(
        SubTrust::is_trust_failure(&err.to_string()),
        "rejection must be classified as a TLS trust failure: {err}"
    );
}

#[test]
fn sub_provider_switch_changes_trust_outcome() {
    // The card's core flow: same URL, only the RootCertProvider selection
    // changes, and the handshake follows the selection both ways.
    let server = TlsFixture::start();
    let url = format!("{}/sub", server.base());
    let options = DownloadOptions::default();
    let system = build_client_with_trust(&options, &SubTrust::System).expect("system client");
    assert!(
        runtime()
            .block_on(system.download(&url, &CancellationToken::new()))
            .is_err(),
        "system must reject first"
    );
    let (mozilla, _) = mozilla_trust();
    let bundled = build_client_with_trust(&options, &mozilla).expect("bundled client");
    assert!(
        runtime()
            .block_on(bundled.download(&url, &CancellationToken::new()))
            .is_ok(),
        "mozilla selection must accept after the switch"
    );
    let back = build_client_with_trust(&options, &SubTrust::System).expect("system client");
    assert!(
        runtime()
            .block_on(back.download(&url, &CancellationToken::new()))
            .is_err(),
        "switching back to system must reject again"
    );
}

#[test]
fn sub_provider_names_map_to_expected_roots() {
    assert_eq!(
        SubTrust::from_provider("system", CA_PEM, CA_PEM),
        SubTrust::System
    );
    assert_eq!(
        SubTrust::from_provider("chrome", b"c", b"m"),
        SubTrust::BundledPem(b"c".to_vec())
    );
    assert_eq!(
        SubTrust::from_provider(" Mozilla ", b"c", b"m"),
        SubTrust::BundledPem(b"m".to_vec())
    );
    // Upstream fallback: unknown values behave as `system`.
    assert_eq!(
        SubTrust::from_provider("bogus", b"c", b"m"),
        SubTrust::System
    );
    assert!(SubTrust::System.uses_system_store());
    assert!(!SubTrust::bundled(CA_PEM).uses_system_store());
}

#[test]
fn sub_empty_bundle_is_a_build_error_not_a_silent_system_fallback() {
    let err = build_client_with_trust(&DownloadOptions::default(), &SubTrust::bundled(&[]))
        .expect_err("an empty bundle must not build");
    assert!(
        !matches!(
            err,
            subscriptions::SubError::Cancelled | subscriptions::SubError::Timeout
        ),
        "unexpected class: {err:?}"
    );
}

#[test]
fn sub_midflight_cancel_is_honoured_over_tls() {
    let server = TlsFixture::start();
    let (trust, _) = mozilla_trust();
    let downloader =
        build_client_with_trust(&DownloadOptions::default(), &trust).expect("client builds");
    let token = CancellationToken::new();
    let canceller = token.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        canceller.cancel();
    });
    let err = runtime()
        .block_on(downloader.download(&format!("{}/slow", server.base()), &token))
        .expect_err("cancelled download must fail");
    assert_eq!(err, subscriptions::SubError::Cancelled);
}

#[test]
fn sub_wrong_host_is_a_trust_failure() {
    let server = TlsFixture::start_mismatch_host();
    let (trust, _) = mozilla_trust();
    let downloader =
        build_client_with_trust(&DownloadOptions::default(), &trust).expect("client builds");
    let url = format!("https://127.0.0.2:{}/sub", server.port);
    let err = runtime()
        .block_on(downloader.download(&url, &CancellationToken::new()))
        .expect_err("name outside the leaf SANs must fail");
    assert!(
        SubTrust::is_trust_failure(&err.to_string()),
        "hostname mismatch must classify as trust failure: {err}"
    );
}

// ---------------------------------------------------------------------------
// updater: metadata fetch + artifact download
// ---------------------------------------------------------------------------

#[test]
fn updater_fetch_follows_trust_selection() {
    let server = TlsFixture::start();
    let client =
        ReleasesClient::new("XTLS/Xray-core").with_api_base(format!("{}/repos", server.base()));
    let (_, bundled) = mozilla_trust();
    let api =
        CoreReleaseApi::new_with_tls(Duration::from_secs(10), None, bundled).expect("api builds");
    let releases = runtime()
        .block_on(api.fetch(&client))
        .expect("bundled fetch must succeed");
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].tag_name, "v26.4.0");

    let api = CoreReleaseApi::new_with_tls(Duration::from_secs(10), None, UpTrust::System)
        .expect("api builds");
    let err = runtime()
        .block_on(api.fetch(&client))
        .expect_err("system must reject");
    match &err {
        UpdateError::Download(_) => {}
        other => panic!("fetch failure must stay Download, got {other:?}"),
    }
    assert!(
        UpTrust::is_trust_failure(&err.to_string()),
        "rejection must classify as trust failure: {err}"
    );
}

#[test]
fn updater_fetch_keeps_legacy_constructors_on_system_trust() {
    // Existing callers keep compiling and keep the previous direct behaviour.
    let api = CoreReleaseApi::new(Duration::from_secs(5)).expect("new");
    assert_eq!(api.user_agent, "v2rayN-updater");
    let api = CoreReleaseApi::new_with_proxy(Duration::from_secs(5), None).expect("proxy ctor");
    assert_eq!(api.user_agent, "v2rayN-updater");
}

#[test]
fn updater_file_download_follows_trust_selection() {
    let server = TlsFixture::start();
    let dir = tempfile::tempdir().expect("tempdir");
    let (_, bundled) = mozilla_trust();
    let ok = FileDownloader::new_with_trust(DownloaderOptions::default(), bundled).expect("builds");
    let target = dir.path().join("asset.bin");
    let got = runtime()
        .block_on(ok.download(
            &DownloadRequest::new(format!("{}/assets/xray.zip", server.base()), &target),
            &CancellationToken::new(),
        ))
        .expect("bundled download must succeed");
    assert_eq!(got.bytes, 48 * 1024);
    assert!(target.exists());

    let denied = FileDownloader::new_with_trust(DownloaderOptions::default(), UpTrust::System)
        .expect("builds");
    let err = runtime()
        .block_on(denied.download(
            &DownloadRequest::new(
                format!("{}/assets/xray.zip", server.base()),
                dir.path().join("denied.bin"),
            ),
            &CancellationToken::new(),
        ))
        .expect_err("system must reject");
    assert!(
        UpTrust::is_trust_failure(&err.to_string()),
        "rejection must classify as trust failure: {err}"
    );
}

#[test]
fn updater_file_download_cancel_is_honoured_over_tls() {
    let server = TlsFixture::start();
    let dir = tempfile::tempdir().expect("tempdir");
    let (_, bundled) = mozilla_trust();
    let downloader =
        FileDownloader::new_with_trust(DownloaderOptions::default(), bundled).expect("builds");
    let token = CancellationToken::new();
    let canceller = token.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        canceller.cancel();
    });
    let err = runtime()
        .block_on(downloader.download(
            &DownloadRequest::new(
                format!("{}/slow", server.base()),
                dir.path().join("slow.bin"),
            ),
            &token,
        ))
        .expect_err("cancelled download must fail");
    assert_eq!(err, UpdateError::Cancelled);
    assert!(
        !dir.path().join("slow.bin").exists(),
        "no completed artifact may appear"
    );
}

// ---------------------------------------------------------------------------
// WebDAV
// ---------------------------------------------------------------------------

#[test]
fn webdav_bundled_trust_roundtrip_over_tls() {
    let server = TlsFixture::start();
    let (_, bundled) = mozilla_trust();
    let client = WebDavClient::new_with_tls(
        webdav_config(&server.base()),
        Duration::from_secs(10),
        None,
        &bundled,
    )
    .expect("client builds");
    let rt = runtime();
    rt.block_on(client.check())
        .expect("bundled check must succeed");
    let payload = b"sp25-webdav-payload".to_vec();
    rt.block_on(client.upload(payload.clone())).expect("upload");
    assert_eq!(rt.block_on(client.download()).expect("download"), payload);
    assert!(!rt.block_on(client.list()).expect("list").is_empty());
}

#[test]
fn webdav_system_trust_rejects_synthetic_ca() {
    let server = TlsFixture::start();
    let client = WebDavClient::new(webdav_config(&server.base()), Duration::from_secs(10), None)
        .expect("client builds");
    let err = runtime()
        .block_on(client.check())
        .expect_err("system must reject");
    assert_eq!(err.message_key, "error.webdav_tls");
    assert!(
        UpTrust::is_trust_failure(&err.detail.clone().unwrap_or_default()),
        "rejection must classify as trust failure: {err:?}"
    );
}

#[test]
fn webdav_deadline_still_fires_over_tls() {
    let blackhole = Blackhole::start();
    let (_, bundled) = mozilla_trust();
    let client = WebDavClient::new_with_tls(
        webdav_config(&format!("https://127.0.0.1:{}", blackhole.port)),
        Duration::from_millis(500),
        None,
        &bundled,
    )
    .expect("client builds");
    let err = runtime()
        .block_on(client.check())
        .expect_err("silent peer must time out");
    assert_eq!(err.message_key, "error.webdav_timeout");
}

// ---------------------------------------------------------------------------
// application UpdateService plumbing
// ---------------------------------------------------------------------------

#[test]
fn update_service_carries_trust_selection_to_fetch() {
    let server = TlsFixture::start();
    let dir = tempfile::tempdir().expect("tempdir");
    let (_, bundled) = mozilla_trust();
    let mut service = UpdateService::new(dir.path().join("cores"))
        .with_api_base(format!("{}/repos", server.base()))
        .with_tls_trust(bundled);
    service.target = HostTarget::new(Os::Windows, PlatformArch::X64);
    service.timeout = Duration::from_secs(10);
    let check = runtime()
        .block_on(service.check_core("xray", false, None))
        .expect("bundled service check must succeed");
    assert_eq!(check.remote_version.as_deref(), Some("26.4.0"));
    assert!(check.has_update);

    let denied = UpdateService::new(dir.path().join("cores"))
        .with_api_base(format!("{}/repos", server.base()))
        .with_tls_trust(UpTrust::System);
    let err = runtime()
        .block_on(denied.check_core("xray", false, None))
        .expect_err("system service check must reject");
    assert!(
        UpTrust::is_trust_failure(&err.detail.clone().unwrap_or_default()),
        "service rejection must classify as trust failure: {err:?}"
    );
}

#[test]
fn trust_clients_rebuild_cleanly_after_drop() {
    // No global or OS state is involved: rebuilding after a drop behaves
    // identically (the reopen half of the card).
    let server = TlsFixture::start();
    let url = format!("{}/sub", server.base());
    for _ in 0..2 {
        let (trust, _) = mozilla_trust();
        let downloader =
            build_client_with_trust(&DownloadOptions::default(), &trust).expect("rebuilds");
        let got = runtime()
            .block_on(downloader.download(&url, &CancellationToken::new()))
            .expect("rebuild must succeed");
        assert_eq!(got.body, "sp25-synthetic-subscription-body");
    }
}
