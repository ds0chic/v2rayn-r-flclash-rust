//! SQLite persistence, upstream import and versioned migration (T04).
//!
//! The crate is the single writer for the application database (plan §04) and
//! the only place that understands the upstream `guiNDB.db` / `guiNConfig.json`
//! shapes. Highlights:
//!
//! - [`schema`] recreates the eight upstream tables in sqlite-net shape;
//! - [`rows`] maps rows by column name and preserves unknown columns;
//! - [`upstream_db`] identifies a source and snapshots it via the SQLite backup
//!   API (WAL included);
//! - [`migrate`] ports the upstream V2 -> V3 -> V4 migrations;
//! - [`candidate`] implements the six-step import with an all-or-nothing commit;
//! - [`references`] resolves the six upstream reference semantics;
//! - [`validate`] enforces fatal integrity checks and non-fatal warnings;
//! - [`backup`] writes/verifies/restores local backup bundles and recognises
//!   upstream `guiConfigs/` archives.
//!
//! `DomainError`-sized error contracts are intentional throughout the project,
//! so the large-`Err` lint is allowed here.
#![allow(clippy::result_large_err)]

pub mod backup;
pub mod batch;
pub mod blobs;
pub mod candidate;
pub mod error;
pub mod hash;
pub mod mapping;
pub mod migrate;
pub mod references;
pub mod report;
pub mod rows;
pub mod schema;
pub mod store;
pub mod upstream_config;
pub mod upstream_db;
pub mod validate;

pub use batch::{fingerprint, ImportBatch, SourceIdentity};
pub use blobs::{ProtocolExtraBlob, TransportExtraBlob};
pub use candidate::{commit_candidate, import_from_path, ImportFault, ImportOptions};
pub use error::{PersistenceError, Result};
pub use mapping::{parse_rules, serialize_rules, ProfileExRow, ProfileGroupRow, RulesItemStorage};
pub use migrate::{
    run_migrations, HysteriaMigrationInput, MigrationFailure, MigrationStats,
    CONFIG_VERSION_LEGACY, CONFIG_VERSION_PROTO, CONFIG_VERSION_TRANSPORT,
};
pub use references::{
    resolve_child_items, resolve_remarks, resolve_sub_child_items, ChildResolution,
    RemarksResolution, SubChildResolution,
};
pub use report::{EntityCount, ImportReport, ImportStatus, MigrationReport, ReportIssue};
pub use rows::RawRow;
pub use schema::{SqlType, Table, UPSTREAM_TABLES};
pub use store::{MigrationLog, Store};
pub use upstream_config::{parse_config_text, ConfigDocument, ConfigStorage, FieldState};
pub use upstream_db::{
    identify, identify_archive, identify_directory, snapshot, SourceKind, UpstreamSnapshot,
    UpstreamSource,
};
pub use validate::{validate_candidate, CandidateView, ValidationOutcome};

/// Upstream baseline this storage layer was derived from.
pub const SOURCE_COMMIT: &str = "7d6a967c18c697f28dc6917122ed3a4993fcf336";
/// Upstream release version of the frozen baseline.
pub const UPSTREAM_VERSION: &str = "7.25.4";
