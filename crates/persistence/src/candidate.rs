//! The six-step upstream import flow (plan §11).
//!
//! 1. identify the source read-only;
//! 2. take a consistent snapshot (SQLite backup API, WAL included);
//! 3. import into a **candidate** database, remapping old ids to new and
//!    retaining every raw source row;
//! 4. validate counts/fields/references/active-node/paths/unknown keys;
//! 5. produce a machine- and user-readable report;
//! 6. commit only if validation passed — otherwise the target is untouched.
//!
//! The candidate starts as a consistent copy of the existing target (when one
//! exists) so a later, unrelated import appends instead of overwriting user
//! data. Re-importing the same fingerprint is detected up front and is a no-op.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::{DnsProfile, FullConfigTemplate, Profile, RoutingProfile, Subscription, TrafficStats};

use crate::batch::ImportBatch;
use crate::blobs::ProtocolExtraBlob;
use crate::error::{PersistenceError, Result};
use crate::hash::derived_id;
use crate::mapping::{
    map_dns, map_profile, map_routing_profile, map_subscription, map_template, map_traffic,
    ProfileExRow,
};
use crate::migrate::{run_migrations, HysteriaMigrationInput, MigrationStats};
use crate::report::{
    EntityCount, ImportReport, ImportStatus, MigrationReport, ReportIssue, ValidationSummary,
};
use crate::rows::RawRow;
use crate::store::Store;
use crate::upstream_db::{self, UpstreamSnapshot};
use crate::validate::{validate_candidate, CandidateView};

/// Optional fault injection, used by tests to exercise the commit gate
/// (plan §17: mocks are allowed for failure injection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportFault {
    #[default]
    None,
    /// Force candidate validation to fail after the candidate is built.
    ValidationFailure,
}

/// Import tuning. `now` is injected so reports are deterministic.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    pub now: i64,
    pub fault: ImportFault,
}

/// How an upstream source is folded into the target database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportMode {
    /// Append the source rows to the existing target (migration import).
    #[default]
    Merge,
    /// Replace the target with the source rows only (upstream restore, where
    /// the source `guiNDB.db` is the complete desired database).
    Replace,
}

/// Import an upstream source into `target_db`, appending to any existing rows
/// (the migration/merge flow).
///
/// `work_dir` must be a caller-owned temporary directory. Nothing outside
/// `work_dir` is written until the single atomic commit at the end.
pub fn import_from_path(
    source_path: &Path,
    target_db: &Path,
    work_dir: &Path,
    options: &ImportOptions,
) -> Result<ImportReport> {
    import_from_path_with_mode(source_path, target_db, work_dir, options, ImportMode::Merge)
}

/// Restore an upstream source into `target_db`, replacing the existing database
/// with the source rows only (upstream `BackupAndRestoreViewModel`: the selected
/// `guiNDB.db` becomes the database).
///
/// Same six-step gate as [`import_from_path`]; only the candidate seed differs:
/// no existing target rows are copied into the candidate.
pub fn restore_from_path(
    source_path: &Path,
    target_db: &Path,
    work_dir: &Path,
    options: &ImportOptions,
) -> Result<ImportReport> {
    import_from_path_with_mode(
        source_path,
        target_db,
        work_dir,
        options,
        ImportMode::Replace,
    )
}

fn import_from_path_with_mode(
    source_path: &Path,
    target_db: &Path,
    work_dir: &Path,
    options: &ImportOptions,
    mode: ImportMode,
) -> Result<ImportReport> {
    std::fs::create_dir_all(work_dir)?;

    let source = match upstream_db::identify(source_path) {
        Ok(source) => source,
        Err(err @ (PersistenceError::NotASource(_) | PersistenceError::Corrupt(_))) => {
            return Ok(rejected_report(
                &source_path.to_string_lossy(),
                "unknown",
                &err.to_string(),
            ))
        }
        Err(err) => return Err(err),
    };

    let snapshot = upstream_db::snapshot(&source, &work_dir.join("snapshot"))?;
    let source_fingerprint = snapshot.fingerprint();

    // Idempotency: same source fingerprint already recorded -> no-op.
    if target_db.exists() {
        let existing = Store::open_readonly(target_db)?;
        if let Some(batch) = existing.find_batch(&source_fingerprint)? {
            return Ok(already_imported_report(&batch, &snapshot));
        }
    }

    let candidate_path = work_dir.join("candidate.db");
    remove_sqlite_files(&candidate_path);
    // Merge keeps the existing target as the candidate seed; replace starts
    // from an empty database so only the restored source rows survive.
    if mode == ImportMode::Merge && target_db.exists() {
        upstream_db::consistent_copy(target_db, &candidate_path)?;
    }
    let store = Store::create(&candidate_path)?;

    let outcome = build_candidate(&store, &snapshot, options)?;

    // Validation gate.
    let warnings = outcome.warnings;
    let mut errors = outcome.errors;
    if options.fault == ImportFault::ValidationFailure {
        errors.push(ReportIssue::new(
            "E_FAULT_INJECTED",
            "injected validation failure",
        ));
    }

    if !errors.is_empty() {
        drop(store);
        remove_sqlite_files(&candidate_path);
        let mut report = base_report(
            &snapshot,
            outcome.counts,
            outcome.migrations,
            warnings,
            errors,
        );
        report.status = ImportStatus::Failed;
        report.validation = outcome.validation;
        report.user_summary = report.render_user_summary();
        return Ok(report);
    }

    drop(store);
    let target_backup = commit_candidate(&candidate_path, target_db)?;
    let mut report = base_report(
        &snapshot,
        outcome.counts,
        outcome.migrations,
        warnings,
        errors,
    );
    report.status = ImportStatus::Imported;
    report.validation = outcome.validation;
    report.committed = true;
    report.target_backup = target_backup;
    report.batch_id = outcome.batch_id;
    report.user_summary = report.render_user_summary();
    Ok(report)
}

struct CandidateOutcome {
    batch_id: String,
    counts: Vec<EntityCount>,
    migrations: Vec<MigrationReport>,
    warnings: Vec<ReportIssue>,
    errors: Vec<ReportIssue>,
    validation: ValidationSummary,
}

fn build_candidate(
    store: &Store,
    snapshot: &UpstreamSnapshot,
    options: &ImportOptions,
) -> Result<CandidateOutcome> {
    let source_fingerprint = snapshot.fingerprint();
    let batch_id = derived_id("batch", &source_fingerprint);
    let mut warnings = Vec::new();

    let group_rows = snapshot.rows("ProfileGroupItem").to_vec();
    let source_profiles = snapshot.rows("ProfileItem").to_vec();
    let hysteria = snapshot
        .config
        .as_ref()
        .map(|c| c.hysteria_item())
        .unwrap_or_default();
    let hysteria_input = HysteriaMigrationInput {
        up_mbps: hysteria.up_mbps,
        down_mbps: hysteria.down_mbps,
        hop_interval: hysteria.hop_interval,
    };

    let (migrated_profiles, migration_stats) =
        run_migrations(source_profiles.clone(), &group_rows, hysteria_input);
    for failure in &migration_stats.failures {
        warnings.push(
            ReportIssue::new(
                "W_MIGRATION",
                format!("{}: {}", failure.migration_id, failure.message),
            )
            .with_entity("ProfileItem", &failure.source_id),
        );
    }

    let subs_source = snapshot.rows("SubItem").to_vec();
    let routing_source = snapshot.rows("RoutingItem").to_vec();
    let dns_source = snapshot.rows("DNSItem").to_vec();
    let template_source = snapshot.rows("FullConfigTemplateItem").to_vec();
    let stat_source = snapshot.rows("ServerStatItem").to_vec();
    let ex_source = snapshot.rows("ProfileExItem").to_vec();

    let maps = build_id_maps(
        &source_fingerprint,
        &migrated_profiles,
        &subs_source,
        &routing_source,
        &dns_source,
        &template_source,
    );

    let profiles = remap_profiles(&migrated_profiles, &maps)?;
    let subs = remap_simple(&subs_source, "Id", &maps.subs);
    let routing = remap_simple(&routing_source, "Id", &maps.routing);
    let dns = remap_simple(&dns_source, "Id", &maps.dns);
    let templates = remap_simple(&template_source, "Id", &maps.templates);
    let stats = remap_simple(&stat_source, "IndexId", &maps.profiles);
    let ex = remap_simple(&ex_source, "IndexId", &maps.profiles);

    let now = options.now;

    let tx = store.begin()?;
    // Raw retention for every source row, before any transformation.
    for row in snapshot.rows("ProfileItem") {
        store.insert_raw_record(
            &tx,
            &batch_id,
            "ProfileItem",
            &row.string("IndexId"),
            &row.to_json(),
        )?;
    }
    for row in &group_rows {
        store.insert_raw_record(
            &tx,
            &batch_id,
            "ProfileGroupItem",
            &row.string("IndexId"),
            &row.to_json(),
        )?;
    }
    for (table, rows) in [
        ("SubItem", &subs_source),
        ("RoutingItem", &routing_source),
        ("DNSItem", &dns_source),
        ("FullConfigTemplateItem", &template_source),
        ("ServerStatItem", &stat_source),
        ("ProfileExItem", &ex_source),
    ] {
        let id_column = if table == "SubItem" {
            "Id"
        } else {
            id_column_for(table)
        };
        for row in rows {
            store.insert_raw_record(
                &tx,
                &batch_id,
                table,
                &row.string(id_column),
                &row.to_json(),
            )?;
        }
    }

    // id_map old -> new.
    for (old, new) in &maps.profiles {
        store.insert_id_map(&tx, &batch_id, "ProfileItem", old, new)?;
    }
    for (old, new) in &maps.subs {
        store.insert_id_map(&tx, &batch_id, "SubItem", old, new)?;
    }
    for (old, new) in &maps.routing {
        store.insert_id_map(&tx, &batch_id, "RoutingItem", old, new)?;
    }
    for (old, new) in &maps.dns {
        store.insert_id_map(&tx, &batch_id, "DNSItem", old, new)?;
    }
    for (old, new) in &maps.templates {
        store.insert_id_map(&tx, &batch_id, "FullConfigTemplateItem", old, new)?;
    }

    // Upstream tables (migrated + remapped).
    for row in &profiles {
        store.insert_row(&tx, row)?;
    }
    for row in &subs {
        store.insert_row(&tx, row)?;
    }
    for row in &routing {
        store.insert_row(&tx, row)?;
    }
    for row in &dns {
        store.insert_row(&tx, row)?;
    }
    for row in &templates {
        store.insert_row(&tx, row)?;
    }
    for row in &stats {
        store.insert_row(&tx, row)?;
    }
    for row in &ex {
        store.insert_row(&tx, row)?;
    }
    // ProfileGroupItem is a migration source only; its structure exists but the
    // rows are deliberately not imported as live data (MIG-ENT-002).

    // Migration journal.
    let migration_reports = record_migrations(store, &tx, &migration_stats, snapshot, now)?;

    if let Some(config) = &snapshot.config {
        store.set_meta(&tx, "upstream_config", &config.to_json_string()?)?;
        // Batch-scoped copy keyed by this source's fingerprint, so a later
        // activation reads this import's own config and never another batch's
        // global meta (R3-SET-01).
        store.set_meta(
            &tx,
            &format!("upstream_config:{source_fingerprint}"),
            &config.to_json_string()?,
        )?;
    } else {
        store.set_meta(&tx, "upstream_config", "{}")?;
        store.set_meta(&tx, &format!("upstream_config:{source_fingerprint}"), "{}")?;
    }
    store.set_meta(&tx, "last_import_fingerprint", &source_fingerprint)?;
    store.set_meta(
        &tx,
        "last_import_source_version",
        &snapshot.version.to_string(),
    )?;

    let counts = build_counts(
        &source_profiles,
        &profiles,
        &subs,
        &routing,
        &dns,
        &templates,
        &stats,
        &ex,
        &group_rows,
        &migration_stats,
    );
    store.record_import_batch(
        &tx,
        &ImportBatch {
            source_fingerprint: source_fingerprint.clone(),
            source_id: snapshot.source_id.clone(),
            content_hash: snapshot.content_hash.clone(),
            batch_id: batch_id.clone(),
            source_kind: snapshot.source_kind.as_str().to_string(),
            source_version: snapshot.version,
            imported_at: now,
            entity_counts: counts.clone(),
        },
    )?;
    tx.commit()?;

    // Build the validation view from the remapped entities.
    let mut typed_profiles: Vec<Profile> = Vec::new();
    for row in &profiles {
        match map_profile(row) {
            Ok(profile) => typed_profiles.push(profile),
            Err(err) => warnings.push(
                ReportIssue::new("W_PROFILE_SKIP", err.to_string())
                    .with_entity("ProfileItem", row.string("IndexId")),
            ),
        }
    }
    let typed_subs: Vec<Subscription> = subs.iter().map(map_subscription).collect();
    let typed_routing: Vec<RoutingProfile> = routing.iter().map(map_routing_profile).collect();
    let typed_dns: Vec<DnsProfile> = dns.iter().filter_map(|r| map_dns(r).ok()).collect();
    let typed_templates: Vec<FullConfigTemplate> = templates
        .iter()
        .filter_map(|r| map_template(r).ok())
        .collect();
    let typed_stats: Vec<TrafficStats> = stats.iter().map(map_traffic).collect();
    let typed_ex: Vec<ProfileExRow> = ex.iter().map(ProfileExRow::from_raw).collect();
    let _ = typed_stats;

    let active_profile = snapshot
        .config
        .as_ref()
        .map(|c| c.index_id())
        .filter(|s| !s.is_empty())
        .and_then(|old| maps.profiles.get(&old).cloned());
    let active_sub = snapshot
        .config
        .as_ref()
        .map(|c| c.sub_index_id())
        .filter(|s| !s.is_empty())
        .and_then(|old| maps.subs.get(&old).cloned());

    for missing in &snapshot.missing_resources {
        warnings.push(ReportIssue::new(
            "W_RESOURCE_MISSING",
            format!("referenced resource not found: {missing}"),
        ));
    }
    let resource_paths = collect_resource_paths(snapshot);

    let view = CandidateView {
        profiles: &typed_profiles,
        subscriptions: &typed_subs,
        routing: &typed_routing,
        dns: &typed_dns,
        templates: &typed_templates,
        profile_ex: &typed_ex,
        expected_counts: &counts,
        active_profile_id: active_profile.as_deref(),
        active_sub_id: active_sub.as_deref(),
        resource_paths: &resource_paths,
    };
    let validation = validate_candidate(&view);
    warnings.extend(validation.warnings.iter().cloned());

    Ok(CandidateOutcome {
        batch_id,
        counts,
        migrations: migration_reports,
        warnings,
        errors: validation.errors,
        validation: validation.summary,
    })
}

fn id_column_for(table: &str) -> &'static str {
    match table {
        "SubItem" | "RoutingItem" | "DNSItem" | "FullConfigTemplateItem" => "Id",
        _ => "IndexId",
    }
}

fn collect_resource_paths(snapshot: &UpstreamSnapshot) -> Vec<String> {
    let Some(config) = &snapshot.config else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for key in ["CustomSystemProxyPacPath", "CustomSystemProxyScriptPath"] {
        if let Some(value) = config.field_state(&["SystemProxyItem", key]).as_str() {
            if !value.trim().is_empty() {
                paths.push(value.to_string());
            }
        }
    }
    paths
}

struct IdMaps {
    profiles: HashMap<String, String>,
    subs: HashMap<String, String>,
    routing: HashMap<String, String>,
    dns: HashMap<String, String>,
    templates: HashMap<String, String>,
}

fn build_id_maps(
    fingerprint: &str,
    profiles: &[RawRow],
    subs: &[RawRow],
    routing: &[RawRow],
    dns: &[RawRow],
    templates: &[RawRow],
) -> IdMaps {
    let mut maps = IdMaps {
        profiles: HashMap::new(),
        subs: HashMap::new(),
        routing: HashMap::new(),
        dns: HashMap::new(),
        templates: HashMap::new(),
    };
    for row in profiles {
        let old = row.string("IndexId");
        maps.profiles.insert(
            old.clone(),
            derived_id("profile", &format!("{fingerprint}:{old}")),
        );
    }
    for row in subs {
        let old = row.string("Id");
        maps.subs.insert(
            old.clone(),
            derived_id("sub", &format!("{fingerprint}:{old}")),
        );
    }
    for row in routing {
        let old = row.string("Id");
        maps.routing.insert(
            old.clone(),
            derived_id("routing", &format!("{fingerprint}:{old}")),
        );
    }
    for row in dns {
        let old = row.string("Id");
        maps.dns.insert(
            old.clone(),
            derived_id("dns", &format!("{fingerprint}:{old}")),
        );
    }
    for row in templates {
        let old = row.string("Id");
        maps.templates.insert(
            old.clone(),
            derived_id("template", &format!("{fingerprint}:{old}")),
        );
    }
    maps
}

fn remap_id_list(raw: &str, map: &HashMap<String, String>) -> String {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|token| map.get(token).cloned().unwrap_or_else(|| token.to_string()))
        .collect::<Vec<_>>()
        .join(",")
}

fn remap_sub_ids(raw: &str, map: &HashMap<String, String>) -> String {
    if raw.trim().eq_ignore_ascii_case(domain::SELF_SENTINEL) {
        return domain::SELF_SENTINEL.to_string();
    }
    remap_id_list(raw, map)
}

fn remap_profiles(rows: &[RawRow], maps: &IdMaps) -> Result<Vec<RawRow>> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut row = row.clone();
        let old = row.string("IndexId");
        if let Some(new) = maps.profiles.get(&old) {
            row.set("IndexId", serde_json::Value::String(new.clone()));
        }
        let subid = row.string("Subid");
        if !subid.is_empty() {
            if let Some(new_sub) = maps.subs.get(&subid) {
                row.set("Subid", serde_json::Value::String(new_sub.clone()));
            }
        }
        let mut extra = ProtocolExtraBlob::parse(row.opt_string("ProtoExtra").as_deref())?;
        if let Some(child_items) = extra.child_items.clone() {
            extra.child_items = Some(remap_id_list(&child_items, &maps.profiles));
        }
        if let Some(sub_child_items) = extra.sub_child_items.clone() {
            extra.sub_child_items = Some(remap_sub_ids(&sub_child_items, &maps.subs));
        }
        row.set("ProtoExtra", serde_json::Value::String(extra.to_json()?));
        out.push(row);
    }
    Ok(out)
}

fn remap_simple(rows: &[RawRow], column: &str, map: &HashMap<String, String>) -> Vec<RawRow> {
    rows.iter()
        .map(|row| {
            let mut row = row.clone();
            let old = row.string(column);
            if !old.is_empty() {
                if let Some(new) = map.get(&old) {
                    row.set(column, serde_json::Value::String(new.clone()));
                }
            }
            row
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_counts(
    source_profiles: &[RawRow],
    profiles: &[RawRow],
    subs: &[RawRow],
    routing: &[RawRow],
    dns: &[RawRow],
    templates: &[RawRow],
    stats: &[RawRow],
    ex: &[RawRow],
    groups: &[RawRow],
    migration_stats: &MigrationStats,
) -> Vec<EntityCount> {
    let count = |table: &str, source: usize, imported: usize, migrated: u64| EntityCount {
        table: table.to_string(),
        source_rows: source as u64,
        imported_rows: imported as u64,
        migrated_rows: migrated,
        skipped_rows: (source - imported) as u64,
    };
    vec![
        count(
            "ProfileItem",
            source_profiles.len(),
            profiles.len(),
            u64::from(migration_stats.profiles_migrated),
        ),
        count("SubItem", subs.len(), subs.len(), 0),
        count("ServerStatItem", stats.len(), stats.len(), 0),
        count("RoutingItem", routing.len(), routing.len(), 0),
        count("ProfileExItem", ex.len(), ex.len(), 0),
        count("DNSItem", dns.len(), dns.len(), 0),
        count(
            "FullConfigTemplateItem",
            templates.len(),
            templates.len(),
            0,
        ),
        // Group rows are a migration source only: retained as raw, not imported.
        count("ProfileGroupItem", groups.len(), 0, 0),
    ]
}

fn record_migrations(
    store: &Store,
    tx: &rusqlite::Transaction<'_>,
    stats: &MigrationStats,
    snapshot: &UpstreamSnapshot,
    now: i64,
) -> Result<Vec<MigrationReport>> {
    let mut reports = Vec::new();
    for migration_id in &stats.applied {
        let (from, to, touched) = match *migration_id {
            "MIG-ENT-002" => (2, 3, u64::from(stats.groups_migrated)),
            "MIG-ENT-003" => (2, 3, u64::from(stats.profiles_migrated)),
            "MIG-ENT-004" => (3, 4, u64::from(stats.transports_migrated)),
            _ => (0, 0, 0),
        };
        store.record_migration(
            tx,
            crate::store::MigrationLog {
                migration_id,
                source_hash: &snapshot.content_hash,
                from_version: from,
                to_version: to,
                applied_at: now,
                entities_touched: touched,
                notes: None,
            },
        )?;
        reports.push(MigrationReport {
            migration_id: (*migration_id).to_string(),
            from_version: from,
            to_version: to,
            entities_touched: touched,
        });
    }
    Ok(reports)
}

fn base_report(
    snapshot: &UpstreamSnapshot,
    counts: Vec<EntityCount>,
    migrations: Vec<MigrationReport>,
    warnings: Vec<ReportIssue>,
    errors: Vec<ReportIssue>,
) -> ImportReport {
    ImportReport {
        status: ImportStatus::Failed,
        source_id: snapshot.source_id.clone(),
        source_kind: snapshot.source_kind.as_str().to_string(),
        source_version: snapshot.version,
        content_hash: snapshot.content_hash.clone(),
        source_fingerprint: snapshot.fingerprint(),
        batch_id: String::new(),
        counts,
        migrations,
        warnings,
        errors,
        validation: ValidationSummary::default(),
        committed: false,
        target_backup: None,
        user_summary: String::new(),
    }
}

fn already_imported_report(batch: &ImportBatch, snapshot: &UpstreamSnapshot) -> ImportReport {
    let mut report = ImportReport {
        status: ImportStatus::AlreadyImported,
        source_id: snapshot.source_id.clone(),
        source_kind: snapshot.source_kind.as_str().to_string(),
        source_version: snapshot.version,
        content_hash: snapshot.content_hash.clone(),
        source_fingerprint: batch.source_fingerprint.clone(),
        batch_id: batch.batch_id.clone(),
        counts: batch.entity_counts.clone(),
        migrations: Vec::new(),
        warnings: Vec::new(),
        errors: Vec::new(),
        validation: ValidationSummary::default(),
        committed: false,
        target_backup: None,
        user_summary: String::new(),
    };
    report.user_summary = report.render_user_summary();
    report
}

fn rejected_report(source_id: &str, kind: &str, message: &str) -> ImportReport {
    let mut report = ImportReport {
        status: ImportStatus::Rejected,
        source_id: source_id.to_string(),
        source_kind: kind.to_string(),
        source_version: 0,
        content_hash: String::new(),
        source_fingerprint: String::new(),
        batch_id: String::new(),
        counts: Vec::new(),
        migrations: Vec::new(),
        warnings: Vec::new(),
        errors: vec![ReportIssue::new("E_NOT_A_SOURCE", message)],
        validation: ValidationSummary::default(),
        committed: false,
        target_backup: None,
        user_summary: String::new(),
    };
    report.user_summary = report.render_user_summary();
    report
}

/// Atomically replace `target` with `candidate`, backing up the old target.
/// Returns the backup path when one was created.
pub fn commit_candidate(candidate: &Path, target: &Path) -> Result<Option<String>> {
    let backup = if target.exists() {
        let bak = sidecar(target, ".bak");
        if bak.exists() {
            std::fs::remove_file(&bak)?;
        }
        std::fs::rename(target, &bak)?;
        Some(bak.to_string_lossy().to_string())
    } else {
        None
    };

    if let Err(err) = std::fs::rename(candidate, target) {
        if let Some(bak) = &backup {
            let _ = std::fs::rename(bak, target);
        }
        return Err(err.into());
    }
    for suffix in ["-wal", "-shm"] {
        let src = sidecar(candidate, suffix);
        if src.exists() {
            let dst = sidecar(target, suffix);
            let _ = std::fs::rename(src, dst);
        }
    }
    Ok(backup)
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn remove_sqlite_files(path: &Path) {
    for candidate in [
        path.to_path_buf(),
        sidecar(path, "-wal"),
        sidecar(path, "-shm"),
    ] {
        if candidate.exists() {
            let _ = std::fs::remove_file(candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    #[test]
    fn import_v2_source_migrates_and_reports() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(
            src.path().join("guiNConfig.json"),
            r#"{"IndexId":"p1","SubIndexId":"s1"}"#,
        )
        .unwrap();
        let conn = Connection::open(src.path().join("guiNDB.db")).unwrap();
        for table in crate::schema::UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        conn.execute(
            "INSERT INTO SubItem (Id, Remarks, Enabled) VALUES ('s1','sub',1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Subid, Remarks, Security, Id) \
             VALUES ('p1', 3, 2, 's1', 'HK-1', 'aes-256-gcm', 'pw')",
            [],
        )
        .unwrap();
        drop(conn);

        let work = tempfile::tempdir().unwrap();
        let target = src.path().join("target.db");
        let report = import_from_path(
            src.path(),
            &target,
            work.path(),
            &ImportOptions {
                now: 100,
                fault: ImportFault::None,
            },
        )
        .unwrap();
        assert_eq!(report.status, ImportStatus::Imported);
        assert!(report.committed);
        assert!(report
            .migrations
            .iter()
            .any(|m| m.migration_id == "MIG-ENT-003"));

        let store = Store::open_readonly(&target).unwrap();
        let profiles = store.read_rows("ProfileItem").unwrap();
        assert_eq!(profiles.len(), 1);
        assert_ne!(profiles[0].string("IndexId"), "p1");
        let extra =
            ProtocolExtraBlob::parse(profiles[0].opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.ss_method.as_deref(), Some("aes-256-gcm"));
    }

    #[test]
    fn second_import_of_same_source_is_already_imported() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("guiNConfig.json"), r#"{"IndexId":"n1"}"#).unwrap();
        let conn = Connection::open(src.path().join("guiNDB.db")).unwrap();
        for table in crate::schema::UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) VALUES ('p1',5,4,'A')",
            [],
        )
        .unwrap();
        drop(conn);

        let work = tempfile::tempdir().unwrap();
        let target = src.path().join("target.db");
        let opts = ImportOptions {
            now: 1,
            fault: ImportFault::None,
        };
        let first = import_from_path(src.path(), &target, work.path(), &opts).unwrap();
        assert_eq!(first.status, ImportStatus::Imported);
        let second = import_from_path(src.path(), &target, work.path(), &opts).unwrap();
        assert_eq!(second.status, ImportStatus::AlreadyImported);

        let store = Store::open_readonly(&target).unwrap();
        assert_eq!(store.count_rows("ProfileItem").unwrap(), 1);
    }

    #[test]
    fn failed_validation_leaves_target_untouched() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("guiNConfig.json"), r#"{"IndexId":"n1"}"#).unwrap();
        let conn = Connection::open(src.path().join("guiNDB.db")).unwrap();
        for table in crate::schema::UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) VALUES ('p1',5,4,'A')",
            [],
        )
        .unwrap();
        drop(conn);

        let work = tempfile::tempdir().unwrap();
        let target = src.path().join("target.db");
        let report = import_from_path(
            src.path(),
            &target,
            work.path(),
            &ImportOptions {
                now: 1,
                fault: ImportFault::ValidationFailure,
            },
        )
        .unwrap();
        assert_eq!(report.status, ImportStatus::Failed);
        assert!(!target.exists());
    }

    #[test]
    fn unrelated_second_source_appends_without_overwriting() {
        let base = tempfile::tempdir().unwrap();
        let target = base.path().join("target.db");

        let mk = |name: &str, id: &str, remarks: &str| {
            let dir = base.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("guiNConfig.json"), "{}").unwrap();
            let conn = Connection::open(dir.join("guiNDB.db")).unwrap();
            for table in crate::schema::UPSTREAM_TABLES {
                conn.execute_batch(&table.create_sql()).unwrap();
            }
            conn.execute(
                "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) VALUES (?1,5,4,?2)",
                rusqlite::params![id, remarks],
            )
            .unwrap();
            drop(conn);
            dir
        };

        let source_a = mk("a", "pA", "A");
        let source_b = mk("b", "pB", "B");
        let work = tempfile::tempdir().unwrap();
        let opts = ImportOptions {
            now: 1,
            fault: ImportFault::None,
        };
        assert_eq!(
            import_from_path(&source_a, &target, work.path(), &opts)
                .unwrap()
                .status,
            ImportStatus::Imported
        );
        assert_eq!(
            import_from_path(&source_b, &target, work.path(), &opts)
                .unwrap()
                .status,
            ImportStatus::Imported
        );
        let store = Store::open_readonly(&target).unwrap();
        assert_eq!(store.count_rows("ProfileItem").unwrap(), 2);
    }

    #[test]
    fn legacy_group_row_is_migrated_not_imported() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("guiNConfig.json"), r#"{"IndexId":"g1"}"#).unwrap();
        let conn = Connection::open(src.path().join("guiNDB.db")).unwrap();
        for table in crate::schema::UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) VALUES ('g1',101,2,'group')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ProfileGroupItem (IndexId, ChildItems, MultipleLoad) VALUES ('g1','','0')",
            [],
        )
        .unwrap();
        drop(conn);

        let work = tempfile::tempdir().unwrap();
        let target = src.path().join("target.db");
        let report = import_from_path(
            src.path(),
            &target,
            work.path(),
            &ImportOptions {
                now: 1,
                fault: ImportFault::None,
            },
        )
        .unwrap();
        assert_eq!(report.status, ImportStatus::Imported);
        let store = Store::open_readonly(&target).unwrap();
        let profile = store.read_rows("ProfileItem").unwrap();
        let extra =
            ProtocolExtraBlob::parse(profile[0].opt_string("ProtoExtra").as_deref()).unwrap();
        assert_eq!(extra.group_type.as_deref(), Some("PolicyGroup"));
        assert_eq!(store.count_rows("ProfileGroupItem").unwrap(), 0);
        let group_count = report
            .counts
            .iter()
            .find(|c| c.table == "ProfileGroupItem")
            .unwrap();
        assert_eq!(group_count.skipped_rows, 1);
    }

    #[test]
    fn unknown_columns_are_retained_in_raw_records() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("guiNConfig.json"), "{}").unwrap();
        let conn = Connection::open(src.path().join("guiNDB.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE ProfileItem (IndexId TEXT PRIMARY KEY, ConfigType INTEGER, ConfigVersion INTEGER, Remarks TEXT, FutureCol TEXT);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ProfileItem VALUES ('p1', 5, 4, 'A', 'future-value')",
            [],
        )
        .unwrap();
        drop(conn);

        let work = tempfile::tempdir().unwrap();
        let target = src.path().join("target.db");
        let report = import_from_path(
            src.path(),
            &target,
            work.path(),
            &ImportOptions {
                now: 1,
                fault: ImportFault::None,
            },
        )
        .unwrap();
        assert_eq!(report.status, ImportStatus::Imported);
        let store = Store::open_readonly(&target).unwrap();
        let raw: String = store
            .connection()
            .query_row("SELECT raw_json FROM raw_records LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["FutureCol"], json!("future-value"));
    }

    fn mk_upstream_source(
        base: &Path,
        name: &str,
        id: &str,
        remarks: &str,
        config: &str,
    ) -> PathBuf {
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("guiNConfig.json"), config).unwrap();
        let conn = Connection::open(dir.join("guiNDB.db")).unwrap();
        for table in crate::schema::UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) VALUES (?1,5,4,?2)",
            rusqlite::params![id, remarks],
        )
        .unwrap();
        drop(conn);
        dir
    }

    #[test]
    fn restore_replace_removes_existing_and_keeps_source() {
        let base = tempfile::tempdir().unwrap();
        let target = base.path().join("target.db");
        let work = tempfile::tempdir().unwrap();
        let opts = ImportOptions {
            now: 1,
            fault: ImportFault::None,
        };

        let source_a = mk_upstream_source(base.path(), "a", "pA", "A", r#"{"IndexId":"pA"}"#);
        let source_b = mk_upstream_source(base.path(), "b", "pB", "B", r#"{"IndexId":"pB"}"#);
        assert_eq!(
            import_from_path(&source_a, &target, work.path(), &opts)
                .unwrap()
                .status,
            ImportStatus::Imported
        );

        // Restoring B replaces A entirely: only B's rows exist afterwards.
        let report = restore_from_path(&source_b, &target, work.path(), &opts).unwrap();
        assert_eq!(report.status, ImportStatus::Imported);
        let store = Store::open_readonly(&target).unwrap();
        let rows = store.read_rows("ProfileItem").unwrap();
        assert_eq!(rows.len(), 1, "restore must replace, not merge");
        assert_eq!(rows[0].string("Remarks"), "B");
    }

    #[test]
    fn reimport_of_earlier_source_is_noop_and_keeps_scoped_config() {
        let base = tempfile::tempdir().unwrap();
        let target = base.path().join("target.db");
        let work = tempfile::tempdir().unwrap();
        let opts = ImportOptions {
            now: 1,
            fault: ImportFault::None,
        };

        let source_a = mk_upstream_source(
            base.path(),
            "a",
            "pA",
            "A",
            r#"{"IndexId":"pA","UIItem":{"CurrentTheme":"Dark"}}"#,
        );
        let source_b = mk_upstream_source(
            base.path(),
            "b",
            "pB",
            "B",
            r#"{"IndexId":"pB","UIItem":{"CurrentTheme":"Light"}}"#,
        );
        let first = import_from_path(&source_a, &target, work.path(), &opts).unwrap();
        assert_eq!(first.status, ImportStatus::Imported);
        assert_eq!(
            import_from_path(&source_b, &target, work.path(), &opts)
                .unwrap()
                .status,
            ImportStatus::Imported
        );

        // Re-importing A is a no-op and does not touch A's own batch config.
        let again = import_from_path(&source_a, &target, work.path(), &opts).unwrap();
        assert_eq!(again.status, ImportStatus::AlreadyImported);
        let store = Store::open_readonly(&target).unwrap();
        assert_eq!(store.count_rows("ProfileItem").unwrap(), 2);
        let scoped = store
            .get_meta(&format!("upstream_config:{}", first.source_fingerprint))
            .unwrap()
            .expect("batch-scoped config for A");
        let value: serde_json::Value = serde_json::from_str(&scoped).unwrap();
        assert_eq!(
            value
                .get("UIItem")
                .and_then(|ui| ui.get("CurrentTheme"))
                .and_then(|v| v.as_str()),
            Some("Dark"),
            "A's scoped config must survive B's import"
        );
    }
}
