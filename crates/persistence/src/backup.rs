//! Local backup / restore and upstream backup-archive recognition.
//!
//! A bundle is a directory containing a versioned `manifest.json`, a
//! consistency snapshot of the database, the config document and every
//! referenced resource. Restore verifies the whole bundle **before** writing a
//! candidate, so a failed restore never damages the current configuration
//! (plan §15).
//!
//! Upstream `guiConfigs/` ZIP archives are recognised for the import path; the
//! UI-facing flow is T16.

use std::path::{Path, PathBuf};

use rusqlite::OpenFlags;
use serde::{Deserialize, Serialize};

use crate::candidate::commit_candidate;
use crate::error::{PersistenceError, Result};
use crate::hash::{sha256_file, sha256_hex};
use crate::report::EntityCount;
use crate::upstream_db::consistent_copy;

pub const BACKUP_FORMAT_VERSION: u32 = 1;
pub const MANIFEST_NAME: &str = "manifest.json";
pub const DB_FILE_NAME: &str = "guiNDB.db";
pub const CONFIG_FILE_NAME: &str = "guiNConfig.json";

/// One referenced resource bundled alongside the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceEntry {
    pub relative_path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Versioned backup manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub format_version: u32,
    pub created_at: i64,
    pub app_source_commit: String,
    pub db_sha256: String,
    pub config_sha256: Option<String>,
    pub referenced_resources: Vec<ResourceEntry>,
    pub entity_counts: Vec<EntityCount>,
}

/// A written backup bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupBundle {
    pub root: PathBuf,
    pub manifest: BackupManifest,
}

/// Bundle verification result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackupVerification {
    pub ok: bool,
    pub missing: Vec<String>,
    pub mismatched: Vec<String>,
}

/// Restore outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    pub restored: bool,
    pub target_backup: Option<String>,
    pub message: String,
}

/// Recognition result for a possible upstream backup archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveRecognition {
    pub is_upstream: bool,
    pub has_config: bool,
    pub has_db: bool,
    pub layout: String,
    pub entries: Vec<String>,
}

fn reject_unsafe_relative(path: &str) -> Result<()> {
    if path.trim().is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains("..")
        || path.contains(':')
    {
        return Err(PersistenceError::PathRejected(path.to_string()));
    }
    Ok(())
}

/// Write a backup bundle. `resources` are `(relative_path, absolute_source)`
/// pairs; each is verified to stay inside the bundle.
pub fn create_backup(
    root: &Path,
    db_path: &Path,
    config_path: Option<&Path>,
    resources: &[(String, PathBuf)],
    entity_counts: &[EntityCount],
    created_at: i64,
    app_source_commit: &str,
) -> Result<BackupBundle> {
    std::fs::create_dir_all(root)?;
    let db_dest = root.join(DB_FILE_NAME);
    consistent_copy(db_path, &db_dest)?;
    let db_sha256 = sha256_file(&db_dest)?;

    let config_sha256 = match config_path {
        Some(config) => {
            let dest = root.join(CONFIG_FILE_NAME);
            std::fs::copy(config, &dest)?;
            Some(sha256_file(&dest)?)
        }
        None => None,
    };

    let mut resource_entries = Vec::new();
    for (relative, source) in resources {
        reject_unsafe_relative(relative)?;
        let dest = root.join(relative);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(source, &dest)?;
        resource_entries.push(ResourceEntry {
            relative_path: relative.clone(),
            sha256: sha256_file(&dest)?,
            bytes: std::fs::metadata(&dest)?.len(),
        });
    }

    let manifest = BackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        created_at,
        app_source_commit: app_source_commit.to_string(),
        db_sha256,
        config_sha256,
        referenced_resources: resource_entries,
        entity_counts: entity_counts.to_vec(),
    };
    std::fs::write(
        root.join(MANIFEST_NAME),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(BackupBundle {
        root: root.to_path_buf(),
        manifest,
    })
}

/// Read a manifest from a bundle directory.
pub fn read_manifest(root: &Path) -> Result<BackupManifest> {
    let path = root.join(MANIFEST_NAME);
    if !path.is_file() {
        return Err(PersistenceError::NotASource(format!(
            "{} missing {MANIFEST_NAME}",
            root.display()
        )));
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(&path)?)?)
}

/// Verify a bundle's integrity without writing anything.
pub fn verify_backup(root: &Path) -> Result<BackupVerification> {
    let manifest = read_manifest(root)?;
    let mut result = BackupVerification::default();
    if manifest.format_version != BACKUP_FORMAT_VERSION {
        result.mismatched.push(format!(
            "manifest format_version {} != {}",
            manifest.format_version, BACKUP_FORMAT_VERSION
        ));
        result.ok = false;
        return Ok(result);
    }
    let db = root.join(DB_FILE_NAME);
    if !db.is_file() {
        result.missing.push(DB_FILE_NAME.to_string());
    } else if sha256_file(&db)? != manifest.db_sha256 {
        result.mismatched.push(DB_FILE_NAME.to_string());
    }
    if let Some(expected) = &manifest.config_sha256 {
        let config = root.join(CONFIG_FILE_NAME);
        if !config.is_file() {
            result.missing.push(CONFIG_FILE_NAME.to_string());
        } else if &sha256_file(&config)? != expected {
            result.mismatched.push(CONFIG_FILE_NAME.to_string());
        }
    }
    for resource in &manifest.referenced_resources {
        let path = root.join(&resource.relative_path);
        if !path.is_file() {
            result.missing.push(resource.relative_path.clone());
        } else if sha256_file(&path)? != resource.sha256 {
            result.mismatched.push(resource.relative_path.clone());
        }
    }
    result.ok = result.missing.is_empty() && result.mismatched.is_empty();
    Ok(result)
}

/// Verify the database file's SQLite integrity.
pub fn sqlite_integrity_ok(db_path: &Path) -> Result<bool> {
    let conn = rusqlite::Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let result: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    Ok(result == "ok")
}

/// Restore a verified bundle into `target_db`. On any failure the existing
/// target is left untouched.
pub fn restore_backup(root: &Path, target_db: &Path, work_dir: &Path) -> Result<RestoreReport> {
    let verification = verify_backup(root)?;
    if !verification.ok {
        return Err(PersistenceError::Validation(format!(
            "backup verification failed: missing {:?}, mismatched {:?}",
            verification.missing, verification.mismatched
        )));
    }
    std::fs::create_dir_all(work_dir)?;
    let candidate = work_dir.join("restore-candidate.db");
    if candidate.exists() {
        std::fs::remove_file(&candidate)?;
    }
    std::fs::copy(root.join(DB_FILE_NAME), &candidate)?;
    if !sqlite_integrity_ok(&candidate)? {
        let _ = std::fs::remove_file(&candidate);
        return Err(PersistenceError::Corrupt(
            "restored database failed integrity_check".into(),
        ));
    }
    let target_backup = commit_candidate(&candidate, target_db)?;
    Ok(RestoreReport {
        restored: true,
        target_backup,
        message: "restore completed".into(),
    })
}

/// Recognise an upstream `guiConfigs/` ZIP without extracting it.
pub fn recognize_archive(path: &Path) -> Result<ArchiveRecognition> {
    let file = std::fs::File::open(path)?;
    let archive = zip::ZipArchive::new(file)?;
    let entries: Vec<String> = archive.file_names().map(str::to_string).collect();
    let has_config = entries
        .iter()
        .any(|name| name.rsplit(['/', '\\']).next() == Some(CONFIG_FILE_NAME));
    let has_db = entries
        .iter()
        .any(|name| name.rsplit(['/', '\\']).next() == Some(DB_FILE_NAME));
    let layout = if entries.iter().any(|n| n.starts_with("guiConfigs/")) {
        "guiConfigs/".to_string()
    } else {
        "root".to_string()
    };
    Ok(ArchiveRecognition {
        is_upstream: has_config || has_db,
        has_config,
        has_db,
        layout,
        entries,
    })
}

/// Stable content digest of a bundle (manifest-independent helper for naming).
pub fn bundle_digest(manifest: &BackupManifest) -> String {
    sha256_hex(
        format!(
            "{}:{}:{}",
            manifest.db_sha256,
            manifest.config_sha256.as_deref().unwrap_or(""),
            manifest.created_at
        )
        .as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    fn make_db(path: &Path) {
        let store = Store::create(path).unwrap();
        let conn = store.connection();
        conn.execute(
            "INSERT INTO SubItem (Id, Remarks) VALUES ('s1','backup')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn create_verify_and_restore_roundtrip() {
        let src = tempfile::tempdir().unwrap();
        let db = src.path().join("guiNDB.db");
        make_db(&db);
        std::fs::write(src.path().join(CONFIG_FILE_NAME), r#"{"IndexId":"n1"}"#).unwrap();
        let resource = src.path().join("custom.json");
        std::fs::write(&resource, r#"{"x":1}"#).unwrap();

        let backup_root = tempfile::tempdir().unwrap();
        let bundle = create_backup(
            backup_root.path(),
            &db,
            Some(&src.path().join(CONFIG_FILE_NAME)),
            &[("custom/custom.json".to_string(), resource)],
            &[],
            42,
            "7d6a967",
        )
        .unwrap();
        assert_eq!(bundle.manifest.format_version, BACKUP_FORMAT_VERSION);
        assert!(verify_backup(backup_root.path()).unwrap().ok);

        let target = tempfile::tempdir().unwrap();
        let target_db = target.path().join("guiNDB.db");
        let work = tempfile::tempdir().unwrap();
        let report = restore_backup(backup_root.path(), &target_db, work.path()).unwrap();
        assert!(report.restored);
        assert!(target_db.is_file());
        let store = Store::open(&target_db).unwrap();
        assert_eq!(store.count_rows("SubItem").unwrap(), 1);
    }

    #[test]
    fn tampered_bundle_is_rejected_before_writing() {
        let src = tempfile::tempdir().unwrap();
        let db = src.path().join("guiNDB.db");
        make_db(&db);
        let backup_root = tempfile::tempdir().unwrap();
        create_backup(backup_root.path(), &db, None, &[], &[], 1, "c").unwrap();
        // Corrupt the bundled database.
        std::fs::write(backup_root.path().join(DB_FILE_NAME), b"corrupted").unwrap();
        let verification = verify_backup(backup_root.path()).unwrap();
        assert!(!verification.ok);

        let target = tempfile::tempdir().unwrap();
        let target_db = target.path().join("guiNDB.db");
        make_db(&target_db);
        let work = tempfile::tempdir().unwrap();
        let result = restore_backup(backup_root.path(), &target_db, work.path());
        assert!(result.is_err());
        // Existing target unchanged.
        let store = Store::open(&target_db).unwrap();
        assert_eq!(store.count_rows("SubItem").unwrap(), 1);
    }

    #[test]
    fn path_traversal_resource_is_rejected() {
        let src = tempfile::tempdir().unwrap();
        let db = src.path().join("guiNDB.db");
        make_db(&db);
        let resource = src.path().join("x.json");
        std::fs::write(&resource, "{}").unwrap();
        let backup_root = tempfile::tempdir().unwrap();
        let result = create_backup(
            backup_root.path(),
            &db,
            None,
            &[("../escape.json".to_string(), resource)],
            &[],
            1,
            "c",
        );
        assert!(matches!(result, Err(PersistenceError::PathRejected(_))));
    }

    #[test]
    fn recognizes_upstream_gui_configs_archive() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("upstream.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            writer
                .start_file("guiConfigs/guiNConfig.json", options)
                .unwrap();
            writer.write_all(b"{}").unwrap();
            writer.finish().unwrap();
        }
        let recognition = recognize_archive(&zip_path).unwrap();
        assert!(recognition.is_upstream);
        assert!(recognition.has_config);
        assert!(!recognition.has_db);
        assert_eq!(recognition.layout, "guiConfigs/");
    }
}
