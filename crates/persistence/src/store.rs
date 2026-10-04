//! SQLite store: single-writer access to the application database.
//!
//! The store owns the full upstream-compatible schema plus the import
//! bookkeeping tables. Writes go through this type so there is exactly one
//! writer (plan §04), and every multi-row write happens inside a transaction.

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OpenFlags, Transaction};
use serde_json::Value;

use crate::batch::ImportBatch;
use crate::error::{PersistenceError, Result};
use crate::report::EntityCount;
use crate::rows::{self, RawRow};
use crate::schema;

/// A connection to one application database.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Create or open a database, ensuring the schema exists.
    pub fn create(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Open an existing database read/write, ensuring the schema exists.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::create(path)
    }

    /// Open an existing database read-only. Does not create the schema.
    pub fn open_readonly(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(Self { conn })
    }

    /// In-memory store, useful for tests and candidate validation fixtures.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self { conn };
        store.apply_schema()?;
        Ok(store)
    }

    pub fn apply_schema(&self) -> Result<()> {
        for table in schema::UPSTREAM_TABLES {
            self.conn.execute_batch(&table.create_sql())?;
        }
        for statement in schema::APP_TABLES {
            self.conn.execute_batch(statement)?;
        }
        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn begin(&self) -> Result<Transaction<'_>> {
        Ok(self.conn.unchecked_transaction()?)
    }

    /// Insert one raw upstream row into its table, mapping each schema column by
    /// name. Unknown columns are retained separately via [`Store::insert_raw_record`].
    pub fn insert_row(&self, conn: &Connection, row: &RawRow) -> Result<()> {
        let table = schema::upstream_table(&row.table)
            .ok_or_else(|| PersistenceError::internal(format!("unknown table {}", row.table)))?;
        let columns: Vec<&str> = table.columns.iter().map(|c| c.name).collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "INSERT INTO \"{}\" ({}) VALUES ({})",
            table.name,
            columns
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );
        let values: Vec<SqlValue> = columns
            .iter()
            .map(|col| json_to_sql(row.get(col).unwrap_or(&Value::Null)))
            .collect();
        conn.execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(())
    }

    /// Upsert a row by primary key (used by migrations and re-import).
    pub fn upsert_row(&self, conn: &Connection, row: &RawRow) -> Result<()> {
        let table = schema::upstream_table(&row.table)
            .ok_or_else(|| PersistenceError::internal(format!("unknown table {}", row.table)))?;
        let columns: Vec<&str> = table.columns.iter().map(|c| c.name).collect();
        let pk = table
            .columns
            .iter()
            .find(|c| c.primary_key)
            .ok_or_else(|| PersistenceError::internal("table without primary key"))?;
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
        let updates: Vec<String> = columns
            .iter()
            .filter(|c| **c != pk.name)
            .map(|c| format!("\"{c}\" = excluded.\"{c}\""))
            .collect();
        let sql = format!(
            "INSERT INTO \"{}\" ({}) VALUES ({}) ON CONFLICT(\"{}\") DO UPDATE SET {}",
            table.name,
            columns
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", "),
            pk.name,
            updates.join(", ")
        );
        let values: Vec<SqlValue> = columns
            .iter()
            .map(|col| json_to_sql(row.get(col).unwrap_or(&Value::Null)))
            .collect();
        conn.execute(&sql, rusqlite::params_from_iter(values))?;
        Ok(())
    }

    pub fn read_rows(&self, table: &str) -> Result<Vec<RawRow>> {
        rows::read_table(&self.conn, table)
    }

    pub fn count_rows(&self, table: &str) -> Result<u64> {
        let sql = format!("SELECT COUNT(*) FROM \"{}\"", table.replace('"', ""));
        let count: i64 = self.conn.query_row(&sql, [], |r| r.get(0))?;
        Ok(count as u64)
    }

    /// Run an arbitrary `SELECT` and map every row by column name (same
    /// semantics as [`Store::read_rows`]). Used by the application layer for
    /// filtered/sorted/paged profile queries without exposing the connection.
    pub fn query_rows(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<Vec<RawRow>> {
        let mut stmt = self.conn.prepare(sql)?;
        let names: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let mut rows = stmt.query(params)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let mut values = serde_json::Map::new();
            for (index, name) in names.iter().enumerate() {
                values.insert(name.clone(), rows::value_from_ref(row.get_ref(index)?));
            }
            out.push(RawRow {
                table: "ProfileItem".to_string(),
                values,
            });
        }
        Ok(out)
    }

    /// Count the rows matched by `sql` (a `SELECT COUNT(*)` statement).
    pub fn count_query(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<u64> {
        let count: i64 = self.conn.query_row(sql, params, |r| r.get(0))?;
        Ok(count.max(0) as u64)
    }

    /// Execute a parameterized `DELETE`/`UPDATE`, returning affected rows.
    pub fn execute(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<usize> {
        Ok(self.conn.execute(sql, params)?)
    }

    // -- raw retention -----------------------------------------------------

    pub fn insert_raw_record(
        &self,
        conn: &Connection,
        batch_id: &str,
        table: &str,
        source_id: &str,
        raw: &Value,
    ) -> Result<()> {
        conn.execute(
            "INSERT INTO \"raw_records\" (\"batch_id\", \"source_table\", \"source_id\", \"raw_json\") \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![batch_id, table, source_id, serde_json::to_string(raw)?],
        )?;
        Ok(())
    }

    pub fn count_raw_records(&self, batch_id: &str) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM \"raw_records\" WHERE \"batch_id\" = ?1",
            [batch_id],
            |r| r.get(0),
        )?;
        Ok(count as u64)
    }

    // -- id mapping --------------------------------------------------------

    pub fn insert_id_map(
        &self,
        conn: &Connection,
        batch_id: &str,
        source_table: &str,
        source_id: &str,
        new_id: &str,
    ) -> Result<()> {
        conn.execute(
            "INSERT INTO \"id_map\" (\"batch_id\", \"source_table\", \"source_id\", \"new_id\") \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![batch_id, source_table, source_id, new_id],
        )?;
        Ok(())
    }

    pub fn resolve_id(
        &self,
        batch_id: &str,
        source_table: &str,
        source_id: &str,
    ) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT \"new_id\" FROM \"id_map\" WHERE \"batch_id\" = ?1 AND \
             \"source_table\" = ?2 AND \"source_id\" = ?3",
        )?;
        let mut rows = stmt.query(rusqlite::params![batch_id, source_table, source_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    // -- import batches ----------------------------------------------------

    pub fn find_batch(&self, source_fingerprint: &str) -> Result<Option<ImportBatch>> {
        if !rows::table_exists(&self.conn, "import_batches")? {
            return Ok(None);
        }
        let mut stmt = self.conn.prepare(
            "SELECT \"source_fingerprint\", \"source_id\", \"content_hash\", \"batch_id\", \
             \"source_kind\", \"source_version\", \"imported_at\", \"entity_counts\" \
             FROM \"import_batches\" WHERE \"source_fingerprint\" = ?1",
        )?;
        let mut rows = stmt.query([source_fingerprint])?;
        match rows.next()? {
            None => Ok(None),
            Some(row) => {
                let counts_json: String = row.get(7)?;
                Ok(Some(ImportBatch {
                    source_fingerprint: row.get(0)?,
                    source_id: row.get(1)?,
                    content_hash: row.get(2)?,
                    batch_id: row.get(3)?,
                    source_kind: row.get(4)?,
                    source_version: row.get(5)?,
                    imported_at: row.get(6)?,
                    entity_counts: serde_json::from_str(&counts_json)?,
                }))
            }
        }
    }

    pub fn record_import_batch(&self, conn: &Connection, batch: &ImportBatch) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO \"import_batches\" (\"source_fingerprint\", \"source_id\", \
             \"content_hash\", \"batch_id\", \"source_kind\", \"source_version\", \"imported_at\", \
             \"entity_counts\") VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                batch.source_fingerprint,
                batch.source_id,
                batch.content_hash,
                batch.batch_id,
                batch.source_kind,
                batch.source_version,
                batch.imported_at,
                serde_json::to_string(&batch.entity_counts)?,
            ],
        )?;
        Ok(())
    }

    pub fn list_import_batches(&self) -> Result<Vec<ImportBatch>> {
        if !rows::table_exists(&self.conn, "import_batches")? {
            return Ok(Vec::new());
        }
        let fingerprints: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT \"source_fingerprint\" FROM \"import_batches\" ORDER BY \"imported_at\"",
            )?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };
        let mut batches = Vec::new();
        for fp in fingerprints {
            if let Some(batch) = self.find_batch(&fp)? {
                batches.push(batch);
            }
        }
        Ok(batches)
    }

    // -- migrations --------------------------------------------------------

    pub fn record_migration(&self, conn: &Connection, log: MigrationLog<'_>) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO \"migration_records\" (\"migration_id\", \"source_hash\", \
             \"from_version\", \"to_version\", \"applied_at\", \"entities_touched\", \"notes\") \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                log.migration_id,
                log.source_hash,
                log.from_version,
                log.to_version,
                log.applied_at,
                log.entities_touched as i64,
                log.notes
            ],
        )?;
        Ok(())
    }

    pub fn has_migration(&self, migration_id: &str, source_hash: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM \"migration_records\" WHERE \"migration_id\" = ?1 AND \
             \"source_hash\" = ?2",
            rusqlite::params![migration_id, source_hash],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    // -- metadata ----------------------------------------------------------

    pub fn set_meta(&self, conn: &Connection, key: &str, value: &str) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO \"app_meta\" (\"key\", \"value\") VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT \"value\" FROM \"app_meta\" WHERE \"key\" = ?1")?;
        let mut rows = stmt.query([key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }
}

/// A `migration_records` insert.
#[derive(Debug, Clone, Copy)]
pub struct MigrationLog<'a> {
    pub migration_id: &'a str,
    pub source_hash: &'a str,
    pub from_version: i32,
    pub to_version: i32,
    pub applied_at: i64,
    pub entities_touched: u64,
    pub notes: Option<&'a str>,
}

/// Convert a JSON value to a SQLite bindable value.
pub fn json_to_sql(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else if let Some(u) = n.as_u64() {
                SqlValue::Integer(u as i64)
            } else {
                SqlValue::Real(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => SqlValue::Text(s.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}

/// Convenience: build the per-table count rows from a store.
pub fn table_counts(store: &Store) -> Result<Vec<EntityCount>> {
    let mut counts = Vec::new();
    for table in schema::UPSTREAM_TABLES {
        let rows = store.count_rows(table.name)?;
        counts.push(EntityCount {
            table: table.name.to_string(),
            source_rows: rows,
            imported_rows: rows,
            migrated_rows: 0,
            skipped_rows: 0,
        });
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn insert_and_read_row_roundtrip() {
        let store = Store::in_memory().unwrap();
        let mut row = RawRow::new("SubItem");
        row.set("Id", json!("s1"));
        row.set("Remarks", json!("订阅"));
        row.set("Enabled", json!(true));
        row.set("UpdateTime", json!(1700000000));
        store.insert_row(store.connection(), &row).unwrap();
        let rows = store.read_rows("SubItem").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].string("Remarks"), "订阅");
        assert!(rows[0].bool("Enabled"));
        assert_eq!(rows[0].i64("UpdateTime"), 1_700_000_000);
    }

    #[test]
    fn upsert_replaces_on_primary_key() {
        let store = Store::in_memory().unwrap();
        let mut row = RawRow::new("SubItem");
        row.set("Id", json!("s1"));
        row.set("Remarks", json!("first"));
        store.upsert_row(store.connection(), &row).unwrap();
        let mut updated = row.clone();
        updated.set("Remarks", json!("second"));
        store.upsert_row(store.connection(), &updated).unwrap();
        let rows = store.read_rows("SubItem").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].string("Remarks"), "second");
    }

    #[test]
    fn id_map_resolves_within_batch() {
        let store = Store::in_memory().unwrap();
        let conn = store.connection();
        store
            .insert_id_map(conn, "b1", "ProfileItem", "old-1", "new-1")
            .unwrap();
        assert_eq!(
            store.resolve_id("b1", "ProfileItem", "old-1").unwrap(),
            Some("new-1".to_string())
        );
        assert_eq!(
            store.resolve_id("b1", "ProfileItem", "old-2").unwrap(),
            None
        );
        assert_eq!(
            store.resolve_id("b2", "ProfileItem", "old-1").unwrap(),
            None
        );
    }

    #[test]
    fn batch_roundtrip_and_lookup() {
        let store = Store::in_memory().unwrap();
        let batch = ImportBatch {
            source_fingerprint: "fp".into(),
            source_id: "src".into(),
            content_hash: "hash".into(),
            batch_id: "b1".into(),
            source_kind: "directory".into(),
            source_version: 2,
            imported_at: 1,
            entity_counts: vec![EntityCount {
                table: "ProfileItem".into(),
                source_rows: 3,
                imported_rows: 3,
                ..Default::default()
            }],
        };
        store
            .record_import_batch(store.connection(), &batch)
            .unwrap();
        assert_eq!(store.find_batch("fp").unwrap(), Some(batch.clone()));
        assert_eq!(store.list_import_batches().unwrap().len(), 1);
    }

    #[test]
    fn migration_and_meta_records() {
        let store = Store::in_memory().unwrap();
        let conn = store.connection();
        store
            .record_migration(
                conn,
                MigrationLog {
                    migration_id: "MIG-ENT-003",
                    source_hash: "h",
                    from_version: 2,
                    to_version: 3,
                    applied_at: 5,
                    entities_touched: 4,
                    notes: None,
                },
            )
            .unwrap();
        assert!(store.has_migration("MIG-ENT-003", "h").unwrap());
        assert!(!store.has_migration("MIG-ENT-003", "other").unwrap());
        store.set_meta(conn, "k", "v").unwrap();
        assert_eq!(store.get_meta("k").unwrap(), Some("v".to_string()));
    }

    #[test]
    fn corrupt_database_is_a_structured_sqlite_error() {
        // R4-27: a bad database is reported with its stable code, not swallowed.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guiNDB.db");
        std::fs::write(&path, b"this is not a sqlite database").unwrap();
        let err = match Store::create(&path) {
            Ok(_) => panic!("corrupt database must not open"),
            Err(error) => error,
        };
        assert_eq!(err.code(), crate::error::codes::SQLITE);
        assert!(err.to_string().contains("E_PERSIST_SQLITE"));
    }

    #[test]
    fn unopenable_database_path_is_a_structured_error() {
        // A directory where the database file is expected cannot be opened.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guiNDB.db");
        std::fs::create_dir(&path).unwrap();
        let err = match Store::create(&path) {
            Ok(_) => panic!("a directory cannot open as a database"),
            Err(error) => error,
        };
        assert!(matches!(
            err,
            PersistenceError::Sqlite(_) | PersistenceError::Io(_)
        ));
    }

    #[test]
    fn write_lock_conflict_is_reported_not_lost() {
        // R4-27: a lock conflict surfaces as a structured Sqlite error and the
        // existing database stays intact.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locked.db");
        let holder = Store::create(&path).unwrap();
        let writer = Store::create(&path).unwrap();
        writer
            .connection()
            .busy_timeout(std::time::Duration::from_millis(25))
            .unwrap();
        holder
            .connection()
            .execute_batch("BEGIN IMMEDIATE")
            .unwrap();
        let mut row = RawRow::new("SubItem");
        row.set("Id", json!("s1"));
        let err = writer.upsert_row(writer.connection(), &row).unwrap_err();
        assert!(matches!(err, PersistenceError::Sqlite(_)), "{err}");
        // Releasing the lock lets the same write succeed: not a lost database.
        holder.connection().execute_batch("ROLLBACK").unwrap();
        assert!(writer.upsert_row(writer.connection(), &row).is_ok());
    }
}
