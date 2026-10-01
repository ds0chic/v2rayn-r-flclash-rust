//! T10: real-core validation matrix for PolicyGroup (five `MultipleLoad`
//! modes), ProxyChain, Custom pass-through and FullConfigTemplate injection.
//!
//! Each case is assembled through the real `AppEngine` (`save_profile` +
//! `build_codegen_input` + `generate`), written to `target/t10/matrix/` and
//! checked by the real kernels (`xray run -test` / `sing-box check`).
//! Listening ports are never opened (`-test` / `check` only validate); every
//! emitted port is probed free beforehand and is `>= 11808` (never 10808).
//! Each kernel wait is bounded at 60 s; only our own child is ever killed.
//! The mihomo (mixin) case covers YAML merge plus a file write/read
//! round-trip only — the mihomo kernel is never executed.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use application::codegen::CodegenOptions;
use application::AppEngine;
use domain::{ConfigType, CoreType, DesiredRevision, FullConfigTemplate, MultipleLoad, Profile};

const XRAY_EXE: &str = "tools/cores/xray/v26.3.27/xray.exe";
const SINGBOX_EXE: &str = "tools/cores/singbox/v1.14.2/sing-box.exe";
const TIMEOUT: Duration = Duration::from_secs(60);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn matrix_dir() -> PathBuf {
    let dir = repo_root().join("target").join("t10").join("matrix");
    std::fs::create_dir_all(dir.join("logs")).expect("matrix dir");
    dir
}

/// Pick a free port at/above `base` (127.0.0.1, TCP probe then release).
/// Never returns 10808.
fn free_port(mut base: u16) -> u16 {
    loop {
        assert!(base < 60000, "port scan exhausted");
        if base == 10808 {
            base += 1;
            continue;
        }
        if std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return base;
        }
        base += 1;
    }
}

fn leaf(id: &str, address: &str) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::Vless,
        remarks: id.into(),
        address: address.into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    // Xray requires explicit `encryption: none` for VLESS users.
    profile.proto_extra.vless_encryption = Some("none".into());
    profile
}

fn save(engine: &AppEngine, profile: Profile) -> Profile {
    let revision = engine.desired_revision();
    engine
        .save_profile(profile, DesiredRevision::new(revision))
        .expect("save")
        .0
}

fn group(id: &str, remarks: &str, load: MultipleLoad, children: &[&str]) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::PolicyGroup,
        remarks: remarks.into(),
        ..Default::default()
    };
    profile.proto_extra.child_items = Some(children.join(","));
    profile.proto_extra.multiple_load = Some(load);
    profile
}

fn seed(engine: &AppEngine) {
    save(engine, leaf("v1", "192.0.2.11"));
    save(engine, leaf("v2", "192.0.2.12"));
    let modes = [
        ("g-least-ping", MultipleLoad::LeastPing),
        ("g-fallback", MultipleLoad::Fallback),
        ("g-random", MultipleLoad::Random),
        ("g-round-robin", MultipleLoad::RoundRobin),
        ("g-least-load", MultipleLoad::LeastLoad),
    ];
    for (id, mode) in modes {
        save(engine, group(id, id, mode, &["v1", "v2"]));
    }
    let mut chain = group(
        "chain-1",
        "chain-1",
        MultipleLoad::RoundRobin,
        &["v1", "v2"],
    );
    chain.config_type = ConfigType::ProxyChain;
    save(engine, chain);

    // Custom pass-through: reuse a generated baseline client config as content.
    for (core, id) in [
        (CoreType::Xray, "custom-xray"),
        (CoreType::SingBox, "custom-sbox"),
    ] {
        let opts = CodegenOptions {
            local_port: 11808,
            state_port: 11809,
            state_port2: 11810,
            ..Default::default()
        };
        let input = engine.build_codegen_input("v1", core, &opts).unwrap();
        let generated = application::codegen::generate(core, &input).unwrap();
        let text = serde_json::to_string(&generated.main).unwrap();
        let mut custom = Profile {
            index_id: id.into(),
            config_type: ConfigType::Custom,
            core_type: Some(core),
            remarks: id.into(),
            address: format!("{id}.json"),
            ..Default::default()
        };
        custom.proto_extra.extra.insert(
            application::codegen::CUSTOM_CONFIG_KEY.into(),
            serde_json::Value::String(text),
        );
        save(engine, custom);
    }

    // Template injection rows: AddProxyOnly + a detour tag that exists in the
    // merged output (xray `det1` freedom outbound, sing-box `corp-detour`).
    // Seeded DISABLED so group/chain/custom cases generate without template
    // injection; the template cases enable them for exactly one build each.
    let xray_template = FullConfigTemplate {
        remarks: "V2ray".into(),
        core_type: CoreType::Xray,
        enabled: false,
        config: Some(
            r#"{
              "log": {"access": "Vaccess.log", "error": "Verror.log", "loglevel": "warning"},
              "inbounds": [],
              "outbounds": [
                {"protocol": "freedom", "settings": {}, "tag": "det1"},
                {"protocol": "freedom", "tag": "direct"},
                {"protocol": "blackhole", "tag": "block"}
              ],
              "routing": {"domainStrategy": "IPIfNonMatch", "rules": []}
            }"#
            .into(),
        ),
        tun_config: None,
        add_proxy_only: Some(true),
        proxy_detour: Some("det1".into()),
        ..Default::default()
    };
    engine.save_template(xray_template).unwrap();
    let sbox_template = FullConfigTemplate {
        remarks: "sing-box".into(),
        core_type: CoreType::SingBox,
        enabled: false,
        config: Some(
            r#"{
              "log": {"level": "warning", "timestamp": true},
              "inbounds": [],
              "outbounds": [{"type": "socks", "tag": "corp-detour", "server": "192.0.2.250", "server_port": 11890}],
              "endpoints": [],
              "route": {"rules": []}
            }"#
            .into(),
        ),
        tun_config: None,
        add_proxy_only: Some(true),
        proxy_detour: Some("corp-detour".into()),
        ..Default::default()
    };
    engine.save_template(sbox_template).unwrap();
}

struct Case {
    core: CoreType,
    id: &'static str,
    profile_id: &'static str,
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for core in [CoreType::Xray, CoreType::SingBox] {
        for id in [
            "g-least-ping",
            "g-fallback",
            "g-random",
            "g-round-robin",
            "g-least-load",
            "chain-1",
        ] {
            out.push(Case {
                core,
                id,
                profile_id: id,
            });
        }
        out.push(Case {
            core,
            id: if core == CoreType::Xray {
                "custom-xray"
            } else {
                "custom-sbox"
            },
            profile_id: if core == CoreType::Xray {
                "custom-xray"
            } else {
                "custom-sbox"
            },
        });
        out.push(Case {
            core,
            id: "template",
            profile_id: "v1",
        });
    }
    out
}

struct CaseResult {
    core: &'static str,
    case: String,
    file: String,
    status: String,
    exit: i32,
    error: String,
}

/// Run the kernel validator with a 60 s bound; only our own child is killed.
fn run_core(exe: &std::path::Path, core: CoreType, config: &std::path::Path) -> (i32, String) {
    let mut command = Command::new(exe);
    match core {
        CoreType::SingBox => {
            command.arg("check").arg("-c").arg(config);
        }
        _ => {
            command.arg("run").arg("-test").arg("-config").arg(config);
        }
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn core validator");
    let start = Instant::now();
    loop {
        match child.try_wait().expect("poll core validator") {
            Some(status) => {
                let mut tail = String::new();
                if let Some(mut out) = child.stdout.take() {
                    use std::io::Read;
                    let _ = out.read_to_string(&mut tail);
                }
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read;
                    let _ = err.read_to_string(&mut tail);
                }
                return (status.code().unwrap_or(-1), tail);
            }
            None => {
                if start.elapsed() > TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return (-2, "validator timed out after 60s (killed)".into());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

#[test]
fn t10_real_core_matrix() {
    let root = repo_root();
    let xray = root.join(XRAY_EXE);
    let sbox = root.join(SINGBOX_EXE);
    assert!(xray.is_file(), "missing {}", xray.display());
    assert!(sbox.is_file(), "missing {}", sbox.display());
    let dir = matrix_dir();

    let engine = AppEngine::in_memory();
    seed(&engine);

    let mut results: Vec<CaseResult> = Vec::new();
    let mut pass = 0usize;
    let mut fail = 0usize;
    for (index, case) in cases().into_iter().enumerate() {
        let local = free_port(11808 + (index as u16) * 10);
        let opts = CodegenOptions {
            local_port: local as i32,
            state_port: free_port(local + 1) as i32,
            state_port2: free_port(local + 2) as i32,
            ..Default::default()
        };
        // Template cases run with their template enabled for exactly one
        // build; every other case generates from the bare node graph.
        let template_guard = case.id == "template";
        if template_guard {
            let mut row = engine.get_template_for_core(case.core).unwrap().unwrap();
            row.enabled = true;
            engine.save_template(row).unwrap();
        }
        let input = engine
            .build_codegen_input(case.profile_id, case.core, &opts)
            .expect("build input");
        if template_guard {
            let mut row = engine.get_template_for_core(case.core).unwrap().unwrap();
            row.enabled = false;
            engine.save_template(row).unwrap();
            assert!(
                application::codegen::template_for(
                    &engine.get_template_for_core(case.core).unwrap().unwrap()
                )
                .is_none(),
                "template disabled again after the template case"
            );
        }
        let generated = application::codegen::generate(case.core, &input).expect("generate");
        let text = serde_json::to_string_pretty(&generated.main).unwrap();
        assert!(
            !text.contains("10808"),
            "case {} references the live port",
            case.id
        );
        let core_name = if case.core == CoreType::SingBox {
            "singbox"
        } else {
            "xray"
        };
        let file = dir.join(format!("{core_name}--{}.json", case.id));
        std::fs::write(&file, &text).expect("write case config");

        let exe = if case.core == CoreType::SingBox {
            &sbox
        } else {
            &xray
        };
        let (exit, output) = run_core(exe, case.core, &file);
        let log_path = dir
            .join("logs")
            .join(format!("{core_name}--{}.log", case.id));
        std::fs::write(&log_path, &output).expect("write log");
        let status = if exit == 0 { "PASS" } else { "FAIL" };
        if exit == 0 {
            pass += 1;
        } else {
            fail += 1;
        }
        let mut error = output.trim().to_string();
        if error.len() > 600 {
            error = error[error.len() - 600..].to_string();
        }
        println!("[{status}] {core_name} {} (exit {exit})", case.id);
        results.push(CaseResult {
            core: core_name,
            case: case.id.to_string(),
            file: file.to_string_lossy().into_owned(),
            status: status.into(),
            exit,
            error,
        });
    }

    let summary: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "core": r.core, "case": r.case, "file": r.file,
                "status": r.status, "exit": r.exit, "error": r.error,
            })
        })
        .collect();
    let mut handle = std::fs::File::create(dir.join("results.json")).expect("results.json");
    write!(
        handle,
        "{}",
        serde_json::to_string_pretty(&summary).unwrap()
    )
    .unwrap();
    println!("T10 matrix: {pass} pass, {fail} fail -> {}", dir.display());
    assert_eq!(
        fail, 0,
        "{pass} passed, {fail} failed (see target/t10/matrix/results.json)"
    );
}

#[test]
fn t10_mihomo_mixin_file_round_trip() {
    use application::mixin::{generate_mihomo, MixinOptions};
    let base = "mode: direct\nsecret: hunter2\nproxies:\n  - name: a\n    type: ss\nrules:\n  - DOMAIN,example.com,DIRECT\n";
    let mixin = "mode: rule\nprepend-rules:\n  - DOMAIN,ads.example,BLOCK\n";
    let opts = MixinOptions::default();
    let merged = generate_mihomo(base, Some(mixin), None, &opts).unwrap();
    assert!(merged.contains("mixed-port"));
    assert!(!merged.contains("10808"));

    // File write/read round-trip (no kernel is ever executed for mihomo).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mihomo.yaml");
    std::fs::write(&path, &merged).unwrap();
    let back = std::fs::read_to_string(&path).unwrap();
    assert_eq!(back, merged);
    let map: serde_yaml::Value = serde_yaml::from_str(&back).unwrap();
    assert!(map.get("mixed-port").is_some());
    assert!(map.get("secret").is_none());
}
