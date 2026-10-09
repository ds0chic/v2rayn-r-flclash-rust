//! T16 local backup / restore use cases over the T04 persistence bundle.
//!
//! A local backup is a versioned bundle directory (`manifest.json` + a
//! consistency snapshot of `guiNDB.db` + `guiNConfig.json` + referenced
//! resources). Restore verifies the whole bundle and writes a candidate before
//! touching the live database, so a failed restore never damages the current
//! configuration (plan §15). Upstream `guiConfigs/` ZIP archives are recognised
//! and imported through the T04 candidate flow.

use std::io::Write;
use std::path::{Path, PathBuf};

use domain::{codes, DomainError};
use persistence::backup::{
    self, ArchiveRecognition, BackupManifest, BackupVerification, ResourceEntry, RestoreReport,
    CONFIG_FILE_NAME, DB_FILE_NAME, MANIFEST_NAME,
};
use persistence::candidate::restore_from_path;
use persistence::hash::{derived_id, sha256_file};
use persistence::store::table_counts;
use persistence::{
    import_from_path, ConfigDocument, ImportOptions, ImportReport, ImportStatus, PersistenceError,
    Store,
};
use serde_json::Value;
use updater::unpack::{safe_unpack_zip, UnpackLimits};

use crate::engine::AppEngine;

/// Upstream commit recorded in every manifest (T00 frozen baseline).
pub const APP_SOURCE_COMMIT: &str = persistence::SOURCE_COMMIT;

/// A written local backup bundle.
#[derive(Debug, Clone)]
pub struct LocalBackup {
    pub root: PathBuf,
    pub manifest: BackupManifest,
}

/// Map a persistence failure onto the shared domain error contract without
/// leaking raw SQL/paths beyond the already-safe `Display` text.
pub fn persist_error(err: PersistenceError) -> DomainError {
    DomainError::new(err.code(), err.message_key()).with_detail(err.to_string())
}

fn internal(message: impl Into<String>) -> DomainError {
    DomainError::new(codes::INTERNAL, "error.backup_failed").with_detail(message.into())
}

/// Local backup / restore rooted at one data directory.
#[derive(Debug, Clone)]
pub struct BackupService {
    data_dir: PathBuf,
}

impl BackupService {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join(DB_FILE_NAME)
    }

    /// Write a versioned bundle under `dest_root`. `created_at` is injected so
    /// manifests are deterministic in tests.
    pub fn create_local(
        &self,
        dest_root: &Path,
        created_at: i64,
    ) -> Result<LocalBackup, DomainError> {
        let db = self.db_path();
        if !db.is_file() {
            return Err(
                DomainError::new(codes::NOT_FOUND, "error.backup_no_database")
                    .with_detail(db.display().to_string()),
            );
        }
        let config = self.data_dir.join(CONFIG_FILE_NAME);
        let config_ref = config.is_file().then_some(config.as_path());
        let resources = collect_resources(&self.data_dir)?;
        let counts = Store::open(&db)
            .ok()
            .and_then(|store| table_counts(&store).ok())
            .unwrap_or_default();
        let bundle = backup::create_backup(
            dest_root,
            &db,
            config_ref,
            &resources,
            &counts,
            created_at,
            APP_SOURCE_COMMIT,
        )
        .map_err(persist_error)?;
        Ok(LocalBackup {
            root: bundle.root,
            manifest: bundle.manifest,
        })
    }

    /// Verify a bundle's manifest and every hashed member.
    pub fn verify(&self, root: &Path) -> Result<BackupVerification, DomainError> {
        backup::verify_backup(root).map_err(persist_error)
    }

    /// Restore a verified bundle into the live database, config and every
    /// referenced resource (including nested configuration subdirectories).
    ///
    /// The database is swapped through the T04 candidate commit; the bundled
    /// config and resources are copied with atomic temp+rename. If any copy
    /// fails, the previously restored members (database, config, resources)
    /// are rolled back so the live directory is never left half-restored.
    pub fn restore(&self, root: &Path, work_dir: &Path) -> Result<RestoreReport, DomainError> {
        let verification = self.verify(root)?;
        if !verification.ok {
            return Err(
                DomainError::new(codes::FIELD_FORMAT, "error.backup_invalid").with_detail(format!(
                    "missing {:?}, mismatched {:?}",
                    verification.missing, verification.mismatched
                )),
            );
        }
        let manifest = backup::read_manifest(root).map_err(persist_error)?;
        // SP-03: the replacement creates a new dataset generation. Capture
        // both epochs before the exchange so the persisted value advances
        // monotonically however the bundled config relates to the live one.
        let config_path = self.data_dir.join(CONFIG_FILE_NAME);
        let live_epoch = read_dataset_epoch(&config_path);
        let bundled_epoch = read_dataset_epoch(&root.join(CONFIG_FILE_NAME));
        let report =
            backup::restore_backup(root, &self.db_path(), work_dir).map_err(persist_error)?;

        // Config first, then resources; roll the whole group back on failure.
        let mut config_prior: Option<PathBuf> = None;
        let bundled_config = root.join(CONFIG_FILE_NAME);
        let config_replaced = bundled_config.is_file();
        if config_replaced {
            // Establishing the config rollback copy can fail; the database is
            // already swapped, so undo it before reporting the failure.
            match backup_previous(&config_path) {
                Ok(prior) => config_prior = prior,
                Err(error) => {
                    rollback_database(&self.db_path(), report.target_backup.as_deref());
                    return Err(error);
                }
            }
            if let Err(error) = copy_atomic(&bundled_config, &config_path) {
                restore_previous(&config_path, config_prior.as_deref());
                rollback_database(&self.db_path(), report.target_backup.as_deref());
                return Err(error);
            }
        }
        match self.restore_resources(root, &manifest.referenced_resources) {
            Ok(_) => {
                // The exchange committed: publish the new dataset generation
                // so pre-restore requests stay rejected after reopen (plan
                // §3.1). A failed epoch publish rolls back like any other
                // late activation failure.
                let new_epoch = live_epoch.max(bundled_epoch).saturating_add(1);
                if let Err(error) = write_dataset_epoch(&config_path, new_epoch) {
                    // Only undo the config when this restore replaced it; a
                    // bundle without config leaves the live file untouched, so
                    // there is nothing to restore (deleting it would destroy
                    // data this restore never wrote).
                    if config_replaced {
                        restore_previous(&config_path, config_prior.as_deref());
                    }
                    rollback_database(&self.db_path(), report.target_backup.as_deref());
                    return Err(error);
                }
                if let Some(prior) = &config_prior {
                    let _ = std::fs::remove_file(prior);
                }
                Ok(report)
            }
            Err(error) => {
                restore_previous(&config_path, config_prior.as_deref());
                rollback_database(&self.db_path(), report.target_backup.as_deref());
                Err(error)
            }
        }
    }

    /// Copy every manifest-listed resource (nested paths included) back into
    /// the data directory. Returns the number copied.
    fn restore_resources(
        &self,
        root: &Path,
        resources: &[ResourceEntry],
    ) -> Result<usize, DomainError> {
        let mut applied: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
        for entry in resources {
            let src = root.join(&entry.relative_path);
            let dest = self.data_dir.join(&entry.relative_path);
            let prior = match backup_previous(&dest) {
                Ok(prior) => prior,
                Err(error) => {
                    rollback_previous(&applied);
                    return Err(error);
                }
            };
            if let Err(error) = copy_atomic(&src, &dest) {
                restore_previous(&dest, prior.as_deref());
                rollback_previous(&applied);
                return Err(error);
            }
            applied.push((dest, prior));
        }
        for (_, prior) in &applied {
            if let Some(prior) = prior {
                let _ = std::fs::remove_file(prior);
            }
        }
        Ok(applied.len())
    }

    /// Recognise an upstream `guiConfigs/` archive (or this project's ZIP).
    pub fn recognize(&self, path: &Path) -> Result<ArchiveRecognition, DomainError> {
        backup::recognize_archive(path).map_err(persist_error)
    }

    /// Restore an upstream directory or ZIP through the T04 candidate flow and
    /// activate the restored configuration.
    ///
    /// The upstream `guiNDB.db` is the complete desired database (upstream
    /// `BackupAndRestoreViewModel` exits, replaces the database file, then
    /// restarts), so the existing target rows are **replaced**, never merged.
    /// The append/migration import is the separately named
    /// [`Self::import_upstream_merge`].
    ///
    /// The upstream `guiNConfig.json` is written to the live `guiNConfig.json`
    /// with `IndexId`/`SubIndexId` remapped to the restored rows and the engine
    /// `active_index_id` set, so settings/active/group become the active
    /// configuration on the next load/reopen.
    pub fn import_upstream(
        &self,
        path: &Path,
        work_dir: &Path,
        now: i64,
    ) -> Result<ImportReport, DomainError> {
        self.import_upstream_inner(path, work_dir, now, true)
    }

    /// Import an upstream directory or ZIP by appending its rows to the live
    /// database (the migration flow). Kept as the explicitly named merge entry;
    /// the restore path is [`Self::import_upstream`].
    pub fn import_upstream_merge(
        &self,
        path: &Path,
        work_dir: &Path,
        now: i64,
    ) -> Result<ImportReport, DomainError> {
        self.import_upstream_inner(path, work_dir, now, false)
    }

    fn import_upstream_inner(
        &self,
        path: &Path,
        work_dir: &Path,
        now: i64,
        replace: bool,
    ) -> Result<ImportReport, DomainError> {
        let options = ImportOptions {
            now,
            ..ImportOptions::default()
        };
        let report = if replace {
            restore_from_path(path, &self.db_path(), work_dir, &options)
        } else {
            import_from_path(path, &self.db_path(), work_dir, &options)
        }
        .map_err(persist_error)?;
        // Only a real commit activates. `AlreadyImported` is an idempotent
        // no-op: activating from the last global `upstream_config` remapped
        // another source's config under this fingerprint and left the active
        // node dangling (R3-SET-01).
        if report.status == ImportStatus::Imported {
            // Two-phase close: the candidate database is already committed, so
            // any later failure must undo it together with the config and every
            // resource written after it.
            if let Err(error) = self.activate_and_install(
                path,
                work_dir,
                &report.source_fingerprint,
                report.target_backup.as_deref(),
            ) {
                return Err(with_rollback_note(error));
            }
        }
        Ok(report)
    }

    /// Restore a verified bundle under the SR-03 lifecycle: stop the managed
    /// session + scheduler, quiesce, exchange the files, then reopen.
    ///
    /// The engine is always reopened after a successful quiesce so it can never
    /// keep serving the empty in-memory backends. A reopen failure is surfaced
    /// rather than swallowed: the live files are the restored state but the
    /// engine could not reload them, so the caller must not report success.
    pub fn restore_with_lifecycle(
        &self,
        engine: &AppEngine,
        root: &Path,
        work_dir: &Path,
    ) -> Result<RestoreReport, DomainError> {
        engine.prepare_restore()?;
        let outcome = self.restore(root, work_dir);
        finish_lifecycle(outcome, engine.reopen(), "restore")
    }

    /// Import an upstream directory/ZIP under the same lifecycle as
    /// [`Self::restore_with_lifecycle`] (`prepare_restore` -> import ->
    /// `reopen`), so a restore that changes the database never leaves the live
    /// engine bound to the pre-import state.
    pub fn import_upstream_with_lifecycle(
        &self,
        engine: &AppEngine,
        path: &Path,
        work_dir: &Path,
        now: i64,
    ) -> Result<ImportReport, DomainError> {
        engine.prepare_restore()?;
        let outcome = self.import_upstream(path, work_dir, now);
        finish_lifecycle(outcome, engine.reopen(), "import")
    }

    /// Same lifecycle as [`Self::import_upstream_with_lifecycle`] but for the
    /// explicitly named merge/migration flow (`import_upstream_merge`).
    pub fn import_upstream_merge_with_lifecycle(
        &self,
        engine: &AppEngine,
        path: &Path,
        work_dir: &Path,
        now: i64,
    ) -> Result<ImportReport, DomainError> {
        engine.prepare_restore()?;
        let outcome = self.import_upstream_merge(path, work_dir, now);
        finish_lifecycle(outcome, engine.reopen(), "import")
    }

    /// Close a committed upstream import: write the activated config and
    /// install every upstream resource (whole `guiConfigs/` directory content
    /// except the database and config). On any failure the committed candidate
    /// database (`db_backup`), the live config and the already-written
    /// resources are rolled back to the pre-import state.
    fn activate_and_install(
        &self,
        source: &Path,
        work_dir: &Path,
        fingerprint: &str,
        db_backup: Option<&str>,
    ) -> Result<(), DomainError> {
        // SP-03: capture the live generation before the config is replaced so
        // the activation can publish the next one.
        let config_path = self.data_dir.join(CONFIG_FILE_NAME);
        let live_epoch = read_dataset_epoch(&config_path);
        // Stage from the read-only source first so a staging failure never
        // leaves the committed database dangling.
        let staged = match stage_upstream_resources(source, work_dir) {
            Ok(staged) => staged,
            Err(error) => {
                rollback_database(&self.db_path(), db_backup);
                return Err(error);
            }
        };
        let config_prior = match backup_previous(&config_path) {
            Ok(prior) => prior,
            Err(error) => {
                rollback_database(&self.db_path(), db_backup);
                return Err(error);
            }
        };
        if let Err(error) = self.activate_upstream_config(fingerprint, live_epoch.saturating_add(1))
        {
            restore_previous(&config_path, config_prior.as_deref());
            rollback_database(&self.db_path(), db_backup);
            return Err(error);
        }
        if let Err(error) = self.install_staged_resources(&staged) {
            restore_previous(&config_path, config_prior.as_deref());
            rollback_database(&self.db_path(), db_backup);
            return Err(error);
        }
        if let Some(prior) = config_prior {
            let _ = std::fs::remove_file(prior);
        }
        Ok(())
    }

    /// Copy staged resources into the live data directory with per-file hash
    /// verification. Only paths that came from the source are written; no
    /// unrelated live file is deleted. Returns the number installed.
    fn install_staged_resources(&self, staged: &[StagedResource]) -> Result<usize, DomainError> {
        let mut applied: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
        for resource in staged {
            let dest = self.data_dir.join(&resource.relative_path);
            let prior = match backup_previous(&dest) {
                Ok(prior) => prior,
                Err(error) => {
                    rollback_previous(&applied);
                    return Err(error);
                }
            };
            match copy_verified(&resource.staged_path, &dest) {
                Ok(()) => applied.push((dest, prior)),
                Err(error) => {
                    restore_previous(&dest, prior.as_deref());
                    rollback_previous(&applied);
                    return Err(error);
                }
            }
        }
        for (_, prior) in &applied {
            if let Some(prior) = prior {
                let _ = std::fs::remove_file(prior);
            }
        }
        Ok(applied.len())
    }

    /// Apply the `upstream_config` recorded by the last import to the live
    /// settings file. No-op when the source carried no config. The upstream
    /// PascalCase tree and every unknown key are preserved verbatim; only the
    /// id references and engine-owned meta keys are (re)written.
    ///
    /// SP-03: the default resolves with the same migration priority as an
    /// open/reopen (plan §5.2) — the remapped engine mirror
    /// `active_index_id` first when it names a restored row, then the
    /// remapped canonical `IndexId` — so activating a bundle written before
    /// the dual-identity unification still lands on the true default instead
    /// of a stale canonical id. `new_epoch` is the post-activation dataset
    /// generation, published with the same write.
    pub fn activate_upstream_config(
        &self,
        fingerprint: &str,
        new_epoch: u64,
    ) -> Result<Option<String>, DomainError> {
        let store = Store::open(self.db_path()).map_err(persist_error)?;
        // Prefer the batch-scoped config for this exact source, so activation
        // never applies another batch's global `upstream_config` (R3-SET-01).
        let scoped = store
            .get_meta(&format!("upstream_config:{fingerprint}"))
            .map_err(persist_error)?;
        let raw = match scoped {
            Some(raw) => Some(raw),
            None => store.get_meta("upstream_config").map_err(persist_error)?,
        };
        let Some(raw) = raw else {
            return Ok(None);
        };
        if raw.trim().is_empty() || raw.trim() == "{}" {
            return Ok(None);
        }
        let doc = ConfigDocument::parse(&raw).map_err(persist_error)?;
        let mut value = doc.to_json_value();
        let Some(object) = value.as_object_mut() else {
            return Err(internal("upstream_config root is not an object"));
        };
        let old_active = object
            .get("IndexId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let old_mirror = object
            .get("active_index_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let old_sub = object
            .get("SubIndexId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let remapped_mirror = (!old_mirror.is_empty())
            .then(|| derived_id("profile", &format!("{fingerprint}:{old_mirror}")));
        let remapped_canonical = (!old_active.is_empty())
            .then(|| derived_id("profile", &format!("{fingerprint}:{old_active}")));
        // Prefer the mirror when it names a restored row (it is the newest
        // explicit default choice); otherwise the canonical id, even when
        // dangling — the next open/reopen repairs that per the upstream rule.
        let active = match (&remapped_mirror, &remapped_canonical) {
            (Some(mirror), _) if profile_exists(&store, mirror) => Some(mirror.clone()),
            (_, Some(canonical)) => Some(canonical.clone()),
            (Some(mirror), None) => Some(mirror.clone()),
            (None, None) => None,
        };
        let sub =
            (!old_sub.is_empty()).then(|| derived_id("sub", &format!("{fingerprint}:{old_sub}")));
        object.insert(
            "IndexId".to_string(),
            active.clone().map(Value::String).unwrap_or(Value::Null),
        );
        if let Some(sub) = &sub {
            object.insert("SubIndexId".to_string(), Value::String(sub.clone()));
        }
        object.insert(
            "active_index_id".to_string(),
            active.clone().map(Value::String).unwrap_or(Value::Null),
        );
        object
            .entry("desired_revision".to_string())
            .or_insert_with(|| Value::from(0));
        object.insert("dataset_epoch".to_string(), Value::from(new_epoch));
        object
            .entry("rule_mode".to_string())
            .or_insert_with(|| Value::String("Rule".to_string()));
        write_json_atomic(&self.data_dir.join(CONFIG_FILE_NAME), &value)?;
        Ok(active)
    }

    /// List bundles directly under `parent` (a directory with a manifest),
    /// newest first, each with its bundle root.
    pub fn list(parent: &Path) -> Result<Vec<(PathBuf, BackupManifest)>, DomainError> {
        if !parent.is_dir() {
            return Ok(Vec::new());
        }
        let mut manifests = Vec::new();
        let entries = std::fs::read_dir(parent).map_err(|e| internal(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| internal(e.to_string()))?;
            let path = entry.path();
            if path.is_dir() && path.join(MANIFEST_NAME).is_file() {
                if let Ok(manifest) = backup::read_manifest(&path) {
                    manifests.push((path, manifest));
                }
            }
        }
        manifests.sort_by_key(|(_, manifest)| std::cmp::Reverse(manifest.created_at));
        Ok(manifests)
    }
}

/// One upstream resource extracted under the import work directory, keyed by
/// its forward-slash path relative to the live data directory.
struct StagedResource {
    relative_path: String,
    staged_path: PathBuf,
}

/// Stage every upstream resource from `source` (a directory or a `guiConfigs/`
/// ZIP) into `<work_dir>/resources`. The database and `guiNConfig.json` members
/// are excluded: they have their own commit path. Directory and archive members
/// resolve to the same live-relative path, so `guiConfigs/config/custom.json`
/// lands at `<data>/config/custom.json`.
fn stage_upstream_resources(
    source: &Path,
    work_dir: &Path,
) -> Result<Vec<StagedResource>, DomainError> {
    let staging = work_dir.join("resources");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| internal(e.to_string()))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| internal(e.to_string()))?;
    let mut staged = Vec::new();
    if source.is_dir() {
        let mut stack = vec![source.to_path_buf()];
        while let Some(current) = stack.pop() {
            let entries = std::fs::read_dir(&current).map_err(|e| internal(e.to_string()))?;
            for entry in entries {
                let entry = entry.map_err(|e| internal(e.to_string()))?;
                let path = entry.path();
                let file_type = entry.file_type().map_err(|e| internal(e.to_string()))?;
                if file_type.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !file_type.is_file() {
                    continue;
                }
                let relative = path
                    .strip_prefix(source)
                    .map_err(|_| internal("resource escaped source"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                stage_file(&relative, &path, &staging, &mut staged)?;
            }
        }
    } else {
        let file = std::fs::File::open(source).map_err(|e| internal(e.to_string()))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| internal(e.to_string()))?;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|e| internal(e.to_string()))?;
            if entry.is_dir() {
                continue;
            }
            let raw = entry.name().replace('\\', "/");
            let Some(relative) = sanitize_upstream_relative(&raw) else {
                continue;
            };
            let staged_path = staging.join(&relative);
            if let Some(parent) = staged_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| internal(e.to_string()))?;
            }
            let mut out =
                std::fs::File::create(&staged_path).map_err(|e| internal(e.to_string()))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| internal(e.to_string()))?;
            staged.push(StagedResource {
                relative_path: relative,
                staged_path,
            });
        }
    }
    staged.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(staged)
}

/// Stage one directory resource; the reserved database/config names are skipped.
fn stage_file(
    relative: &str,
    source: &Path,
    staging: &Path,
    staged: &mut Vec<StagedResource>,
) -> Result<(), DomainError> {
    let Some(relative) = sanitize_upstream_relative(relative) else {
        return Ok(());
    };
    let staged_path = staging.join(&relative);
    if let Some(parent) = staged_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| internal(e.to_string()))?;
    }
    std::fs::copy(source, &staged_path).map_err(|e| internal(e.to_string()))?;
    staged.push(StagedResource {
        relative_path: relative,
        staged_path,
    });
    Ok(())
}

/// Normalize an upstream member path to a live-relative path and reject
/// anything that could escape the data directory. `guiConfigs/` (the upstream
/// top-level wrapper) is stripped. The database and config themselves are not
/// resources.
fn sanitize_upstream_relative(raw: &str) -> Option<String> {
    let normalized = raw.replace('\\', "/");
    let trimmed = normalized.trim_start_matches("./").trim_start_matches('/');
    let relative = trimmed
        .strip_prefix("guiConfigs/")
        .unwrap_or(trimmed)
        .trim_start_matches('/');
    if relative.is_empty()
        || relative == DB_FILE_NAME
        || relative == CONFIG_FILE_NAME
        || relative.contains("..")
        || relative.contains(':')
    {
        return None;
    }
    Some(relative.to_string())
}

/// Copy `src` over `dest` atomically and verify the written bytes hash-match
/// the source. A mismatch removes the destination.
fn copy_verified(src: &Path, dest: &Path) -> Result<(), DomainError> {
    let expected = sha256_file(src).map_err(|e| internal(e.to_string()))?;
    copy_atomic(src, dest)?;
    let actual = sha256_file(dest).map_err(|e| internal(e.to_string()))?;
    if expected != actual {
        let _ = std::fs::remove_file(dest);
        return Err(internal(format!(
            "resource hash mismatch for {}",
            dest.display()
        )));
    }
    Ok(())
}

/// Close a restore/import lifecycle by combining the operation result with the
/// engine reopen result. A successful operation whose reopen failed is an
/// error, not a success: the on-disk state changed but the engine could not
/// reload it, so the UI must not claim the restore took effect.
fn finish_lifecycle<T>(
    outcome: Result<T, DomainError>,
    reopened: Result<(), DomainError>,
    operation: &str,
) -> Result<T, DomainError> {
    match (outcome, reopened) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(reopen)) => Err(reopen_failure(operation, None, reopen)),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(reopen)) => Err(reopen_failure(operation, Some(error), reopen)),
    }
}

/// Build the structured error for a restore/import whose storage exchange
/// completed but whose engine reopen failed, preserving any prior operation
/// error detail so a rollback outcome is never hidden.
fn reopen_failure(
    operation: &str,
    primary: Option<DomainError>,
    reopen: DomainError,
) -> DomainError {
    let reopen_detail = reopen.detail.unwrap_or(reopen.message_key);
    let mut error = match primary {
        Some(primary) => primary,
        None => DomainError::new(codes::INTERNAL, "error.restore_engine_reopen_failed"),
    };
    let note = format!("{operation} engine reopen failed after storage exchange: {reopen_detail}");
    error.detail = Some(match error.detail {
        Some(detail) => format!("{detail}; {note}"),
        None => note,
    });
    error
}

/// Attach the actual post-commit outcome to a two-phase import failure so the
/// UI can never claim "no data modified" when a rollback happened.
fn with_rollback_note(mut error: DomainError) -> DomainError {
    let note =
        "import failed after commit; database/config/resources rolled back to pre-import state";
    error.detail = Some(match error.detail {
        Some(detail) => format!("{detail}; {note}"),
        None => note.to_string(),
    });
    error
}

/// Every regular file under `data_dir` (excluding the database, the config,
/// the manifest, temp leftovers and the engine work dir) becomes a referenced
/// resource, keyed by its forward-slash relative path so nested configuration
/// subdirectories round-trip exactly.
fn collect_resources(data_dir: &Path) -> Result<Vec<(String, PathBuf)>, DomainError> {
    let mut resources = Vec::new();
    let mut stack = vec![data_dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).map_err(|e| internal(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| internal(e.to_string()))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|e| internal(e.to_string()))?;
            if file_type.is_dir() {
                // Core installations and runtime logs are reproducible state,
                // not user configuration. Never copy them into backups.
                let name = entry.file_name();
                if path.parent() == Some(data_dir)
                    && [".work", "cores", "guilogs", "logs", "run"]
                        .iter()
                        .any(|excluded| name.to_string_lossy().eq_ignore_ascii_case(excluded))
                {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() || is_reserved_name(&path) {
                continue;
            }
            let relative = path
                .strip_prefix(data_dir)
                .map_err(|_| internal("resource escaped data dir"))?
                .to_string_lossy()
                .replace('\\', "/");
            resources.push((relative, path));
        }
    }
    resources.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(resources)
}

/// Files that are part of the bundle's fixed members, not referenced
/// resources (and transient leftovers).
fn is_reserved_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return true;
    };
    name == DB_FILE_NAME
        || name == CONFIG_FILE_NAME
        || name == MANIFEST_NAME
        || name.ends_with(".tmp")
        || name.ends_with(".partial")
        || name.ends_with("-wal")
        || name.ends_with("-shm")
}

/// Safely extract a `backup.zip` bundle into `dest` (rejects traversal and
/// symlinks) before a remote restore.
pub fn extract_bundle_zip(zip_path: &Path, dest: &Path) -> Result<(), DomainError> {
    safe_unpack_zip(zip_path, dest, UnpackLimits::default())
        .map(|_| ())
        .map_err(|e| {
            DomainError::new(codes::FIELD_FORMAT, "error.backup_archive").with_detail(e.to_string())
        })
}

/// Pack a local bundle directory into an in-memory ZIP (`backup.zip`), keeping
/// only the bundle's own files. Used by the WebDAV upload path.
pub fn zip_bundle(root: &Path) -> Result<Vec<u8>, DomainError> {
    use std::io::Write;

    if !root.is_dir() {
        return Err(DomainError::new(codes::NOT_FOUND, "error.backup_not_found")
            .with_detail(root.display().to_string()));
    }
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![root.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).map_err(|e| internal(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| internal(e.to_string()))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|e| internal(e.to_string()))?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| internal("bundle path escaped root"))?
                .to_string_lossy()
                .replace('\\', "/");
            writer
                .start_file(relative, options)
                .map_err(|e| internal(e.to_string()))?;
            let bytes = std::fs::read(&path).map_err(|e| internal(e.to_string()))?;
            writer
                .write_all(&bytes)
                .map_err(|e| internal(e.to_string()))?;
        }
    }
    let cursor = writer.finish().map_err(|e| internal(e.to_string()))?;
    Ok(cursor.into_inner())
}

/// Upstream wraps the whole config directory in this top-level member before
/// zipping it (`BackupAndRestoreViewModel._guiConfigs`).
pub const UPSTREAM_GUI_CONFIGS: &str = "guiConfigs";

/// Pack a bundle directory into an upstream-interoperable ZIP: database,
/// config and every nested resource are placed under a single `guiConfigs/`
/// root, exactly like `BackupAndRestoreViewModel.CreateZipFileFromDirectory`.
/// The project's own `manifest.json` is not part of the upstream layout and is
/// dropped, so the remote `backup.zip` a `v2rayN` client GETs restores as-is.
pub fn zip_upstream_layout(bundle_root: &Path) -> Result<Vec<u8>, DomainError> {
    let output = zip_upstream_layout_into(bundle_root, std::io::Cursor::new(Vec::new()))?;
    Ok(output.into_inner())
}

/// Stream an upstream-compatible ZIP directly to disk for WebDAV upload.
pub fn zip_upstream_layout_to_file(
    bundle_root: &Path,
    destination: &Path,
) -> Result<u64, DomainError> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| internal(e.to_string()))?;
    }
    let file = std::fs::File::create(destination).map_err(|e| internal(e.to_string()))?;
    let output = zip_upstream_layout_into(bundle_root, file)?;
    Ok(output
        .metadata()
        .map_err(|e| internal(e.to_string()))?
        .len())
}

fn zip_upstream_layout_into<W: Write + std::io::Seek>(
    bundle_root: &Path,
    output: W,
) -> Result<W, DomainError> {
    if !bundle_root.is_dir() {
        return Err(DomainError::new(codes::NOT_FOUND, "error.backup_not_found")
            .with_detail(bundle_root.display().to_string()));
    }
    let mut writer = zip::ZipWriter::new(output);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![bundle_root.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).map_err(|e| internal(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| internal(e.to_string()))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|e| internal(e.to_string()))?;
            if file_type.is_dir() {
                if entry.file_name() == ".work" {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) == Some(MANIFEST_NAME) {
                continue;
            }
            let relative = path
                .strip_prefix(bundle_root)
                .map_err(|_| internal("bundle path escaped root"))?
                .to_string_lossy()
                .replace('\\', "/");
            writer
                .start_file(format!("{UPSTREAM_GUI_CONFIGS}/{relative}"), options)
                .map_err(|e| internal(e.to_string()))?;
            let mut source = std::fs::File::open(&path).map_err(|e| internal(e.to_string()))?;
            std::io::copy(&mut source, &mut writer).map_err(|e| internal(e.to_string()))?;
        }
    }
    writer.finish().map_err(|e| internal(e.to_string()))
}

fn copy_atomic(src: &Path, dest: &Path) -> Result<(), DomainError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| internal(e.to_string()))?;
    }
    let tmp = sibling_with_suffix(dest, ".tmp-restore");
    std::fs::copy(src, &tmp).map_err(|e| internal(e.to_string()))?;
    std::fs::rename(&tmp, dest).map_err(|e| internal(e.to_string()))?;
    Ok(())
}

fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(ToOwned::to_owned).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// Snapshot an existing destination before overwriting it during a restore.
fn backup_previous(dest: &Path) -> Result<Option<PathBuf>, DomainError> {
    if !dest.is_file() {
        return Ok(None);
    }
    let prior = sibling_with_suffix(dest, ".restore-prev");
    let _ = std::fs::remove_file(&prior);
    std::fs::copy(dest, &prior).map_err(|e| internal(e.to_string()))?;
    Ok(Some(prior))
}

/// Restore a destination from its snapshot, or remove it when newly created.
fn restore_previous(dest: &Path, prior: Option<&Path>) {
    match prior {
        Some(prior) => {
            let _ = std::fs::copy(prior, dest);
            let _ = std::fs::remove_file(prior);
        }
        None => {
            let _ = std::fs::remove_file(dest);
        }
    }
}

fn rollback_previous(applied: &[(PathBuf, Option<PathBuf>)]) {
    for (dest, prior) in applied.iter().rev() {
        restore_previous(dest, prior.as_deref());
    }
}

/// Undo a committed database swap by moving the candidate's `.bak` back.
fn rollback_database(db: &Path, target_backup: Option<&str>) {
    let Some(backup) = target_backup else {
        return;
    };
    let backup = Path::new(backup);
    if backup.is_file() {
        let _ = std::fs::remove_file(db);
        let _ = std::fs::rename(backup, db);
    }
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), DomainError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| internal(e.to_string()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| internal(e.to_string()))?;
    let tmp = sibling_with_suffix(path, ".tmp-restore");
    std::fs::write(&tmp, text).map_err(|e| internal(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| internal(e.to_string()))?;
    Ok(())
}

/// Whether a profile row exists in an opened store (activation-time guard).
fn profile_exists(store: &Store, index_id: &str) -> bool {
    store
        .count_query(
            "SELECT COUNT(*) FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
            &[&index_id],
        )
        .map(|count| count > 0)
        .unwrap_or(false)
}

/// Persisted dataset generation in a config file. A missing or unreadable
/// file means the pre-SP-03 generation zero, never an error: the activation
/// always publishes a strictly larger value.
fn read_dataset_epoch(config_path: &Path) -> u64 {
    std::fs::read_to_string(config_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.get("dataset_epoch").and_then(Value::as_u64))
        .unwrap_or(0)
}

/// Read-modify-write the dataset generation of the live config, preserving
/// every other key. A missing config becomes a minimal document (same
/// first-run semantics as a missing file: absent groups take defaults). A
/// present-but-unparseable (or non-object) file is a structured corrupt error
/// (SP-01 fail-closed): the activation must roll back rather than pave over
/// the evidence with a fresh document.
fn write_dataset_epoch(config_path: &Path, epoch: u64) -> Result<(), DomainError> {
    let mut value = match std::fs::read_to_string(config_path).ok() {
        None => Value::Object(serde_json::Map::new()),
        Some(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value @ Value::Object(_)) => value,
            _ => {
                return Err(
                    DomainError::new(codes::FIELD_FORMAT, "error.config_corrupt")
                        .with_detail(format!(
                            "{} is present but is not a JSON object",
                            config_path.display()
                        ))
                        .retryable(),
                );
            }
        },
    };
    value
        .as_object_mut()
        .expect("filtered to objects")
        .insert("dataset_epoch".to_string(), Value::from(epoch));
    write_json_atomic(config_path, &value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_source(dir: &Path, id: &str, theme: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("guiNConfig.json"),
            format!(r#"{{"IndexId":"{id}","UIItem":{{"CurrentTheme":"{theme}"}}}}"#),
        )
        .unwrap();
        let store = Store::create(dir.join("guiNDB.db")).unwrap();
        let conn = store.connection();
        for table in persistence::UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) \
             VALUES (?1, 5, 4, ?2)",
            rusqlite::params![id, id],
        )
        .unwrap();
        drop(store);
    }

    #[test]
    fn merge_import_a_b_a_is_idempotent_and_active_stays_resolvable() {
        let base = tempfile::tempdir().unwrap();
        let data = base.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let service = BackupService::new(&data);
        let a = base.path().join("a");
        let b = base.path().join("b");
        write_source(&a, "pA", "Dark");
        write_source(&b, "pB", "Light");

        assert_eq!(
            service
                .import_upstream_merge(&a, &base.path().join("wa"), 1)
                .unwrap()
                .status,
            ImportStatus::Imported
        );
        let second = service
            .import_upstream_merge(&b, &base.path().join("wb"), 1)
            .unwrap();
        assert_eq!(second.status, ImportStatus::Imported);
        // Re-importing A is a no-op, so B stays active under B's fingerprint.
        assert_eq!(
            service
                .import_upstream_merge(&a, &base.path().join("wa2"), 1)
                .unwrap()
                .status,
            ImportStatus::AlreadyImported
        );

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(data.join("guiNConfig.json")).unwrap())
                .unwrap();
        let active = config
            .get("active_index_id")
            .and_then(|v| v.as_str())
            .expect("active id");
        let expected = derived_id("profile", &format!("{}:pB", second.source_fingerprint));
        assert_eq!(
            active, expected,
            "A's no-op must not remap B's config under A's fingerprint"
        );
        assert_eq!(
            config
                .get("UIItem")
                .and_then(|ui| ui.get("CurrentTheme"))
                .and_then(|v| v.as_str()),
            Some("Light"),
            "settings must stay with the active (B) source"
        );

        let store = Store::open_readonly(data.join("guiNDB.db")).unwrap();
        let exists: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM ProfileItem WHERE IndexId = ?1",
                [active],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "the active node must exist in the database");
    }

    #[test]
    fn restore_replace_drops_existing_and_keeps_only_source() {
        let base = tempfile::tempdir().unwrap();
        let data = base.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let service = BackupService::new(&data);
        let a = base.path().join("a");
        let b = base.path().join("b");
        write_source(&a, "pA", "Dark");
        write_source(&b, "pB", "Light");

        let ra = service
            .import_upstream(&a, &base.path().join("wa"), 1)
            .unwrap();
        assert_eq!(ra.status, ImportStatus::Imported);
        let rb = service
            .import_upstream(&b, &base.path().join("wb"), 1)
            .unwrap();
        assert_eq!(rb.status, ImportStatus::Imported);

        let store = Store::open_readonly(data.join("guiNDB.db")).unwrap();
        let rows = store.read_rows("ProfileItem").unwrap();
        assert_eq!(rows.len(), 1, "restore must replace the existing rows");
        assert_eq!(rows[0].string("Remarks"), "pB");
        let a_id = derived_id("profile", &format!("{}:pA", ra.source_fingerprint));
        assert!(
            !rows.iter().any(|row| row.string("IndexId") == a_id),
            "the pre-restore node must be gone"
        );

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(data.join("guiNConfig.json")).unwrap())
                .unwrap();
        let expected = derived_id("profile", &format!("{}:pB", rb.source_fingerprint));
        assert_eq!(
            config.get("active_index_id").and_then(|v| v.as_str()),
            Some(expected.as_str())
        );
        assert_eq!(
            config.get("IndexId").and_then(|v| v.as_str()),
            Some(expected.as_str())
        );
    }
}
