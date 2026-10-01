//! Dynamic row mapping.
//!
//! Upstream rows are read with `SELECT *` and mapped **by column name**, never
//! by index. Columns the model does not know about are kept verbatim in
//! [`RawRow::values`], and missing columns simply yield `None`; a candidate
//! import therefore survives upstream adding or dropping columns.

use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde_json::{Map, Number, Value};

use crate::error::Result;

/// One source row: table name plus a column-name -> JSON value map.
#[derive(Debug, Clone, PartialEq)]
pub struct RawRow {
    pub table: String,
    pub values: Map<String, Value>,
}

impl RawRow {
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            values: Map::new(),
        }
    }

    pub fn get(&self, column: &str) -> Option<&Value> {
        self.values.get(column)
    }

    /// Raw string form of a column; numbers/bools are stringified so legacy
    /// columns that changed type still import.
    pub fn string(&self, column: &str) -> String {
        self.opt_string(column).unwrap_or_default()
    }

    pub fn opt_string(&self, column: &str) -> Option<String> {
        match self.values.get(column) {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Bool(b)) => Some(b.to_string()),
            Some(Value::Number(n)) => Some(n.to_string()),
            Some(other) => Some(other.to_string()),
        }
    }

    pub fn i64(&self, column: &str) -> i64 {
        self.opt_i64(column).unwrap_or(0)
    }

    pub fn opt_i64(&self, column: &str) -> Option<i64> {
        match self.values.get(column) {
            Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
            Some(Value::Bool(b)) => Some(i64::from(*b)),
            _ => None,
        }
    }

    pub fn f64(&self, column: &str) -> f64 {
        match self.values.get(column) {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
            Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    pub fn opt_f64(&self, column: &str) -> Option<f64> {
        match self.values.get(column) {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    pub fn bool(&self, column: &str) -> bool {
        match self.values.get(column) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
            Some(Value::String(s)) => {
                matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
            }
            _ => false,
        }
    }

    pub fn set(&mut self, column: &str, value: Value) {
        self.values.insert(column.to_string(), value);
    }

    /// Serialize the row to a JSON object for raw retention.
    pub fn to_json(&self) -> Value {
        Value::Object(self.values.clone())
    }
}

/// Convert a borrowed SQLite value into an owned JSON value. Blobs (which the
/// upstream model does not use) are preserved as an array of byte values rather
/// than dropped.
pub fn value_from_ref(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::Number(Number::from(i)),
        ValueRef::Real(f) => Number::from_f64(f).map_or(Value::Null, Value::Number),
        ValueRef::Text(bytes) => Value::String(String::from_utf8_lossy(bytes).into_owned()),
        ValueRef::Blob(bytes) => Value::Array(bytes.iter().map(|b| Value::from(*b)).collect()),
    }
}

/// Read every row of `table` by column name. Returns an empty vector when the
/// table is absent (older databases legitimately lack e.g. `ProfileGroupItem`).
pub fn read_table(conn: &Connection, table: &str) -> Result<Vec<RawRow>> {
    if !table_exists(conn, table)? {
        return Ok(Vec::new());
    }
    let sql = format!("SELECT * FROM \"{}\"", table.replace('"', ""));
    let mut stmt = conn.prepare(&sql)?;
    let names: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut values = Map::new();
        for (index, name) in names.iter().enumerate() {
            values.insert(name.clone(), value_from_ref(row.get_ref(index)?));
        }
        out.push(RawRow {
            table: table.to_string(),
            values,
        });
    }
    Ok(out)
}

/// Does a table exist in the connected database?
pub fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// List user table names in a database.
pub fn list_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;

    #[test]
    fn reads_rows_by_name_and_keeps_unknown_column() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE SubItem (Id TEXT PRIMARY KEY, Remarks TEXT, FutureCol TEXT);\
             INSERT INTO SubItem VALUES ('s1', 'hello', 'kept');",
        )
        .unwrap();
        let rows = read_table(&conn, "SubItem").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].string("Remarks"), "hello");
        assert_eq!(rows[0].string("FutureCol"), "kept");
        assert!(rows[0].get("Missing").is_none());
    }

    #[test]
    fn missing_table_is_empty_not_error() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(read_table(&conn, "ProfileGroupItem").unwrap().is_empty());
    }

    #[test]
    fn upstream_tables_create_and_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        for table in schema::UPSTREAM_TABLES {
            conn.execute_batch(&table.create_sql()).unwrap();
        }
        for table in schema::UPSTREAM_TABLES {
            assert!(table_exists(&conn, table.name).unwrap());
            let rows = read_table(&conn, table.name).unwrap();
            assert!(rows.is_empty());
        }
    }
}
