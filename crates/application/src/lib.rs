//! Application layer: use cases, repositories, revisions, jobs and snapshots.
//!
//! T02 provides an in-memory skeleton only. It contains no database, process
//! or network access; the runtime boundary is a trait ([`RuntimeClient`]) that
//! T03 implements, and the storage boundary is a trait
//! ([`ProfileRepository`]) that T04 implements with SQLite.

// `DomainError` is the shared error contract; boxing it would ripple through
// every signature, so the large-Err lint is allowed with intent (see `domain`).
#![allow(clippy::result_large_err)]

pub mod engine;
pub mod jobs;
pub mod net_host_client;
pub mod repository;
pub mod runtime_client;
pub mod snapshot;
pub mod synthetic;

// T01 compatibility surface.
pub use synthetic::{
    blocking_probe, generate_profiles_page, generate_synthetic_profiles, ping, synthetic_profile,
};

pub use engine::{capability_table, empty_snapshot, AppEngine};
pub use jobs::{JobManager, JobView};
pub use net_host_client::NetHostClient;
pub use repository::{
    InMemoryProfileRepository, PageRequest, ProfileFilter, ProfilePage, ProfileRepository,
    ProfileSort, RevisionStore,
};
pub use runtime_client::{
    ApplyOutcome, EventSink, NullRuntimeClient, RuntimeClient, RuntimeSnapshot,
};
pub use snapshot::{assemble, CapabilityEntry, Snapshot, StartupRecovery};
