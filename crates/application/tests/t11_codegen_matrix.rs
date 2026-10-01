//! T11 real-core validation matrix for routing + DNS (F-ROUTING-001..006,
//! F-DNS-001..009 codegen wiring).
//!
//! Each case is assembled through the real `AppEngine` (`save_routing` /
//! `save_dns` / settings + `build_codegen_input` + `generate`), written to
//! `target/t11/matrix/` and checked by the real kernels (`xray run -test` /
//! `sing-box check`). Listening ports are never opened (`-test` / `check`
//! only validate); every emitted port is probed free beforehand and is
//! `>= 11808` (never 10808). Each kernel wait is bounded at 60 s; only our
//! own child is ever killed.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use application::codegen::CodegenOptions;
use application::AppEngine;
use domain::{ConfigType, CoreType, Profile, RoutingProfile, RoutingRule, RuleMode};

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
    let dir = repo_root().join("target").join("t11").join("matrix");
    std::fs::create_dir_all(&dir).expect("matrix dir");
    dir
}

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

fn leaf() -> Profile {
    let mut profile = Profile {
        index_id: "n1".into(),
        config_type: ConfigType::Vless,
        remarks: "n1".into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    profile.proto_extra.vless_encryption = Some("none".into());
    profile
}

fn new_rule(remarks: &str, outbound: &str) -> RoutingRule {
    RoutingRule {
        id: format!("id-{remarks}"),
        outbound_tag: Some(outbound.to_string()),
        enabled: true,
        remarks: Some(remarks.to_string()),
        rule_type: Some(domain::RuleType::Routing),
        ..Default::default()
    }
}

fn dns_rule(remarks: &str, outbound: &str, domain: &str) -> RoutingRule {
    let mut rule = new_rule(remarks, outbound);
    rule.rule_type = Some(domain::RuleType::Dns);
    rule.domain = Some(vec![domain.to_string()]);
    rule
}

fn save_scheme(engine: &AppEngine, remarks: &str, rules: Vec<RoutingRule>) -> RoutingProfile {
    let mut profile = RoutingProfile {
        remarks: remarks.to_string(),
        ..Default::default()
    };
    profile.set_rules(&rules).expect("set rules");
    let saved = engine.save_routing(profile).expect("save routing");
    // Generation consumes the *active* scheme: promote the test scheme.
    engine
        .set_default_routing(&saved.id)
        .expect("default routing");
    engine
        .get_routing(&saved.id)
        .expect("get")
        .expect("present")
}

fn seed_node(engine: &AppEngine) {
    let revision = engine.desired_revision();
    engine
        .save_profile(leaf(), domain::DesiredRevision::new(revision))
        .expect("save node");
}

fn opts(base: u16) -> CodegenOptions {
    CodegenOptions {
        local_port: free_port(base) as i32,
        state_port: free_port(base + 100) as i32,
        state_port2: free_port(base + 200) as i32,
        log_directory: "logs".into(),
        bin_directory: "bin".into(),
        log_date: "2026-01-01".into(),
        speed_ping_test_url: Some("https://example.com/".into()),
    }
}

fn generate_case(
    engine: &AppEngine,
    core: CoreType,
    base: u16,
) -> (serde_json::Value, Vec<config_codegen::Diagnostic>) {
    let options = opts(base);
    // The real settings tree carries the default 10808 inbound (the user's
    // live proxy, rejected by the generator guard); point it at the free
    // test port so the matrix validates the wiring, not the guard.
    let loaded = engine.load_settings().expect("settings");
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = options.local_port;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("settings port");
    let input = engine
        .build_codegen_input("n1", core, &options)
        .expect("input");
    let generated = application::codegen::generate(core, &input).expect("generate");
    (generated.main, generated.diagnostics)
}

struct CaseResult {
    core: &'static str,
    case: String,
    file: String,
    status: String,
    exit: i32,
    error: String,
}

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
    // Xray resolves geosite:/geoip: via the asset dir next to the binary.
    if core != CoreType::SingBox {
        if let Some(dir) = exe.parent() {
            command.env("XRAY_LOCATION_ASSET", dir);
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

fn check_case(
    engine: &AppEngine,
    core: CoreType,
    core_name: &'static str,
    case: &str,
    dir: &std::path::Path,
    base: u16,
    assert_fn: &dyn Fn(&serde_json::Value, &[config_codegen::Diagnostic]),
) -> CaseResult {
    let (main, diagnostics) = generate_case(engine, core, base);
    assert_fn(&main, &diagnostics);
    let file_name = format!("{core_name}_{case}.json");
    let path = dir.join(&file_name);
    let text = serde_json::to_string_pretty(&main).expect("serialize");
    assert!(!text.contains("10808"), "reserved port leaked in {case}");
    std::fs::write(&path, &text).expect("write config");
    let exe = repo_root().join(if core == CoreType::SingBox {
        SINGBOX_EXE
    } else {
        XRAY_EXE
    });
    let (exit, error) = run_core(&exe, core, &path);
    CaseResult {
        core: core_name,
        case: case.to_string(),
        file: file_name,
        status: if exit == 0 {
            "PASS".into()
        } else {
            "FAIL".into()
        },
        exit,
        error: error.chars().take(2000).collect(),
    }
}

fn write_report(dir: &std::path::Path, results: &[CaseResult]) {
    let mut report = String::from(
        "# T11 real-core matrix\n\n| core | case | file | status | exit |\n|---|---|---|---|---|\n",
    );
    for r in results {
        report.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            r.core, r.case, r.file, r.status, r.exit
        ));
    }
    report.push_str("\n## failures\n\n");
    for r in results.iter().filter(|r| r.status != "PASS") {
        report.push_str(&format!(
            "### {} {}\n\n```\n{}\n```\n\n",
            r.core, r.case, r.error
        ));
    }
    let mut file = std::fs::File::create(dir.join("report.md")).expect("report");
    file.write_all(report.as_bytes()).expect("write report");
}

#[test]
fn t11_real_core_matrix() {
    let root = repo_root();
    let xray = root.join(XRAY_EXE);
    let sbox = root.join(SINGBOX_EXE);
    assert!(xray.is_file(), "missing {}", xray.display());
    assert!(sbox.is_file(), "missing {}", sbox.display());
    let dir = matrix_dir();
    let mut results: Vec<CaseResult> = Vec::new();
    let mut base: u16 = 12108;

    for core in [CoreType::Xray, CoreType::SingBox] {
        let core_name = if core == CoreType::SingBox {
            "singbox"
        } else {
            "xray"
        };
        // 1. domain rule.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("proxy-google", "proxy");
            r.domain = Some(vec!["geosite:google".into()]);
            save_scheme(&engine, "s-domain", vec![r]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "domain-rule",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    assert!(text.contains("google"), "domain rule missing");
                },
            ));
        }
        // 2. ip rule.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("bypass-lan", "direct");
            r.ip = Some(vec!["geoip:private".into()]);
            save_scheme(&engine, "s-ip", vec![r]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "ip-rule",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    assert!(text.contains("direct"), "direct outbound missing");
                },
            ));
        }
        // 3. port rule.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("block-443-udp", "block");
            r.port = Some("443".into());
            r.network = Some("udp".into());
            save_scheme(&engine, "s-port", vec![r]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "port-rule",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    assert!(text.contains("443"), "port rule missing");
                },
            ));
        }
        // 4. network rule.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("net-both", "direct");
            r.network = Some("tcp,udp".into());
            r.port = Some("53".into());
            save_scheme(&engine, "s-network", vec![r]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "network-rule",
                &dir,
                base,
                &|_, _| {},
            ));
        }
        // 5. protocol rule.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("block-bt", "block");
            r.protocol = Some(vec!["bittorrent".into()]);
            save_scheme(&engine, "s-protocol", vec![r]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "protocol-rule",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    assert!(text.contains("bittorrent"), "protocol rule missing");
                },
            ));
        }
        // 6. process rule.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("proc-direct", "direct");
            r.process = Some(vec!["chrome.exe".into()]);
            save_scheme(&engine, "s-process", vec![r]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "process-rule",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    assert!(text.contains("chrome"), "process rule missing");
                },
            ));
        }
        // 7. inboundTag rule.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("in-socks", "direct");
            r.inbound_tag = Some(vec!["socks".into()]);
            save_scheme(&engine, "s-inbound", vec![r]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "inbound-rule",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    assert!(text.contains("socks"), "inbound rule missing");
                },
            ));
        }
        // 8. multi-rule order: routing output preserves scheme order.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut a = new_rule("first", "proxy");
            a.domain = Some(vec!["geosite:google".into()]);
            let mut b = new_rule("second", "direct");
            b.ip = Some(vec!["10.0.0.0/8".into()]);
            let mut c = new_rule("third", "block");
            c.port = Some("23".into());
            save_scheme(&engine, "s-order", vec![a, b, c]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "multi-rule-order",
                &dir,
                base,
                &|main, _| {
                    // The user rules must appear in scheme order inside the
                    // routing/rule array (upstream: array order is the order).
                    let rules: Vec<String> = if core_name == "xray" {
                        main.get("routing")
                            .and_then(|r| r.get("rules"))
                            .and_then(|r| r.as_array())
                            .cloned()
                            .unwrap_or_default()
                            .iter()
                            .map(|r| serde_json::to_string(r).unwrap())
                            .collect()
                    } else {
                        main.get("route")
                            .and_then(|r| r.get("rules"))
                            .and_then(|r| r.as_array())
                            .cloned()
                            .unwrap_or_default()
                            .iter()
                            .map(|r| serde_json::to_string(r).unwrap())
                            .collect()
                    };
                    let pos = |needle: &str| rules.iter().position(|r| r.contains(needle));
                    // Xray keeps `geosite:google`; sing-box converts to the
                    // `geosite-google` rule_set tag.
                    let a = pos("google").expect("domain rule present");
                    let b = pos("10.0.0.0").expect("ip rule present");
                    let c = pos("23").expect("port rule present");
                    assert!(a < b && b < c, "rule order not preserved");
                },
            ));
        }
        // 9. DomainStrategy dual-core difference.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut scheme = RoutingProfile {
                remarks: "s-strategy".into(),
                domain_strategy: "UseIP".into(),
                domain_strategy4_singbox: "ipv4_only".into(),
                ..Default::default()
            };
            let mut r = new_rule("r", "proxy");
            r.domain = Some(vec!["geosite:cn".into()]);
            scheme.set_rules(&[r]).expect("rules");
            let saved = engine.save_routing(scheme).expect("save");
            engine.set_default_routing(&saved.id).expect("default");
            // sing-box only emits the resolve rule (carrying the strategy)
            // under IPIfNonMatch / IPOnDemand (upstream `SingboxRoutingService`).
            let loaded = engine.load_settings().expect("settings");
            let mut settings = loaded.settings;
            settings.routing_basic_item.domain_strategy = Some("IPIfNonMatch".to_string());
            engine
                .save_settings(settings, loaded.revision)
                .expect("strategy mode");
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "domain-strategy",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    if core_name == "xray" {
                        assert!(text.contains("UseIP"), "xray strategy missing");
                    } else {
                        assert!(
                            text.contains("ipv4_only") || text.contains("ipv4"),
                            "singbox strategy missing"
                        );
                    }
                },
            ));
        }
        // 10. DNS RuleType: feeds DNS servers, stays out of routing rules.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let d = dns_rule("dns-cn", "direct", "geosite:cn");
            let mut r = new_rule("route-g", "proxy");
            r.domain = Some(vec!["geosite:google".into()]);
            save_scheme(&engine, "s-dnsrule", vec![d, r]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "dns-rule",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    assert!(text.contains("dns"), "dns section missing");
                },
            ));
        }
        // 11. fakeip + custom hosts + block AAAA.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("r", "proxy");
            r.domain = Some(vec!["geosite:google".into()]);
            save_scheme(&engine, "s-fakeip", vec![r]);
            let loaded = engine.load_settings().expect("settings");
            let mut settings = loaded.settings;
            settings.simple_dns_item.fake_ip = Some(true);
            settings.simple_dns_item.hosts = Some("custom.example.com 93.184.216.34".to_string());
            settings.simple_dns_item.block_aaaa_query = Some(true);
            engine
                .save_settings(settings, loaded.revision)
                .expect("save dns settings");
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "fakeip-hosts-aaaa",
                &dir,
                base,
                &|main, _| {
                    let text = serde_json::to_string(main).unwrap();
                    assert!(text.contains("custom.example.com"), "custom hosts missing");
                    assert!(
                        text.contains("fakedns") || text.contains("fake"),
                        "fakeip missing"
                    );
                },
            ));
        }
        // 12. dangling outbound reference -> structured warning + proxy fallback.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            // NOTE: the domain must exist in geosite.dat (`xray run -test`
            // resolves it); the *outbound* is what dangles here.
            let r = new_rule("dangling", "no-such-node");
            let mut with_domain = r;
            with_domain.domain = Some(vec!["geosite:google".into()]);
            save_scheme(&engine, "s-dangling", vec![with_domain]);
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "dangling-warning",
                &dir,
                base,
                &|_, diagnostics| {
                    assert!(
                        diagnostics
                            .iter()
                            .any(|d| d.code == "routing_dangling_reference"),
                        "expected dangling warning"
                    );
                },
            ));
        }
        // 13. RuleMode Global vs Direct vs Rule differ.
        {
            let engine = AppEngine::in_memory();
            seed_node(&engine);
            let mut r = new_rule("r", "proxy");
            r.domain = Some(vec!["geosite:google".into()]);
            save_scheme(&engine, "s-mode", vec![r]);
            engine.set_rule_mode(RuleMode::Global).expect("mode");
            let (global_main, _) = generate_case(&engine, core, free_port(base + 400));
            engine.set_rule_mode(RuleMode::Direct).expect("mode");
            let (direct_main, _) = generate_case(&engine, core, free_port(base + 500));
            engine.set_rule_mode(RuleMode::Rule).expect("mode");
            let (rule_main, _) = generate_case(&engine, core, free_port(base + 600));
            let (g, d, r) = (
                serde_json::to_string(&global_main).unwrap(),
                serde_json::to_string(&direct_main).unwrap(),
                serde_json::to_string(&rule_main).unwrap(),
            );
            assert_ne!(g, r, "Global must differ from Rule");
            assert_ne!(d, r, "Direct must differ from Rule");
            base += 10;
            results.push(check_case(
                &engine,
                core,
                core_name,
                "rule-mode",
                &dir,
                base,
                &|_, _| {},
            ));
        }
    }
    // 14. sing-box custom ruleset path (file must exist and parse).
    //
    // NOTE: `sing-box check` opens local rule_set files, so the fixture is a
    // real source-format rule-set JSON at an absolute path (relative paths
    // resolve against the test CWD and fail, which `check` caught).
    {
        let engine = AppEngine::in_memory();
        seed_node(&engine);
        let srs_path = dir.join("geosite-test.srs.json");
        std::fs::write(
            &srs_path,
            r#"{"version": 3, "rules": [{"domain_suffix": ["example.com"]}]}"#,
        )
        .expect("write ruleset");
        assert!(srs_path.is_file(), "custom ruleset file must exist");
        let custom_path = dir.join("custom_ruleset.json");
        std::fs::write(
            &custom_path,
            format!(
                r#"[{{"tag": "geosite-test", "type": "local", "format": "source", "path": {}}}]"#,
                serde_json::to_string(&srs_path.to_string_lossy()).unwrap()
            ),
        )
        .expect("write ruleset manifest");
        assert!(custom_path.is_file(), "custom ruleset file must exist");
        let parsed = application::routing::read_custom_ruleset(&custom_path.to_string_lossy());
        assert!(parsed.is_some(), "custom ruleset must parse");
        assert!(
            application::routing::read_custom_ruleset("C:\\nonexistent\\ruleset.json").is_none()
        );
        let mut scheme = RoutingProfile {
            remarks: "s-ruleset".into(),
            custom_ruleset_path4_singbox: custom_path.to_string_lossy().to_string(),
            ..Default::default()
        };
        let mut r = new_rule("r", "proxy");
        r.domain = Some(vec!["geosite:test".into()]);
        scheme.set_rules(&[r]).expect("rules");
        let saved = engine.save_routing(scheme).expect("save");
        engine.set_default_routing(&saved.id).expect("default");
        base += 10;
        results.push(check_case(
            &engine,
            CoreType::SingBox,
            "singbox",
            "custom-ruleset",
            &dir,
            base,
            &|main, _| {
                let text = serde_json::to_string(main).unwrap();
                assert!(text.contains("geosite-test"), "custom ruleset missing");
            },
        ));
        // Same scheme must also validate under Xray (ruleset is sing-box-only).
        results.push(check_case(
            &engine,
            CoreType::Xray,
            "xray",
            "custom-ruleset-ignored",
            &dir,
            base + 5,
            &|_, _| {},
        ));
    }

    write_report(&dir, &results);
    let failed: Vec<_> = results.iter().filter(|r| r.status != "PASS").collect();
    assert!(
        failed.is_empty(),
        "{} of {} cases failed (see target/t11/matrix/report.md)",
        failed.len(),
        results.len()
    );
    // 13 per core (12 shared x2) + 2 ruleset = 28 cases.
    assert_eq!(results.len(), 28, "matrix case count");
}
