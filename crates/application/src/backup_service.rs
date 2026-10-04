//! T16 local backup / restore use cases over the T04 persistence bundle.
//!
//! A local backup is a versioned bundle directory (`manifest.json` + a
//! consistency snapshot of `guiNDB.db` + `guiNConfig.json` + referenced
//! resources). Restore verifies the whole bundle and writes a candidate before
//! touching the live database, so a failed restore never damages the current
//! configuration (plan §15). Upstream `guiConfigs/` ZIP archives are recognised
//! and imported through the T04 candidate flow.

use std::path::{Path, PathBuf};

use domain::{codes, DomainError};
use persistence::backup::{
    self, ArchiveRecognition, BackupManifest, BackupVerification, ResourceEntry, RestoreReport,
    CONFIG_FILE_NAME, DB_FILE_NAME, MANIFEST_NAME,
};
use persistence::hash::{derived_id, sha256_file};
use persistence::store::table_counts;
use persistence::{
    import_from_path, ConfigDocument, ImportOptions, ImportReport, ImportStatus, PersistenceError,
    Store,
};
use serde_json::Value;
use updater::unpack::{safe_unpack_zip, UnpackLimits};

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
        let report =
            backup::restore_backup(root, &self.db_path(), work_dir).map_err(persist_error)?;

        // Config first, then resources; roll the whole group back on failure.
        let config_path = self.data_dir.join(CONFIG_FILE_NAME);
        let mut config_prior: Option<PathBuf> = None;
        let bundled_config = root.join(CONFIG_FILE_NAME);
        if bundled_config.is_file() {
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

    /// Import an upstream directory or ZIP through the T04 candidate flow and
    /// activate the imported configuration.
    ///
    /// The upstream `guiNConfig.json` is not left stranded in the DB meta:
    /// after a successful candidate commit it is written to the live
    /// `guiNConfig.json` with `IndexId`/`SubIndexId` remapped to the imported
    /// rows and the engine `active_index_id` set, so settings/active/group
    /// become the active configuration on the next load/reopen.
    pub fn import_upstream(
        &self,
        path: &Path,
        work_dir: &Path,
        now: i64,
    ) -> Result<ImportReport, DomainError> {
        let report = import_from_path(
            path,
            &self.db_path(),
            work_dir,
            &ImportOptions {
                now,
                ..ImportOptions::default()
            },
        )
        .map_err(persist_error)?;
        if matches!(
            report.status,
            ImportStatus::Imported | ImportStatus::AlreadyImported
        ) {
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
        // Stage from the read-only source first so a staging failure never
        // leaves the committed database dangling.
        let staged = match stage_upstream_resources(source, work_dir) {
            Ok(staged) => staged,
            Err(error) => {
                rollback_database(&self.db_path(), db_backup);
                return Err(error);
            }
        };
        let config_path = self.data_dir.join(CONFIG_FILE_NAME);
        let config_prior = match backup_previous(&config_path) {
            Ok(prior) => prior,
            Err(error) => {
                rollback_database(&self.db_path(), db_backup);
                return Err(error);
            }
        };
        if let Err(error) = self.activate_upstream_config(fingerprint) {
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
    pub fn activate_upstream_config(
        &self,
        fingerprint: &str,
    ) -> Result<Option<String>, DomainError> {
        let store = Store::open(self.db_path()).map_err(persist_error)?;
        let raw = store.get_meta("upstream_config").map_err(persist_error)?;
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
        let old_sub = object
            .get("SubIndexId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let active = (!old_active.is_empty())
            .then(|| derived_id("profile", &format!("{fingerprint}:{old_active}")));
        let sub =
            (!old_sub.is_empty()).then(|| derived_id("sub", &format!("{fingerprint}:{old_sub}")));
        object.insert(
            "IndexId".to_string(),
            Value::String(active.clone().unwrap_or_default()),
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
        object
            .entry("rule_mode".to_string())
            .or_insert_with(|| Value::String("Rule".to_string()));
        write_json_atomic(&self.data_dir.join(CONFIG_FILE_NAME), &value)?;
        Ok(active)
    }

    /// List bundles directly under `parent` (a directory with a manifest).
    pub fn list(parent: &Path) -> Result<Vec<BackupManifest>, DomainError> {
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
                    manifests.push(manifest);
                }
            }
        }
        manifests.sort_by_key(|manifest| std::cmp::Reverse(manifest.created_at));
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
                // Never walk into the transient work dir (restore candidates).
                if entry.file_name() == ".work" {
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
    use std::io::Write;

    if !bundle_root.is_dir() {
        return Err(DomainError::new(codes::NOT_FOUND, "error.backup_not_found")
            .with_detail(bundle_root.display().to_string()));
    }
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
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
            let bytes = std::fs::read(&path).map_err(|e| internal(e.to_string()))?;
            writer
                .write_all(&bytes)
                .map_err(|e| internal(e.to_string()))?;
        }
    }
    let cursor = writer.finish().map_err(|e| internal(e.to_string()))?;
    Ok(cursor.into_inner())
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
