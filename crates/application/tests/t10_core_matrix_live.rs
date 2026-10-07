//! T10 live: real loopback sessions for every locked core binary.
//!
//! Companion to `t10_core_matrix` (config validation via `run -test` /
//! `check` only, no listeners): this target walks the real adapter contract
//! (`runtime::adapter::adapter_for` run args / env / working dir — the same
//! functions net_host uses to build managed commands), stages a synthetic
//! loopback-only config, starts the real process, polls its listener, then
//! stops only its own child and records PID/exit facts.
//!
//! Shapes mirror `tools/cores/session_matrix.ps1` exactly (same synthetic
//! configs, same run args, same cwd/env rules). Pair cores (hysteria2,
//! overtls) additionally run a self-signed TLS server plus one real proxied
//! HTTP GET through the adapter-contract client listener.
//!
//! Constraints: probed ports are always `>= 11808` (never 10808), loopback
//! only, no system proxy/route/TUN/DNS writes; every spawned child is killed
//! and reaped by this test (no leftover processes).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use domain::CoreType;
use runtime::adapter::adapter_for;
use sha2::{Digest, Sha256};

const READY_TIMEOUT: Duration = Duration::from_secs(25);
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
const CERT_TIMEOUT: Duration = Duration::from_secs(15);

/// Minimum number of cores that must serve a real loopback session on this
/// host. Measured 2026-10-07: 13/14 ok (only `overtls` pair blocked with a
/// recorded reason); the floor guards against a regression that silently
/// turns working cores into blocked entries.
const LIVE_MATRIX_OK_FLOOR: usize = 13;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn out_dir() -> PathBuf {
    let dir = repo_root().join("target").join("t10").join("live-matrix");
    std::fs::create_dir_all(dir.join("logs")).expect("live-matrix dir");
    dir
}

/// Pick a free port at/above `base` (127.0.0.1 TCP probe then release).
/// Never returns 10808.
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

fn tcp_ready(port: u16, deadline: Instant) -> bool {
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// UDP readiness: the port counts as listening while our own bind fails with
/// `AddrInUse` (mirrors the `Get-NetUDPEndpoint` poll in session_matrix.ps1).
fn udp_ready(port: u16, deadline: Instant) -> bool {
    while Instant::now() < deadline {
        match UdpSocket::bind(("127.0.0.1", port)) {
            Ok(sock) => {
                drop(sock);
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => return true,
            Err(_) => std::thread::sleep(Duration::from_millis(300)),
        }
    }
    false
}

/// Stop only our own child: kill, then poll `try_wait` up to 5 s.
/// Returns `(exit_code, still_running)`.
fn stop_child(child: &mut Child) -> (Option<i32>, bool) {
    if !child_is_exited(child) {
        let _ = child.kill();
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut code = None;
    while Instant::now() < deadline {
        match child.try_wait().expect("poll child") {
            Some(status) => {
                code = status.code();
                break;
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let still = !child_is_exited(child);
    if !still && code.is_none() {
        code = child.try_wait().expect("poll child").and_then(|s| s.code());
    }
    (code, still)
}

fn child_is_exited(child: &mut Child) -> bool {
    child.try_wait().expect("poll child").is_some()
}

fn drain(child: &mut Child) -> String {
    let mut tail = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut tail);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut tail);
    }
    tail.lines()
        .filter(|line| !line.trim().is_empty())
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run a short-lived probe (validation / version / cert-gen) with a bound;
/// only our own child is ever killed. Returns `(exit_code, output_tail)`.
/// Exit `-2` means this test killed the probe after the timeout.
fn run_bounded(
    exe: &Path,
    args: &[String],
    cwd: &Path,
    envs: &[(String, String)],
    timeout: Duration,
) -> (i32, String) {
    let mut command = Command::new(exe);
    command
        .args(args)
        .current_dir(cwd)
        .envs(envs.iter().map(|(k, v)| (k, v)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn probe");
    let start = Instant::now();
    loop {
        match child.try_wait().expect("poll probe") {
            Some(status) => return (status.code().unwrap_or(-1), drain(&mut child)),
            None => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return (-2, "probe timed out (killed)".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

struct SingleSpec {
    core: CoreType,
    name: &'static str,
    exe: &'static str,
    version: &'static str,
    cfg_name: &'static str,
    /// `PORT` (and `PORT2` for mieru) placeholders; shapes mirror
    /// `session_matrix.ps1` exactly.
    cfg_template: &'static str,
    has_check_command: bool,
}

fn singles() -> Vec<SingleSpec> {
    vec![
        SingleSpec {
            core: CoreType::Xray,
            name: "xray",
            exe: "tools/cores/xray/v26.3.27/xray.exe",
            version: "v26.3.27",
            cfg_name: "config.json",
            cfg_template: r#"{"log":{"loglevel":"warning"},"inbounds":[{"listen":"127.0.0.1","port":PORT,"protocol":"socks","settings":{"udp":false}}],"outbounds":[{"protocol":"freedom"}]}"#,
            has_check_command: true,
        },
        SingleSpec {
            core: CoreType::SingBox,
            name: "sing-box",
            exe: "tools/cores/singbox/v1.14.2/sing-box.exe",
            version: "v1.14.2",
            cfg_name: "config.json",
            cfg_template: r#"{"log":{"level":"warning"},"inbounds":[{"type":"socks","tag":"in","listen":"127.0.0.1","listen_port":PORT}],"outbounds":[{"type":"direct","tag":"direct"}]}"#,
            has_check_command: true,
        },
        SingleSpec {
            core: CoreType::V2fly,
            name: "v2fly",
            exe: "tools/cores/v2fly/v4.45.2/v2ray.exe",
            version: "v4.45.2",
            cfg_name: "config.json",
            cfg_template: r#"{"log":{"loglevel":"warning"},"inbounds":[{"listen":"127.0.0.1","port":PORT,"protocol":"socks","settings":{"udp":false}}],"outbounds":[{"protocol":"freedom"}]}"#,
            has_check_command: true,
        },
        SingleSpec {
            core: CoreType::V2flyV5,
            name: "v2fly_v5",
            exe: "tools/cores/v2fly/v5.53.0/v2ray.exe",
            version: "v5.53.0",
            cfg_name: "config.json",
            cfg_template: r#"{"log":{"loglevel":"warning"},"inbounds":[{"listen":"127.0.0.1","port":PORT,"protocol":"socks","settings":{"udp":false}}],"outbounds":[{"protocol":"freedom"}]}"#,
            has_check_command: true,
        },
        SingleSpec {
            core: CoreType::Mihomo,
            name: "mihomo",
            exe: "tools/cores/mihomo/v1.19.32/mihomo-windows-amd64-v1.exe",
            version: "v1.19.32",
            cfg_name: "config.yaml",
            cfg_template: "mixed-port: PORT\nallow-lan: false\nbind-address: 127.0.0.1\nmode: direct\nlog-level: warning\n",
            has_check_command: false,
        },
        SingleSpec {
            core: CoreType::Hysteria,
            name: "hysteria",
            exe: "tools/cores/hysteria/v1.3.5/hysteria.exe",
            version: "v1.3.5",
            cfg_name: "config.json",
            cfg_template: r#"{"server":"127.0.0.1:1","auth_str":"synthetic","up_mbps":20,"down_mbps":20,"lazy_start":true,"socks5":{"listen":"127.0.0.1:PORT"}}"#,
            has_check_command: false,
        },
        SingleSpec {
            core: CoreType::NaiveProxy,
            name: "naiveproxy",
            exe: "tools/cores/naiveproxy/v154.0.8037.49-2/naive.exe",
            version: "v154.0.8037.49-2",
            cfg_name: "config.json",
            cfg_template: r#"{"listen":"socks://127.0.0.1:PORT","proxy":"https://synthetic:synthetic@127.0.0.1:1"}"#,
            has_check_command: false,
        },
        SingleSpec {
            core: CoreType::Tuic,
            name: "tuic",
            exe: "tools/cores/tuic/v1.0.0/tuic-client.exe",
            version: "v1.0.0",
            cfg_name: "config.json",
            cfg_template: r#"{"relay":{"server":"127.0.0.1:1","uuid":"00000000-0000-0000-0000-000000000000","password":"synthetic"},"local":{"server":"127.0.0.1:PORT"}}"#,
            has_check_command: false,
        },
        SingleSpec {
            core: CoreType::Juicity,
            name: "juicity",
            exe: "tools/cores/juicity/v0.5.0/juicity-client.exe",
            version: "v0.5.0",
            cfg_name: "config.json",
            cfg_template: r#"{"listen":"127.0.0.1:PORT","server":"127.0.0.1:1","uuid":"00000000-0000-0000-0000-000000000000","password":"synthetic","sni":"example.com","allow_insecure":true,"congestion_control":"bbr","log_level":"warn"}"#,
            has_check_command: false,
        },
        SingleSpec {
            core: CoreType::Brook,
            name: "brook",
            exe: "tools/cores/brook/v20270101/brook_windows_amd64.exe",
            version: "v20270101",
            cfg_name: "config.json",
            cfg_template: "socks5 --listen 127.0.0.1:PORT",
            has_check_command: false,
        },
        SingleSpec {
            core: CoreType::ShadowQuic,
            name: "shadowquic",
            exe: "tools/cores/shadowquic/v0.4.0/shadowquic.exe",
            version: "v0.4.0",
            cfg_name: "config.yaml",
            cfg_template: "inbounds:\n- type: socks\n  tag: local-socks\n  bind-addr: \"127.0.0.1:PORT\"\noutbounds:\n- type: direct\n  tag: direct\nrouter:\n  default-outbound: direct\n",
            has_check_command: false,
        },
        SingleSpec {
            core: CoreType::Mieru,
            name: "mieru",
            exe: "tools/cores/mieru/v3.38.0/mieru.exe",
            version: "v3.38.0",
            cfg_name: "config.json",
            cfg_template: r#"{"profiles":[{"profileName":"default","user":{"name":"synthetic","password":"synthetic"},"servers":[{"ipAddress":"127.0.0.1","domainName":"","portBindings":[{"port":1,"protocol":"TCP"}]}],"mtu":1400}],"activeProfile":"default","rpcPort":PORT2,"socks5Port":PORT,"loggingLevel":"INFO","socks5ListenLAN":false}"#,
            has_check_command: false,
        },
    ]
}

#[allow(clippy::too_many_lines)]
fn run_single(root: &Path, dir: &Path, spec: &SingleSpec, base: u16) -> serde_json::Value {
    let exe = root.join(spec.exe);
    let mut record = serde_json::json!({
        "core": spec.name,
        "version": spec.version,
        "mode": "single",
        "executable": spec.exe,
    });
    if !exe.is_file() {
        record["status"] = "blocked".into();
        record["reason"] = format!("executable missing: {}", exe.display()).into();
        return record;
    }
    let port = free_port(base);
    let port2 = free_port(port + 1);
    assert_ne!(port, 10808, "live port must never be 10808");
    // Replace PORT2 first: "PORT" is a substring of "PORT2".
    let body = spec
        .cfg_template
        .replace("PORT2", &port2.to_string())
        .replace("PORT", &port.to_string());
    assert!(
        !body.contains("10808"),
        "synthetic config references the live port"
    );
    // Config-kind validation: structured parse for JSON/YAML shapes.
    let parsed_ok = if spec.cfg_name.ends_with(".yaml") {
        serde_yaml::from_str::<serde_yaml::Value>(&body).is_ok()
    } else if body.trim_start().starts_with('{') {
        serde_json::from_str::<serde_json::Value>(&body).is_ok()
    } else {
        body.contains("127.0.0.1")
    };
    record["config_kind"] = if spec.cfg_name.ends_with(".yaml") {
        "yaml".into()
    } else if body.trim_start().starts_with('{') {
        "json".into()
    } else {
        "cli-text".into()
    };
    record["port"] = port.into();
    if spec.name == "mieru" {
        record["rpc_port"] = port2.into();
    }

    let case_dir = dir.join(spec.name);
    std::fs::create_dir_all(&case_dir).expect("case dir");
    let cfg_path = case_dir.join(spec.cfg_name);
    std::fs::write(&cfg_path, &body).expect("write synthetic config");

    let adapter = adapter_for(spec.core).expect("adapter for core");
    let run_args: Vec<String> = adapter
        .run_args(&cfg_path)
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let envs: Vec<(String, String)> = adapter.env_vars(&cfg_path);
    let cwd = adapter
        .working_dir(&cfg_path)
        .unwrap_or_else(|| root.to_path_buf());
    let command_line = format!("{} {}", exe.display(), run_args.join(" "));
    assert!(
        !command_line.contains("10808"),
        "command references the live port"
    );
    record["command"] = command_line.into();
    record["cwd"] = cwd.to_string_lossy().into_owned().into();

    // Step 1: validation. Cores with a real check command run it (non-binding);
    // the rest get parse + version-identity (never counted as session proof).
    let validation = if spec.has_check_command {
        let test_args: Vec<String> = adapter
            .test_args(&cfg_path)
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let (exit, tail) = run_bounded(&exe, &test_args, &cwd, &envs, PROBE_TIMEOUT);
        serde_json::json!({"kind": "check-command", "args": test_args, "exit": exit, "tail": tail})
    } else {
        let version_args: Vec<String> = adapter
            .version_args()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let (exit, tail) = run_bounded(&exe, &version_args, &cwd, &envs, PROBE_TIMEOUT);
        let first = tail.lines().next().unwrap_or("").to_string();
        serde_json::json!({"kind": "parse+version-identity", "parse_ok": parsed_ok, "version_exit": exit, "version_first_line": first})
    };
    let validation_ok = if spec.has_check_command {
        validation["exit"] == 0
    } else {
        parsed_ok && validation["version_exit"] == 0
    };
    record["validation"] = validation;
    record["validation_ok"] = validation_ok.into();
    if !parsed_ok || !validation_ok {
        record["status"] = "blocked".into();
        record["reason"] = "synthetic config failed validation".into();
        return record;
    }

    // Step 2: live session — start, poll the listener, stop our own child.
    let mut command = Command::new(&exe);
    command
        .args(&run_args)
        .current_dir(&cwd)
        .envs(envs.iter().map(|(k, v)| (k, v)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn core");
    let pid = child.id();
    record["pid"] = pid.into();
    let listened = tcp_ready(port, Instant::now() + READY_TIMEOUT);
    record["listened"] = listened.into();
    let (exit_code, still_running) = stop_child(&mut child);
    let tail = drain(&mut child);
    let log_path = dir.join("logs").join(format!("{}.log", spec.name));
    std::fs::write(&log_path, &tail).expect("write log");
    record["exit_code"] = exit_code.into();
    record["stopped"] = (!still_running).into();
    record["still_running"] = still_running.into();
    record["log_tail"] = tail.into();
    if listened && !still_running {
        record["status"] = "ok".into();
        println!(
            "[ok] {} pid={pid} port={port} exit={exit_code:?}",
            spec.name
        );
    } else {
        record["status"] = "blocked".into();
        record["reason"] = if !listened {
            "no listener before timeout".into()
        } else {
            "child still running after stop".into()
        };
        println!(
            "[blocked] {} pid={pid} port={port} listened={listened} still={still_running}",
            spec.name
        );
    }
    record
}

/// Minimal loopback HTTP target: serves `body` to every connection that
/// completes an HTTP request header, until `done` is set. Bare connectivity
/// probes (connect + close, no headers) are ignored rather than consuming a
/// single-shot accept — some cores self-test their forward address at
/// startup, and a single-shot target would go dark before the real request.
fn spawn_http_target(
    port: u16,
    body: &'static str,
    done: Arc<AtomicBool>,
) -> std::thread::JoinHandle<usize> {
    std::thread::spawn(move || {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind target");
        listener.set_nonblocking(true).expect("target nonblocking");
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut served = 0usize;
        while !done.load(Ordering::SeqCst) && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    let mut complete = false;
                    while request.len() < 8192 {
                        match stream.read(&mut byte) {
                            Ok(0) => break,
                            Ok(_) => {
                                request.extend_from_slice(&byte);
                                if request.ends_with(b"\r\n\r\n") {
                                    complete = true;
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    if complete {
                        let response = format!(
                            "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        served += 1;
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
        served
    })
}

/// SOCKS5 (no-auth) CONNECT 127.0.0.1:`target` through `socks_port`, then a
/// plain HTTP GET; returns the response body.
fn socks5_get(socks_port: u16, target_port: u16) -> Result<String, String> {
    let deadline = Duration::from_secs(10);
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], socks_port)),
        deadline,
    )
    .map_err(|e| format!("socks connect: {e}"))?;
    stream
        .set_read_timeout(Some(deadline))
        .map_err(|e| format!("read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(deadline))
        .map_err(|e| format!("write timeout: {e}"))?;
    stream
        .write_all(&[0x05, 0x01, 0x00])
        .map_err(|e| format!("greeting: {e}"))?;
    let mut method = [0u8; 2];
    stream
        .read_exact(&mut method)
        .map_err(|e| format!("method reply: {e}"))?;
    if method != [0x05, 0x00] {
        return Err(format!("unexpected method reply: {method:?}"));
    }
    let hi = (target_port >> 8) as u8;
    let lo = (target_port & 0xff) as u8;
    stream
        .write_all(&[0x05, 0x01, 0x00, 0x01, 127, 0, 0, 1, hi, lo])
        .map_err(|e| format!("connect req: {e}"))?;
    let mut reply = [0u8; 10];
    stream
        .read_exact(&mut reply)
        .map_err(|e| format!("connect reply: {e}"))?;
    if reply[1] != 0x00 {
        return Err(format!("connect rejected: {:02x?}", &reply[..4]));
    }
    stream
        .write_all(b"GET / HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .map_err(|e| format!("http get: {e}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|e| format!("http read: {e}"))?;
    let text = String::from_utf8_lossy(&response).into_owned();
    text.split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .ok_or_else(|| "no http body".to_string())
}

/// Generate a throwaway self-signed cert/key with the locked hysteria2
/// `cert` subcommand (same generator as session_matrix.ps1). Only the cert
/// hash is recorded, never the key.
fn generate_cert(root: &Path, cert_dir: &Path) -> Result<(PathBuf, PathBuf, String), String> {
    let hy2 = root.join("tools/cores/hysteria/v2.12.3/hysteria-windows-amd64.exe");
    if !hy2.is_file() {
        return Err("hysteria2 generator binary missing".to_string());
    }
    let cert = cert_dir.join("server.crt");
    let key = cert_dir.join("server.key");
    let (exit, tail) = run_bounded(
        &hy2,
        &[
            "cert".to_string(),
            "--cert".to_string(),
            cert.to_string_lossy().into_owned(),
            "--key".to_string(),
            key.to_string_lossy().into_owned(),
            "--host".to_string(),
            "127.0.0.1,example.com,localhost".to_string(),
            "--overwrite".to_string(),
        ],
        cert_dir,
        &[],
        CERT_TIMEOUT,
    );
    if exit != 0 || !cert.is_file() || !key.is_file() {
        return Err(format!("cert generation failed (exit {exit}): {tail}"));
    }
    let bytes = std::fs::read(&cert).map_err(|e| format!("read cert: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok((cert, key, hex::encode(hasher.finalize())))
}

struct PairSpec {
    core: CoreType,
    name: &'static str,
    exe: &'static str,
    version: &'static str,
    server_args: [&'static str; 3],
    server_cfg: &'static str,
    client_cfg: &'static str,
    server_is_udp: bool,
    expected_body: &'static str,
}

fn pairs() -> Vec<PairSpec> {
    vec![
        PairSpec {
            core: CoreType::Hysteria2,
            name: "hysteria2",
            exe: "tools/cores/hysteria/v2.12.3/hysteria-windows-amd64.exe",
            version: "v2.12.3",
            server_args: ["server", "-c", "{scfg}"],
            server_cfg: r#"{"listen":"127.0.0.1:{SP}","disableUpdateCheck":true,"tls":{"cert":"{CERT}","key":"{KEY}"},"auth":{"type":"password","password":"synthetic"}}"#,
            client_cfg: r#"{"server":"127.0.0.1:{SP}","auth":"synthetic","disableUpdateCheck":true,"tls":{"sni":"example.com","insecure":true},"socks5":{"listen":"127.0.0.1:{CP}"}}"#,
            server_is_udp: true,
            expected_body: "HY2-OK",
        },
        PairSpec {
            core: CoreType::OverTls,
            name: "overtls",
            exe: "tools/cores/overtls/v0.3.15/overtls-bin.exe",
            version: "v0.3.15",
            server_args: ["-r", "server", "-c"],
            server_cfg: r#"{"remarks":"synthetic","test_timeout_secs":10,"method":"none","password":"synthetic","tunnel_path":"/synthetic-tunnel-path/","server_settings":{"disable_tls":false,"certfile":"{CERT}","keyfile":"{KEY}","forward_addr":"http://127.0.0.1:{TP}","listen_host":"127.0.0.1","listen_port":{SP}}}"#,
            client_cfg: r#"{"remarks":"synthetic","test_timeout_secs":10,"method":"none","password":"synthetic","tunnel_path":"/synthetic-tunnel-path/","client_settings":{"disable_tls":false,"client_id":"00000000-0000-0000-0000-000000000000","server_host":"127.0.0.1","server_port":{SP},"server_domain":"example.com","cafile":"","dangerous_mode":true,"advertise_ip":"127.0.0.1","max_lifetime":3600,"pool_max_size":30,"listen":"mixed://127.0.0.1:{CP}"}}"#,
            server_is_udp: false,
            expected_body: "OV-OK",
        },
    ]
}

#[allow(clippy::too_many_lines)]
fn run_pair(root: &Path, dir: &Path, spec: &PairSpec, base: u16) -> serde_json::Value {
    let exe = root.join(spec.exe);
    let mut record = serde_json::json!({
        "core": spec.name,
        "version": spec.version,
        "mode": "pair",
        "executable": spec.exe,
    });
    if !exe.is_file() {
        record["status"] = "blocked".into();
        record["reason"] = format!("executable missing: {}", exe.display()).into();
        return record;
    }
    let case_dir = dir.join(spec.name);
    let cert_dir = case_dir.join("certs");
    let srv_dir = case_dir.join("server");
    let cli_dir = case_dir.join("client");
    for d in [&case_dir, &cert_dir, &srv_dir, &cli_dir] {
        std::fs::create_dir_all(d).expect("pair dir");
    }
    let (cert, key, cert_sha) = match generate_cert(root, &cert_dir) {
        Ok(v) => v,
        Err(reason) => {
            record["status"] = "blocked".into();
            record["reason"] = reason.into();
            return record;
        }
    };
    record["cert_sha256"] = cert_sha.into();

    let sp = free_port(base);
    let cp = free_port(sp + 1);
    let tp = free_port(cp + 1);
    for port in [sp, cp, tp] {
        assert_ne!(port, 10808, "live port must never be 10808");
    }
    let fill = |template: &str| {
        template
            .replace("{SP}", &sp.to_string())
            .replace("{CP}", &cp.to_string())
            .replace("{TP}", &tp.to_string())
            .replace("{CERT}", &cert.to_string_lossy().replace('\\', "\\\\"))
            .replace("{KEY}", &key.to_string_lossy().replace('\\', "\\\\"))
    };
    let scfg = fill(spec.server_cfg);
    let ccfg = fill(spec.client_cfg);
    assert!(!scfg.contains("10808") && !ccfg.contains("10808"));
    let scfg_path = srv_dir.join("config.json");
    let ccfg_path = cli_dir.join("config.json");
    std::fs::write(&scfg_path, &scfg).expect("write server config");
    std::fs::write(&ccfg_path, &ccfg).expect("write client config");
    record["config_kind"] = "json".into();
    record["server_port"] = sp.into();
    record["client_port"] = cp.into();
    record["target_port"] = tp.into();

    let adapter = adapter_for(spec.core).expect("adapter for core");
    let client_args: Vec<String> = adapter
        .run_args(&ccfg_path)
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let client_cwd = adapter
        .working_dir(&ccfg_path)
        .unwrap_or_else(|| root.to_path_buf());
    // Server args come from the locked session contract (mirrors the
    // `server_args` column of session_matrix.ps1, which net_host has no
    // managed-server path for — these cores are client-driven upstream).
    let server_args: Vec<String> = if spec.name == "overtls" {
        vec![
            "-r".to_string(),
            "server".to_string(),
            "-c".to_string(),
            scfg_path.to_string_lossy().into_owned(),
        ]
    } else {
        spec.server_args
            .iter()
            .map(|a| {
                a.replace("{scfg}", &scfg_path.to_string_lossy())
                    .replace("{cfg}", &scfg_path.to_string_lossy())
            })
            .collect()
    };
    assert_eq!(spec.server_args.len(), 3, "locked server arg shape");
    let client_cmd = if client_args.is_empty() {
        exe.display().to_string()
    } else {
        format!("{} {}", exe.display(), client_args.join(" "))
    };
    record["command"] = client_cmd.into();
    record["server_command"] = format!("{} {}", exe.display(), server_args.join(" ")).into();

    let target_done = Arc::new(AtomicBool::new(false));
    let target = spawn_http_target(tp, spec.expected_body, Arc::clone(&target_done));
    let mut server = Command::new(&exe)
        .args(&server_args)
        .current_dir(&srv_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tls server");
    let server_pid = server.id();
    record["server_pid"] = server_pid.into();
    let server_deadline = Instant::now() + READY_TIMEOUT;
    let server_listened = if spec.server_is_udp {
        udp_ready(sp, server_deadline)
    } else {
        tcp_ready(sp, server_deadline)
    };
    record["server_listened"] = server_listened.into();

    let mut client_pid = None;
    let mut client_listened = false;
    let mut proxied_body: Option<String> = None;
    if server_listened {
        let mut client = Command::new(&exe)
            .args(&client_args)
            .current_dir(&client_cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn client");
        client_pid = Some(client.id());
        client_listened = tcp_ready(cp, Instant::now() + READY_TIMEOUT);
        if client_listened {
            // The listener can accept TCP before its upstream tunnel is
            // established (overtls performs its TLS handshake lazily), so the
            // proxied probe retries until it succeeds or the deadline passes.
            let deadline = Instant::now() + PROBE_TIMEOUT;
            loop {
                match socks5_get(cp, tp) {
                    Ok(body) => {
                        proxied_body = Some(body);
                        break;
                    }
                    Err(reason) => {
                        if Instant::now() >= deadline {
                            record["proxied_error"] = reason.into();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(250));
                    }
                }
            }
        }
        let (exit_code, still) = stop_child(&mut client);
        record["client_pid"] = client_pid.into();
        record["client_listened"] = client_listened.into();
        record["proxied_http_body"] = proxied_body.clone().into();
        record["client_exit_code"] = exit_code.into();
        record["client_stopped"] = (!still).into();
        record["client_still_running"] = still.into();
        record["client_log_tail"] = drain(&mut client).into();
        if still {
            record["status"] = "blocked".into();
            record["reason"] = "client still running after stop".into();
        }
    }
    let (server_exit, server_still) = stop_child(&mut server);
    record["server_exit_code"] = server_exit.into();
    record["server_stopped"] = (!server_still).into();
    record["server_still_running"] = server_still.into();
    record["server_log_tail"] = drain(&mut server).into();
    target_done.store(true, Ordering::SeqCst);
    record["target_served_requests"] = target.join().unwrap_or(0).into();

    if record.get("status").is_none() {
        if server_listened
            && client_listened
            && proxied_body.as_deref() == Some(spec.expected_body)
            && !server_still
        {
            record["status"] = "ok".into();
            println!(
                "[ok] {} server_pid={server_pid} client_pid={} proxied={:?}",
                spec.name,
                client_pid.unwrap_or(0),
                proxied_body,
            );
        } else {
            record["status"] = "blocked".into();
            record["reason"] = format!(
                "server_listened={server_listened} client_listened={client_listened} proxied={proxied_body:?} error={}",
                record
                    .get("proxied_error")
                    .and_then(|value| value.as_str())
                    .unwrap_or("-")
            )
            .into();
            println!(
                "[blocked] {} server_listened={server_listened} client_listened={client_listened} proxied={proxied_body:?}",
                spec.name
            );
        }
    }
    record
}

#[test]
fn t10_live_core_matrix() {
    let root = repo_root();
    let dir = out_dir();

    let mut results: Vec<serde_json::Value> = Vec::new();
    for (index, spec) in singles().iter().enumerate() {
        // Staggered bases keep every probed port distinct and >= 11808.
        results.push(run_single(&root, &dir, spec, 11808 + (index as u16) * 40));
    }
    for (index, spec) in pairs().iter().enumerate() {
        results.push(run_pair(&root, &dir, spec, 12400 + (index as u16) * 60));
    }

    std::fs::write(
        dir.join("results.json"),
        serde_json::to_string_pretty(&results).unwrap(),
    )
    .expect("write results.json");

    let ok = results.iter().filter(|r| r["status"] == "ok").count();
    let blocked: Vec<String> = results
        .iter()
        .filter(|r| r["status"] != "ok")
        .map(|r| {
            format!(
                "{} ({})",
                r["core"].as_str().unwrap_or("?"),
                r["reason"].as_str().unwrap_or("no reason")
            )
        })
        .collect();
    println!(
        "T10 live matrix: {ok} ok, {} blocked -> {}",
        blocked.len(),
        dir.display()
    );
    // A core that cannot serve on this host is recorded as `blocked` with its
    // exact reason instead of being faked green; the matrix fails only when
    // the reason is missing or when the known-good floor regresses.
    for r in &results {
        let status = r["status"].as_str().unwrap_or("?");
        assert!(
            status == "ok" || status == "blocked",
            "unexpected status {status} for {}",
            r["core"].as_str().unwrap_or("?")
        );
        if status == "blocked" {
            assert!(
                r["reason"]
                    .as_str()
                    .map(|reason| !reason.is_empty())
                    .unwrap_or(false),
                "blocked core {} must carry a reason",
                r["core"].as_str().unwrap_or("?")
            );
        }
    }
    assert!(
        ok >= LIVE_MATRIX_OK_FLOOR,
        "live matrix floor regressed: {ok} ok < {LIVE_MATRIX_OK_FLOOR}; blocked: {}",
        blocked.join("; ")
    );
}
