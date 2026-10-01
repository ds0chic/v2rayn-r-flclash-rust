//! Application layer: use cases, repositories, revisions, jobs and snapshots.
//!
//! T02 provides an in-memory skeleton only. It contains no database, process
//! or network access; the runtime boundary is a trait ([`RuntimeClient`]) that
//! T03 implements, and the storage boundary is a trait
//! ([`ProfileRepository`]) that T04 implements with SQLite.

// `DomainError` is the shared error contract; boxing it would ripple through
// every signature, so the large-Err lint is allowed with intent (see `domain`).
#![allow(clippy::result_large_err)]

pub mod backup_service;
pub mod codegen;
pub mod custom;
pub mod dns;
pub mod engine;
pub mod groups;
pub mod jobs;
pub mod mixin;
pub mod monitor;
pub mod net_host_client;
pub mod platform_service;
pub mod repository;
pub mod routing;
pub mod runtime_client;
pub mod settings;
pub mod snapshot;
pub mod speedtest;
pub mod store_repo;
pub mod subs;
pub mod synthetic;
pub mod templates;
pub mod update_service;
pub mod webdav;

// T01 compatibility surface.
pub use synthetic::{
    blocking_probe, generate_profiles_page, generate_synthetic_profiles, ping, synthetic_profile,
};

pub use backup_service::{
    extract_bundle_zip, persist_error, zip_bundle, BackupService, LocalBackup, APP_SOURCE_COMMIT,
};
pub use dns::{new_dns_id, DnsRepository, InMemoryDnsRepository, RegionalPreset};
pub use domain::CancellationToken;
pub use engine::{capability_table, empty_snapshot, AppEngine};
pub use jobs::{JobManager, JobView};
pub use monitor::{
    epoch_day, BucketTotals, ClashApiService, InMemoryTrafficStore, LogEntry, LogPage, LogService,
    StatsService, StatsUpdate, TrafficStore, DEFAULT_MAX_LOG_BYTES, DEFAULT_MAX_LOG_LINES,
    DELAY_TEST_URL,
};
pub use net_host_client::NetHostClient;
pub use persistence::backup::BackupManifest;
pub use persistence::ImportStatus;
pub use platform_service::{
    derived_local_port, mode_from_domain, mode_from_value, mode_value, Ownership, PacHandle,
    PlatformService, ProxyApplyOutcome, ProxyApplyRequest, ProxyRestoreOutcome, ProxyStateView,
    INBOUND_PROTOCOL_OFFSETS,
};
pub use repository::{
    new_index_id, InMemoryProfileRepository, InMemorySubRepository, PageRequest, ProfileFilter,
    ProfilePage, ProfileRepository, ProfileSort, RevisionStore, SubRepository,
};
pub use routing::{new_routing_id, InMemoryRoutingRepository, RoutingRepository};
pub use runtime_client::{
    ApplyOutcome, EventSink, NullRuntimeClient, RuntimeClient, RuntimeSnapshot,
};
pub use settings::{
    apply_group_patch, normalize_for_save, validate_settings, LoadedSettings, SaveSettingsOutcome,
    SettingsState,
};
pub use snapshot::{assemble, CapabilityEntry, Snapshot, StartupRecovery};
pub use speedtest::{
    find_free_test_port, http_get_via_socks, tcping, DownloadOutcome, ProfileExItem,
    ProfileExStore, SpeedTestJobs, SpeedTestOutcome, SpeedTestResult, SpeedTestRunner,
    SpeedTestSession, SpeedTestSettings, SpeedTestSnapshot, StopReason, TestNode, TestSession,
    UnsupportedSession, BATCH_FLUSH_MS, MIN_MIXED_CONCURRENCY, MIN_SPEEDTEST_TIMEOUT_SECS,
    TEST_PORT_FLOOR,
};
pub use store_repo::{
    DnsStore, ProfileStore, RoutingStore, SqliteProfileRepository, SqliteSubRepository, SubStore,
};
pub use subs::{
    build_candidates, download_all, is_due, merge_options, new_sub_id, parse_request_headers,
    parse_subscription, report_to_json, sub_error_outcome, unix_now, SubItem, SubScheduler,
    SubUpdateEntry, SubUpdateOutcome, SubUpdateReport, SubUpdateRequest,
};
pub use update_service::{
    builtin_targets, cleanup_logs_tmp, parse_dgst_sha256, update_error, CleanupReport,
    CoreApplyOutcome, CoreApplyRequest, CoreUpdateCheck, InstalledCore, UpdateService,
    UpdateTargetInfo, BUILTIN_TARGETS, UPDATE_TIMEOUT,
};
pub use webdav::{WebDavCheck, WebDavClient, WebDavConfig, WebDavEntry, BACKUP_FILE, DEFAULT_DIR};
