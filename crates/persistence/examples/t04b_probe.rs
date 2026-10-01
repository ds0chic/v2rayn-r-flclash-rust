//! Temporary T04b probe. Reads the real upstream sample read-only and reports
//! only structural counts. Never prints node/credential values.

use std::path::{Path, PathBuf};

use persistence::{import_from_path, ConfigDocument, ImportOptions, Store};

fn options() -> ImportOptions {
    ImportOptions {
        now: 1_900_000_000,
        fault: persistence::ImportFault::None,
    }
}

fn copy_db_only(src_db: &Path, dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::copy(src_db, dir.join("guiNDB.db")).unwrap();
}

fn import_once(label: &str, source: &Path, base: &Path) -> Option<PathBuf> {
    let work = base.join(format!("work-{label}"));
    let target = base.join(format!("target-{label}.db"));
    let report = match import_from_path(source, &target, &work, &options()) {
        Ok(r) => r,
        Err(e) => {
            println!("{label}: ERROR {}", e);
            return None;
        }
    };
    println!(
        "{label}: status={:?} committed={} source_version={} migrations={} warnings={} errors={}",
        report.status,
        report.committed,
        report.source_version,
        report.migrations.len(),
        report.warnings.len(),
        report.errors.len(),
    );
    for m in &report.migrations {
        println!(
            "  mig {} {}->{} touched={}",
            m.migration_id, m.from_version, m.to_version, m.entities_touched
        );
    }
    for c in &report.counts {
        println!(
            "  count {} source={} imported={} migrated={} skipped={}",
            c.table, c.source_rows, c.imported_rows, c.migrated_rows, c.skipped_rows
        );
    }
    let mut warn_codes: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
    for w in &report.warnings {
        *warn_codes.entry(w.code.as_str()).or_default() += 1;
    }
    println!("  warn_codes {warn_codes:?}");
    let mut err_codes: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
    for e in &report.errors {
        *err_codes.entry(e.code.as_str()).or_default() += 1;
    }
    println!("  err_codes {err_codes:?}");
    if !report.committed {
        return None;
    }

    let second = import_from_path(source, &target, &work, &options()).unwrap();
    println!("  second_import_status={:?}", second.status);
    let store = Store::open_readonly(&target).unwrap();
    for t in [
        "ProfileItem",
        "SubItem",
        "ServerStatItem",
        "RoutingItem",
        "ProfileExItem",
        "DNSItem",
        "FullConfigTemplateItem",
        "ProfileGroupItem",
    ] {
        println!("  final {}={}", t, store.count_rows(t).unwrap());
    }
    println!(
        "  active_profile_resolved={} active_sub_resolved={}",
        store.get_meta("last_import_fingerprint").unwrap().is_some(),
        store.count_rows("ProfileItem").unwrap() > 0
    );
    Some(target)
}

fn main() {
    let sample =
        PathBuf::from(r"C:\Users\Colby\AppData\Local\Temp\opencode\v2rayn-upstream-sample");
    let base = std::env::temp_dir().join("t04b-probe-out");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    println!("== config accessors (real files) ==");
    for cfg in ["guiNConfig.json", "upstream-old-config-20260602.json"] {
        let text = std::fs::read_to_string(sample.join(cfg)).unwrap();
        let doc = ConfigDocument::parse(&text).unwrap();
        let windows = doc.window_states().map(|v| v.len()).unwrap_or(999);
        let cols = doc.main_columns().map(|v| v.len()).unwrap_or(999);
        let clash = doc.clash_columns().map(|v| v.len()).unwrap_or(999);
        println!(
            "{cfg}: theme={:?} lang={:?} font_size={:?} windows={} main_cols={} clash_cols={}",
            doc.theme(),
            doc.language(),
            doc.font_size(),
            windows,
            cols,
            clash
        );
    }

    println!("== imports ==");
    let _ = import_once("current", &sample, &base);

    let old1 = base.join("src-old1");
    copy_db_only(&sample.join("upstream-old-20260602.db"), &old1);
    let _ = import_once("old1", &old1, &base);

    let old2 = base.join("src-old2");
    copy_db_only(&sample.join("upstream-old-1780395234.db"), &old2);
    let _ = import_once("old2", &old2, &base);

    let bak = base.join("src-bak");
    copy_db_only(&sample.join("upstream-bak-1788539222.db"), &bak);
    let _ = import_once("bak", &bak, &base);

    // Moved-directory idempotency for the current sample.
    let moved = base.join("moved");
    std::fs::create_dir_all(&moved).unwrap();
    for name in ["guiNConfig.json", "guiNDB.db"] {
        std::fs::copy(sample.join(name), moved.join(name)).unwrap();
    }
    let target = base.join("target-current.db");
    let work = base.join("work-current");
    let again = import_from_path(&moved, &target, &work, &options()).unwrap();
    println!(
        "moved-import status={:?} committed={}",
        again.status, again.committed
    );
}
