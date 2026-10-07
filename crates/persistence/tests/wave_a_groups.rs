//! Wave A acceptance: group 5-state retain + patch isolation + reopen (SP-23).
//!
//! One sibling case per persistence-package Wave A instance (FLD-CFG-003..021
//! containers, 106..113/124/126..128/161 leaves, 116 migration). Every case
//! asserts the same gate: canonical value persists, the group keeps its
//! 5-state retention (present / absent / empty / null / partial, unknown keys
//! kept), a targeted patch leaves every other group byte-identical, and an
//! independent reopen reads back the same document.
//!
//! Synthetic data only. No sockets are opened, no OS settings are touched,
//! and no fixture references 127.0.0.1:10808.

use std::path::PathBuf;

use persistence::{
    import_from_path, ConfigDocument, ImportFault, ImportOptions, ImportReport, ImportStatus, Store,
};
use serde_json::Value;

fn options() -> ImportOptions {
    ImportOptions {
        now: 1_900_000_001,
        fault: ImportFault::None,
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    source: PathBuf,
    work: PathBuf,
    target: PathBuf,
}

fn fresh_fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    let work = dir.path().join("work");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let target = dir.path().join("app.db");
    Fixture {
        _dir: dir,
        source,
        work,
        target,
    }
}

fn import_json(fx: &Fixture, json: &str) -> ImportReport {
    std::fs::write(fx.source.join("guiNConfig.json"), json).unwrap();
    import_from_path(&fx.source, &fx.target, &fx.work, &options()).unwrap()
}

fn read_doc(fx: &Fixture) -> ConfigDocument {
    let store = Store::open_readonly(&fx.target).unwrap();
    let raw = store.get_meta("upstream_config").unwrap().unwrap();
    ConfigDocument::parse(&raw).unwrap()
}

fn doc_string(doc: &ConfigDocument) -> String {
    doc.to_json_string().unwrap()
}

fn assert_no_10808(doc: &ConfigDocument) {
    assert!(
        !doc_string(doc).contains("10808"),
        "synthetic fixtures must never reference 10808"
    );
}

/// Sentinel groups bundled with every seed. They never equal `target_key`, so
/// a targeted patch must leave them byte-identical.
fn sentinel_pairs(target_key: &str) -> [(&'static str, &'static str); 2] {
    let first = if target_key == "CheckUpdateItem" {
        ("GrpcItem", r#"{"IdleTimeout":61}"#)
    } else {
        ("CheckUpdateItem", r#"{"CheckPreReleaseUpdate":true}"#)
    };
    let second = if target_key == "SystemProxyItem" {
        (
            "HysteriaItem",
            r#"{"UpMbps":11,"DownMbps":12,"HopInterval":13}"#,
        )
    } else {
        (
            "SystemProxyItem",
            r#"{"SystemProxyExceptions":"syn-bypass.local","NotProxyLocalAddress":true}"#,
        )
    };
    [first, second]
}

fn seed_doc(target_key: &str, target_payload: &str) -> String {
    let [(a_key, a_val), (b_key, b_val)] = sentinel_pairs(target_key);
    format!(
        r#"{{"IndexId":"syn-seed","SubIndexId":"syn-sub","{target_key}":{target_payload},"FutureGroup999":{{"SynFuture":7}},"{a_key}":{a_val},"{b_key}":{b_val}}}"#
    )
}

fn seed_without(target_key: &str) -> String {
    let [(a_key, a_val), (b_key, b_val)] = sentinel_pairs(target_key);
    format!(
        r#"{{"IndexId":"syn-seed","SubIndexId":"syn-sub","FutureGroup999":{{"SynFuture":7}},"{a_key}":{a_val},"{b_key}":{b_val}}}"#
    )
}

fn assert_sentinels_intact(doc: &ConfigDocument, target_key: &str) {
    let [(a_key, a_val), (b_key, b_val)] = sentinel_pairs(target_key);
    let raw = doc.raw();
    let expect_a: Value = serde_json::from_str(a_val).unwrap();
    let expect_b: Value = serde_json::from_str(b_val).unwrap();
    assert_eq!(&raw[a_key], &expect_a, "sentinel {a_key} must survive");
    assert_eq!(&raw[b_key], &expect_b, "sentinel {b_key} must survive");
    assert_eq!(
        raw["FutureGroup999"]["SynFuture"], 7,
        "unknown group must survive"
    );
}

fn assert_reopen_stable(fx: &Fixture, before: &ConfigDocument) {
    let again = read_doc(fx);
    assert_eq!(
        doc_string(&again),
        doc_string(before),
        "independent reopen must read back the same document"
    );
}

/// One object-group acceptance case.
///
/// `present` carries the canonical payload plus a `SynUnknown` key;
/// `patched` changes only the target group's canonical value; `partial`
/// carries a subset of the group's keys. `check` / `check_patched` assert the
/// canonical values on the present / patched group objects.
struct GroupCase {
    fld: &'static str,
    group: &'static str,
    present: &'static str,
    patched: &'static str,
    partial: &'static str,
    check: fn(&Value),
    check_patched: fn(&Value),
}

impl GroupCase {
    fn run(&self) {
        // Present: canonical persists, unknown key kept, sentinels kept.
        let fx = fresh_fixture();
        let report = import_json(&fx, &seed_doc(self.group, self.present));
        assert_eq!(report.status, ImportStatus::Imported, "{}", self.fld);
        let doc = read_doc(&fx);
        let group = doc.raw().get(self.group).unwrap_or(&Value::Null);
        assert!(group.is_object(), "{}: group must persist", self.fld);
        (self.check)(group);
        assert!(
            group.get("SynUnknown").is_some(),
            "{}: unknown key must be retained",
            self.fld
        );
        assert_sentinels_intact(&doc, self.group);
        assert_no_10808(&doc);
        assert_reopen_stable(&fx, &doc);

        // Absent: missing key stays missing, siblings intact.
        let fx = fresh_fixture();
        let report = import_json(&fx, &seed_without(self.group));
        assert_eq!(report.status, ImportStatus::Imported, "{}", self.fld);
        let doc = read_doc(&fx);
        assert!(
            doc.field_state(&[self.group]).is_missing(),
            "{}: absent group must stay missing",
            self.fld
        );
        assert_sentinels_intact(&doc, self.group);
        assert_reopen_stable(&fx, &doc);

        // Empty object: present-but-empty is distinct from absent/null.
        let fx = fresh_fixture();
        let report = import_json(&fx, &seed_doc(self.group, "{}"));
        assert_eq!(report.status, ImportStatus::Imported, "{}", self.fld);
        let doc = read_doc(&fx);
        let group = doc.raw().get(self.group).unwrap_or(&Value::Null);
        assert!(
            group.as_object().is_some_and(|o| o.is_empty()),
            "{}: empty object must roundtrip as empty",
            self.fld
        );
        assert_sentinels_intact(&doc, self.group);
        assert_reopen_stable(&fx, &doc);

        // Explicit null: distinct from absent and empty.
        let fx = fresh_fixture();
        let report = import_json(&fx, &seed_doc(self.group, "null"));
        assert_eq!(report.status, ImportStatus::Imported, "{}", self.fld);
        let doc = read_doc(&fx);
        assert!(
            doc.field_state(&[self.group]).is_null(),
            "{}: null group must stay null",
            self.fld
        );
        assert_sentinels_intact(&doc, self.group);
        assert_reopen_stable(&fx, &doc);

        // Partial: subset of keys persists, no defaults injected into storage.
        let fx = fresh_fixture();
        let report = import_json(&fx, &seed_doc(self.group, self.partial));
        assert_eq!(report.status, ImportStatus::Imported, "{}", self.fld);
        let doc = read_doc(&fx);
        let group = doc.raw().get(self.group).unwrap_or(&Value::Null);
        assert!(group.is_object(), "{}: partial group persists", self.fld);
        assert!(
            group.get("SynUnknown").is_none(),
            "{}: partial must not invent the unknown key",
            self.fld
        );
        assert_sentinels_intact(&doc, self.group);
        assert_reopen_stable(&fx, &doc);

        // Patch isolation: resubmit with only the target group changed; every
        // other top-level key must be byte-identical, then reopen.
        let fx = fresh_fixture();
        import_json(&fx, &seed_doc(self.group, self.present));
        let before = read_doc(&fx);
        let report = import_json(&fx, &seed_doc(self.group, self.patched));
        assert_eq!(report.status, ImportStatus::Imported, "{}", self.fld);
        let after = read_doc(&fx);
        (self.check_patched)(after.raw().get(self.group).unwrap_or(&Value::Null));
        for (key, value) in before.raw().as_object().unwrap() {
            if key == self.group {
                continue;
            }
            assert_eq!(
                &after.raw()[key],
                value,
                "{}: sibling group {key} must be untouched by the patch",
                self.fld
            );
        }
        assert_no_10808(&after);
        assert_reopen_stable(&fx, &after);
    }
}

// --- containers 003-006 (verify the all-fields smoke refs; these are the
// real 5-state sibling cases the ledger rows point at) ---

#[test]
fn fld_003_core_basic_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-003",
        group: "CoreBasicItem",
        present: r#"{"LogEnabled":true,"Loglevel":"debug","SynUnknown":"keep-003"}"#,
        patched: r#"{"LogEnabled":false,"Loglevel":"warning","SynUnknown":"keep-003"}"#,
        partial: r#"{"LogEnabled":true}"#,
        check: |g| {
            assert_eq!(g["LogEnabled"], true);
            assert_eq!(g["Loglevel"], "debug");
        },
        check_patched: |g| {
            assert_eq!(g["LogEnabled"], false);
            assert_eq!(g["Loglevel"], "warning");
        },
    }
    .run();
}

#[test]
fn fld_004_tun_mode_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-004",
        group: "TunModeItem",
        present: r#"{"EnableTun":true,"AutoRoute":true,"Mtu":1500,"SynUnknown":"keep-004"}"#,
        patched: r#"{"EnableTun":false,"AutoRoute":true,"Mtu":1500,"SynUnknown":"keep-004"}"#,
        partial: r#"{"Mtu":1500}"#,
        check: |g| {
            assert_eq!(g["EnableTun"], true);
            assert_eq!(g["Mtu"], 1500);
        },
        check_patched: |g| {
            assert_eq!(g["EnableTun"], false);
            assert_eq!(g["Mtu"], 1500);
        },
    }
    .run();
}

#[test]
fn fld_005_kcp_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-005",
        group: "KcpItem",
        present: r#"{"Mtu":1350,"Tti":20,"SynUnknown":"keep-005"}"#,
        patched: r#"{"Mtu":1400,"Tti":20,"SynUnknown":"keep-005"}"#,
        partial: r#"{"Tti":20}"#,
        check: |g| {
            assert_eq!(g["Mtu"], 1350);
            assert_eq!(g["Tti"], 20);
        },
        check_patched: |g| {
            assert_eq!(g["Mtu"], 1400);
            assert_eq!(g["Tti"], 20);
        },
    }
    .run();
}

#[test]
fn fld_006_grpc_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-006",
        group: "GrpcItem",
        present: r#"{"IdleTimeout":60,"HealthCheckTimeout":20,"SynUnknown":"keep-006"}"#,
        patched: r#"{"IdleTimeout":90,"HealthCheckTimeout":20,"SynUnknown":"keep-006"}"#,
        partial: r#"{"IdleTimeout":60}"#,
        check: |g| {
            assert_eq!(g["IdleTimeout"], 60);
        },
        check_patched: |g| {
            assert_eq!(g["IdleTimeout"], 90);
        },
    }
    .run();
}

#[test]
fn fld_007_routing_basic_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-007",
        group: "RoutingBasicItem",
        present: r#"{"DomainStrategy":"IPIfNonMatch","DomainStrategy4Singbox":"IPIfNonMatch","SynUnknown":"keep-007"}"#,
        patched: r#"{"DomainStrategy":"IPOnDemand","DomainStrategy4Singbox":"IPIfNonMatch","SynUnknown":"keep-007"}"#,
        partial: r#"{"DomainStrategy":"IPIfNonMatch"}"#,
        check: |g| {
            assert_eq!(g["DomainStrategy"], "IPIfNonMatch");
        },
        check_patched: |g| {
            assert_eq!(g["DomainStrategy"], "IPOnDemand");
        },
    }
    .run();
}

#[test]
fn fld_008_gui_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-008",
        group: "GuiItem",
        present: r#"{"EnableLog":true,"TrayMenuServersLimit":25,"SynUnknown":"keep-008"}"#,
        patched: r#"{"EnableLog":false,"TrayMenuServersLimit":25,"SynUnknown":"keep-008"}"#,
        partial: r#"{"TrayMenuServersLimit":25}"#,
        check: |g| {
            assert_eq!(g["EnableLog"], true);
            assert_eq!(g["TrayMenuServersLimit"], 25);
        },
        check_patched: |g| {
            assert_eq!(g["EnableLog"], false);
        },
    }
    .run();
}

#[test]
fn fld_009_msg_ui_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-009",
        group: "MsgUIItem",
        present: r#"{"MainMsgFilter":"syn-filter","AutoRefresh":true,"SynUnknown":"keep-009"}"#,
        patched: r#"{"MainMsgFilter":"syn-filter-v2","AutoRefresh":true,"SynUnknown":"keep-009"}"#,
        partial: r#"{"AutoRefresh":true}"#,
        check: |g| {
            assert_eq!(g["MainMsgFilter"], "syn-filter");
            assert_eq!(g["AutoRefresh"], true);
        },
        check_patched: |g| {
            assert_eq!(g["MainMsgFilter"], "syn-filter-v2");
        },
    }
    .run();
}

#[test]
fn fld_010_ui_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-010",
        group: "UiItem",
        present: r#"{"CurrentTheme":"Dark","CurrentLanguage":"en","CurrentFontSize":13,"SynUnknown":"keep-010"}"#,
        patched: r#"{"CurrentTheme":"Light","CurrentLanguage":"en","CurrentFontSize":13,"SynUnknown":"keep-010"}"#,
        partial: r#"{"CurrentTheme":"Dark"}"#,
        check: |g| {
            assert_eq!(g["CurrentTheme"], "Dark");
            assert_eq!(g["CurrentFontSize"], 13);
        },
        check_patched: |g| {
            assert_eq!(g["CurrentTheme"], "Light");
        },
    }
    .run();
}

#[test]
fn fld_011_const_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-011",
        group: "ConstItem",
        present: r#"{"SubConvertUrl":"https://syn.example/convert","GeoSourceUrl":"https://syn.example/geo","SynUnknown":"keep-011"}"#,
        patched: r#"{"SubConvertUrl":"https://syn.example/convert-v2","GeoSourceUrl":"https://syn.example/geo","SynUnknown":"keep-011"}"#,
        partial: r#"{"SubConvertUrl":"https://syn.example/convert"}"#,
        check: |g| {
            assert_eq!(g["SubConvertUrl"], "https://syn.example/convert");
        },
        check_patched: |g| {
            assert_eq!(g["SubConvertUrl"], "https://syn.example/convert-v2");
        },
    }
    .run();
}

#[test]
fn fld_012_speed_test_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-012",
        group: "SpeedTestItem",
        present: r#"{"SpeedTestTimeout":15,"MixedConcurrencyCount":4,"SynUnknown":"keep-012"}"#,
        patched: r#"{"SpeedTestTimeout":20,"MixedConcurrencyCount":4,"SynUnknown":"keep-012"}"#,
        partial: r#"{"SpeedTestTimeout":15}"#,
        check: |g| {
            assert_eq!(g["SpeedTestTimeout"], 15);
            assert_eq!(g["MixedConcurrencyCount"], 4);
        },
        check_patched: |g| {
            assert_eq!(g["SpeedTestTimeout"], 20);
            assert_eq!(g["MixedConcurrencyCount"], 4);
        },
    }
    .run();
}

#[test]
fn fld_013_mux4ray_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-013",
        group: "Mux4RayItem",
        present: r#"{"Concurrency":8,"XudpConcurrency":16,"SynUnknown":"keep-013"}"#,
        patched: r#"{"Concurrency":4,"XudpConcurrency":16,"SynUnknown":"keep-013"}"#,
        partial: r#"{"Concurrency":8}"#,
        check: |g| {
            assert_eq!(g["Concurrency"], 8);
        },
        check_patched: |g| {
            assert_eq!(g["Concurrency"], 4);
        },
    }
    .run();
}

#[test]
fn fld_014_mux4sbox_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-014",
        group: "Mux4SboxItem",
        present: r#"{"Protocol":"h2mux","MaxConnections":4,"SynUnknown":"keep-014"}"#,
        patched: r#"{"Protocol":"smux","MaxConnections":4,"SynUnknown":"keep-014"}"#,
        partial: r#"{"Protocol":"h2mux"}"#,
        check: |g| {
            assert_eq!(g["Protocol"], "h2mux");
            assert_eq!(g["MaxConnections"], 4);
        },
        check_patched: |g| {
            assert_eq!(g["Protocol"], "smux");
        },
    }
    .run();
}

#[test]
fn fld_015_hysteria_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-015",
        group: "HysteriaItem",
        present: r#"{"UpMbps":250,"DownMbps":500,"HopInterval":45,"SynUnknown":"keep-015"}"#,
        patched: r#"{"UpMbps":100,"DownMbps":500,"HopInterval":45,"SynUnknown":"keep-015"}"#,
        partial: r#"{"UpMbps":250}"#,
        check: |g| {
            assert_eq!(g["UpMbps"], 250);
            assert_eq!(g["DownMbps"], 500);
            assert_eq!(g["HopInterval"], 45);
        },
        check_patched: |g| {
            assert_eq!(g["UpMbps"], 100);
        },
    }
    .run();
}

#[test]
fn fld_016_clash_ui_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-016",
        group: "ClashUIItem",
        present: r#"{"EnableIPv6":true,"ProxiesSorting":1,"SynUnknown":"keep-016"}"#,
        patched: r#"{"EnableIPv6":false,"ProxiesSorting":1,"SynUnknown":"keep-016"}"#,
        partial: r#"{"EnableIPv6":true}"#,
        check: |g| {
            assert_eq!(g["EnableIPv6"], true);
        },
        check_patched: |g| {
            assert_eq!(g["EnableIPv6"], false);
        },
    }
    .run();
}

#[test]
fn fld_017_system_proxy_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-017",
        group: "SystemProxyItem",
        present: r#"{"SysProxyType":1,"SystemProxyExceptions":"syn-bypass-017","SynUnknown":"keep-017"}"#,
        patched: r#"{"SysProxyType":2,"SystemProxyExceptions":"syn-bypass-017","SynUnknown":"keep-017"}"#,
        partial: r#"{"SysProxyType":1}"#,
        check: |g| {
            assert_eq!(g["SysProxyType"], 1);
        },
        check_patched: |g| {
            assert_eq!(g["SysProxyType"], 2);
        },
    }
    .run();
}

#[test]
fn fld_018_webdav_item_group_retain_patch_reopen() {
    // Secrets-adjacent group: synthetic placeholders only, never real
    // credentials; values are stored verbatim and never logged.
    GroupCase {
        fld: "FLD-CFG-018",
        group: "WebDavItem",
        present: r#"{"Url":"https://syn.example/dav","UserName":"syn-user","Password":"SYNTHETIC-PLACEHOLDER","DirName":"syn-dir","SynUnknown":"keep-018"}"#,
        patched: r#"{"Url":"https://syn.example/dav-v2","UserName":"syn-user","Password":"SYNTHETIC-PLACEHOLDER","DirName":"syn-dir","SynUnknown":"keep-018"}"#,
        partial: r#"{"Url":"https://syn.example/dav"}"#,
        check: |g| {
            assert_eq!(g["Url"], "https://syn.example/dav");
            assert_eq!(g["Password"], "SYNTHETIC-PLACEHOLDER");
        },
        check_patched: |g| {
            assert_eq!(g["Url"], "https://syn.example/dav-v2");
        },
    }
    .run();
}

#[test]
fn fld_019_check_update_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-019",
        group: "CheckUpdateItem",
        present: r#"{"CheckPreReleaseUpdate":true,"UpdateViaProxy":true,"SynUnknown":"keep-019"}"#,
        patched: r#"{"CheckPreReleaseUpdate":false,"UpdateViaProxy":true,"SynUnknown":"keep-019"}"#,
        partial: r#"{"CheckPreReleaseUpdate":true}"#,
        check: |g| {
            assert_eq!(g["CheckPreReleaseUpdate"], true);
        },
        check_patched: |g| {
            assert_eq!(g["CheckPreReleaseUpdate"], false);
        },
    }
    .run();
}

#[test]
fn fld_020_fragment_item_group_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-020",
        group: "Fragment4RayItem",
        present: r#"{"Packets":"tlshello","Lengths":["100-200"],"Delays":["200-300"],"SynUnknown":"keep-020"}"#,
        patched: r#"{"Packets":"tlshello","Lengths":["50-100"],"Delays":["200-300"],"SynUnknown":"keep-020"}"#,
        partial: r#"{"Packets":"tlshello"}"#,
        check: |g| {
            assert_eq!(g["Packets"], "tlshello");
            assert_eq!(g["Lengths"], serde_json::json!(["100-200"]));
        },
        check_patched: |g| {
            assert_eq!(g["Lengths"], serde_json::json!(["50-100"]));
        },
    }
    .run();
}

// --- SP-29 leaves 106-109/111/113 (SpeedTestItem leaf isolation) ---

#[test]
fn fld_106_speed_test_timeout_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-106",
        group: "SpeedTestItem",
        present: r#"{"SpeedTestTimeout":15,"SpeedTestUrl":"https://syn.example/speed-106.bin","SynUnknown":"keep-106"}"#,
        patched: r#"{"SpeedTestTimeout":30,"SpeedTestUrl":"https://syn.example/speed-106.bin","SynUnknown":"keep-106"}"#,
        partial: r#"{"SpeedTestTimeout":15}"#,
        check: |g| {
            assert_eq!(g["SpeedTestTimeout"], 15);
        },
        check_patched: |g| {
            assert_eq!(g["SpeedTestTimeout"], 30);
            assert_eq!(
                g["SpeedTestUrl"], "https://syn.example/speed-106.bin",
                "sibling leaf must survive the leaf patch"
            );
        },
    }
    .run();
}

#[test]
fn fld_107_speed_test_url_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-107",
        group: "SpeedTestItem",
        present: r#"{"SpeedTestUrl":"https://syn.example/speed-107.bin","SpeedTestTimeout":15,"SynUnknown":"keep-107"}"#,
        patched: r#"{"SpeedTestUrl":"https://syn.example/speed-107-v2.bin","SpeedTestTimeout":15,"SynUnknown":"keep-107"}"#,
        partial: r#"{"SpeedTestUrl":"https://syn.example/speed-107.bin"}"#,
        check: |g| {
            assert_eq!(g["SpeedTestUrl"], "https://syn.example/speed-107.bin");
        },
        check_patched: |g| {
            assert_eq!(g["SpeedTestUrl"], "https://syn.example/speed-107-v2.bin");
            assert_eq!(
                g["SpeedTestTimeout"], 15,
                "sibling leaf must survive the leaf patch"
            );
        },
    }
    .run();
}

#[test]
fn fld_108_speed_ping_test_url_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-108",
        group: "SpeedTestItem",
        present: r#"{"SpeedPingTestUrl":"https://syn.example/generate_204","SpeedTestTimeout":15,"SynUnknown":"keep-108"}"#,
        patched: r#"{"SpeedPingTestUrl":"https://syn.example/generate_204-v2","SpeedTestTimeout":15,"SynUnknown":"keep-108"}"#,
        partial: r#"{"SpeedPingTestUrl":"https://syn.example/generate_204"}"#,
        check: |g| {
            assert_eq!(g["SpeedPingTestUrl"], "https://syn.example/generate_204");
        },
        check_patched: |g| {
            assert_eq!(
                g["SpeedPingTestUrl"],
                "https://syn.example/generate_204-v2"
            );
            assert_eq!(
                g["SpeedTestTimeout"], 15,
                "sibling leaf must survive the leaf patch"
            );
        },
    }
    .run();
}

#[test]
fn fld_109_mixed_concurrency_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-109",
        group: "SpeedTestItem",
        present: r#"{"MixedConcurrencyCount":4,"SpeedTestTimeout":15,"SynUnknown":"keep-109"}"#,
        patched: r#"{"MixedConcurrencyCount":1,"SpeedTestTimeout":15,"SynUnknown":"keep-109"}"#,
        partial: r#"{"MixedConcurrencyCount":4}"#,
        check: |g| {
            assert_eq!(g["MixedConcurrencyCount"], 4);
        },
        check_patched: |g| {
            assert_eq!(g["MixedConcurrencyCount"], 1);
            assert_eq!(
                g["SpeedTestTimeout"], 15,
                "sibling leaf must survive the leaf patch"
            );
        },
    }
    .run();
}

#[test]
fn fld_111_udp_test_target_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-111",
        group: "SpeedTestItem",
        present: r#"{"UdpTestTarget":"ntp:syn.example","SpeedTestTimeout":15,"SynUnknown":"keep-111"}"#,
        patched: r#"{"UdpTestTarget":"ntp:syn-v2.example","SpeedTestTimeout":15,"SynUnknown":"keep-111"}"#,
        partial: r#"{"UdpTestTarget":"ntp:syn.example"}"#,
        check: |g| {
            assert_eq!(g["UdpTestTarget"], "ntp:syn.example");
        },
        check_patched: |g| {
            assert_eq!(g["UdpTestTarget"], "ntp:syn-v2.example");
            assert_eq!(
                g["SpeedTestTimeout"], 15,
                "sibling leaf must survive the leaf patch"
            );
        },
    }
    .run();
}

#[test]
fn fld_113_speed_test_delay_interval_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-113",
        group: "SpeedTestItem",
        present: r#"{"SpeedTestDelayInterval":3,"SpeedTestTimeout":15,"SynUnknown":"keep-113"}"#,
        patched: r#"{"SpeedTestDelayInterval":5,"SpeedTestTimeout":15,"SynUnknown":"keep-113"}"#,
        partial: r#"{"SpeedTestDelayInterval":3}"#,
        check: |g| {
            assert_eq!(g["SpeedTestDelayInterval"], 3);
        },
        check_patched: |g| {
            assert_eq!(g["SpeedTestDelayInterval"], 5);
            assert_eq!(
                g["SpeedTestTimeout"], 15,
                "sibling leaf must survive the leaf patch"
            );
        },
    }
    .run();
}

// --- SP-24 leaves with persistence references ---

#[test]
fn fld_124_mux_max_connections_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-124",
        group: "Mux4SboxItem",
        present: r#"{"MaxConnections":4,"Protocol":"h2mux","SynUnknown":"keep-124"}"#,
        patched: r#"{"MaxConnections":16,"Protocol":"h2mux","SynUnknown":"keep-124"}"#,
        partial: r#"{"MaxConnections":4}"#,
        check: |g| {
            assert_eq!(g["MaxConnections"], 4);
        },
        check_patched: |g| {
            assert_eq!(g["MaxConnections"], 16);
            assert_eq!(
                g["Protocol"], "h2mux",
                "sibling leaf must survive the leaf patch"
            );
        },
    }
    .run();
}

#[test]
fn fld_126_hysteria_up_mbps_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-126",
        group: "HysteriaItem",
        present: r#"{"UpMbps":250,"DownMbps":500,"HopInterval":45,"SynUnknown":"keep-126"}"#,
        patched: r#"{"UpMbps":100,"DownMbps":500,"HopInterval":45,"SynUnknown":"keep-126"}"#,
        partial: r#"{"UpMbps":250}"#,
        check: |g| {
            assert_eq!(g["UpMbps"], 250);
        },
        check_patched: |g| {
            assert_eq!(g["UpMbps"], 100);
            assert_eq!(g["DownMbps"], 500);
            assert_eq!(g["HopInterval"], 45);
        },
    }
    .run();
}

#[test]
fn fld_127_hysteria_down_mbps_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-127",
        group: "HysteriaItem",
        present: r#"{"UpMbps":250,"DownMbps":500,"HopInterval":45,"SynUnknown":"keep-127"}"#,
        patched: r#"{"UpMbps":250,"DownMbps":100,"HopInterval":45,"SynUnknown":"keep-127"}"#,
        partial: r#"{"DownMbps":500}"#,
        check: |g| {
            assert_eq!(g["DownMbps"], 500);
        },
        check_patched: |g| {
            assert_eq!(g["DownMbps"], 100);
            assert_eq!(g["UpMbps"], 250);
            assert_eq!(g["HopInterval"], 45);
        },
    }
    .run();
}

#[test]
fn fld_128_hysteria_hop_interval_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-128",
        group: "HysteriaItem",
        present: r#"{"UpMbps":250,"DownMbps":500,"HopInterval":45,"SynUnknown":"keep-128"}"#,
        patched: r#"{"UpMbps":250,"DownMbps":500,"HopInterval":30,"SynUnknown":"keep-128"}"#,
        partial: r#"{"HopInterval":45}"#,
        check: |g| {
            assert_eq!(g["HopInterval"], 45);
        },
        check_patched: |g| {
            assert_eq!(g["HopInterval"], 30);
            assert_eq!(g["UpMbps"], 250);
            assert_eq!(g["DownMbps"], 500);
        },
    }
    .run();
}

#[test]
fn fld_161_simple_dns_fake_ip_leaf_retain_patch_reopen() {
    GroupCase {
        fld: "FLD-CFG-161",
        group: "SimpleDNSItem",
        present: r#"{"FakeIP":true,"RemoteDNS":"syn-dns.example","SynUnknown":"keep-161"}"#,
        patched: r#"{"FakeIP":false,"RemoteDNS":"syn-dns.example","SynUnknown":"keep-161"}"#,
        partial: r#"{"FakeIP":true}"#,
        check: |g| {
            assert_eq!(g["FakeIP"], true);
        },
        check_patched: |g| {
            assert_eq!(g["FakeIP"], false);
            assert_eq!(
                g["RemoteDNS"], "syn-dns.example",
                "sibling leaf must survive the leaf patch"
            );
        },
    }
    .run();
}

// --- FLD-CFG-021: root-level Inbound list (bespoke: array, not object) ---

#[test]
fn fld_021_inbound_list_retain_patch_reopen() {
    let listener = r#"{"LocalPort":11911,"Protocol":0,"UdpEnabled":true,"SynListener":"keep-021"}"#;
    let seed = format!(
        r#"{{"IndexId":"syn-seed","SubIndexId":"syn-sub","Inbound":[{listener}],"FutureGroup999":{{"SynFuture":7}},"CheckUpdateItem":{{"CheckPreReleaseUpdate":true}},"SystemProxyItem":{{"SystemProxyExceptions":"syn-bypass.local"}}}}"#
    );
    let fx = fresh_fixture();
    let report = import_json(&fx, &seed);
    assert_eq!(report.status, ImportStatus::Imported);
    let doc = read_doc(&fx);
    let inbound = doc.raw().get("Inbound").unwrap_or(&Value::Null);
    assert!(inbound.is_array(), "Inbound list must persist");
    assert_eq!(inbound.as_array().unwrap().len(), 1);
    assert_eq!(inbound[0]["LocalPort"], 11911);
    assert_eq!(inbound[0]["SynListener"], "keep-021");
    assert_eq!(doc.raw()["CheckUpdateItem"]["CheckPreReleaseUpdate"], true);
    assert_no_10808(&doc);
    assert_reopen_stable(&fx, &doc);

    // Absent stays absent; empty list roundtrips as empty; null stays null.
    for (state, payload) in [
        ("absent", r#"{"IndexId":"syn-seed"}"#.to_string()),
        (
            "empty",
            r#"{"IndexId":"syn-seed","Inbound":[]}"#.to_string(),
        ),
        (
            "null",
            r#"{"IndexId":"syn-seed","Inbound":null}"#.to_string(),
        ),
    ] {
        let fx = fresh_fixture();
        let report = import_json(&fx, &payload);
        assert_eq!(report.status, ImportStatus::Imported, "{state}");
        let doc = read_doc(&fx);
        match state {
            "absent" => assert!(doc.field_state(&["Inbound"]).is_missing()),
            "empty" => assert!(
                doc.raw()["Inbound"]
                    .as_array()
                    .is_some_and(|a| a.is_empty()),
                "empty Inbound must roundtrip as empty"
            ),
            _ => assert!(doc.field_state(&["Inbound"]).is_null()),
        }
        assert_reopen_stable(&fx, &doc);
    }

    // Patch: change only the listener port; siblings byte-identical, reopen.
    let fx = fresh_fixture();
    import_json(&fx, &seed);
    let before = read_doc(&fx);
    let patched_listener =
        r#"{"LocalPort":11912,"Protocol":0,"UdpEnabled":true,"SynListener":"keep-021"}"#;
    let patched = format!(
        r#"{{"IndexId":"syn-seed","SubIndexId":"syn-sub","Inbound":[{patched_listener}],"FutureGroup999":{{"SynFuture":7}},"CheckUpdateItem":{{"CheckPreReleaseUpdate":true}},"SystemProxyItem":{{"SystemProxyExceptions":"syn-bypass.local"}}}}"#
    );
    let report = import_json(&fx, &patched);
    assert_eq!(report.status, ImportStatus::Imported);
    let after = read_doc(&fx);
    assert_eq!(after.raw()["Inbound"][0]["LocalPort"], 11912);
    for (key, value) in before.raw().as_object().unwrap() {
        if key == "Inbound" {
            continue;
        }
        assert_eq!(
            &after.raw()[key],
            value,
            "sibling group {key} must be untouched by the Inbound patch"
        );
    }
    assert_no_10808(&after);
    assert_reopen_stable(&fx, &after);
}

// --- FLD-CFG-116: legacy RoutingIndexId -> IsActive roundtrip (G-14) ---
//
// The one-way legacy->IsActive migration producer is not implemented (gap
// G-14, owner SP-13): no import path flips `RoutingItem.IsActive`. This case
// pins the roundtrip half of the gate: the legacy id is preserved verbatim
// through import + reopen for both a known and an unknown id, and neither
// import touches any `IsActive` flag.

#[test]
fn fld_116_routing_index_id_legacy_roundtrip_no_clobber() {
    use persistence::UPSTREAM_TABLES;
    use rusqlite::Connection;

    let fx = fresh_fixture();
    let source_db = fx.source.join("guiNDB.db");
    {
        let conn = Connection::open(&source_db).unwrap();
        for table in UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        conn.execute(
            "INSERT INTO RoutingItem (Id, Remarks, Enabled, IsActive) VALUES ('route-a', 'syn-a', 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO RoutingItem (Id, Remarks, Enabled, IsActive) VALUES ('route-b', 'syn-b', 1, 0)",
            [],
        )
        .unwrap();
        conn.close().unwrap();
    }

    // Known legacy id: preserved verbatim, IsActive flags untouched.
    let seed = r#"{"IndexId":"syn-seed","RoutingBasicItem":{"DomainStrategy":"IPIfNonMatch","RoutingIndexId":"route-a"}}"#;
    let report = import_json(&fx, seed);
    assert_eq!(report.status, ImportStatus::Imported);
    let doc = read_doc(&fx);
    assert_eq!(doc.raw()["RoutingBasicItem"]["RoutingIndexId"], "route-a");
    assert_no_10808(&doc);
    let flag_map = |fx: &Fixture| {
        // Imported rows get content-derived ids (id_map), and a changed
        // config hash re-imports source rows as a new batch; match on the
        // synthetic remarks instead of ids or exact counts.
        let store = Store::open_readonly(&fx.target).unwrap();
        let rows = store.read_rows("RoutingItem").unwrap();
        assert!(!rows.is_empty(), "routing rows must survive the import");
        rows.iter()
            .map(|r| (r.string("Remarks"), r.bool("IsActive")))
            .collect::<Vec<_>>()
    };
    let check_flags = |fx: &Fixture, why: &str| {
        for (remarks, is_active) in flag_map(fx) {
            let expect = match remarks.as_str() {
                "syn-a" => true,
                "syn-b" => false,
                other => panic!("unexpected routing row {other}"),
            };
            assert_eq!(is_active, expect, "{why}");
        }
    };
    check_flags(&fx, "config import must not flip IsActive flags");
    assert_reopen_stable(&fx, &doc);

    // Unknown legacy id: falls back (no selection change), id still stored
    // verbatim, flags still untouched.
    let patched = r#"{"IndexId":"syn-seed","RoutingBasicItem":{"DomainStrategy":"IPIfNonMatch","RoutingIndexId":"route-zzz-unknown"}}"#;
    let report = import_json(&fx, patched);
    assert_eq!(report.status, ImportStatus::Imported);
    let after = read_doc(&fx);
    assert_eq!(
        after.raw()["RoutingBasicItem"]["RoutingIndexId"],
        "route-zzz-unknown"
    );
    check_flags(
        &fx,
        "unknown id must fall back without touching IsActive flags",
    );
    assert_no_10808(&after);
    assert_reopen_stable(&fx, &after);
}
