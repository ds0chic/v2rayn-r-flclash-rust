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
    self, ArchiveRecognition, BackupManifest, BackupVerification, RestoreReport, CONFIG_FILE_NAME,
    DB_FILE_NAME, MANIFEST_NAME,
};
use persistence::store::table_counts;
use persistence::{import_from_path, ImportOptions, ImportReport, PersistenceError, Store};
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

    /// Restore a verified bundle into the live database (and config).
    ///
    /// The database is swapped through the T04 candidate commit; the bundled
    /// config is copied last with a temp+rename so a failure cannot leave a
    /// half-restored pair.
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
        let report =
            backup::restore_backup(root, &self.db_path(), work_dir).map_err(persist_error)?;
        let bundled_config = root.join(CONFIG_FILE_NAME);
        if bundled_config.is_file() {
            copy_atomic(&bundled_config, &self.data_dir.join(CONFIG_FILE_NAME))?;
        }
        Ok(report)
    }

    /// Recognise an upstream `guiConfigs/` archive (or this project's ZIP).
    pub fn recognize(&self, path: &Path) -> Result<ArchiveRecognition, DomainError> {
        backup::recognize_archive(path).map_err(persist_error)
    }

    /// Import an upstream directory or ZIP through the T04 candidate flow.
    pub fn import_upstream(
        &self,
        path: &Path,
        work_dir: &Path,
        now: i64,
    ) -> Result<ImportReport, DomainError> {
        import_from_path(
            path,
            &self.db_path(),
            work_dir,
            &ImportOptions {
                now,
                ..ImportOptions::default()
            },
        )
        .map_err(persist_error)
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

/// Regular files directly under `data_dir` (excluding the database, the config
/// and temp leftovers) become the bundle's referenced resource list.
fn collect_resources(data_dir: &Path) -> Result<Vec<(String, PathBuf)>, DomainError> {
    let mut resources = Vec::new();
    let entries = std::fs::read_dir(data_dir).map_err(|e| internal(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| internal(e.to_string()))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| internal(e.to_string()))?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == DB_FILE_NAME
            || name == CONFIG_FILE_NAME
            || name == MANIFEST_NAME
            || name.ends_with(".tmp")
            || name.ends_with(".partial")
        {
            continue;
        }
        resources.push((name, path));
    }
    resources.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(resources)
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

fn copy_atomic(src: &Path, dest: &Path) -> Result<(), DomainError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| internal(e.to_string()))?;
    }
    let tmp = dest.with_extension("tmp-restore");
    std::fs::copy(src, &tmp).map_err(|e| internal(e.to_string()))?;
    std::fs::rename(&tmp, dest).map_err(|e| internal(e.to_string()))?;
    Ok(())
}
