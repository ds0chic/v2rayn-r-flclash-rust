//! Upstream source identification, consistent snapshot and reading.
//!
//! A source is either a directory or an upstream backup ZIP containing
//! `guiNConfig.json` / `guiNDB.db` (optionally under `guiConfigs/`). The
//! database is copied with the SQLite backup API so committed WAL content is
//! included — copying only the main file is forbidden (plan §11 step 2).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::DatabaseName;
use rusqlite::OpenFlags;

use crate::batch::SourceIdentity;
use crate::error::{PersistenceError, Result};
use crate::hash::sha256_hex;
use crate::rows::{self, RawRow};
use crate::schema;
use crate::upstream_config::ConfigDocument;

/// How the source was presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Directory,
    Archive,
}

impl SourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            SourceKind::Directory => "directory",
            SourceKind::Archive => "archive",
        }
    }
}

/// A readable upstream source before snapshotting.
#[derive(Debug, Clone)]
pub struct UpstreamSource {
    pub kind: SourceKind,
    pub path: PathBuf,
    /// The database file inside the directory/archive, when present.
    pub db_member: Option<String>,
    /// The config JSON member, when present.
    pub config_member: Option<String>,
    pub version: i32,
    pub present_tables: Vec<String>,
    pub config: Option<ConfigDocument>,
    pub missing_resources: Vec<String>,
}

const CONFIG_NAME: &str = "guiNConfig.json";
const DB_NAME: &str = "guiNDB.db";

/// Identify a directory source.
pub fn identify_directory(dir: &Path) -> Result<UpstreamSource> {
    if !dir.is_dir() {
        return Err(PersistenceError::NotASource(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    let config_path = dir.join(CONFIG_NAME);
    let db_path = dir.join(DB_NAME);
    let config = if config_path.is_file() {
        Some(ConfigDocument::parse(&std::fs::read_to_string(
            &config_path,
        )?)?)
    } else {
        None
    };
    let (version, present_tables) = if db_path.is_file() {
        read_db_meta(&db_path)?
    } else {
        (0, Vec::new())
    };
    if config.is_none() && !db_path.is_file() {
        return Err(PersistenceError::NotASource(format!(
            "{} contains neither {CONFIG_NAME} nor {DB_NAME}",
            dir.display()
        )));
    }
    let missing_resources = config
        .as_ref()
        .map(|c| check_resources(dir, c))
        .unwrap_or_default();
    Ok(UpstreamSource {
        kind: SourceKind::Directory,
        path: dir.to_path_buf(),
        db_member: db_path.is_file().then(|| DB_NAME.to_string()),
        config_member: config.is_some().then(|| CONFIG_NAME.to_string()),
        version,
        present_tables,
        config,
        missing_resources,
    })
}

/// Identify a ZIP archive source (upstream `guiConfigs/` layout supported).
pub fn identify_archive(path: &Path) -> Result<UpstreamSource> {
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut config_member = None;
    let mut db_member = None;
    for name in archive.file_names() {
        if is_member(name, CONFIG_NAME) && config_member.is_none() {
            config_member = Some(name.to_string());
        } else if is_member(name, DB_NAME) && db_member.is_none() {
            db_member = Some(name.to_string());
        }
    }
    if config_member.is_none() && db_member.is_none() {
        return Err(PersistenceError::NotASource(format!(
            "{} contains no {CONFIG_NAME}/{DB_NAME}",
            path.display()
        )));
    }
    let config = match &config_member {
        Some(name) => {
            let mut file = archive.by_name(name)?;
            let mut text = String::new();
            std::io::Read::read_to_string(&mut file, &mut text)?;
            Some(ConfigDocument::parse(&text)?)
        }
        None => None,
    };
    Ok(UpstreamSource {
        kind: SourceKind::Archive,
        path: path.to_path_buf(),
        db_member,
        config_member,
        version: 0,
        present_tables: Vec::new(),
        config,
        missing_resources: Vec::new(),
    })
}

/// Identify either a directory or a ZIP archive.
pub fn identify(path: &Path) -> Result<UpstreamSource> {
    if path.is_dir() {
        identify_directory(path)
    } else if path.is_file() {
        identify_archive(path)
    } else {
        Err(PersistenceError::NotASource(format!(
            "{} does not exist",
            path.display()
        )))
    }
}

fn is_member(name: &str, base: &str) -> bool {
    let leaf = name.rsplit(['/', '\\']).next().unwrap_or(name);
    leaf.eq_ignore_ascii_case(base)
}

/// Open a database read-only and read its version + present tables.
fn read_db_meta(db_path: &Path) -> Result<(i32, Vec<String>)> {
    let conn = rusqlite::Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let present = rows::list_tables(&conn)?;
    let version = if rows::table_exists(&conn, "ProfileItem")? {
        conn.query_row(
            "SELECT COALESCE(MAX(\"ConfigVersion\"), 0) FROM \"ProfileItem\"",
            [],
            |r| r.get::<_, i64>(0),
        )? as i32
    } else {
        0
    };
    Ok((version, present))
}

fn check_resources(dir: &Path, config: &ConfigDocument) -> Vec<String> {
    let mut missing = Vec::new();
    let candidates = [
        &["SystemProxyItem", "CustomSystemProxyPacPath"][..],
        &["SystemProxyItem", "CustomSystemProxyScriptPath"][..],
    ];
    for path in candidates {
        if let Some(raw) = config.field_state(path).as_str() {
            if raw.trim().is_empty() {
                continue;
            }
            let candidate = dir.join(raw);
            if !candidate.exists() {
                missing.push(raw.to_string());
            }
        }
    }
    missing
}

/// A consistent, hashed copy of an upstream source plus its parsed content.
#[derive(Debug, Clone)]
pub struct UpstreamSnapshot {
    pub source_id: String,
    pub content_hash: String,
    pub source_kind: SourceKind,
    pub version: i32,
    pub config: Option<ConfigDocument>,
    pub tables: BTreeMap<String, Vec<RawRow>>,
    pub missing_resources: Vec<String>,
}

impl UpstreamSnapshot {
    pub fn identity(&self) -> SourceIdentity {
        SourceIdentity {
            source_id: self.source_id.clone(),
            content_hash: self.content_hash.clone(),
        }
    }

    pub fn fingerprint(&self) -> String {
        self.identity().fingerprint()
    }

    pub fn rows(&self, table: &str) -> &[RawRow] {
        self.tables.get(table).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn present_tables(&self) -> impl Iterator<Item = &str> {
        schema::UPSTREAM_TABLES
            .iter()
            .map(|t| t.name)
            .filter(|name| {
                self.tables
                    .get(*name)
                    .map(|r| !r.is_empty())
                    .unwrap_or(false)
            })
    }
}

/// Take a consistent snapshot of `source` into `work_dir`.
///
/// `work_dir` must be a caller-owned temporary directory; the original source is
/// never modified. Returns the identified snapshot with parsed rows.
pub fn snapshot(source: &UpstreamSource, work_dir: &Path) -> Result<UpstreamSnapshot> {
    std::fs::create_dir_all(work_dir)?;
    let db_dest = work_dir.join(DB_NAME);
    let config_dest = work_dir.join(CONFIG_NAME);

    match source.kind {
        SourceKind::Directory => {
            if let Some(member) = &source.db_member {
                consistent_copy(&source.path.join(member), &db_dest)?;
            }
            if let Some(member) = &source.config_member {
                std::fs::copy(source.path.join(member), &config_dest)?;
            }
        }
        SourceKind::Archive => {
            let file = File::open(&source.path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            if let Some(member) = &source.db_member {
                extract_member(&mut archive, member, &db_dest)?;
            }
            if let Some(member) = &source.config_member {
                extract_member(&mut archive, member, &config_dest)?;
            }
        }
    }

    let config = if config_dest.is_file() {
        Some(ConfigDocument::parse(&std::fs::read_to_string(
            &config_dest,
        )?)?)
    } else {
        None
    };

    // Combined content hash: config bytes || db bytes, order-stable.
    let config_hash = if config_dest.is_file() {
        crate::hash::sha256_file(&config_dest)?
    } else {
        sha256_hex(b"")
    };
    let db_hash = if db_dest.is_file() {
        crate::hash::sha256_file(&db_dest)?
    } else {
        sha256_hex(b"")
    };
    let content_hash = sha256_hex(format!("{config_hash}:{db_hash}").as_bytes());
    let source_id = source_identity(&source.path, source.kind);

    let (version, _) = if db_dest.is_file() {
        read_db_meta(&db_dest)?
    } else {
        (0, Vec::new())
    };

    let mut tables = BTreeMap::new();
    if db_dest.is_file() {
        let conn =
            rusqlite::Connection::open_with_flags(&db_dest, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        for table in schema::UPSTREAM_TABLES {
            let rows = rows::read_table(&conn, table.name)?;
            tables.insert(table.name.to_string(), rows);
        }
    }

    Ok(UpstreamSnapshot {
        source_id,
        content_hash,
        source_kind: source.kind,
        version,
        config,
        tables,
        missing_resources: source.missing_resources.clone(),
    })
}

fn source_identity(path: &Path, kind: SourceKind) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let digest = sha256_hex(canonical.to_string_lossy().as_bytes());
    format!("{}:{}", kind.as_str(), &digest[..16])
}

/// Copy a SQLite database through the backup API, including WAL content.
pub fn consistent_copy(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        std::fs::remove_file(dst)?;
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = rusqlite::Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.backup(DatabaseName::Main, dst, None)?;
    Ok(())
}

fn extract_member(archive: &mut zip::ZipArchive<File>, member: &str, dest: &Path) -> Result<()> {
    // Only the resolved basenames are extracted, so no archive-supplied path
    // can escape the work directory.
    let mut entry = archive.by_name(member)?;
    let mut out = File::create(dest)?;
    std::io::copy(&mut entry, &mut out)?;
    out.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    fn write_source(dir: &Path) {
        std::fs::write(dir.join(CONFIG_NAME), r#"{"IndexId":"n1","UIItem":{}}"#).unwrap();
        let conn = Connection::open(dir.join(DB_NAME)).unwrap();
        conn.execute_batch(&schema::UPSTREAM_TABLES[1].create_sql())
            .unwrap();
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks, Network) \
             VALUES ('p1', 1, 2, 'node', 'ws')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn identify_directory_reports_version_and_tables() {
        let dir = tempfile::tempdir().unwrap();
        write_source(dir.path());
        let source = identify_directory(dir.path()).unwrap();
        assert_eq!(source.version, 2);
        assert!(source.present_tables.contains(&"ProfileItem".to_string()));
        assert!(source.config.is_some());
    }

    #[test]
    fn snapshot_reads_committed_wal_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_NAME), r#"{"IndexId":"n1"}"#).unwrap();
        let db_path = dir.path().join(DB_NAME);
        let conn = Connection::open(&db_path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.execute_batch(&schema::UPSTREAM_TABLES[1].create_sql())
            .unwrap();
        conn.execute(
            "INSERT INTO ProfileItem (IndexId, ConfigType, ConfigVersion, Remarks) \
             VALUES ('wal', 5, 4, 'from-wal')",
            [],
        )
        .unwrap();
        // Keep `conn` open and do not checkpoint: the row lives only in the WAL.
        let source = identify_directory(dir.path()).unwrap();
        let work = tempfile::tempdir().unwrap();
        let snapshot = snapshot(&source, work.path()).unwrap();
        let profiles = snapshot.rows("ProfileItem");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].string("Remarks"), "from-wal");
        drop(conn);
    }

    #[test]
    fn corrupt_database_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(DB_NAME), b"this is not sqlite").unwrap();
        let result = identify_directory(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn missing_sources_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let result = identify_directory(dir.path());
        assert!(matches!(result, Err(PersistenceError::NotASource(_))));
    }

    #[test]
    fn archive_roundtrip_from_gui_configs_layout() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("backup.zip");
        {
            let file = File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
            writer
                .start_file("guiConfigs/guiNConfig.json", options)
                .unwrap();
            writer.write_all(br#"{"IndexId":"n9"}"#).unwrap();
            writer.finish().unwrap();
        }
        let source = identify_archive(&zip_path).unwrap();
        assert_eq!(
            source.config_member.as_deref(),
            Some("guiConfigs/guiNConfig.json")
        );
        assert!(source.config.is_some());
        let work = tempfile::tempdir().unwrap();
        let snapshot = snapshot(&source, work.path()).unwrap();
        assert_eq!(snapshot.config.as_ref().unwrap().index_id(), "n9");
        assert_eq!(
            snapshot.config.unwrap().to_json_value()["IndexId"],
            json!("n9")
        );
    }
}
