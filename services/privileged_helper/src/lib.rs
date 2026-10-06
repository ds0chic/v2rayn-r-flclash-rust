//! Privileged helper for the v2rayN Rust refactor (T14).
//!
//! The helper performs a finite, enumerated set of elevated operations:
//! add/remove routes, configure a TUN adapter address, and start/stop an
//! allow-listed core. It exposes no shell, no arbitrary program execution and
//! no operation taking an arbitrary command line (plan §5).
//!
//! All real platform code lives in [`windows`] and is compiled but never
//! executed in this repository's test environment; tests drive a
//! [`backend::FakeBackend`] over an in-memory session.

pub mod audit;
pub mod backend;
pub mod journal;
pub mod server;

#[cfg(windows)]
mod pipe_security;
#[cfg(windows)]
pub mod windows;

pub use audit::{AuditLog, AuditOutcome, AuditRecord};
pub use backend::{
    FakeBackend, FakeCall, FakeOp, HelperBackend, RouteRemovalOutcome, StartedCore, TunResetOutcome,
};
pub use journal::{
    core_label, route_label, tun_label, JournalEntry, JournalKind, JournalState, ResourceJournal,
};
pub use server::{
    serve_connection, sid_matches, ConnectionLease, HelperServer, HelperServerConfig, LeasePolicy,
    LEN_PREFIX_BYTES,
};
