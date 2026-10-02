//! T18b end-to-end: the real `RuntimePlan` runs the real pinned kernels.
//!
//! Every case assembles state through `AppEngine` (`save_profile` +
//! settings/routing/DNS + `build_runtime_plan`), writes the generated body to
//! a temp file and starts the pinned `xray.exe` / `sing-box.exe` directly.
//! Traffic assertions go through the generated mixed inbound (SOCKS5 and
//! HTTP) against a loopback origin server. Ports are all `>= 11808` (never
//! 10808); each case enforces a 60 s budget and only ever kills the PID it
//! started. TUN is never enabled (out of scope for this round).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use application::{AppEngine, NullRuntimeClient};
use domain::{ConfigType, CoreType, DesiredRevision, Profile};

const XRAY_EXE: &str = "tools/cores/xray/v26.3.27/xray.exe";
const SINGBOX_EXE: &str = "tools/cores/singbox/v1.14.2/sing-box.exe";
const BUDGET: Duration = Duration::from_secs(60);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn free_port(mut base: u16) -> u16 {
    loop {
        assert!(base < 60000, "port scan exhausted");
        if base == 10808 {
            base += 1;
            continue;
        }
        if TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return base;
        }
        base += 1;
    }
}

/// Loopback origin server answering `200 t18b-ok` to any request.
struct Origin {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    port: u16,
}

impl Origin {
    fn start(port: u16) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind origin");
        listener.set_nonblocking(true).expect("origin nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let body = b"t18b-ok";
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(body);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            stop,
            handle: Some(handle),
            port,
        }
    }
}

impl Drop for Origin {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Unblock the accept loop with one connection.
        let _ = TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", self.port).parse().unwrap(),
            Duration::from_millis(300),
        );
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// A spawned kernel; kills only its own PID on drop.
struct Kernel {
    child: Option<Child>,
    pid: u32,
}

impl Kernel {
    fn spawn(exe: &str, config: &std::path::Path) -> Self {
        let root = repo_root();
        let binary = root.join(exe);
        assert!(
            binary.exists(),
            "missing pinned binary {}",
            binary.display()
        );
        let log_path = config.with_extension("stderr.log");
        let log_file = std::fs::File::create(&log_path).expect("create stderr log");
        let mut child = Command::new(&binary)
            .arg("run")
            .arg("-c")
            .arg(config)
            .current_dir(binary.parent().unwrap())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log_file)
            .spawn()
            .expect("spawn kernel");
        let pid = child.id();
        // Fail fast when the core exits during startup.
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(Some(status)) = child.try_wait() {
            let tail = std::fs::read_to_string(&log_path)
                .map(|text| {
                    let lines: Vec<&str> = text.lines().collect();
                    let skip = lines.len().saturating_sub(15);
                    lines[skip..].join("\n")
                })
                .unwrap_or_default();
            panic!("kernel exited during startup: {status}\n--- stderr tail ---\n{tail}");
        }
        Self {
            child: Some(child),
            pid,
        }
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Kernel {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_port(port: u16, timeout: Duration) {
    let start = Instant::now();
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "port {port} never opened within {timeout:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn assert_port_closed(port: u16) {
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_ok() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "port {port} still open after stop"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Persistent temp engine: the production `open()` path, so the generated
/// log/cache directories are absolute and really created.
fn engine_in(dir: &tempfile::TempDir) -> AppEngine {
    AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new()))
        .expect("open engine")
}

fn engine() -> (tempfile::TempDir, AppEngine) {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = engine_in(&dir);
    (dir, engine)
}

fn save(engine: &AppEngine, profile: Profile) -> Profile {
    let revision = engine.desired_revision();
    engine
        .save_profile(profile, DesiredRevision::new(revision))
        .expect("save profile")
        .0
}

fn vless_leaf(id: &str, core: CoreType) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::Vless,
        core_type: Some(core),
        remarks: id.into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    profile.proto_extra.vless_encryption = Some("none".into());
    profile
}

fn set_base_port(engine: &AppEngine, port: u16) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = port as i32;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

/// Route `ip` matches to the `direct` (freedom) outbound.
fn route_ip_direct(engine: &AppEngine, ip: &str) {
    let mut profile = domain::RoutingProfile {
        remarks: "t18b-e2e".into(),
        ..Default::default()
    };
    profile
        .set_rules(&[domain::RoutingRule {
            id: "r1".into(),
            outbound_tag: Some("direct".into()),
            ip: Some(vec![ip.into()]),
            enabled: true,
            rule_type: Some(domain::RuleType::Routing),
            ..Default::default()
        }])
        .expect("set rules");
    let saved = engine.save_routing(profile).expect("save routing");
    engine
        .set_default_routing(&saved.id)
        .expect("set default routing");
}

fn plan_config(engine: &AppEngine, target: &str) -> String {
    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan(target, revision)
        .expect("build runtime plan");
    match &plan.target.config {
        domain::runtime_plan::ConfigSource::Inline { body } => {
            assert!(!body.contains("10808"), "plan must never emit 10808");
            body.clone()
        }
        other => panic!("expected inline config, got {other:?}"),
    }
}

fn write_config(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write config");
    path
}

/// SOCKS5 CONNECT to `127.0.0.1:target`, then `GET /`, returning the raw reply.
fn socks5_get(proxy: u16, target: u16, auth: Option<(&str, &str)>) -> std::io::Result<String> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{proxy}").parse().unwrap(),
        Duration::from_secs(5),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    if let Some((user, pass)) = auth {
        stream.write_all(&[0x05, 0x01, 0x02])?;
        let mut method = [0u8; 2];
        stream.read_exact(&mut method)?;
        assert_eq!(method, [0x05, 0x02], "socks auth offered");
        let mut request = vec![0x01, user.len() as u8];
        request.extend_from_slice(user.as_bytes());
        request.push(pass.len() as u8);
        request.extend_from_slice(pass.as_bytes());
        stream.write_all(&request)?;
        let mut status = [0u8; 2];
        stream.read_exact(&mut status)?;
        if status != [0x01, 0x00] {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "socks auth rejected",
            ));
        }
    } else {
        stream.write_all(&[0x05, 0x01, 0x00])?;
        let mut method = [0u8; 2];
        stream.read_exact(&mut method)?;
        if method != [0x05, 0x00] {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("socks method rejected: {method:?}"),
            ));
        }
    }
    let mut request = vec![0x05, 0x01, 0x00, 0x01, 127, 0, 0, 1];
    request.extend_from_slice(&target.to_be_bytes());
    stream.write_all(&request)?;
    let mut reply = [0u8; 10];
    stream.read_exact(&mut reply)?;
    if reply[1] != 0x00 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("socks connect refused: {:02x?}", &reply[..4]),
        ));
    }
    stream.write_all(
        format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{target}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn http_proxy_get(proxy: u16, host: &str, port: u16) -> std::io::Result<String> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{proxy}").parse().unwrap(),
        Duration::from_secs(5),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(
        format!(
            "GET http://{host}:{port}/ HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
        )
        .as_bytes(),
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

#[test]
fn xray_plan_serves_local_http_through_mixed_inbound() {
    let start = Instant::now();
    let origin_port = free_port(11980);
    let _origin = Origin::start(origin_port);
    let proxy_port = free_port(11908);

    let (_keep, engine) = engine();
    save(&engine, vless_leaf("n1", CoreType::Xray));
    set_base_port(&engine, proxy_port);
    route_ip_direct(&engine, "127.0.0.1");
    let body = plan_config(&engine, "n1");
    let value: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(value["inbounds"][0]["port"], proxy_port);
    assert_eq!(value["inbounds"][0]["listen"], "127.0.0.1");

    let dir = tempfile::tempdir().expect("tempdir");
    let config = write_config(dir.path(), "xray.json", &body);
    let _kernel = Kernel::spawn(XRAY_EXE, &config);
    eprintln!("xray pid={} proxy=127.0.0.1:{proxy_port}", _kernel.pid);
    wait_port(proxy_port, Duration::from_secs(20));

    let via_socks = socks5_get(proxy_port, origin_port, None).expect("socks GET");
    assert!(
        via_socks.contains("200") && via_socks.contains("t18b-ok"),
        "unexpected socks reply: {via_socks}"
    );
    let via_http = http_proxy_get(proxy_port, "127.0.0.1", origin_port).expect("http GET");
    assert!(
        via_http.contains("200") && via_http.contains("t18b-ok"),
        "unexpected http reply: {via_http}"
    );
    assert!(start.elapsed() < BUDGET, "exceeded 60s budget");
}

#[test]
fn xray_socks_and_http_outbounds_reach_local_service() {
    let start = Instant::now();
    let origin_port = free_port(11981);
    let _origin = Origin::start(origin_port);
    // Upstream core: mixed inbound + freedom outbound.
    let upstream_port = free_port(11982);
    let (_keep_up, upstream_engine) = engine();
    save(&upstream_engine, vless_leaf("up-node", CoreType::Xray));
    set_base_port(&upstream_engine, upstream_port);
    route_ip_direct(&upstream_engine, "127.0.0.1");
    let upstream_body = plan_config(&upstream_engine, "up-node");
    let dir = tempfile::tempdir().expect("tempdir");
    let upstream_config = write_config(dir.path(), "upstream.json", &upstream_body);
    let _upstream = Kernel::spawn(XRAY_EXE, &upstream_config);
    wait_port(upstream_port, Duration::from_secs(20));

    for (node_id, config_type) in [
        ("via-socks", ConfigType::Socks),
        ("via-http", ConfigType::Http),
    ] {
        let downstream_port = free_port(if config_type == ConfigType::Socks {
            11909
        } else {
            11915
        });
        let (_keep, engine) = engine();
        save(
            &engine,
            Profile {
                index_id: node_id.into(),
                config_type,
                core_type: Some(CoreType::Xray),
                remarks: node_id.into(),
                address: "127.0.0.1".into(),
                port: upstream_port as i32,
                ..Default::default()
            },
        );
        set_base_port(&engine, downstream_port);
        route_ip_direct(&engine, "127.0.0.1");
        let body = plan_config(&engine, node_id);
        let config = write_config(dir.path(), &format!("downstream-{node_id}.json"), &body);
        let _downstream = Kernel::spawn(XRAY_EXE, &config);
        eprintln!(
            "downstream {node_id} pid={} port={downstream_port}",
            _downstream.pid
        );
        wait_port(downstream_port, Duration::from_secs(20));
        let reply = socks5_get(downstream_port, origin_port, None).expect("downstream socks GET");
        assert!(
            reply.contains("200") && reply.contains("t18b-ok"),
            "downstream {node_id} reply: {reply}"
        );
    }
    assert!(start.elapsed() < BUDGET, "exceeded 60s budget");
}

#[test]
fn singbox_plan_serves_local_http() {
    let start = Instant::now();
    let origin_port = free_port(11983);
    let _origin = Origin::start(origin_port);
    let proxy_port = free_port(11910);

    let (_keep, engine) = engine();
    save(&engine, vless_leaf("sb1", CoreType::SingBox));
    set_base_port(&engine, proxy_port);
    route_ip_direct(&engine, "127.0.0.1");
    let body = plan_config(&engine, "sb1");
    let value: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(value["inbounds"][0]["listen_port"], proxy_port);

    let dir = tempfile::tempdir().expect("tempdir");
    let config = write_config(dir.path(), "singbox.json", &body);
    let _kernel = Kernel::spawn(SINGBOX_EXE, &config);
    eprintln!("sing-box pid={} proxy=127.0.0.1:{proxy_port}", _kernel.pid);
    wait_port(proxy_port, Duration::from_secs(25));

    let via_socks = socks5_get(proxy_port, origin_port, None).expect("socks GET");
    assert!(
        via_socks.contains("200") && via_socks.contains("t18b-ok"),
        "unexpected sing-box reply: {via_socks}"
    );
    assert!(start.elapsed() < BUDGET, "exceeded 60s budget");
}

#[test]
fn routing_domain_rule_direct_vs_block_differs() {
    let start = Instant::now();
    let origin_port = free_port(11984);
    let _origin = Origin::start(origin_port);
    let proxy_port = free_port(11911);
    let dir = tempfile::tempdir().expect("tempdir");

    for (round, outbound) in [("direct", "direct"), ("block", "block")] {
        let (_keep, engine) = engine();
        save(&engine, vless_leaf("n1", CoreType::Xray));
        set_base_port(&engine, proxy_port);
        let mut profile = domain::RoutingProfile {
            remarks: "t18b-domain".into(),
            ..Default::default()
        };
        profile
            .set_rules(&[domain::RoutingRule {
                id: "r1".into(),
                outbound_tag: Some(outbound.into()),
                domain: Some(vec!["full:localhost".into()]),
                enabled: true,
                rule_type: Some(domain::RuleType::Routing),
                ..Default::default()
            }])
            .expect("set rules");
        let saved = engine.save_routing(profile).expect("save routing");
        engine
            .set_default_routing(&saved.id)
            .expect("set default routing");
        let body = plan_config(&engine, "n1");
        assert!(
            body.contains("localhost") && body.contains(outbound),
            "round {round}: rule missing"
        );
        let config = write_config(dir.path(), &format!("xray-{round}.json"), &body);
        let kernel = Kernel::spawn(XRAY_EXE, &config);
        wait_port(proxy_port, Duration::from_secs(20));
        let outcome = http_proxy_get(proxy_port, "localhost", origin_port);
        drop(kernel);
        assert_port_closed(proxy_port);
        match round {
            "direct" => {
                let reply = outcome.expect("direct round must succeed");
                assert!(
                    reply.contains("200") && reply.contains("t18b-ok"),
                    "direct reply: {reply}"
                );
            }
            _ => {
                let blocked = match outcome {
                    Err(_) => true,
                    Ok(reply) => !reply.contains("t18b-ok"),
                };
                assert!(blocked, "block round must not serve the origin");
            }
        }
    }
    assert!(start.elapsed() < BUDGET, "exceeded 60s budget");
}

#[test]
fn settings_port_change_and_auth_apply_to_listener() {
    let start = Instant::now();
    let origin_port = free_port(11985);
    let _origin = Origin::start(origin_port);
    let dir = tempfile::tempdir().expect("tempdir");

    // Port change: the new listener serves, the old one stays shut.
    let first = free_port(11912);
    let (_keep, engine) = engine();
    save(&engine, vless_leaf("n1", CoreType::Xray));
    set_base_port(&engine, first);
    route_ip_direct(&engine, "127.0.0.1");
    let config = write_config(dir.path(), "xray-a.json", &plan_config(&engine, "n1"));
    let kernel_a = Kernel::spawn(XRAY_EXE, &config);
    wait_port(first, Duration::from_secs(20));
    let reply = socks5_get(first, origin_port, None).expect("first listener");
    assert!(reply.contains("t18b-ok"), "first reply: {reply}");
    drop(kernel_a);
    assert_port_closed(first);

    // Re-apply on a new port with LAN auth: unauthenticated use fails,
    // authenticated use succeeds on the +2 (socks3) listener.
    let second = free_port(11913);
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    if let Some(inbound) = settings.inbound.first_mut() {
        inbound.local_port = second as i32;
        inbound.allow_lan_conn = true;
        inbound.new_port4_lan = true;
        inbound.user = "t18b-user".into();
        inbound.pass = "t18b-pass".into();
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
    let body = plan_config(&engine, "n1");
    assert!(body.contains("t18b-user"), "auth user missing");
    let config = write_config(dir.path(), "xray-b.json", &body);
    let _kernel_b = Kernel::spawn(XRAY_EXE, &config);
    wait_port(second, Duration::from_secs(20));
    let authed_port = second + 2;
    wait_port(authed_port, Duration::from_secs(20));
    let denied = socks5_get(authed_port, origin_port, None);
    assert!(
        denied.is_err(),
        "unauthenticated use must fail, got {denied:?}"
    );
    let allowed = socks5_get(authed_port, origin_port, Some(("t18b-user", "t18b-pass")))
        .expect("authenticated GET");
    assert!(
        allowed.contains("200") && allowed.contains("t18b-ok"),
        "authenticated reply: {allowed}"
    );
    let wrong = socks5_get(authed_port, origin_port, Some(("t18b-user", "wrong")));
    assert!(wrong.is_err(), "wrong password must fail, got {wrong:?}");
    assert!(start.elapsed() < BUDGET, "exceeded 60s budget");
}

#[test]
fn dns_switch_is_reflected_in_generated_body() {
    let (_keep, engine) = engine();
    save(&engine, vless_leaf("n1", CoreType::Xray));
    set_base_port(&engine, free_port(11916));
    // The SimpleDNS builtin always contributes its own servers; the
    // per-core row switch is proven with a unique marker address.
    let plain = plan_config(&engine, "n1");
    assert!(
        !plain.contains("9.9.9.11"),
        "disabled DNS row must not leak its servers"
    );
    // Same flow as the DNS window: edit the stored row in place (one row
    // per core), rather than adding a second row for the core.
    let mut row = engine
        .get_dns_for_core(CoreType::Xray)
        .expect("get dns")
        .expect("builtin dns row");
    row.enabled = true;
    row.normal_dns = Some(r#"{"servers":["https://9.9.9.11/dns-query"]}"#.into());
    engine.save_dns(row).expect("save dns");
    let with_dns = plan_config(&engine, "n1");
    assert!(
        with_dns.contains("9.9.9.11"),
        "enabled DNS must reach the plan"
    );
}

#[test]
fn stop_releases_listener_port() {
    let start = Instant::now();
    let origin_port = free_port(11986);
    let _origin = Origin::start(origin_port);
    let proxy_port = free_port(11914);

    let (_keep, engine) = engine();
    save(&engine, vless_leaf("n1", CoreType::Xray));
    set_base_port(&engine, proxy_port);
    route_ip_direct(&engine, "127.0.0.1");
    let dir = tempfile::tempdir().expect("tempdir");
    let config = write_config(dir.path(), "xray-stop.json", &plan_config(&engine, "n1"));
    let mut kernel = Kernel::spawn(XRAY_EXE, &config);
    eprintln!("stop-case xray pid={}", kernel.pid);
    wait_port(proxy_port, Duration::from_secs(20));
    let reply = socks5_get(proxy_port, origin_port, None).expect("serving");
    assert!(reply.contains("t18b-ok"));
    // Stopping the managed process releases the port (the net-host journal
    // `Finalized` line belongs to the IPC path covered by T03 case a).
    kernel.stop();
    assert_port_closed(proxy_port);
    engine.stop_runtime().expect("stop runtime is idempotent");
    assert!(start.elapsed() < BUDGET, "exceeded 60s budget");
}
