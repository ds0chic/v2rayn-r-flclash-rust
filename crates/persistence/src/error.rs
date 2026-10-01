//! Persistence error model. Mirrors the stable-code convention of
//! [`domain::error`] so the application/bridge layer can map storage failures
//! to user-facing message keys without leaking raw SQL/text.

use std::fmt;

/// Stable machine codes for persistence failures.
pub mod codes {
    pub const IO: &str = "E_PERSIST_IO";
    pub const SQLITE: &str = "E_PERSIST_SQLITE";
    pub const JSON: &str = "E_PERSIST_JSON";
    pub const ARCHIVE: &str = "E_PERSIST_ARCHIVE";
    pub const NOT_A_SOURCE: &str = "E_PERSIST_NOT_A_SOURCE";
    pub const CORRUPT: &str = "E_PERSIST_CORRUPT";
    pub const PATH_REJECTED: &str = "E_PERSIST_PATH_REJECTED";
    pub const VALIDATION: &str = "E_PERSIST_VALIDATION";
    pub const INTERNAL: &str = "E_PERSIST_INTERNAL";
}

/// One persistence failure.
#[derive(Debug)]
pub enum PersistenceError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    Zip(zip::result::ZipError),
    /// The directory/archive is not a recognizable upstream source.
    NotASource(String),
    /// A source file exists but cannot be parsed/opened safely.
    Corrupt(String),
    /// A supplied path escaped the allowed root (plan §15 path bounds).
    PathRejected(String),
    /// Candidate validation failed; the report carries the details.
    Validation(String),
    Internal(String),
}

impl PersistenceError {
    pub const fn code(&self) -> &'static str {
        match self {
            PersistenceError::Io(_) => codes::IO,
            PersistenceError::Sqlite(_) => codes::SQLITE,
            PersistenceError::Json(_) => codes::JSON,
            PersistenceError::Zip(_) => codes::ARCHIVE,
            PersistenceError::NotASource(_) => codes::NOT_A_SOURCE,
            PersistenceError::Corrupt(_) => codes::CORRUPT,
            PersistenceError::PathRejected(_) => codes::PATH_REJECTED,
            PersistenceError::Validation(_) => codes::VALIDATION,
            PersistenceError::Internal(_) => codes::INTERNAL,
        }
    }

    /// User-facing message key resolved by the UI layer.
    pub const fn message_key(&self) -> &'static str {
        match self {
            PersistenceError::Io(_) => "error.persist_io",
            PersistenceError::Sqlite(_) => "error.persist_sqlite",
            PersistenceError::Json(_) => "error.persist_json",
            PersistenceError::Zip(_) => "error.persist_archive",
            PersistenceError::NotASource(_) => "error.persist_not_a_source",
            PersistenceError::Corrupt(_) => "error.persist_corrupt",
            PersistenceError::PathRejected(_) => "error.persist_path_rejected",
            PersistenceError::Validation(_) => "error.persist_validation",
            PersistenceError::Internal(_) => "error.persist_internal",
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        PersistenceError::Internal(message.into())
    }

    pub fn corrupt(message: impl Into<String>) -> Self {
        PersistenceError::Corrupt(message.into())
    }
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())?;
        let detail = match self {
            PersistenceError::Io(e) => e.to_string(),
            PersistenceError::Sqlite(e) => e.to_string(),
            PersistenceError::Json(e) => e.to_string(),
            PersistenceError::Zip(e) => e.to_string(),
            PersistenceError::NotASource(s)
            | PersistenceError::Corrupt(s)
            | PersistenceError::PathRejected(s)
            | PersistenceError::Validation(s)
            | PersistenceError::Internal(s) => s.clone(),
        };
        if !detail.is_empty() {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PersistenceError::Io(e) => Some(e),
            PersistenceError::Sqlite(e) => Some(e),
            PersistenceError::Json(e) => Some(e),
            PersistenceError::Zip(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PersistenceError {
    fn from(value: std::io::Error) -> Self {
        PersistenceError::Io(value)
    }
}

impl From<rusqlite::Error> for PersistenceError {
    fn from(value: rusqlite::Error) -> Self {
        PersistenceError::Sqlite(value)
    }
}

impl From<serde_json::Error> for PersistenceError {
    fn from(value: serde_json::Error) -> Self {
        PersistenceError::Json(value)
    }
}

impl From<zip::result::ZipError> for PersistenceError {
    fn from(value: zip::result::ZipError) -> Self {
        PersistenceError::Zip(value)
    }
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, PersistenceError>;
