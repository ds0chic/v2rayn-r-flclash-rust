//! In-memory AppEngine skeleton.
//!
//! Wires the repository, revision store, job manager and runtime client into
//! the T02 use cases the FRB layer exposes. No database, process or network
//! access: T03 replaces [`NullRuntimeClient`] with the net-host client and T04
//! replaces [`InMemoryProfileRepository`] with SQLite.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use domain::runtime_plan::{
    ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, PortRequest, PortTransport,
    ProcessGraph, ProcessNode, RequiredPrivilege, RuntimePlan, RuntimeTarget,
};
use domain::{
    job_state_from_stable_name, stable_operation_name, AppSettings, AppliedRevision, CancelOutcome,
    CancellationToken, ConfigType, CoreType, DesiredRevision, DnsProfile, DomainError,
    FrozenAppliedTarget, FullConfigTemplate, JobId, Profile, RoutingProfile, RuleMode,
    RuntimeState,
};
use serde_json::Value;

use crate::jobs::{JobManager, JobView};
use crate::net_host_client::NetHostClient;
use crate::recoverable_commit::{
    blocked_receipt, commit_id_for, committed_receipt, contract_error, document_token,
    domain_to_contract, not_started_receipt, parse_receipt, persist_to_contract, rejected_receipt,
    settings_hash_of, unknown_receipt, CommitStage, CommitTestFault, MemCommit, MemPending,
    SettingsCommitSnapshot, ValidatedSave,
};
use crate::repository::{
    InMemoryProfileRepository, InMemorySubRepository, PageRequest, ProfileFilter, ProfilePage,
    ProfileRepository, ProfileSort, RevisionStore, SubRepository,
};
use crate::runtime_client::{
    AppliedSession, ApplyOutcome, EventSink, NullRuntimeClient, OperationStatusView, RuntimeClient,
    RuntimeSnapshot,
};
use crate::selection::{pick_default, present as present_id, resolve_visible_selection};
use crate::settings::{
    apply_group_patch, normalize_for_save, validate_settings, LoadedSettings, SaveSettingsOutcome,
    SettingsState,
};
use crate::snapshot::{assemble, CapabilityEntry, Snapshot, StartupRecovery};
use persistence::Store;
use updater::download::{DownloadRequest, DownloaderOptions, FileDownloader};
use updater::UpdateError;

use crate::dns::{
    effective_geo_source, effective_srs_source, DnsRepository, InMemoryDnsRepository,
};
use crate::routing::{InMemoryRoutingRepository, RoutingRepository};
use crate::store_repo::{
    persistence_storage_error, storage_error, DnsStore, ProfileStore, RoutingStore,
    SqliteDnsRepository, SqliteProfileRepository, SqliteRoutingRepository, SqliteSubRepository,
    SqliteTrafficStore, SubStore,
};
use crate::subs::{
    build_candidates, download_all, new_sub_id, report_to_json, sub_error_outcome, unix_now,
    SubItem, SubScheduler, SubUpdateEntry, SubUpdateOutcome, SubUpdateReport, SubUpdateRequest,
};
use crate::tun_plan::{self, TunPlanHints};
use ipc_contract::stable::{DatasetEpoch, SettingsSaveReceipt};

/// Application data directory override.
pub const DATA_DIR_ENV: &str = "V2RAYN_R_DATA_DIR";

/// Process-node id for the pre-SOCKS / LegacyProtect sidecar (FIX-13).
pub const PRE_SOCKS_PROCESS_ID: &str = "pre-socks";

/// Pre-SOCKS sidecar decision (`ConfigHandler.GetPreSocksItem` parity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreSocksDecision {
    pub core: CoreType,
    pub address: String,
    pub port: u16,
}

/// Applied-session facts the monitor pipeline needs (FIX-11).
///
/// These runtime facts (running core, statistics/API ports, applied node) are
/// published only while a managed core is actually `Running`; the desired
/// active node is never reported as a running session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorSession {
    pub core: CoreType,
    pub active_index_id: Option<String>,
    pub proxy_port: Option<u16>,
    pub state_port: u16,
    pub state_port2: u16,
    /// Clash `secret` from a full Custom config, when the config declares one.
    /// Data only: never written to logs or evidence.
    pub api_secret: Option<String>,
}

/// Core + statistics/API ports and proxy scheme captured when an apply was
/// accepted. For a full Custom config the values come from the emitted JSON
/// (RR-07 / R3-07); otherwise they are the generated-config expectations.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedFacts {
    core: CoreType,
    state_port: u16,
    state_port2: u16,
    scheme: runtime::ProxyProtocol,
    /// API kind (`ClashApi` / `XrayStats`) when a Custom config declared one.
    api_kind: Option<runtime::ApiKind>,
    /// Clash `secret`; data only, never logged.
    api_secret: Option<String>,
}

/// The managed cores root for a given data directory: `V2RAYN_R_CORES_DIR`
/// overrides, otherwise `<data_dir>/cores`, falling back to the default data
/// directory. Shared by install (bridge `cores_root`) and run (NetHostClient
/// forwards it to net-host as `V2RAYN_R_CORES_ROOT`).
pub fn managed_cores_root(data_dir: Option<&Path>) -> PathBuf {
    if let Ok(dir) = std::env::var("V2RAYN_R_CORES_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    match data_dir {
        Some(dir) => dir.join("cores"),
        None => AppEngine::default_data_dir().join("cores"),
    }
}

/// Shared engine handle. Cloning shares all state.
#[derive(Clone)]
pub struct AppEngine {
    repo: Arc<Mutex<ProfileStore>>,
    subs: Arc<Mutex<SubStore>>,
    routing: Arc<Mutex<RoutingStore>>,
    dns_items: Arc<Mutex<DnsStore>>,
    rule_mode: Arc<Mutex<RuleMode>>,
    revisions: Arc<Mutex<RevisionStore>>,
    active: Arc<Mutex<Option<String>>>,
    templates: Arc<Mutex<Vec<FullConfigTemplate>>>,
    settings: Arc<Mutex<SettingsState>>,
    data_dir: Option<PathBuf>,
    jobs: JobManager,
    runtime: Arc<dyn RuntimeClient>,
    sub_scheduler: Arc<Mutex<Option<SubScheduler>>>,
    /// Cooperative Geo/SRS resource task (R4-34). Runs on its own thread so it
    /// never blocks the subscription scheduler; started/stopped together with
    /// it, matching upstream `TaskManager`.
    resource_scheduler: Arc<Mutex<Option<ResourceScheduler>>>,
    /// Structured outcome of the most recent Geo/SRS resource pass (R4-34), so
    /// the UI can read the last result without re-running a download.
    last_resource_report: Arc<Mutex<Option<ResourceUpdateReport>>>,
    /// The local socks/mixed port of the running session, when known.
    local_proxy_port: Arc<Mutex<Option<u16>>>,
    /// The applied-session fact (FIX-07): published only while net-host
    /// reports a `Running` session, withdrawn on stop/failure.
    applied_session: Arc<Mutex<Option<AppliedSession>>>,
    /// The active node id captured when an apply was accepted, so the applied
    /// session reports the node the running config was built for, not the
    /// current desired selection.
    ///
    /// SP-05: the whole submit-time fact, frozen **before** the runtime call
    /// (target/plan/revision/operation/intent/generation). A later default
    /// change never rewrites it; a stop withdraws the live session but keeps
    /// this history so reopen still reports what actually ran.
    applied_frozen: Arc<Mutex<Option<FrozenAppliedTarget>>>,
    /// Monotonic submit sequence (plan §3.2 `intentSeq`), allocated before
    /// the runtime call so concurrent submits order by admission.
    intent_seq: Arc<AtomicU64>,
    /// Monotonic actual generation (plan §3.1 `actualGeneration`). Bumped on
    /// every applied-fact transition (accept/publish/withdraw), even when the
    /// desired revision is unchanged.
    actual_generation: Arc<AtomicU64>,
    /// `operation_id -> job_id` correlation for one accepted apply (RUN-05):
    /// the runtime owns the operation, the job manager owns the job, and this
    /// map binds them so either id resolves to one terminal state.
    operation_jobs: Arc<Mutex<HashMap<String, JobId>>>,
    /// Core + statistics/API ports captured when an apply was accepted, so the
    /// monitor pipeline polls the running core instead of re-deriving facts
    /// from a desired (possibly changed) plan.
    apply_facts: Arc<Mutex<Option<AppliedFacts>>>,
    /// Monotonic epoch bumped before every restore/import file exchange. A
    /// subscription commit that captured the epoch before the bump is rejected
    /// so a slow download cannot write into the swapped database (R3-SET-03).
    restore_epoch: Arc<AtomicU64>,
    /// Set when production storage could not be opened. The engine then fails
    /// closed: persistence and runtime use cases return this structured error
    /// instead of silently serving an in-memory database or a fake `Accepted`
    /// (D20). Always `None` for a successfully opened or in-memory engine.
    storage_error: Arc<Mutex<Option<DomainError>>>,
    /// Fault injection for the recoverable commit path (SP-02). Production
    /// always leaves [`CommitTestFault::None`]; tests set a fault to prove
    /// each stage fails and recovers. Never read from user input.
    commit_fault: Arc<Mutex<CommitTestFault>>,
    /// Serialises recoverable commits against recovery: a save holds it
    /// briefly, recovery holds it for the whole pass. A save that cannot take
    /// it reports `RecoveryRequired` instead of queuing behind recovery.
    commit_lock: Arc<Mutex<()>>,
    /// Serialises runtime-affecting commands in this process (SP-04): apply,
    /// stop and cleanup join one authoritative sequence in admission order so
    /// two windows/isolates cannot interleave submits behind the UI queue.
    /// A stop admitted later always runs after the earlier command; queries
    /// (`snapshot`, `operation_status`) never take it, so reconciliation
    /// reads stay responsive while a command is in flight. Never held across
    /// re-entrant engine calls (the runtime client never calls back in).
    runtime_cmd_lock: Arc<Mutex<()>>,
    /// Completed mutations for engines without a data directory (memory only).
    mem_commits: Arc<Mutex<HashMap<String, MemCommit>>>,
    /// Unresolved mutation for engines without a data directory.
    mem_pending: Arc<Mutex<Option<MemPending>>>,
    /// Set while an in-memory engine has an unresolved commit.
    mem_recovery: Arc<AtomicBool>,
    /// SP-14 preview registry: token -> content digest (memory only; registering
    /// a preview never touches SQLite or受控文件).
    import_previews: Arc<Mutex<HashMap<String, String>>>,
    /// SP-14 completed import commits keyed by mutation id (idempotent replay).
    import_commits: Arc<Mutex<HashMap<String, crate::import_batch::ImportReceipt>>>,
    /// SP-14 fault injection: the next commit fails after staging (tests only).
    import_fault: Arc<AtomicBool>,
}

// ---------------------------------------------------------------------------
// R4-34: Geo/SRS resource task (upstream `TaskManager.UpdateTaskRunGeo`).
// ---------------------------------------------------------------------------

/// One remote resource file to keep up to date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRequest {
    pub url: String,
    pub target: PathBuf,
}

/// A download that did not succeed. The previous file is left untouched and no
/// success is reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceFailure {
    pub url: String,
    pub code: String,
    pub detail: String,
}

/// Outcome of one resource pass. `due == false` means the cadence did not fire.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceUpdateReport {
    pub due: bool,
    pub attempted: usize,
    pub downloaded: Vec<String>,
    pub failed: Vec<ResourceFailure>,
}

impl ResourceUpdateReport {
    /// Readable aggregate: a pass with any failure is not `ok`.
    pub fn ok(&self) -> bool {
        self.failed.is_empty()
    }
}

/// Default geosite rule-set names appended by upstream
/// `UpdateService.GetSrsFileAllRequest`.
pub const DEFAULT_SRS_GEOSITE: [&str; 4] = ["google", "cn", "geolocation-cn", "category-ads-all"];

fn srs_request(template: &str, kind: &str, name: &str, bin_dir: &Path) -> ResourceRequest {
    let file = format!("{kind}-{name}");
    ResourceRequest {
        url: template.replace("{0}", kind).replace("{1}", &file),
        target: bin_dir.join("srss").join(format!("{file}.srs")),
    }
}

/// Build the Geo `.dat` + SRS download set from the stored sources.
///
/// `GeoSourceUrl` / `SrsSourceUrl` are the effective templates (the upstream
/// built-in when unset). Targets mirror upstream `Utils.GetBinPath`:
/// `<bin>/<name>.dat` and `<bin>/srss/<type>-<name>.srs`.
pub fn build_resource_requests(
    const_item: &domain::ConstItem,
    bin_dir: &Path,
) -> Vec<ResourceRequest> {
    let geo_template = effective_geo_source(const_item);
    let srs_template = effective_srs_source(const_item);
    let mut requests = Vec::new();
    for name in ["geoip", "geosite"] {
        requests.push(ResourceRequest {
            url: geo_template.replace("{0}", name),
            target: bin_dir.join(format!("{name}.dat")),
        });
    }
    for name in DEFAULT_SRS_GEOSITE {
        requests.push(srs_request(&srs_template, "geosite", name, bin_dir));
    }
    requests
}

/// Cooperative periodic Geo resource task. Owns a thread + a current-thread
/// Tokio runtime; `stop()` wakes the loop immediately (channel), so no timer
/// survives a stop.
struct ResourceScheduler {
    stop_tx: std::sync::mpsc::Sender<()>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ResourceScheduler {
    fn start(engine: AppEngine, bin_dir: PathBuf, tick: Duration) -> Self {
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let handle = std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            let mut ticks: u64 = 0;
            loop {
                match stop_rx.recv_timeout(tick) {
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
                ticks += 1;
                // Upstream runs the Geo pass once per hour tick and only when
                // `hours % AutoUpdateInterval == 0`. The daily check-update slot
                // is registered separately; see docs/evidence/repair/R4-34.
                if ticks.is_multiple_of(60) {
                    let cancel = CancellationToken::new();
                    let _ =
                        runtime.block_on(engine.run_resource_pass(&bin_dir, ticks / 60, &cancel));
                }
            }
        });
        Self {
            stop_tx,
            handle: Some(handle),
        }
    }

    fn stop(&self) {
        let _ = self.stop_tx.send(());
    }

    #[cfg(test)]
    fn is_finished(&self) -> bool {
        self.handle
            .as_ref()
            .map(std::thread::JoinHandle::is_finished)
            .unwrap_or(true)
    }
}

impl Drop for ResourceScheduler {
    fn drop(&mut self) {
        // Wake the loop and detach rather than join: joining could delay process
        // exit on an in-flight download, which FIX-09D forbids.
        let _ = self.stop_tx.send(());
        let _ = self.handle.take();
    }
}

impl AppEngine {
    pub fn in_memory() -> Self {
        Self::with_runtime(Arc::new(NullRuntimeClient::new()))
    }

    /// Build an engine backed by the real net-host client (T03 production
    /// default). Constructing the client does not connect or spawn anything;
    /// the first runtime call does.
    pub fn production() -> Self {
        Self::with_runtime(Arc::new(NetHostClient::new()))
    }

    pub fn with_runtime(runtime: Arc<dyn RuntimeClient>) -> Self {
        let engine = Self {
            repo: Arc::new(Mutex::new(ProfileStore::Memory(
                InMemoryProfileRepository::new(),
            ))),
            subs: Arc::new(Mutex::new(SubStore::Memory(InMemorySubRepository::new()))),
            routing: Arc::new(Mutex::new(RoutingStore::Memory(
                InMemoryRoutingRepository::new(),
            ))),
            dns_items: Arc::new(Mutex::new(DnsStore::Memory(InMemoryDnsRepository::new()))),
            rule_mode: Arc::new(Mutex::new(RuleMode::Rule)),
            revisions: Arc::new(Mutex::new(RevisionStore::new())),
            active: Arc::new(Mutex::new(None)),
            templates: Arc::new(Mutex::new(crate::templates::builtins())),
            settings: Arc::new(Mutex::new(SettingsState::default())),
            data_dir: None,
            jobs: JobManager::new(),
            runtime,
            sub_scheduler: Arc::new(Mutex::new(None)),
            resource_scheduler: Arc::new(Mutex::new(None)),
            last_resource_report: Arc::new(Mutex::new(None)),
            local_proxy_port: Arc::new(Mutex::new(None)),
            applied_session: Arc::new(Mutex::new(None)),
            applied_frozen: Arc::new(Mutex::new(None)),
            intent_seq: Arc::new(AtomicU64::new(0)),
            actual_generation: Arc::new(AtomicU64::new(0)),
            operation_jobs: Arc::new(Mutex::new(HashMap::new())),
            apply_facts: Arc::new(Mutex::new(None)),
            restore_epoch: Arc::new(AtomicU64::new(0)),
            storage_error: Arc::new(Mutex::new(None)),
            commit_fault: Arc::new(Mutex::new(CommitTestFault::None)),
            commit_lock: Arc::new(Mutex::new(())),
            runtime_cmd_lock: Arc::new(Mutex::new(())),
            mem_commits: Arc::new(Mutex::new(HashMap::new())),
            mem_pending: Arc::new(Mutex::new(None)),
            mem_recovery: Arc::new(AtomicBool::new(false)),
            import_previews: Arc::new(Mutex::new(HashMap::new())),
            import_commits: Arc::new(Mutex::new(HashMap::new())),
            import_fault: Arc::new(AtomicBool::new(false)),
        };
        engine.ensure_builtin_routing_dns();
        engine
    }

    /// Open a real SQLite-backed engine rooted at `data_dir` (creating
    /// `guiNDB.db` / `guiNConfig.json` as needed). Runtime events go to the
    /// real net-host client.
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, DomainError> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let runtime: Arc<dyn RuntimeClient> = Arc::new(NetHostClient::with_cores_root(
            std::env::var("V2RAYN_R_PIPE")
                .unwrap_or_else(|_| runtime::NET_HOST_PIPE_NAME.to_string()),
            managed_cores_root(Some(&data_dir)),
        ));
        Self::open_with_runtime(data_dir, runtime)
    }

    /// Open a real SQLite-backed engine with an injected runtime client.
    pub fn open_with_runtime(
        data_dir: impl AsRef<Path>,
        runtime: Arc<dyn RuntimeClient>,
    ) -> Result<Self, DomainError> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir).map_err(storage_error)?;
        let db_path = data_dir.join("guiNDB.db");
        let sqlite = SqliteProfileRepository::open(&db_path)?;
        let config = read_config(&data_dir)?;

        let desired = config
            .get("desired_revision")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let active = canonical_active_from_config(&config);
        let dataset_epoch = dataset_epoch_from_config(&config);

        let sub_store = SqliteSubRepository::from_store(
            Store::open(&db_path).map_err(persistence_storage_error)?,
        );
        let routing_store = SqliteRoutingRepository::from_store(
            Store::open(&db_path).map_err(persistence_storage_error)?,
        );
        let dns_store = SqliteDnsRepository::from_store(
            Store::open(&db_path).map_err(persistence_storage_error)?,
        );
        let rule_mode = config
            .get("rule_mode")
            .and_then(Value::as_str)
            .map(|s| match s {
                "Global" => RuleMode::Global,
                "Direct" => RuleMode::Direct,
                _ => RuleMode::Rule,
            })
            .unwrap_or(RuleMode::Rule);

        let engine = Self {
            repo: Arc::new(Mutex::new(ProfileStore::Sqlite(sqlite))),
            subs: Arc::new(Mutex::new(SubStore::Sqlite(sub_store))),
            routing: Arc::new(Mutex::new(RoutingStore::Sqlite(routing_store))),
            dns_items: Arc::new(Mutex::new(DnsStore::Sqlite(dns_store))),
            rule_mode: Arc::new(Mutex::new(rule_mode)),
            revisions: Arc::new(Mutex::new(RevisionStore::with_desired(
                DesiredRevision::new(desired),
            ))),
            active: Arc::new(Mutex::new(active)),
            templates: Arc::new(Mutex::new(read_templates(&config))),
            settings: Arc::new(Mutex::new(read_settings_state(&config)?)),
            data_dir: Some(data_dir),
            jobs: JobManager::new(),
            runtime,
            sub_scheduler: Arc::new(Mutex::new(None)),
            resource_scheduler: Arc::new(Mutex::new(None)),
            last_resource_report: Arc::new(Mutex::new(None)),
            local_proxy_port: Arc::new(Mutex::new(None)),
            applied_session: Arc::new(Mutex::new(None)),
            applied_frozen: Arc::new(Mutex::new(None)),
            intent_seq: Arc::new(AtomicU64::new(0)),
            actual_generation: Arc::new(AtomicU64::new(0)),
            operation_jobs: Arc::new(Mutex::new(HashMap::new())),
            apply_facts: Arc::new(Mutex::new(None)),
            restore_epoch: Arc::new(AtomicU64::new(dataset_epoch)),
            storage_error: Arc::new(Mutex::new(None)),
            commit_fault: Arc::new(Mutex::new(CommitTestFault::None)),
            commit_lock: Arc::new(Mutex::new(())),
            runtime_cmd_lock: Arc::new(Mutex::new(())),
            mem_commits: Arc::new(Mutex::new(HashMap::new())),
            mem_pending: Arc::new(Mutex::new(None)),
            mem_recovery: Arc::new(AtomicBool::new(false)),
            import_previews: Arc::new(Mutex::new(HashMap::new())),
            import_commits: Arc::new(Mutex::new(HashMap::new())),
            import_fault: Arc::new(AtomicBool::new(false)),
        };
        engine.ensure_builtin_routing_dns();
        // SP-05: reload the frozen applied history and the fact counters so an
        // independent reopen reports the same applied-vs-actual truth instead
        // of inventing the desired default as the running target.
        engine.restore_applied_history(&config);
        // Heal a dangling default the same way upstream `SetDefaultServer`
        // does on list load; a no-op for fresh/consistent stores.
        let _ = engine.repair_default_selection();
        Ok(engine)
    }

    /// Build a fail-closed engine used when production storage cannot be
    /// opened (unreadable/corrupt database, read-only directory, full disk or a
    /// lock conflict). Every persistence and runtime use case returns
    /// `error` instead of silently serving an in-memory database or a fake
    /// `Accepted`; the process may retry by opening a fresh engine once the
    /// condition is repaired (R4-27 / D20).
    pub fn storage_unavailable(data_dir: Option<PathBuf>, error: DomainError) -> Self {
        let mut engine = Self::with_runtime(Arc::new(NullRuntimeClient::new()));
        engine.data_dir = data_dir;
        if let Ok(mut guard) = engine.storage_error.lock() {
            *guard = Some(error);
        }
        engine
    }

    /// The storage-open failure this engine is failing closed on, if any.
    pub fn storage_failure(&self) -> Option<DomainError> {
        self.storage_error
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Returns the storage-open failure before any persistence/runtime use
    /// case runs, so a broken store never answers as success.
    fn guard_storage(&self) -> Result<(), DomainError> {
        match self.storage_failure() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Resolve the application data directory: `V2RAYN_R_DATA_DIR` when set,
    /// otherwise `%LOCALAPPDATA%\v2rayn-r\data\` (or the platform equivalent).
    pub fn default_data_dir() -> PathBuf {
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
            if !dir.is_empty() {
                return PathBuf::from(dir);
            }
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("v2rayn-r").join("data");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("v2rayn-r")
                .join("data");
        }
        PathBuf::from("v2rayn-r-data")
    }

    /// Open the default data directory with the real runtime client.
    pub fn open_default() -> Result<Self, DomainError> {
        Self::open(Self::default_data_dir())
    }

    /// True when this engine is backed by real SQLite storage.
    pub fn is_persistent(&self) -> bool {
        self.data_dir.is_some()
    }

    /// The data directory this engine is rooted at, when persistent.
    pub fn data_dir(&self) -> Option<&Path> {
        self.data_dir.as_deref()
    }

    /// The managed cores root - the single source of truth shared by the update
    /// pipeline (install) and the runtime locator (run). `V2RAYN_R_CORES_DIR`
    /// overrides; otherwise it is `<data_dir>/cores`.
    pub fn cores_root(&self) -> PathBuf {
        managed_cores_root(self.data_dir.as_deref())
    }

    /// Drop every live SQLite handle so the data directory's files can be
    /// exchanged by a restore/replace. In-memory engines are a no-op. After
    /// this call the engine must be [`AppEngine::reopen`]ed before serving
    /// queries; otherwise it answers from the empty in-memory backends.
    pub fn quiesce(&self) -> Result<(), DomainError> {
        if self.data_dir.is_none() {
            return Ok(());
        }
        *self.repo.lock().map_err(|_| lock_error())? =
            ProfileStore::Memory(InMemoryProfileRepository::new());
        *self.subs.lock().map_err(|_| lock_error())? =
            SubStore::Memory(InMemorySubRepository::new());
        *self.routing.lock().map_err(|_| lock_error())? =
            RoutingStore::Memory(InMemoryRoutingRepository::new());
        *self.dns_items.lock().map_err(|_| lock_error())? =
            DnsStore::Memory(InMemoryDnsRepository::new());
        Ok(())
    }

    /// Reopen SQLite storage and reload every config-derived in-memory value
    /// (`settings`, active id, revisions, templates, rule mode) from
    /// `guiNConfig.json`. Used after `quiesce` + a file exchange so later
    /// saves never overwrite a restored configuration.
    pub fn reopen(&self) -> Result<(), DomainError> {
        let Some(dir) = self.data_dir.clone() else {
            return Ok(());
        };
        let db_path = dir.join("guiNDB.db");
        let repo = SqliteProfileRepository::open(&db_path)?;
        let subs = SqliteSubRepository::from_store(
            Store::open(&db_path).map_err(persistence_storage_error)?,
        );
        let routing = SqliteRoutingRepository::from_store(
            Store::open(&db_path).map_err(persistence_storage_error)?,
        );
        let dns = SqliteDnsRepository::from_store(
            Store::open(&db_path).map_err(persistence_storage_error)?,
        );
        let config = read_config(&dir)?;
        let desired = config
            .get("desired_revision")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let active = canonical_active_from_config(&config);
        let dataset_epoch = dataset_epoch_from_config(&config);
        let rule_mode = config
            .get("rule_mode")
            .and_then(Value::as_str)
            .map(|s| match s {
                "Global" => RuleMode::Global,
                "Direct" => RuleMode::Direct,
                _ => RuleMode::Rule,
            })
            .unwrap_or(RuleMode::Rule);

        *self.repo.lock().map_err(|_| lock_error())? = ProfileStore::Sqlite(repo);
        *self.subs.lock().map_err(|_| lock_error())? = SubStore::Sqlite(subs);
        *self.routing.lock().map_err(|_| lock_error())? = RoutingStore::Sqlite(routing);
        *self.dns_items.lock().map_err(|_| lock_error())? = DnsStore::Sqlite(dns);
        *self.rule_mode.lock().map_err(|_| lock_error())? = rule_mode;
        *self.revisions.lock().map_err(|_| lock_error())? =
            RevisionStore::with_desired(DesiredRevision::new(desired));
        *self.active.lock().map_err(|_| lock_error())? = active;
        *self.templates.lock().map_err(|_| lock_error())? = read_templates(&config);
        *self.settings.lock().map_err(|_| lock_error())? = read_settings_state(&config)?;
        self.restore_epoch.store(dataset_epoch, Ordering::Release);
        // SP-05: same frozen-history restore as `open_with_runtime`, so a
        // reopened engine reports the same applied-vs-actual truth.
        self.restore_applied_history(&config);
        self.ensure_builtin_routing_dns();
        // A restore/import may have left a default that no longer resolves;
        // fall back per the upstream repair rule so the reopened engine never
        // serves a dangling default as the current selection.
        self.repair_default_selection()?;
        Ok(())
    }

    /// Stop the subscription scheduler and wait (bounded) for an in-flight
    /// tick to finish, so no scheduler pass can write to the database while a
    /// restore/import exchanges the live files. `None` when no scheduler runs.
    ///
    /// A scheduler that does not finish before `timeout` returns a structured
    /// error: the restore must be blocked, never silently continue onto a
    /// database a stale tick can still write (R3-SET-03).
    pub fn stop_sub_scheduler_blocking(
        &self,
        timeout: std::time::Duration,
    ) -> Result<(), DomainError> {
        let scheduler = self
            .sub_scheduler
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(scheduler) = scheduler else {
            return Ok(());
        };
        scheduler.stop();
        let deadline = std::time::Instant::now() + timeout;
        while !scheduler.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if scheduler.is_finished() {
            Ok(())
        } else {
            Err(DomainError::new(
                domain::codes::UNAVAILABLE,
                "error.restore_scheduler_timeout",
            )
            .with_detail("subscription scheduler did not stop before the restore deadline"))
        }
    }

    /// Cancel every in-flight manual subscription update and wait (bounded) for
    /// it to reach a terminal state. Returns the number cancelled. A task that
    /// does not finish before `timeout` blocks the restore with a structured
    /// error instead of silently continuing onto a swapped database.
    pub fn cancel_and_drain_sub_tasks(
        &self,
        timeout: std::time::Duration,
    ) -> Result<usize, DomainError> {
        let active_ids: Vec<JobId> = self
            .jobs
            .active()
            .into_iter()
            .filter(|job| job.kind == "update_subscription")
            .map(|job| job.job_id)
            .collect();
        for id in &active_ids {
            let _ = self.jobs.cancel(id);
        }
        if active_ids.is_empty() {
            return Ok(0);
        }
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = self
                .jobs
                .active()
                .into_iter()
                .filter(|job| job.kind == "update_subscription")
                .count();
            if remaining == 0 {
                return Ok(active_ids.len());
            }
            if std::time::Instant::now() >= deadline {
                return Err(DomainError::new(
                    domain::codes::UNAVAILABLE,
                    "error.restore_sub_task_timeout",
                )
                .with_detail(format!(
                    "{remaining} subscription task(s) did not stop before the restore deadline"
                )));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// The current restore/import epoch. A subscription run captures this
    /// before downloading and passes it to
    /// [`Self::replace_sub_profiles_at_epoch`].
    pub fn restore_epoch(&self) -> u64 {
        self.restore_epoch.load(Ordering::Acquire)
    }

    /// Advance the restore epoch (called before every restore/import exchange).
    fn bump_restore_epoch(&self) -> u64 {
        self.restore_epoch.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// SR-03 restore lifecycle, storage half: stop the managed runtime session
    /// and the subscription scheduler, then quiesce the SQLite handles so a
    /// restore/import can exchange the live files without a competing session
    /// or timer.
    ///
    /// The restore epoch is bumped first, then the manual subscription workers
    /// are cancelled and drained and the scheduler is stopped and drained; a
    /// timeout is a structured error that aborts the restore before any file is
    /// touched. A runtime that reports a live session is stopped through the
    /// idempotent boundary and a failure is returned *before* any file is
    /// touched; an idle or unreachable runtime is left alone so a backup-only
    /// session can still restore. Quiesce failures are propagated for the same
    /// reason.
    pub fn prepare_restore(&self) -> Result<(), DomainError> {
        self.bump_restore_epoch();
        let timeout = std::time::Duration::from_millis(2000);
        self.cancel_and_drain_sub_tasks(timeout)?;
        self.stop_sub_scheduler_blocking(timeout)?;
        let live = self
            .runtime
            .snapshot()
            .map(|snapshot| !matches!(snapshot.state, RuntimeState::Stopped))
            .unwrap_or(false);
        if live {
            self.stop_runtime()?;
        }
        self.quiesce()
    }

    /// Seed profiles (test/bootstrap helper).
    pub fn seed(&self, profiles: Vec<Profile>) {
        if let Ok(mut repo) = self.repo.lock() {
            *repo = ProfileStore::Memory(InMemoryProfileRepository::with_profiles(profiles));
        }
    }

    pub fn profile_count(&self) -> u64 {
        self.repo.lock().map(|r| r.count() as u64).unwrap_or(0)
    }

    /// Current desired revision without touching the runtime.
    pub fn desired_revision(&self) -> u64 {
        self.revisions
            .lock()
            .map(|r| r.desired().get())
            .unwrap_or(0)
    }

    /// Fetch one profile by stable id.
    pub fn profile_by_id(&self, index_id: &str) -> Result<Option<Profile>, DomainError> {
        self.guard_storage()?;
        self.repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .get(index_id)
    }

    /// `query_profiles` use case.
    pub fn query_profiles(
        &self,
        filter: ProfileFilter,
        sort: ProfileSort,
        page: PageRequest,
    ) -> Result<ProfilePage, DomainError> {
        self.guard_storage()?;
        self.repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .query(&filter, sort, page)
    }

    /// Current `CoreBasicItem.DefFingerprint` from the live settings tree
    /// (frozen `AddServerCommon` reads `config.CoreBasicItem.DefFingerprint`).
    fn default_reality_fingerprint(&self) -> Option<String> {
        self.settings
            .lock()
            .map(|guard| guard.settings.core_basic_item.def_fingerprint.clone())
            .unwrap_or(None)
    }

    /// `save_profile` use case: optimistic-concurrency save.
    ///
    /// Rejects a stale `expected_revision`, validates the draft, persists it
    /// and bumps the desired revision. Group drafts (`PolicyGroup` /
    /// `ProxyChain`) are normalized and cycle-checked against the full profile
    /// set; `Custom` / `Outbound` drafts go through the AddServer2 validation.
    pub fn save_profile(
        &self,
        draft: Profile,
        expected_revision: DesiredRevision,
    ) -> Result<(Profile, DesiredRevision), DomainError> {
        self.guard_storage()?;
        // Read the Reality fallback default before taking the profile-revision
        // lock, so the settings lock is never nested inside it.
        let def_fingerprint = self.default_reality_fingerprint();
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        revisions.check(expected_revision)?;
        let mut draft = draft;
        draft.config_version = 4;
        draft.validate()?;
        if crate::groups::is_group(draft.config_type) {
            draft = crate::groups::normalize_group(draft);
            let mut all = self.all_profiles_map_locked()?;
            all.insert(draft.index_id.clone(), draft.clone());
            crate::groups::validate_group(&draft, &all)?;
        } else if matches!(draft.config_type, ConfigType::Custom | ConfigType::Outbound) {
            draft = crate::custom::normalize_custom(draft);
            crate::custom::validate_custom(&draft)?;
        } else {
            // Ordinary protocols normalize like the frozen `Add*Server`
            // entry points; the UI defaults do not own the persisted shape
            // (RE-PROF-07). The Reality fingerprint is then frozen to the
            // current `CoreBasicItem.DefFingerprint` (R3-PROF-11).
            draft = crate::custom::normalize_server(draft);
            draft =
                crate::custom::apply_reality_fingerprint_default(draft, def_fingerprint.as_deref());
            crate::custom::validate_server(&draft)?;
        }

        let mut repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        repo.upsert(draft.clone())?;
        let new_revision = revisions.bump();
        self.persist_config(&revisions)?;
        Ok((draft, new_revision))
    }

    /// `save_imported_profile` use case (FIX-04): persist one node arriving
    /// from an import pipeline (inner `v2rayn://`, share URIs, subscriptions).
    ///
    /// Unlike [`AppEngine::save_profile`] (the editor-draft contract), empty
    /// remarks/address/port are accepted: upstream `AddBatchServers4InnerUri`
    /// and the per-type `Add*Server` entry points never required them.
    /// Group drafts still normalize and cycle-check; `Custom`/`Outbound`
    /// drafts normalize without the editor's file-path requirement (file
    /// materialization happens at apply time).
    pub fn save_imported_profile(
        &self,
        draft: Profile,
        expected_revision: DesiredRevision,
    ) -> Result<(Profile, DesiredRevision), DomainError> {
        self.guard_storage()?;
        // See `save_profile`: read the Reality fallback default before locking
        // the profile revisions.
        let def_fingerprint = self.default_reality_fingerprint();
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        revisions.check(expected_revision)?;
        let mut draft = draft;
        if draft.index_id.trim().is_empty() {
            draft.index_id = crate::repository::new_index_id();
        }
        draft.config_version = 4;
        if crate::groups::is_group(draft.config_type) {
            draft = crate::groups::normalize_group(draft);
            let mut all = self.all_profiles_map_locked()?;
            all.insert(draft.index_id.clone(), draft.clone());
            crate::groups::validate_group(&draft, &all)?;
        } else if matches!(draft.config_type, ConfigType::Custom | ConfigType::Outbound) {
            draft = crate::custom::normalize_custom(draft);
        } else {
            // Import/subscription entries normalize the same way as the
            // editor save, independent of any UI defaults (RE-PROF-07). The
            // tolerant-import contract (FIX-04/PR-09) is preserved: a node
            // with an empty credential is still persisted (honestly flagged
            // invalid) instead of silently dropped, so no credential gate runs
            // on this path. The Reality fingerprint freeze (R3-PROF-11) still
            // applies.
            draft = crate::custom::normalize_server(draft);
            draft =
                crate::custom::apply_reality_fingerprint_default(draft, def_fingerprint.as_deref());
        }

        let mut repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        repo.upsert(draft.clone())?;
        let new_revision = revisions.bump();
        self.persist_config(&revisions)?;
        Ok((draft, new_revision))
    }

    /// Delete a set of profiles. Returns how many rows were removed.
    pub fn delete_profiles(&self, ids: &[String]) -> Result<u64, DomainError> {
        self.guard_storage()?;
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut removed = 0u64;
        for id in ids {
            if repo.remove(id)? {
                removed += 1;
            }
        }
        if removed > 0 {
            revisions.bump();
            self.persist_config(&revisions)?;
        }
        Ok(removed)
    }

    /// Load all persisted `ProfileExItem` rows (test results + manual `Sort`).
    ///
    /// Returns an empty vec for the in-memory backend (no persistence). Used at
    /// startup so a reopened process sees the previous run's delay/speed/sort.
    pub fn profile_ex_all(&self) -> Result<Vec<crate::speedtest::ProfileExItem>, DomainError> {
        self.guard_storage()?;
        let repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        match &*repo {
            ProfileStore::Memory(_) => Ok(Vec::new()),
            ProfileStore::Sqlite(sqlite) => {
                let rows = sqlite
                    .store()
                    .read_rows("ProfileExItem")
                    .map_err(persistence_storage_error)?;
                Ok(rows
                    .iter()
                    .map(|row| {
                        let mapped = persistence::mapping::ProfileExRow::from_raw(row);
                        crate::speedtest::ProfileExItem {
                            index_id: mapped.index_id,
                            delay: mapped.delay,
                            speed: mapped.speed,
                            sort: mapped.sort,
                            message: mapped.message.unwrap_or_default(),
                            ip_info: mapped.ip_info.unwrap_or_default(),
                        }
                    })
                    .collect())
            }
        }
    }

    /// Upsert `ProfileExItem` rows into SQLite (no-op for in-memory).
    pub fn profile_ex_flush(
        &self,
        rows: &[crate::speedtest::ProfileExItem],
    ) -> Result<(), DomainError> {
        self.guard_storage()?;
        let repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let ProfileStore::Sqlite(sqlite) = &*repo else {
            return Ok(());
        };
        let conn = sqlite.store().connection();
        for row in rows {
            let mut raw = persistence::rows::RawRow::new("ProfileExItem");
            raw.set("IndexId", serde_json::json!(row.index_id));
            raw.set("Delay", serde_json::json!(row.delay));
            raw.set("Speed", serde_json::json!(row.speed));
            raw.set("Sort", serde_json::json!(row.sort));
            raw.set("Message", serde_json::json!(row.message));
            raw.set("IpInfo", serde_json::json!(row.ip_info));
            sqlite
                .store()
                .upsert_row(conn, &raw)
                .map_err(persistence_storage_error)?;
        }
        Ok(())
    }

    /// `ConfigHandler.RemoveInvalidServerResult`: delete every non-complex
    /// profile of `subid` whose persisted `ProfileExItem.Delay == -1`, then
    /// remove the corresponding result rows. Returns the deleted profile count.
    pub fn remove_invalid_profiles(&self, subid: &str) -> Result<u64, DomainError> {
        let failed: std::collections::HashSet<String> = self
            .profile_ex_all()?
            .into_iter()
            .filter(|r| r.delay == -1)
            .map(|r| r.index_id)
            .collect();
        let page = self.query_profiles(
            ProfileFilter {
                subid: Some(subid.to_string()),
                ..Default::default()
            },
            ProfileSort::IndexId,
            PageRequest {
                cursor: 0,
                page_size: u32::MAX,
            },
        )?;
        let targets: Vec<String> = page
            .items
            .iter()
            .filter(|p| !p.config_type.is_complex())
            .filter(|p| failed.contains(&p.index_id))
            .map(|p| p.index_id.clone())
            .collect();
        if targets.is_empty() {
            return Ok(0);
        }
        self.delete_profiles(&targets)
    }

    /// `ConfigHandler.DedupServerList`: collapse transport-identical,
    /// non-complex profiles of `subid`. Returns `(total, kept, removed_ids)`.
    pub fn deduplicate_profiles(
        &self,
        subid: &str,
        keep_older: bool,
    ) -> Result<(usize, usize, Vec<String>), DomainError> {
        let page = self.query_profiles(
            ProfileFilter {
                subid: Some(subid.to_string()),
                ..Default::default()
            },
            ProfileSort::IndexId,
            PageRequest {
                cursor: 0,
                page_size: u32::MAX,
            },
        )?;
        let (kept, removed_ids) = crate::speedtest::deduplicate_profiles(&page.items, keep_older);
        Ok((page.items.len(), kept, removed_ids))
    }

    /// `ConfigHandler.MoveServer`/`SortServers`: rewrite the manual `Sort`
    /// field for `ordered_ids` (`(i + 1) * 10`) and persist it. Returns whether
    /// any row was written.
    pub fn set_profile_sort(&self, ordered_ids: &[String]) -> Result<(), DomainError> {
        if ordered_ids.is_empty() {
            return Ok(());
        }
        let mut store = crate::speedtest::ProfileExStore::new();
        store.replace_all(self.profile_ex_all()?);
        store.apply_order(ordered_ids);
        self.profile_ex_flush(&store.all())
    }

    /// `AddCustomServer`/`AddCustomOutboundServer` browse step: copy a
    /// user-selected config file into `<data>/config/` under a fresh name and
    /// return that stored name for `Profile.address`. Existing destination
    /// files are never overwritten.
    pub fn import_custom_file(&self, source: &std::path::Path) -> Result<String, DomainError> {
        self.guard_storage()?;
        if !source.is_file() {
            return Err(DomainError::not_found(
                "custom_file",
                &source.display().to_string(),
            ));
        }
        let data_dir = self.data_dir().ok_or_else(|| {
            DomainError::new(domain::codes::INTERNAL, "error.custom_file_no_data_dir")
        })?;
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| !e.is_empty());
        let new_name = match ext {
            Some(ext) => format!("{}.{ext}", crate::repository::new_index_id()),
            None => crate::repository::new_index_id(),
        };
        let dir = data_dir.join("config");
        std::fs::create_dir_all(&dir)
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.custom_file_copy"))?;
        let dest = dir.join(&new_name);
        std::fs::copy(source, &dest)
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.custom_file_copy"))?;
        Ok(new_name)
    }

    /// Resolve a custom/outbound profile's config text for codegen (RT-08):
    /// inline `customConfigText` first, then a file referenced by `address`
    /// (absolute path, or a stored name under `<data>/config/`).
    fn custom_file_text(&self, profile: &Profile) -> Option<String> {
        let address = profile.address.trim();
        if address.is_empty() {
            return None;
        }
        let path = std::path::Path::new(address);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.data_dir()?.join("config").join(path)
        };
        std::fs::read_to_string(resolved).ok()
    }

    /// Copy a set of profiles, assigning fresh stable ids and a "(副本)" suffix.
    pub fn copy_profiles(&self, ids: &[String]) -> Result<Vec<Profile>, DomainError> {
        self.guard_storage()?;
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut copies = Vec::new();
        for id in ids {
            if let Some(source) = repo.get(id)? {
                let mut copy = source;
                copy.index_id = crate::repository::new_index_id();
                if !copy.remarks.is_empty() {
                    copy.remarks = format!("{} (副本)", copy.remarks);
                }
                repo.upsert(copy.clone())?;
                copies.push(copy);
            }
        }
        if !copies.is_empty() {
            revisions.bump();
            self.persist_config(&revisions)?;
        }
        Ok(copies)
    }

    /// Update only the remarks of one profile.
    pub fn set_remarks(&self, id: &str, remarks: String) -> Result<Profile, DomainError> {
        self.guard_storage()?;
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut profile = repo
            .get(id)?
            .ok_or_else(|| DomainError::not_found("profile", id))?;
        if remarks.trim().is_empty() {
            return Err(
                DomainError::new(domain::codes::FIELD_REQUIRED, "error.remarks_required")
                    .with_field("remarks"),
            );
        }
        profile.remarks = remarks;
        repo.upsert(profile.clone())?;
        revisions.bump();
        self.persist_config(&revisions)?;
        Ok(profile)
    }

    /// Mark one profile as the active node (persisted across restarts).
    ///
    /// Upstream `ConfigHandler.SetDefaultServer` is idempotent: re-selecting the
    /// current default changes nothing and must not advance the revision.
    /// Switching to a different node advances the desired revision so the UI can
    /// show "saved, not applied" even when apply later fails (R4-02 / D27).
    /// A persist failure rolls the in-memory active id and revision back, so a
    /// broken store never leaves a fake active node behind.
    ///
    /// SP-03: the canonical `IndexId` and the engine mirror `active_index_id`
    /// are always written together (§5.2), so backups carry the active node
    /// and a restore never drifts back to a stale default.
    pub fn set_active(&self, id: Option<String>) -> Result<(), DomainError> {
        self.guard_storage()?;
        if let Some(target) = &id {
            let repo = self
                .repo
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            if repo.get(target)?.is_none() {
                return Err(DomainError::not_found("profile", target));
            }
        }
        let previous_active = self
            .active
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .clone();
        if previous_active == id {
            // Idempotent for the revision, but still heal a drifted canonical
            // `IndexId` left by pre-SP-03 writes (no desired bump for a heal).
            let canonical = self
                .settings
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
                .settings
                .index_id
                .clone();
            if canonical == id {
                return Ok(());
            }
            return self.write_identity_unified(id, false);
        }
        self.write_identity_unified(id, true)
    }

    /// Write the default identity to both the canonical `IndexId` and the
    /// engine mirror `active_index_id` in one persisted step. `bump_desired`
    /// marks an explicit user default change; healing writes (open/reopen
    /// repair, drift unification) do not advance the revision.
    ///
    /// Lock discipline: every lock is taken and released in turn, never nested
    /// across [`Self::persist_config_standalone`], matching the existing
    /// save paths. A persist failure rolls all three values back.
    fn write_identity_unified(
        &self,
        id: Option<String>,
        bump_desired: bool,
    ) -> Result<(), DomainError> {
        let lock_error = || DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned");
        let previous_active = self.active.lock().map_err(|_| lock_error())?.clone();
        let previous_index = self
            .settings
            .lock()
            .map_err(|_| lock_error())?
            .settings
            .index_id
            .clone();
        let previous_desired = self.revisions.lock().map_err(|_| lock_error())?.desired();
        if let Ok(mut guard) = self.settings.lock() {
            guard.settings.index_id = id.clone();
        }
        if let Ok(mut guard) = self.active.lock() {
            *guard = id.clone();
        }
        if bump_desired {
            if let Ok(mut revisions) = self.revisions.lock() {
                revisions.bump();
            }
        }
        if let Err(error) = self.persist_config_standalone() {
            if let Ok(mut guard) = self.settings.lock() {
                guard.settings.index_id = previous_index;
            }
            if let Ok(mut guard) = self.active.lock() {
                *guard = previous_active;
            }
            if let Ok(mut revisions) = self.revisions.lock() {
                *revisions = crate::repository::RevisionStore::with_desired(previous_desired);
            }
            return Err(error);
        }
        Ok(())
    }

    /// Repair a dangling persisted default after open/reopen, following
    /// upstream `ConfigHandler.SetDefaultServer`: keep the mirror when it
    /// resolves, else the canonical `IndexId` when it resolves, else the
    /// first row with a real port, persisted as the new unified default.
    /// Nothing persisted means nothing repaired (a fresh store opens without
    /// touching the file — T18 no-drift), and with no rows the config is left
    /// untouched (upstream also saves nothing then). Returns the resolved
    /// default. Healing writes never bump the desired revision.
    pub fn repair_default_selection(&self) -> Result<Option<String>, DomainError> {
        self.guard_storage()?;
        if self.data_dir.is_none() {
            return Ok(self.active_profile());
        }
        let mirror = self.active_profile();
        let canonical = self
            .settings
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .settings
            .index_id
            .clone();
        let page = self.query_profiles(
            ProfileFilter::default(),
            ProfileSort::IndexId,
            PageRequest {
                cursor: 0,
                page_size: u32::MAX,
            },
        )?;
        if page.items.is_empty() {
            return Ok(mirror);
        }
        let exists = |id: &str| page.items.iter().any(|p| p.index_id == id);
        let first_with_port = page
            .items
            .iter()
            .find(|p| p.port > 0)
            .map(|p| p.index_id.clone());
        // Only a persisted-but-unresolvable default is repaired; a store that
        // never named a default keeps none until the user chooses one (reopen
        // must not invent file content — T18 no-drift).
        let persisted = present_id(mirror.as_deref()).or(present_id(canonical.as_deref()));
        let resolved = match persisted {
            None => None,
            Some(_) => pick_default(
                mirror.as_deref(),
                canonical.as_deref(),
                &exists,
                first_with_port,
            ),
        };
        if resolved == mirror && canonical == mirror {
            return Ok(mirror);
        }
        if resolved.is_none() {
            return Ok(mirror);
        }
        self.write_identity_unified(resolved.clone(), false)?;
        Ok(resolved)
    }

    /// Visible-table selection for a list load: in-memory pending id, then the
    /// persisted default, then the first visible row (upstream
    /// `RefreshServersBiz` 366–375). Pure display resolution: it never writes
    /// the default, so a temporary selection of C leaves B persisted.
    pub fn resolve_startup_selection(&self, pending: Option<&str>) -> Option<String> {
        let visible: Vec<String> = self
            .query_profiles(
                ProfileFilter::default(),
                ProfileSort::IndexId,
                PageRequest {
                    cursor: 0,
                    page_size: u32::MAX,
                },
            )
            .map(|page| page.items.into_iter().map(|p| p.index_id).collect())
            .unwrap_or_default();
        resolve_visible_selection(&visible, pending, self.active_profile().as_deref())
    }

    /// Resolve the persisted current group against the live subscriptions: a
    /// group that still exists stays, a dangling one resolves to `None` (the
    /// "All" view, upstream `RefreshSubscriptions`). Never persists a repair.
    pub fn resolve_current_group(&self) -> Option<String> {
        let persisted = self
            .settings
            .lock()
            .ok()
            .and_then(|guard| guard.settings.sub_index_id.clone());
        let existing: Vec<String> = self
            .list_sub_items()
            .map(|items| items.into_iter().map(|s| s.id).collect())
            .unwrap_or_default();
        crate::selection::resolve_current_group(persisted.as_deref(), &existing)
    }

    /// Switch the current group (`SubIndexId`, the view filter). `None`/empty
    /// means the "All" view. A named group must exist; the switch persists
    /// immediately so backups carry it. Unlike [`Self::set_active`] this never
    /// touches the default node or the desired revision.
    pub fn set_current_group(&self, id: Option<String>) -> Result<(), DomainError> {
        self.guard_storage()?;
        let normalized = present_id(id.as_deref()).map(str::to_string);
        if let Some(target) = &normalized {
            if self.get_sub_item(target)?.is_none() {
                return Err(DomainError::not_found("subscription", target));
            }
        }
        let previous = self
            .settings
            .lock()
            .ok()
            .and_then(|guard| guard.settings.sub_index_id.clone());
        if let Ok(mut guard) = self.settings.lock() {
            guard.settings.sub_index_id = normalized;
        }
        if let Err(error) = self.persist_config_standalone() {
            if let Ok(mut guard) = self.settings.lock() {
                guard.settings.sub_index_id = previous;
            }
            return Err(error);
        }
        Ok(())
    }

    /// Currently active profile id, if any.
    pub fn active_profile(&self) -> Option<String> {
        self.active.lock().ok().and_then(|guard| guard.clone())
    }

    /// Reload the frozen applied history and fact counters from one raw
    /// config tree (SP-05 reopen). A malformed history entry loads as absent
    /// history, never as a guessed target; counters fall back to zero and
    /// stay monotonic afterwards.
    fn restore_applied_history(&self, config: &Value) {
        let frozen: Option<FrozenAppliedTarget> = config
            .get(APPLIED_TARGET_KEY)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .flatten();
        if let Ok(mut guard) = self.applied_frozen.lock() {
            *guard = frozen;
        }
        self.intent_seq.store(
            config
                .get(INTENT_SEQ_KEY)
                .and_then(Value::as_u64)
                .unwrap_or(0),
            Ordering::Release,
        );
        self.actual_generation.store(
            config
                .get(ACTUAL_GENERATION_KEY)
                .and_then(Value::as_u64)
                .unwrap_or(0),
            Ordering::Release,
        );
    }

    // -- T11 routing / DNS use cases ----------------------------------------

    /// Seed built-in routing profiles + DNS rows when the stores are empty
    /// (upstream `ConfigHandler.InitBuiltinRouting` / `InitBuiltinDNS`).
    fn ensure_builtin_routing_dns(&self) {
        let empty_routing = self.routing.lock().map(|r| r.count() == 0).unwrap_or(false);
        if empty_routing {
            let mut seq = 0;
            for (mut profile, _) in domain::routing::builtin_profiles() {
                seq += 1;
                profile.id = crate::routing::new_routing_id();
                profile.sort = seq;
                // First profile (whitelist) becomes the default.
                profile.is_active = seq == 1;
                if let Ok(normalized) = crate::routing::normalize_routing(profile.clone()) {
                    profile = normalized;
                }
                let _ = self.routing.lock().map(|mut r| r.upsert(profile));
            }
        }
        let empty_dns = self
            .dns_items
            .lock()
            .map(|d| d.count() == 0)
            .unwrap_or(false);
        if empty_dns {
            for mut item in domain::dns::builtin_dns_profiles() {
                item.id = crate::dns::new_dns_id();
                let _ = self.dns_items.lock().map(|mut d| d.upsert(item));
            }
        }
    }

    /// All routing profiles in `Sort` order.
    pub fn list_routings(&self) -> Result<Vec<RoutingProfile>, DomainError> {
        self.routing
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .list()
    }

    /// One routing profile by id.
    pub fn get_routing(&self, id: &str) -> Result<Option<RoutingProfile>, DomainError> {
        self.routing
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .get(id)
    }

    /// The active routing profile, falling back to the first row (upstream
    /// `ConfigHandler.GetDefaultRouting`).
    pub fn default_routing(&self) -> Result<Option<RoutingProfile>, DomainError> {
        let items = self.list_routings()?;
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            items
                .iter()
                .find(|r| r.is_active)
                .or_else(|| items.first())
                .cloned()
                .unwrap(),
        ))
    }

    /// Save (insert or replace) one routing profile. Remarks are required and
    /// the embedded rules must each carry a match criterion.
    pub fn save_routing(&self, profile: RoutingProfile) -> Result<RoutingProfile, DomainError> {
        let mut normalized = crate::routing::normalize_routing(profile)?;
        // The DTO shape has no room for unknown keys: merge stored extras
        // (profile + per-rule, by id) back so an unrelated edit never wipes
        // them. New ids have no stored counterpart and keep their own.
        if !normalized.id.trim().is_empty() {
            if let Ok(Some(stored)) = self.get_routing(&normalized.id) {
                crate::routing::preserve_extras(Some(&stored), &mut normalized);
            }
        }
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        self.routing
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .upsert(normalized.clone())?;
        revisions.bump();
        self.persist_config(&revisions)?;
        Ok(normalized)
    }

    /// Delete one routing profile. Deleting the active profile promotes the
    /// first remaining row to active (upstream always has a default).
    pub fn delete_routing(&self, id: &str) -> Result<bool, DomainError> {
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let was_active = self.get_routing(id)?.map(|r| r.is_active).unwrap_or(false);
        let removed = self
            .routing
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .remove(id)?;
        if removed && was_active {
            let items = self
                .routing
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
                .list()?;
            if let Some(first) = items.into_iter().next() {
                let mut first = first;
                first.is_active = true;
                self.routing
                    .lock()
                    .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
                    .upsert(first)?;
            }
        }
        if removed {
            revisions.bump();
            self.persist_config(&revisions)?;
        }
        Ok(removed)
    }

    /// Mark one routing profile as the default (`IsActive` switch, upstream
    /// `ConfigHandler.SetDefaultRouting`). Already-active is a no-op error.
    pub fn set_default_routing(&self, id: &str) -> Result<(), DomainError> {
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut store = self
            .routing
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let items = store.list()?;
        if items.iter().any(|r| r.id == id && r.is_active) {
            return Err(DomainError::new(
                domain::codes::CONFLICT,
                "error.routing_already_default",
            ));
        }
        if !items.iter().any(|r| r.id == id) {
            return Err(DomainError::not_found("routing", id));
        }
        for mut item in items {
            item.is_active = item.id == id;
            store.upsert(item)?;
        }
        revisions.bump();
        drop(store);
        self.persist_config(&revisions)?;
        Ok(())
    }

    /// Move one rule inside a profile's rule set and persist the new order.
    pub fn move_routing_rule(
        &self,
        routing_id: &str,
        index: usize,
        direction: domain::routing::MoveDirection,
    ) -> Result<RoutingProfile, DomainError> {
        let mut profile = self
            .get_routing(routing_id)?
            .ok_or_else(|| DomainError::not_found("routing", routing_id))?;
        let mut rules = domain::routing::parse_rules(&profile)?;
        domain::routing::move_rule(&mut rules, index, direction)?;
        domain::routing::set_rules(&mut profile, &rules)?;
        self.save_routing(profile)
    }

    /// Replace or append imported rules on a profile and persist.
    pub fn import_routing_rules(
        &self,
        routing_id: &str,
        text: &str,
        replace: bool,
    ) -> Result<RoutingProfile, DomainError> {
        let mut profile = self
            .get_routing(routing_id)?
            .ok_or_else(|| DomainError::not_found("routing", routing_id))?;
        let current = domain::routing::parse_rules(&profile)?;
        let merged = crate::routing::merge_imported_rules(&current, text, replace)?;
        domain::routing::set_rules(&mut profile, &merged)?;
        self.save_routing(profile)
    }

    /// Export rules of a profile (all, or the `ids` selection) as JSON text.
    pub fn export_routing_rules(
        &self,
        routing_id: &str,
        ids: Option<&[String]>,
    ) -> Result<String, DomainError> {
        let profile = self
            .get_routing(routing_id)?
            .ok_or_else(|| DomainError::not_found("routing", routing_id))?;
        let rules = domain::routing::parse_rules(&profile)?;
        match ids {
            Some(ids) => crate::routing::export_selected_rules(&rules, ids),
            None => domain::routing::export_rules(&rules),
        }
    }

    /// Structured dangling-reference warnings for a profile's rule set
    /// (upstream: fall back to `proxy` with a warning).
    pub fn routing_warnings(
        &self,
        routing_id: &str,
    ) -> Result<Vec<domain::routing::RoutingWarning>, DomainError> {
        let profile = self
            .get_routing(routing_id)?
            .ok_or_else(|| DomainError::not_found("routing", routing_id))?;
        let rules = domain::routing::parse_rules(&profile)?;
        let remarks = self.profile_remarks()?;
        Ok(domain::routing::collect_warnings(&rules, &remarks))
    }

    fn profile_remarks(&self) -> Result<Vec<String>, DomainError> {
        let repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let page = repo.query(
            &crate::repository::ProfileFilter::default(),
            crate::repository::ProfileSort::IndexId,
            crate::repository::PageRequest {
                cursor: 0,
                page_size: u32::MAX,
            },
        )?;
        Ok(page.items.into_iter().map(|p| p.remarks).collect())
    }

    /// All DNS profiles ordered by core then remarks.
    pub fn list_dns(&self) -> Result<Vec<DnsProfile>, DomainError> {
        self.dns_items
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .list()
    }

    /// The DNS row for one core, if any.
    pub fn get_dns_for_core(&self, core: CoreType) -> Result<Option<DnsProfile>, DomainError> {
        Ok(self.list_dns()?.into_iter().find(|d| d.core_type == core))
    }

    /// Save (insert or replace) one DNS profile with per-core validation.
    pub fn save_dns(&self, profile: DnsProfile) -> Result<DnsProfile, DomainError> {
        let mut normalized = crate::dns::normalize_dns(profile)?;
        if !normalized.id.trim().is_empty() {
            let stored = self
                .dns_items
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
                .get(&normalized.id)?;
            crate::dns::preserve_dns_extras(stored.as_ref(), &mut normalized);
        }
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        self.dns_items
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .upsert(normalized.clone())?;
        revisions.bump();
        self.persist_config(&revisions)?;
        Ok(normalized)
    }

    /// Fill a core's DNS row with the embedded default config
    /// (`ImportDefConfig4V2ray` / `ImportDefConfig4Singbox`). Creates the row
    /// when missing. Returns the saved profile.
    pub fn import_default_dns(&self, core: CoreType) -> Result<DnsProfile, DomainError> {
        let existing = self.get_dns_for_core(core)?;
        let mut profile = existing.unwrap_or(DnsProfile {
            id: String::new(),
            remarks: if core == CoreType::SingBox {
                "sing-box".to_string()
            } else {
                "V2ray".to_string()
            },
            enabled: false,
            core_type: core,
            ..Default::default()
        });
        let (normal, tun) = if core == CoreType::SingBox {
            (
                domain::dns::DEFAULT_SINGBOX_DNS.to_string(),
                domain::dns::DEFAULT_TUN_SINGBOX_DNS.to_string(),
            )
        } else {
            (
                domain::dns::DEFAULT_V2RAY_DNS.to_string(),
                domain::dns::DEFAULT_V2RAY_DNS.to_string(),
            )
        };
        profile.normal_dns = Some(normal);
        profile.tun_dns = Some(tun);
        self.save_dns(profile)
    }

    /// Apply a regional preset (upstream `ConfigHandler.ApplyRegionalPreset`).
    ///
    /// `Default` resets geo/SRS/routing-template URLs, re-seeds built-in DNS
    /// rows and restores the built-in SimpleDNS. Russia / Iran set the region
    /// URLs and enable custom DNS with the embedded defaults; the remote
    /// per-region templates need network, so their URLs are reported pending
    /// (no silent fake content is written).
    pub fn apply_regional_preset(
        &self,
        preset: crate::dns::RegionalPreset,
    ) -> Result<(Vec<String>, crate::dns::RegionalPreset), DomainError> {
        {
            let mut settings = self
                .settings
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            match preset {
                crate::dns::RegionalPreset::Default => {
                    settings.settings.const_item.geo_source_url = None;
                    settings.settings.const_item.srs_source_url = None;
                    settings.settings.const_item.route_rules_template_source_url = None;
                    settings.settings.simple_dns_item = domain::SimpleDnsItem::builtin();
                }
                // Russia / Iran: write the canonical upstream source URLs into
                // the settings tree. Consumers (generation, downloads) read the
                // tree, so the preset and the settings stay linked; a preset
                // with no source table is an explicit error, not a silent skip.
                _ => {
                    let Some(sources) = crate::dns::region_sources(&preset) else {
                        return Err(DomainError::new(
                            domain::codes::INVALID_ARGUMENT,
                            "error.preset_no_source",
                        ));
                    };
                    settings.settings.const_item.geo_source_url =
                        Some(sources.geo_source.to_string());
                    settings.settings.const_item.srs_source_url =
                        Some(sources.srs_source.to_string());
                    settings.settings.const_item.route_rules_template_source_url =
                        Some(sources.routing_rules_source.to_string());
                }
            }
            settings.revision += 1;
        }
        if preset == crate::dns::RegionalPreset::Default {
            let mut store = self
                .dns_items
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            for item in store.list()? {
                let _ = store.remove(&item.id.clone());
            }
            drop(store);
            self.ensure_builtin_routing_dns();
            let revisions = self
                .revisions
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            self.persist_config(&revisions)?;
            return Ok((Vec::new(), preset));
        }
        // Offline fallback: enable custom DNS rows with the embedded defaults
        // so generation keeps working; remote region templates stay pending.
        for core in [CoreType::Xray, CoreType::SingBox] {
            let mut profile = self.get_dns_for_core(core)?.unwrap_or(DnsProfile {
                id: String::new(),
                remarks: if core == CoreType::SingBox {
                    "sing-box".to_string()
                } else {
                    "V2ray".to_string()
                },
                enabled: true,
                core_type: core,
                ..Default::default()
            });
            profile.enabled = true;
            if profile
                .normal_dns
                .as_ref()
                .is_none_or(|s| s.trim().is_empty())
            {
                let (normal, tun) = if core == CoreType::SingBox {
                    (
                        domain::dns::DEFAULT_SINGBOX_DNS.to_string(),
                        domain::dns::DEFAULT_TUN_SINGBOX_DNS.to_string(),
                    )
                } else {
                    (
                        domain::dns::DEFAULT_V2RAY_DNS.to_string(),
                        domain::dns::DEFAULT_V2RAY_DNS.to_string(),
                    )
                };
                profile.normal_dns = Some(normal);
                profile.tun_dns = Some(tun);
            }
            let _ = self.save_dns(profile);
        }
        let pending = crate::dns::pending_remote_templates(&preset);
        Ok((pending, preset))
    }

    /// Current routing mode (`ERuleMode`: Rule / Global / Direct).
    pub fn rule_mode(&self) -> RuleMode {
        self.rule_mode
            .lock()
            .ok()
            .map(|g| *g)
            .unwrap_or(RuleMode::Rule)
    }

    /// Switch the routing mode (persisted in `guiNConfig.json`).
    pub fn set_rule_mode(&self, mode: RuleMode) -> Result<(), DomainError> {
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        *self
            .rule_mode
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))? = mode;
        revisions.bump();
        self.persist_config(&revisions)?;
        Ok(())
    }

    // -- T12a settings use cases -------------------------------------------

    /// Current whole-tree settings revision.
    pub fn settings_revision(&self) -> u64 {
        self.settings
            .lock()
            .map(|guard| guard.revision)
            .unwrap_or(0)
    }

    /// `load_settings` — the normalised settings tree plus revision counters.
    pub fn load_settings(&self) -> Result<LoadedSettings, DomainError> {
        self.guard_storage()?;
        let guard = self
            .settings
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        Ok(LoadedSettings {
            settings: guard.settings.clone(),
            revision: guard.revision,
            group_revisions: guard.group_revisions.clone(),
        })
    }

    /// `save_settings` — whole-tree optimistic save.
    ///
    /// A stale `expected_revision` is rejected and a validation failure leaves
    /// the previous value untouched; on success the tree is persisted
    /// atomically and the changed fields are classified by `apply_timing`.
    ///
    /// SP-03: the caller payload never carries node identity. The canonical
    /// `IndexId`/`SubIndexId` are pinned to the authoritative default/group
    /// before validation, so a stale whole-tree draft can neither drift the
    /// backup identity nor clear the default (identity changes go through
    /// [`Self::set_active`] / [`Self::set_current_group`]).
    pub fn save_settings(
        &self,
        settings: AppSettings,
        expected_revision: u64,
    ) -> Result<SaveSettingsOutcome, DomainError> {
        self.guard_storage()?;
        self.ensure_commit_writable()?;
        let (pinned_index, pinned_sub) = self.authoritative_identity();
        let mut guard = self
            .settings
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        if guard.revision != expected_revision {
            return Err(DomainError::stale_revision(
                expected_revision,
                guard.revision,
            ));
        }
        let mut next = normalize_for_save(settings);
        next.index_id = pinned_index;
        next.sub_index_id = pinned_sub;
        validate_settings(&next)?;
        let changes = guard.settings.classified_changes(&next);
        let previous_settings = guard.settings.clone();
        let previous_revision = guard.revision;
        let previous_groups = guard.group_revisions.clone();
        guard.settings = next;
        guard.revision += 1;
        for group in domain::SETTINGS_GROUPS {
            *guard
                .group_revisions
                .entry((*group).to_string())
                .or_insert(0) += 1;
        }
        let new_revision = guard.revision;
        drop(guard);
        // A persist failure must not leave the in-memory tree ahead of disk:
        // roll back to the previous value so `load_settings` matches the file.
        if let Err(error) = self.persist_config_standalone() {
            if let Ok(mut guard) = self.settings.lock() {
                guard.settings = previous_settings;
                guard.revision = previous_revision;
                guard.group_revisions = previous_groups;
            }
            return Err(error);
        }
        // A settings change invalidates the planted runtime until re-applied.
        self.bump_desired_for_change()?;
        Ok(SaveSettingsOutcome::from_changes(new_revision, changes))
    }

    /// `save_settings_group` — patch one top-level group, leaving the others
    /// untouched. The expected revision is that group's own counter.
    ///
    /// SP-03: patches to unrelated groups cannot drift node identity: the
    /// canonical ids are re-pinned to the authoritative values. An explicit
    /// `IndexId` group write is validated like [`Self::set_active`] and syncs
    /// the engine mirror; an explicit `SubIndexId` write is the group switch.
    pub fn save_settings_group(
        &self,
        group: &str,
        patch: Value,
        expected_revision: u64,
    ) -> Result<SaveSettingsOutcome, DomainError> {
        // Same store guard as `save_settings`: a broken store must never
        // answer a group save as success (AUD-ROOT-04).
        self.guard_storage()?;
        self.ensure_commit_writable()?;
        if group == "IndexId" {
            return self.save_identity_group(patch, expected_revision);
        }
        let (pinned_index, pinned_sub) = self.authoritative_identity();
        let mut guard = self
            .settings
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let current_group = guard.group_revision(group);
        if current_group != expected_revision {
            return Err(DomainError::stale_revision(
                expected_revision,
                current_group,
            ));
        }
        let mut next = apply_group_patch(&guard.settings, group, patch)?;
        // An explicit `SubIndexId` write is the group switch itself; any other
        // group must not disturb either identity.
        if group != "SubIndexId" {
            next.index_id = pinned_index;
            next.sub_index_id = pinned_sub;
        } else {
            next.index_id = pinned_index;
        }
        validate_settings(&next)?;
        let changes = guard.settings.classified_changes(&next);
        let previous_settings = guard.settings.clone();
        let previous_revision = guard.revision;
        let previous_groups = guard.group_revisions.clone();
        guard.settings = next;
        guard.revision += 1;
        *guard.group_revisions.entry(group.to_string()).or_insert(0) += 1;
        let new_revision = guard.revision;
        drop(guard);
        if let Err(error) = self.persist_config_standalone() {
            if let Ok(mut guard) = self.settings.lock() {
                guard.settings = previous_settings;
                guard.revision = previous_revision;
                guard.group_revisions = previous_groups;
            }
            return Err(error);
        }
        // A settings change invalidates the applied runtime until re-applied.
        self.bump_desired_for_change()?;
        Ok(SaveSettingsOutcome::from_changes(new_revision, changes))
    }

    /// The authoritative node identity: the engine-mirror default plus the
    /// current group. Whole-tree and unrelated group saves pin the caller
    /// payload to these values so stale drafts cannot drift them.
    fn authoritative_identity(&self) -> (Option<String>, Option<String>) {
        let active = self.active.lock().ok().and_then(|guard| guard.clone());
        let sub = self
            .settings
            .lock()
            .ok()
            .and_then(|guard| guard.settings.sub_index_id.clone());
        (active, sub)
    }

    /// Explicit `IndexId` group write: validated like [`Self::set_active`] and
    /// mirrored, revision-guarded by the `IndexId` group counter.
    fn save_identity_group(
        &self,
        patch: Value,
        expected_revision: u64,
    ) -> Result<SaveSettingsOutcome, DomainError> {
        let id: Option<String> = match &patch {
            Value::Null => None,
            Value::String(text) => present_id(Some(text)).map(str::to_string),
            _ => {
                return Err(
                    DomainError::new(domain::codes::FIELD_FORMAT, "error.settings_patch")
                        .with_field("IndexId"),
                );
            }
        };
        if let Some(target) = &id {
            let repo = self
                .repo
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            if repo.get(target)?.is_none() {
                return Err(DomainError::not_found("profile", target));
            }
        }
        let mut guard = self
            .settings
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let current_group = guard.group_revision("IndexId");
        if current_group != expected_revision {
            return Err(DomainError::stale_revision(
                expected_revision,
                current_group,
            ));
        }
        let previous_settings = guard.settings.clone();
        let previous_revision = guard.revision;
        let previous_groups = guard.group_revisions.clone();
        guard.settings.index_id = id.clone();
        guard.revision += 1;
        *guard
            .group_revisions
            .entry("IndexId".to_string())
            .or_insert(0) += 1;
        let new_revision = guard.revision;
        let changes = previous_settings.classified_changes(&guard.settings);
        let previous_active = self.active_profile();
        drop(guard);
        if let Ok(mut active) = self.active.lock() {
            *active = id;
        }
        if let Err(error) = self.persist_config_standalone() {
            if let Ok(mut guard) = self.settings.lock() {
                guard.settings = previous_settings;
                guard.revision = previous_revision;
                guard.group_revisions = previous_groups;
            }
            if let Ok(mut active) = self.active.lock() {
                *active = previous_active;
            }
            return Err(error);
        }
        self.bump_desired_for_change()?;
        Ok(SaveSettingsOutcome::from_changes(new_revision, changes))
    }

    // -- SP-02 recoverable commit ------------------------------------------

    /// The current dataset epoch. Ordinary settings mutations keep it; a
    /// restore/import replacement advances it both in memory (via
    /// [`Self::prepare_restore`], guarding in-flight subscription commits)
    /// and in the persisted config (at activation, so an independent reopen
    /// observes it). A stale request can never take effect after a restore
    /// (plan §3.1).
    pub fn dataset_epoch(&self) -> DatasetEpoch {
        self.restore_epoch.load(Ordering::Acquire)
    }

    /// Test-only fault injection for the recoverable commit path. Production
    /// always leaves [`CommitTestFault::None`]; the value is never read from
    /// user input or persisted.
    pub fn set_commit_test_fault(&self, fault: CommitTestFault) {
        if let Ok(mut guard) = self.commit_fault.lock() {
            *guard = fault;
        }
    }

    fn commit_fault(&self) -> CommitTestFault {
        self.commit_fault
            .lock()
            .map(|guard| *guard)
            .unwrap_or(CommitTestFault::None)
    }

    /// Whether new writes must stay blocked until recovery confirms the
    /// pending journals. An unreadable journal fails closed (blocked).
    pub fn pending_commit_recovery(&self) -> bool {
        if self.commit_lock.try_lock().is_err() {
            return true;
        }
        match &self.data_dir {
            Some(dir) => {
                if persistence::commit::recovery_required(dir) {
                    return true;
                }
                match persistence::commit::pending(dir) {
                    Ok(list) => !list.is_empty(),
                    Err(_) => true,
                }
            }
            None => self.mem_recovery.load(Ordering::Acquire),
        }
    }

    fn ensure_commit_writable(&self) -> Result<(), DomainError> {
        if self.pending_commit_recovery() {
            return Err(
                DomainError::new(domain::codes::INTERNAL, "error.recovery_required")
                    .with_detail("a previous commit is unresolved; run recovery before writing")
                    .retryable(),
            );
        }
        Ok(())
    }

    /// `save_settings_commit` — recoverable whole-tree save (plan §3.3/§5.1).
    ///
    /// Coordinates one SQLite transaction with one `guiNConfig.json` publish
    /// through the [`persistence::commit`] journal and reports a frozen
    /// [`SettingsSaveReceipt`]: rejected saves pin no `new_revision`; a DB
    /// half committed without its file publish is `CommitUnknown` and blocks
    /// new writes with `RecoveryRequired` until recovery confirms.
    pub fn save_settings_commit(
        &self,
        dataset_epoch: DatasetEpoch,
        expected_revision: u64,
        mutation_id: &str,
        settings: AppSettings,
    ) -> SettingsSaveReceipt {
        if let Err(error) = self.guard_storage() {
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "storage",
                domain_to_contract(&error),
            );
        }
        let _commit_guard = match self.commit_lock.try_lock() {
            Ok(guard) => guard,
            Err(_) => return blocked_receipt(mutation_id, dataset_epoch),
        };
        if mutation_id.trim().is_empty() {
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "mutation",
                contract_error(
                    domain::codes::INVALID_ARGUMENT,
                    "error.invalid_mutation",
                    Some("mutation_id must not be empty".to_string()),
                    false,
                ),
            );
        }
        if dataset_epoch != self.dataset_epoch() {
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "epoch",
                contract_error(
                    domain::codes::CONFLICT,
                    "error.stale_epoch",
                    Some(format!(
                        "mutation epoch {dataset_epoch} does not match dataset epoch {}",
                        self.dataset_epoch()
                    )),
                    false,
                ),
            );
        }
        // Pure checks first: normalise + validate, then the idempotency key.
        // A replayed mutation returns its stored receipt even if the live
        // revision has moved on; different content under the same mutation id
        // is a conflict. Both precede the stale-revision gate so a retry with
        // the original parameters never looks like a new stale write.
        let next = normalize_for_save(settings);
        if let Err(error) = validate_settings(&next) {
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "validation",
                domain_to_contract(&error),
            );
        }
        let settings_hash = settings_hash_of(&next);
        if self.data_dir.is_none() {
            return self.save_settings_commit_memory(
                dataset_epoch,
                expected_revision,
                mutation_id,
                next,
                &settings_hash,
            );
        }
        let dir = self.data_dir.clone().unwrap_or_default();
        if persistence::commit::recovery_required(&dir)
            || persistence::commit::pending(&dir)
                .map(|list| !list.is_empty())
                .unwrap_or(true)
        {
            return blocked_receipt(mutation_id, dataset_epoch);
        }
        if let Ok(Some(done)) = persistence::commit::load_receipt(&dir, mutation_id) {
            if done.content_hash == settings_hash {
                if let Some(receipt) = parse_receipt(&done.receipt_json) {
                    return receipt;
                }
                return unknown_receipt(
                    mutation_id,
                    "",
                    dataset_epoch,
                    "journal",
                    contract_error(
                        domain::codes::INTERNAL,
                        "error.storage",
                        Some("stored completion is unreadable".to_string()),
                        true,
                    ),
                );
            }
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "mutation",
                contract_error(
                    domain::codes::CONFLICT,
                    "error.mutation_conflict",
                    Some(format!(
                        "mutation `{mutation_id}` was already committed with different content"
                    )),
                    false,
                ),
            );
        }
        let snapshot = match self.snapshot_settings_for_commit() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return rejected_receipt(
                    mutation_id,
                    dataset_epoch,
                    "storage",
                    domain_to_contract(&error),
                );
            }
        };
        if snapshot.revision != expected_revision {
            let stale = DomainError::stale_revision(expected_revision, snapshot.revision);
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "revision",
                domain_to_contract(&stale),
            );
        }
        let validated = self.build_validated_save(&snapshot, mutation_id, next, &settings_hash);
        let fault = self.commit_fault();
        let staged_rel = format!(
            "{}/{}/{mutation_id}/guiNConfig.json",
            persistence::commit::JOURNAL_DIR,
            "staging"
        );
        let record = persistence::commit::new_record(
            mutation_id,
            &validated.commit_id,
            1,
            dataset_epoch,
            expected_revision,
            &validated.settings_hash,
            &validated.doc_hash,
            &staged_rel,
            "guiNConfig.json",
        );
        if let Err(error) = persistence::commit::begin(&dir, &record, &validated.staged_text, fault)
        {
            let _ = persistence::commit::set_recovery_required(&dir);
            return unknown_receipt(
                mutation_id,
                &validated.commit_id,
                dataset_epoch,
                "journal",
                persist_to_contract(&error),
            );
        }
        if fault == CommitTestFault::CrashAfterStage {
            let _ = persistence::commit::set_recovery_required(&dir);
            return unknown_receipt(
                mutation_id,
                &validated.commit_id,
                dataset_epoch,
                "commit",
                contract_error(
                    "E_COMMIT_UNKNOWN",
                    "error.commit_unknown",
                    Some("stopped after staging, before the database commit".to_string()),
                    true,
                ),
            );
        }
        if fault == CommitTestFault::FailDbCommit {
            let _ = persistence::commit::abandon(&dir, mutation_id);
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "db",
                contract_error(
                    "E_FAULT_INJECTED",
                    "error.storage",
                    Some("injected database commit failure".to_string()),
                    true,
                ),
            );
        }
        if let Err(error) = self.commit_settings_db(&dir, mutation_id, &validated) {
            let _ = persistence::commit::abandon(&dir, mutation_id);
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "db",
                persist_to_contract(&error),
            );
        }
        if let Err(error) = persistence::commit::mark_db_committed(&dir, mutation_id) {
            let _ = persistence::commit::set_recovery_required(&dir);
            return unknown_receipt(
                mutation_id,
                &validated.commit_id,
                dataset_epoch,
                "journal",
                persist_to_contract(&error),
            );
        }
        if fault == CommitTestFault::CrashAfterDbCommit {
            let _ = persistence::commit::set_recovery_required(&dir);
            return unknown_receipt(
                mutation_id,
                &validated.commit_id,
                dataset_epoch,
                "commit",
                contract_error(
                    "E_COMMIT_UNKNOWN",
                    "error.commit_unknown",
                    Some("stopped after the database commit, before the file publish".to_string()),
                    true,
                ),
            );
        }
        if let Err(error) = persistence::commit::publish_staged(&dir, mutation_id, fault) {
            let _ = persistence::commit::set_recovery_required(&dir);
            return unknown_receipt(
                mutation_id,
                &validated.commit_id,
                dataset_epoch,
                "publish",
                persist_to_contract(&error),
            );
        }
        let token = document_token(
            &validated.commit_id,
            validated.new_revision,
            &validated.settings_hash,
        );
        let receipt = committed_receipt(
            mutation_id,
            &validated.commit_id,
            dataset_epoch,
            validated.new_revision,
            &token,
            &validated.settings_hash,
        );
        let done = persistence::commit::DoneRecord {
            receipt_json: serde_json::to_string(&receipt).unwrap_or_else(|_| "{}".to_string()),
            content_hash: validated.settings_hash.clone(),
        };
        if let Err(error) = persistence::commit::finish(&dir, mutation_id, &done) {
            let _ = persistence::commit::set_recovery_required(&dir);
            return unknown_receipt(
                mutation_id,
                &validated.commit_id,
                dataset_epoch,
                "journal",
                persist_to_contract(&error),
            );
        }
        self.install_committed_doc(&validated.doc, validated.new_desired);
        let _ = persistence::commit::clear_recovery_required(&dir);
        receipt
    }

    /// Query the stored outcome of one mutation. Unknown mutations are
    /// `NotStarted` with no revision; unresolved journals are `CommitUnknown`.
    /// A query never replays a non-idempotent write.
    pub fn query_settings_mutation(
        &self,
        dataset_epoch: DatasetEpoch,
        mutation_id: &str,
    ) -> SettingsSaveReceipt {
        if let Err(error) = self.guard_storage() {
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "storage",
                domain_to_contract(&error),
            );
        }
        if self.data_dir.is_none() {
            if let Ok(commits) = self.mem_commits.lock() {
                if let Some(found) = commits.get(mutation_id) {
                    if let Some(receipt) = parse_receipt(&found.receipt_json) {
                        return receipt;
                    }
                }
            }
            if let Ok(pending) = self.mem_pending.lock() {
                if let Some(active) = pending.as_ref() {
                    if active.mutation_id == mutation_id {
                        return unknown_receipt(
                            mutation_id,
                            &active.commit_id,
                            active.dataset_epoch,
                            "commit",
                            contract_error(
                                "E_COMMIT_UNKNOWN",
                                "error.commit_unknown",
                                Some("mutation has an unresolved commit".to_string()),
                                true,
                            ),
                        );
                    }
                }
            }
            return not_started_receipt(mutation_id, dataset_epoch);
        }
        let dir = self.data_dir.clone().unwrap_or_default();
        if let Ok(Some(done)) = persistence::commit::load_receipt(&dir, mutation_id) {
            if let Some(receipt) = parse_receipt(&done.receipt_json) {
                return receipt;
            }
        }
        if let Ok(Some(record)) = persistence::commit::load(&dir, mutation_id) {
            let phase = if record.stage == CommitStage::DbCommitted {
                "publish"
            } else {
                "commit"
            };
            return unknown_receipt(
                mutation_id,
                &record.commit_id,
                record.dataset_epoch,
                phase,
                contract_error(
                    "E_COMMIT_UNKNOWN",
                    "error.commit_unknown",
                    Some(format!(
                        "mutation `{mutation_id}` has an unresolved journal"
                    )),
                    true,
                ),
            );
        }
        not_started_receipt(mutation_id, dataset_epoch)
    }

    /// Recover every unresolved journal: discard staged-only mutations
    /// (rollback) and publish DB-committed ones (roll-forward). Returns one
    /// receipt per resolved mutation, oldest first. While this runs, new
    /// writes report `RecoveryRequired`; afterwards the snapshot is consistent
    /// and writable again.
    pub fn recover_pending_commits(&self) -> Vec<SettingsSaveReceipt> {
        if let Err(error) = self.guard_storage() {
            return vec![rejected_receipt(
                "recovery",
                self.dataset_epoch(),
                "storage",
                domain_to_contract(&error),
            )];
        }
        let _commit_guard = match self.commit_lock.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return vec![blocked_receipt("recovery", self.dataset_epoch())];
            }
        };
        if self.data_dir.is_none() {
            return self.recover_memory_commits();
        }
        let dir = self.data_dir.clone().unwrap_or_default();
        let pendings = match persistence::commit::pending(&dir) {
            Ok(list) => list,
            Err(error) => {
                let _ = persistence::commit::set_recovery_required(&dir);
                return vec![unknown_receipt(
                    "recovery",
                    "",
                    self.dataset_epoch(),
                    "journal",
                    persist_to_contract(&error),
                )];
            }
        };
        let mut out = Vec::new();
        for record in &pendings {
            out.push(self.recover_one_commit(&dir, record));
        }
        if persistence::commit::pending(&dir)
            .map(|list| list.is_empty())
            .unwrap_or(false)
        {
            let _ = persistence::commit::clear_recovery_required(&dir);
        }
        out
    }

    /// Memory-only variant of [`Self::save_settings_commit`] for engines
    /// without a data directory. Same receipt contract, same stage order, no
    /// files involved.
    fn save_settings_commit_memory(
        &self,
        dataset_epoch: DatasetEpoch,
        expected_revision: u64,
        mutation_id: &str,
        next: AppSettings,
        settings_hash: &str,
    ) -> SettingsSaveReceipt {
        if let Ok(commits) = self.mem_commits.lock() {
            if let Some(found) = commits.get(mutation_id) {
                if found.content_hash == settings_hash {
                    if let Some(receipt) = parse_receipt(&found.receipt_json) {
                        return receipt;
                    }
                }
                return rejected_receipt(
                    mutation_id,
                    dataset_epoch,
                    "mutation",
                    contract_error(
                        domain::codes::CONFLICT,
                        "error.mutation_conflict",
                        Some(format!(
                            "mutation `{mutation_id}` was already committed with different content"
                        )),
                        false,
                    ),
                );
            }
        }
        let snapshot = match self.snapshot_settings_for_commit() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return rejected_receipt(
                    mutation_id,
                    dataset_epoch,
                    "storage",
                    domain_to_contract(&error),
                );
            }
        };
        if snapshot.revision != expected_revision {
            let stale = DomainError::stale_revision(expected_revision, snapshot.revision);
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "revision",
                domain_to_contract(&stale),
            );
        }
        let validated = self.build_validated_save(&snapshot, mutation_id, next, settings_hash);
        let fault = self.commit_fault();
        if fault == CommitTestFault::FailJournalWrite {
            self.mem_recovery.store(true, Ordering::Release);
            return unknown_receipt(
                mutation_id,
                &validated.commit_id,
                dataset_epoch,
                "journal",
                contract_error(
                    "E_FAULT_INJECTED",
                    "error.storage",
                    Some("injected journal failure".to_string()),
                    true,
                ),
            );
        }
        if fault == CommitTestFault::CrashAfterStage || fault == CommitTestFault::FailDbCommit {
            if fault == CommitTestFault::CrashAfterStage {
                if let Ok(mut pending) = self.mem_pending.lock() {
                    *pending = Some(MemPending {
                        mutation_id: mutation_id.to_string(),
                        commit_id: validated.commit_id.clone(),
                        dataset_epoch,
                        expected_revision,
                        content_hash: validated.settings_hash.clone(),
                        stage: CommitStage::Staged,
                        doc: validated.doc.clone(),
                        new_revision: validated.new_revision,
                        new_desired: validated.new_desired,
                    });
                }
                self.mem_recovery.store(true, Ordering::Release);
                return unknown_receipt(
                    mutation_id,
                    &validated.commit_id,
                    dataset_epoch,
                    "commit",
                    contract_error(
                        "E_COMMIT_UNKNOWN",
                        "error.commit_unknown",
                        Some("stopped before the database commit".to_string()),
                        true,
                    ),
                );
            }
            return rejected_receipt(
                mutation_id,
                dataset_epoch,
                "db",
                contract_error(
                    "E_FAULT_INJECTED",
                    "error.storage",
                    Some("injected database commit failure".to_string()),
                    true,
                ),
            );
        }
        if fault == CommitTestFault::CrashAfterDbCommit || fault == CommitTestFault::FailFilePublish
        {
            if let Ok(mut pending) = self.mem_pending.lock() {
                *pending = Some(MemPending {
                    mutation_id: mutation_id.to_string(),
                    commit_id: validated.commit_id.clone(),
                    dataset_epoch,
                    expected_revision,
                    content_hash: validated.settings_hash.clone(),
                    stage: CommitStage::DbCommitted,
                    doc: validated.doc.clone(),
                    new_revision: validated.new_revision,
                    new_desired: validated.new_desired,
                });
            }
            self.mem_recovery.store(true, Ordering::Release);
            return unknown_receipt(
                mutation_id,
                &validated.commit_id,
                dataset_epoch,
                "publish",
                contract_error(
                    if fault == CommitTestFault::FailFilePublish {
                        "E_FAULT_INJECTED"
                    } else {
                        "E_COMMIT_UNKNOWN"
                    },
                    if fault == CommitTestFault::FailFilePublish {
                        "error.storage"
                    } else {
                        "error.commit_unknown"
                    },
                    Some("database committed without the file publish".to_string()),
                    true,
                ),
            );
        }
        let token = document_token(
            &validated.commit_id,
            validated.new_revision,
            &validated.settings_hash,
        );
        let receipt = committed_receipt(
            mutation_id,
            &validated.commit_id,
            dataset_epoch,
            validated.new_revision,
            &token,
            &validated.settings_hash,
        );
        if let Ok(mut commits) = self.mem_commits.lock() {
            commits.insert(
                mutation_id.to_string(),
                MemCommit {
                    receipt_json: serde_json::to_string(&receipt)
                        .unwrap_or_else(|_| "{}".to_string()),
                    content_hash: validated.settings_hash.clone(),
                },
            );
        }
        self.install_committed_doc(&validated.doc, validated.new_desired);
        receipt
    }

    /// Memory-only recovery: discard staged-only pendings, roll forward
    /// DB-committed ones.
    fn recover_memory_commits(&self) -> Vec<SettingsSaveReceipt> {
        let pending = self.mem_pending.lock().map(|mut guard| guard.take());
        let Some(active) = pending.unwrap_or(None) else {
            self.mem_recovery.store(false, Ordering::Release);
            return Vec::new();
        };
        if active.stage == CommitStage::Staged {
            let receipt = rejected_receipt(
                &active.mutation_id,
                active.dataset_epoch,
                "commit",
                contract_error(
                    "E_COMMIT_ROLLED_BACK",
                    "error.commit_rolled_back",
                    Some(
                        "the database commit never happened; staged content discarded".to_string(),
                    ),
                    false,
                ),
            );
            if let Ok(mut commits) = self.mem_commits.lock() {
                commits.insert(
                    active.mutation_id.clone(),
                    MemCommit {
                        receipt_json: serde_json::to_string(&receipt)
                            .unwrap_or_else(|_| "{}".to_string()),
                        content_hash: active.content_hash.clone(),
                    },
                );
            }
            self.mem_recovery.store(false, Ordering::Release);
            return vec![receipt];
        }
        let token = document_token(&active.commit_id, active.new_revision, &active.content_hash);
        let receipt = committed_receipt(
            &active.mutation_id,
            &active.commit_id,
            active.dataset_epoch,
            active.new_revision,
            &token,
            &active.content_hash,
        );
        if let Ok(mut commits) = self.mem_commits.lock() {
            commits.insert(
                active.mutation_id.clone(),
                MemCommit {
                    receipt_json: serde_json::to_string(&receipt)
                        .unwrap_or_else(|_| "{}".to_string()),
                    content_hash: active.content_hash.clone(),
                },
            );
        }
        self.install_committed_doc(&active.doc, active.new_desired);
        self.mem_recovery.store(false, Ordering::Release);
        vec![receipt]
    }

    fn recover_one_commit(
        &self,
        dir: &Path,
        record: &persistence::commit::JournalRecord,
    ) -> SettingsSaveReceipt {
        let mutation_id = record.mutation_id.as_str();
        if record.stage == CommitStage::Staged {
            let _ = persistence::commit::abandon(dir, mutation_id);
            let receipt = rejected_receipt(
                mutation_id,
                record.dataset_epoch,
                "commit",
                contract_error(
                    "E_COMMIT_ROLLED_BACK",
                    "error.commit_rolled_back",
                    Some(
                        "the database commit never happened; staged content discarded".to_string(),
                    ),
                    false,
                ),
            );
            let done = persistence::commit::DoneRecord {
                receipt_json: serde_json::to_string(&receipt).unwrap_or_else(|_| "{}".to_string()),
                content_hash: record.content_hash.clone(),
            };
            let _ = persistence::commit::finish(dir, mutation_id, &done);
            return receipt;
        }
        let staged = dir.join(&record.staged_file);
        let staged_text = match std::fs::read_to_string(&staged).ok() {
            Some(text) => text,
            None => {
                // The staged payload is gone. If the target already carries
                // the committed content, the publish happened and only the
                // bookkeeping is left; otherwise the commit stays unresolved.
                let target = dir.join(&record.target_file);
                match std::fs::read_to_string(&target) {
                    Ok(target_text)
                        if persistence::hash::sha256_hex(target_text.as_bytes())
                            == record.doc_hash =>
                    {
                        return self.finish_rolled_forward(dir, record, &target_text);
                    }
                    _ => {
                        let _ = persistence::commit::set_recovery_required(dir);
                        return unknown_receipt(
                            mutation_id,
                            &record.commit_id,
                            record.dataset_epoch,
                            "publish",
                            contract_error(
                                "E_COMMIT_UNKNOWN",
                                "error.commit_unknown",
                                Some(
                                    "staged payload missing; target does not carry the commit"
                                        .to_string(),
                                ),
                                true,
                            ),
                        );
                    }
                }
            }
        };
        if persistence::hash::sha256_hex(staged_text.as_bytes()) != record.doc_hash {
            let _ = persistence::commit::set_recovery_required(dir);
            return unknown_receipt(
                mutation_id,
                &record.commit_id,
                record.dataset_epoch,
                "publish",
                contract_error(
                    "E_COMMIT_UNKNOWN",
                    "error.commit_unknown",
                    Some("staged payload does not match the journal hash".to_string()),
                    true,
                ),
            );
        }
        if let Err(error) =
            persistence::commit::publish_staged(dir, mutation_id, self.commit_fault())
        {
            let _ = persistence::commit::set_recovery_required(dir);
            return unknown_receipt(
                mutation_id,
                &record.commit_id,
                record.dataset_epoch,
                "publish",
                persist_to_contract(&error),
            );
        }
        self.finish_rolled_forward(dir, record, &staged_text)
    }

    /// Complete a roll-forward whose document text is known: parse, install,
    /// record the done receipt and clean the journal.
    fn finish_rolled_forward(
        &self,
        dir: &Path,
        record: &persistence::commit::JournalRecord,
        doc_text: &str,
    ) -> SettingsSaveReceipt {
        let mutation_id = record.mutation_id.as_str();
        let doc: Value = match serde_json::from_str(doc_text) {
            Ok(doc) => doc,
            Err(error) => {
                let _ = persistence::commit::set_recovery_required(dir);
                return unknown_receipt(
                    mutation_id,
                    &record.commit_id,
                    record.dataset_epoch,
                    "publish",
                    contract_error(
                        domain::codes::FIELD_FORMAT,
                        "error.config_corrupt",
                        Some(error.to_string()),
                        false,
                    ),
                );
            }
        };
        let new_revision = doc
            .get("settings_revision")
            .and_then(Value::as_u64)
            .unwrap_or(record.expected_revision.saturating_add(1));
        let new_desired = doc
            .get("desired_revision")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let token = document_token(&record.commit_id, new_revision, &record.content_hash);
        let receipt = committed_receipt(
            mutation_id,
            &record.commit_id,
            record.dataset_epoch,
            new_revision,
            &token,
            &record.content_hash,
        );
        let done = persistence::commit::DoneRecord {
            receipt_json: serde_json::to_string(&receipt).unwrap_or_else(|_| "{}".to_string()),
            content_hash: record.content_hash.clone(),
        };
        if persistence::commit::finish(dir, mutation_id, &done).is_err() {
            let _ = persistence::commit::set_recovery_required(dir);
            return unknown_receipt(
                mutation_id,
                &record.commit_id,
                record.dataset_epoch,
                "journal",
                contract_error(
                    domain::codes::INTERNAL,
                    "error.storage",
                    Some("could not record the recovered commit".to_string()),
                    true,
                ),
            );
        }
        self.install_committed_doc(&doc, new_desired);
        receipt
    }

    fn snapshot_settings_for_commit(&self) -> Result<SettingsCommitSnapshot, DomainError> {
        let (settings, revision, group_revisions) = self
            .settings
            .lock()
            .map(|guard| {
                (
                    guard.settings.clone(),
                    guard.revision,
                    guard.group_revisions.clone(),
                )
            })
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let desired = self
            .revisions
            .lock()
            .map(|guard| guard.desired().get())
            .unwrap_or(0);
        let active = self.active.lock().ok().and_then(|guard| guard.clone());
        let sub_index_id = self
            .settings
            .lock()
            .ok()
            .and_then(|guard| guard.settings.sub_index_id.clone());
        let rule_mode = self
            .rule_mode
            .lock()
            .ok()
            .map(|mode| match *mode {
                RuleMode::Global => "Global".to_string(),
                RuleMode::Direct => "Direct".to_string(),
                _ => "Rule".to_string(),
            })
            .unwrap_or_else(|| "Rule".to_string());
        let templates = self
            .templates
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        Ok(SettingsCommitSnapshot {
            settings,
            revision,
            group_revisions,
            desired,
            active,
            sub_index_id,
            rule_mode,
            templates,
        })
    }

    fn build_validated_save(
        &self,
        snapshot: &SettingsCommitSnapshot,
        mutation_id: &str,
        mut next: AppSettings,
        settings_hash: &str,
    ) -> ValidatedSave {
        // SP-03: the caller payload never carries node identity; pin the
        // canonical ids to the authoritative snapshot before hashing the
        // document, so every commit keeps the dual identity unified.
        next.index_id.clone_from(&snapshot.active);
        next.sub_index_id.clone_from(&snapshot.sub_index_id);
        let new_revision = snapshot.revision.saturating_add(1);
        let new_desired = snapshot.desired.saturating_add(1);
        let mut groups: HashMap<String, u64> = snapshot
            .group_revisions
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        for group in domain::SETTINGS_GROUPS {
            *groups.entry((*group).to_string()).or_insert(0) += 1;
        }
        let mut object = match serde_json::to_value(&next).unwrap_or(Value::Null) {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        object.insert(
            "desired_revision".to_string(),
            serde_json::json!(new_desired),
        );
        object.insert(
            "active_index_id".to_string(),
            serde_json::json!(snapshot.active),
        );
        // SP-03: ordinary commits preserve the dataset epoch; only a
        // restore/import replacement advances it (plan §3.1).
        object.insert(
            DATASET_EPOCH_KEY.to_string(),
            serde_json::json!(self.dataset_epoch()),
        );
        object.insert(
            "rule_mode".to_string(),
            serde_json::json!(snapshot.rule_mode),
        );
        object.insert(
            "full_config_templates".to_string(),
            serde_json::json!(snapshot.templates),
        );
        object.insert(
            "settings_revision".to_string(),
            serde_json::json!(new_revision),
        );
        object.insert(
            "settings_group_revisions".to_string(),
            serde_json::json!(groups),
        );
        let doc = Value::Object(object);
        let staged_text = serde_json::to_string_pretty(&doc).unwrap_or_default();
        let doc_hash = persistence::hash::sha256_hex(staged_text.as_bytes());
        ValidatedSave {
            doc,
            settings_hash: settings_hash.to_string(),
            doc_hash,
            staged_text,
            commit_id: commit_id_for(mutation_id),
            new_revision,
            new_desired,
            new_groups: groups,
        }
    }

    /// Record the commit in `guiNDB.db` inside one SQLite transaction. The
    /// row only tracks the commit identity; the settings document itself is
    /// published through the staged file.
    fn commit_settings_db(
        &self,
        dir: &Path,
        mutation_id: &str,
        validated: &ValidatedSave,
    ) -> Result<(), persistence::PersistenceError> {
        let store = persistence::Store::open(dir.join("guiNDB.db"))?;
        let tx = store.begin()?;
        let meta = serde_json::json!({
            "commit_id": validated.commit_id,
            "dataset_epoch": self.dataset_epoch(),
            "settings_revision": validated.new_revision,
            "content_hash": validated.settings_hash,
            "doc_hash": validated.doc_hash,
        });
        store.set_meta(
            &tx,
            &format!("settings_commit:{mutation_id}"),
            &meta.to_string(),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Install a committed document into memory after its file publish (or
    /// roll-forward) succeeded. The file stays the source of truth; memory
    /// only mirrors it.
    fn install_committed_doc(&self, doc: &Value, new_desired: u64) {
        if let Ok(state) = read_settings_state(doc) {
            if let Ok(mut guard) = self.settings.lock() {
                guard.settings = state.settings;
                guard.revision = state.revision;
                guard.group_revisions = state.group_revisions;
            }
        }
        if let Ok(mut revisions) = self.revisions.lock() {
            let mut spins = 0;
            while revisions.desired().get() < new_desired && spins < 1_000_000 {
                revisions.bump();
                spins += 1;
            }
        }
    }

    /// Record a stored change that requires a runtime apply: bump the global
    /// desired revision and persist it. Node/routing/DNS saves already bump
    /// `desired`; settings and rule-mode go through here so the UI can show
    /// "saved, not applied" and offer an apply entry point.
    fn bump_desired_for_change(&self) -> Result<(), DomainError> {
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        revisions.bump();
        self.persist_config(&revisions)
    }

    fn persist_config_standalone(&self) -> Result<(), DomainError> {
        let revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        self.persist_config(&revisions)
    }

    fn persist_config(&self, revisions: &RevisionStore) -> Result<(), DomainError> {
        let Some(dir) = &self.data_dir else {
            return Ok(());
        };
        let active = self.active.lock().ok().and_then(|guard| guard.clone());
        let rule_mode = self
            .rule_mode
            .lock()
            .ok()
            .map(|g| *g)
            .unwrap_or(RuleMode::Rule);
        let templates = self
            .templates
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let (settings, settings_revision, group_revisions) = self
            .settings
            .lock()
            .map(|guard| {
                (
                    guard.settings.clone(),
                    guard.revision,
                    guard.group_revisions.clone(),
                )
            })
            .unwrap_or((AppSettings::default(), 0, Default::default()));
        let mut object = match serde_json::to_value(&settings).unwrap_or(Value::Null) {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        object.insert(
            "desired_revision".to_string(),
            serde_json::json!(revisions.desired().get()),
        );
        object.insert("active_index_id".to_string(), serde_json::json!(active));
        // SP-05: the frozen applied history and the fact counters ride with
        // the document so an independent reopen reports the same
        // applied-vs-actual truth. Only saving the default never touches
        // them: this write preserves, never invents, the running target.
        let frozen = self
            .applied_frozen
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        object.insert(
            APPLIED_TARGET_KEY.to_string(),
            serde_json::to_value(&frozen).unwrap_or(Value::Null),
        );
        object.insert(
            INTENT_SEQ_KEY.to_string(),
            serde_json::json!(self.intent_seq.load(Ordering::Acquire)),
        );
        object.insert(
            ACTUAL_GENERATION_KEY.to_string(),
            serde_json::json!(self.actual_generation.load(Ordering::Acquire)),
        );
        // SP-03: the epoch rides with the document so an independent reopen
        // observes the post-restore generation; ordinary saves preserve it.
        object.insert(
            DATASET_EPOCH_KEY.to_string(),
            serde_json::json!(self.dataset_epoch()),
        );
        object.insert(
            "rule_mode".to_string(),
            serde_json::json!(match rule_mode {
                RuleMode::Global => "Global",
                RuleMode::Direct => "Direct",
                _ => "Rule",
            }),
        );
        object.insert(
            "full_config_templates".to_string(),
            serde_json::json!(templates),
        );
        object.insert(
            "settings_revision".to_string(),
            serde_json::json!(settings_revision),
        );
        object.insert(
            "settings_group_revisions".to_string(),
            serde_json::json!(group_revisions),
        );
        write_config(dir, &Value::Object(object))?;
        Ok(())
    }

    /// `apply_runtime` use case with an explicit submit-time target.
    ///
    /// The target/plan/revision is frozen **before** anything reaches the
    /// runtime (SP-05/RUN-04): `target_id` is the node the plan was built
    /// for, recorded with the plan id, config hash, revision, operation id,
    /// intent sequence and actual generation. A later `set_active` (desired
    /// default change) never rewrites this record, and a stop withdraws the
    /// live session without rewriting the history entry.
    pub fn apply_runtime_for_target(
        &self,
        plan: RuntimePlan,
        target_id: &str,
        expected_revision: DesiredRevision,
    ) -> Result<String, DomainError> {
        self.guard_storage()?;
        {
            let revisions = self
                .revisions
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            revisions.check(expected_revision)?;
        }
        plan.validate()?;

        // SP-04: join the single in-process command sequence after the frozen
        // revision check, so a stale submit never reaches the runtime and
        // concurrent submits from several windows serialize in admission
        // order instead of interleaving behind the UI queue.
        let _cmd = self
            .runtime_cmd_lock
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;

        // SP-05: allocate the submit sequence and the frozen fact before the
        // runtime call. Nothing below re-reads the desired default.
        let intent_seq = self.intent_seq.fetch_add(1, Ordering::AcqRel) + 1;
        let mut frozen = FrozenAppliedTarget::new(
            target_id.to_string(),
            plan.plan_id.clone(),
            plan.target.config_sha256.as_str().to_string(),
            plan.target.core_type,
            plan.desired_revision,
            String::new(),
            intent_seq,
            self.actual_generation.load(Ordering::Acquire),
        );

        match self.runtime.apply(&plan)? {
            ApplyOutcome::Accepted { operation_id } => {
                // SP-05: freeze the submit-time fact. The actual generation
                // is the current counter: it advances only when applied
                // facts transition (first Running publish / live withdraw),
                // never on the submit itself.
                frozen.operation_id = operation_id.clone();
                if let Ok(mut guard) = self.applied_frozen.lock() {
                    *guard = Some(frozen);
                }
                // Record the core + statistics/API ports of the accepted plan
                // so the monitor pipeline can poll the applied session without
                // re-deriving facts from a later desired plan.
                let facts = self.applied_facts_for(&plan);
                if let Ok(mut guard) = self.apply_facts.lock() {
                    *guard = Some(facts);
                }
                let job = self.jobs.start("apply_runtime");
                if let Ok(mut guard) = self.operation_jobs.lock() {
                    guard.insert(operation_id.clone(), job.job_id.clone());
                }
                // Persist the frozen history so an independent reopen reports
                // the same applied-vs-actual truth. A persist failure keeps
                // the in-memory fact (still correct for this process); the
                // reopen path then treats history as absent, never as B.
                if let Ok(revisions) = self.revisions.lock() {
                    let _ = self.persist_config(&revisions);
                }
                // The correlation the UI uses is the job id; the runtime's
                // operation id is embedded in the plan correlation.
                Ok(format!("{}:{}", operation_id, job.job_id))
            }
            ApplyOutcome::Unavailable => Err(DomainError::new(
                domain::codes::UNAVAILABLE,
                "error.runtime_unavailable",
            )
            .retryable()),
        }
    }

    /// `apply_runtime` use case.
    ///
    /// Same freeze contract as [`Self::apply_runtime_for_target`]; the target
    /// falls back to the desired default read **before** the runtime call.
    /// Prefer the explicit entry point when the caller already resolved the
    /// target (the bridge does): a pre-read default still cannot cover an
    /// explicit `applyTarget(B)` issued while active is `A`.
    pub fn apply_runtime(
        &self,
        plan: RuntimePlan,
        expected_revision: DesiredRevision,
    ) -> Result<String, DomainError> {
        // Pre-read, never post-read: capturing the default after `Accepted`
        // is the RUN-04 race (a concurrent `set_active(B)` would relabel A's
        // session as B).
        let target = self.active_profile().unwrap_or_default();
        self.apply_runtime_for_target(plan, &target, expected_revision)
    }

    /// `stop_runtime` use case: ask net-host to stop the managed core.
    ///
    /// SP-04: joins the same in-process command sequence as `apply_runtime`
    /// (stop is the cleanup barrier: never superseded, never overtaken).
    /// SP-05: withdrawing the live session is an actual-fact transition, so
    /// the generation advances when a live fact is actually withdrawn, even
    /// though desired is unchanged; the frozen history is retained (a stop
    /// never rewrites what actually ran), and an accepted apply that never
    /// reached Running is closed as Cancelled instead of left Running
    /// forever (RUN-05). A failed stop changes none of this: the backend
    /// fact is still unknown.
    pub fn stop_runtime(&self) -> Result<(), DomainError> {
        let _cmd = self
            .runtime_cmd_lock
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        // A failed stop leaves every fact untouched: the backend outcome is
        // still unknown, so no generation, job or history change applies.
        self.runtime.stop()?;
        self.cancel_pending_apply_jobs();
        let had_live = self
            .applied_session
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .is_some();
        if had_live {
            if let Ok(mut guard) = self.applied_session.lock() {
                *guard = None;
            }
            self.set_local_proxy_port(None);
            self.actual_generation.fetch_add(1, Ordering::AcqRel);
        }
        if let Ok(revisions) = self.revisions.lock() {
            let _ = self.persist_config(&revisions);
        }
        Ok(())
    }

    /// Register the sink for unsolicited net-host events (control + detail).
    pub fn subscribe_runtime_events(&self, sink: EventSink) {
        self.runtime.subscribe_events(sink);
    }

    /// `cancel_job` use case (idempotent).
    ///
    /// The application job manager is the authority for job ids; the runtime
    /// is consulted first so a runtime-side operation is also signalled. A
    /// runtime that cannot cancel returns `NotCancellable`, which does not
    /// override the job-manager result.
    pub fn cancel_job(&self, job_id: &JobId) -> CancelOutcome {
        let outcome = self.jobs.cancel(job_id);
        if outcome == CancelOutcome::AlreadyFinished {
            if let Ok(runtime_outcome) = self.runtime.cancel(job_id) {
                return runtime_outcome;
            }
        }
        outcome
    }

    // -- T09 subscription use cases ----------------------------------------

    /// Record the running session's local socks/mixed port (proxy updates).
    pub fn set_local_proxy_port(&self, port: Option<u16>) {
        if let Ok(mut guard) = self.local_proxy_port.lock() {
            *guard = port;
        }
    }

    /// The applied session fact, when a managed core is actually running.
    ///
    /// `None` means no successful applied session; the desired active node is
    /// never reported as an applied endpoint.
    pub fn applied_session(&self) -> Option<AppliedSession> {
        self.applied_session
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// The applied inbound proxy protocol (`http` / `socks` / `mixed`) of the
    /// running session, or `None` when nothing is applied (R4-24). Never
    /// derived from the desired selection; no credentials are exposed.
    pub fn applied_inbound_protocol(&self) -> Option<String> {
        self.applied_session()?;
        let scheme = self
            .apply_facts
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .map(|facts| facts.scheme)?;
        Some(scheme.as_str().to_string())
    }

    /// The actual bound local proxy port of the running session, if known.
    pub fn applied_inbound_port(&self) -> Option<u16> {
        self.applied_session()
            .and_then(|session| session.proxy_port)
    }

    /// Read the structured status of a prior runtime operation (R4-04
    /// reconcile). Accepts the raw runtime operation id and the compound
    /// `"<operation_id>:<job_id>"` correlation [`Self::apply_runtime`]
    /// returns (RUN-05): the compound id is split, never sent to the runtime
    /// verbatim. A terminal state on either side wins: the runtime view is
    /// authoritative while in flight, but an engine job that already reached
    /// its terminal state (via the snapshot reconcile listener) is reported
    /// as terminal even when the backend record still reads non-terminal.
    /// Unknown ids stay a structured not-found.
    pub fn operation_status(&self, operation_id: &str) -> Result<OperationStatusView, DomainError> {
        let (raw_op, compound_job) = split_operation_id(operation_id);
        let runtime_view = self.runtime.operation_status(raw_op).ok();
        let correlated_job: Option<JobId> = compound_job
            .map(JobId::new)
            .or_else(|| self.operation_job(raw_op));
        let correlated_view = correlated_job
            .as_ref()
            .and_then(|job_id| self.jobs.get(job_id));
        match (runtime_view, correlated_view) {
            (Some(mut view), correlated) => {
                // The runtime never tracks the application job: fill the
                // correlation so one query returns both identities.
                if view.job_id.is_none() {
                    view.job_id = correlated
                        .as_ref()
                        .map(|job| job.job_id.0.clone())
                        .or_else(|| correlated_job.map(|job| job.0));
                }
                if !view.state.is_terminal() {
                    if let Some(job) = correlated {
                        if job.state.is_terminal() {
                            return Ok(OperationStatusView {
                                operation_id: raw_op.to_string(),
                                job_id: Some(job.job_id.0),
                                state: job.state,
                                cancel: None,
                                error: job.error,
                            });
                        }
                    }
                }
                Ok(view)
            }
            (None, Some(job)) => Ok(OperationStatusView {
                operation_id: raw_op.to_string(),
                job_id: Some(job.job_id.0),
                state: job.state,
                cancel: None,
                error: job.error,
            }),
            (None, None) => Err(DomainError::not_found("operation", operation_id)),
        }
    }

    /// The job correlated with one accepted runtime operation, if any.
    pub fn operation_job(&self, operation_id: &str) -> Option<JobId> {
        self.operation_jobs
            .lock()
            .ok()
            .and_then(|guard| guard.get(operation_id).cloned())
    }

    /// The frozen submit-time target of the last accepted apply (SP-05
    /// history). Survives stop and desired-default changes; `None` means no
    /// apply was ever accepted by this engine lineage.
    pub fn applied_target(&self) -> Option<FrozenAppliedTarget> {
        self.applied_frozen
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Current actual generation (plan §3.1 `actualGeneration`).
    pub fn actual_generation(&self) -> u64 {
        self.actual_generation.load(Ordering::Acquire)
    }

    /// Most recently allocated submit sequence (plan §3.2 `intentSeq`).
    pub fn last_intent_seq(&self) -> u64 {
        self.intent_seq.load(Ordering::Acquire)
    }

    /// Close every accepted apply job that never reached Running (RUN-05).
    /// Called on the stop barrier: the stop supersedes those submits, so
    /// they end `Cancelled`, never `Done` and never dangling `Running`.
    /// Terminal jobs are left untouched.
    fn cancel_pending_apply_jobs(&self) {
        let ids: Vec<JobId> = self
            .operation_jobs
            .lock()
            .map(|guard| guard.values().cloned().collect())
            .unwrap_or_default();
        for id in ids {
            if let Some(job) = self.jobs.get(&id) {
                if !job.state.is_terminal() {
                    self.jobs.finish(&id, domain::JobState::Cancelled, None);
                }
            }
        }
    }

    /// Finish the job correlated with one frozen submit when it is still open.
    /// Terminal jobs are never rewritten: a late snapshot cannot turn a
    /// `Cancelled`/`Failed` job into `Done`.
    fn finish_correlated_job(&self, frozen: &FrozenAppliedTarget, state: domain::JobState) {
        self.finish_correlated_job_with(frozen, state, None);
    }

    /// [`Self::finish_correlated_job`] with an attached failure, so a failed
    /// submit closes as `Failed` with its structured error instead of
    /// dangling `Running` forever (RUN-05).
    fn finish_correlated_job_with(
        &self,
        frozen: &FrozenAppliedTarget,
        state: domain::JobState,
        error: Option<DomainError>,
    ) {
        if let Ok(guard) = self.operation_jobs.lock() {
            if let Some(job_id) = guard.get(&frozen.operation_id) {
                if let Some(job) = self.jobs.get(job_id) {
                    if !job.state.is_terminal() {
                        self.jobs.finish(job_id, state, error);
                    }
                }
            }
        }
    }

    /// Reconcile the applied-session fact from a fresh runtime snapshot.
    ///
    /// Only a `Running` session with a bound port publishes an endpoint, and
    /// the published target is the submit-time frozen target (SP-05), never
    /// the current desired default. A stop/failure withdraws the live fact
    /// (and advances the actual generation) while the frozen history stays so
    /// reopen keeps reporting what actually ran. Busy states keep the
    /// previous fact so an in-place restart does not drop the old endpoint
    /// before the new one is proven.
    fn reconcile_applied_session(&self, snapshot: &RuntimeSnapshot) {
        match snapshot.state {
            RuntimeState::Running => {
                let Some(port) = snapshot.ports.first().copied() else {
                    return;
                };
                // SP-05: only a target this engine actually froze at submit
                // counts as the applied target. A fresh engine reconnecting
                // to an already running net-host has no frozen record, and an
                // empty record means no explicit target was ever submitted:
                // inventing the persisted desired node (or a default Xray API)
                // would mislabel the actual session, so publish no applied
                // session instead (R4-05 direction preserved).
                let Some(frozen) = self
                    .applied_frozen
                    .lock()
                    .ok()
                    .and_then(|guard| guard.clone())
                else {
                    return;
                };
                if frozen.target_profile_id.is_empty() {
                    return;
                }
                let was_live = self
                    .applied_session
                    .lock()
                    .ok()
                    .and_then(|guard| guard.clone())
                    .is_some();
                if let Ok(mut guard) = self.applied_session.lock() {
                    *guard = Some(AppliedSession {
                        session_id: snapshot.session_id.clone(),
                        active_index_id: Some(frozen.target_profile_id.clone()),
                        proxy_port: Some(port),
                        applied_revision: snapshot.applied_revision,
                    });
                }
                self.set_local_proxy_port(Some(port));
                if !was_live {
                    // First proof this submit actually runs: advance the
                    // generation, stamp it on the frozen record, and close
                    // its apply job as Done (RUN-05). A later default change
                    // never re-triggers this: the frozen record, not the
                    // desired default, gates it.
                    let generation = self.actual_generation.fetch_add(1, Ordering::AcqRel) + 1;
                    if let Ok(mut guard) = self.applied_frozen.lock() {
                        if let Some(stored) = guard.as_mut() {
                            if stored.operation_id == frozen.operation_id {
                                stored.actual_generation = generation;
                            }
                        }
                    }
                    if let Ok(revisions) = self.revisions.lock() {
                        let _ = self.persist_config(&revisions);
                    }
                    self.finish_correlated_job(&frozen, domain::JobState::Done);
                }
            }
            RuntimeState::Stopped | RuntimeState::Degraded | RuntimeState::RollingBack => {
                let was_live = self
                    .applied_session
                    .lock()
                    .ok()
                    .and_then(|guard| guard.clone())
                    .is_some();
                if let Ok(mut guard) = self.applied_session.lock() {
                    *guard = None;
                }
                self.set_local_proxy_port(None);
                if was_live {
                    // Exit/failure is an actual-fact transition even when
                    // desired is unchanged (SP-05 §3.1). History stays frozen.
                    self.actual_generation.fetch_add(1, Ordering::AcqRel);
                }
                // An accepted submit that already ended is closed: with the
                // backend error when there is one, otherwise as Cancelled
                // (superseded by the stop/external exit). A submit that
                // never published and carries no error is still pending
                // (idle backend), so its job stays open. Open jobs never
                // dangle after a proven end; terminal jobs are never
                // rewritten.
                if was_live || snapshot.error.is_some() {
                    if let Some(frozen) = self
                        .applied_frozen
                        .lock()
                        .ok()
                        .and_then(|guard| guard.clone())
                    {
                        match snapshot.error.clone() {
                            Some(error) => self.finish_correlated_job_with(
                                &frozen,
                                domain::JobState::Failed,
                                Some(error),
                            ),
                            None => {
                                self.finish_correlated_job(&frozen, domain::JobState::Cancelled)
                            }
                        }
                    }
                }
            }
            RuntimeState::Validating
            | RuntimeState::Preparing
            | RuntimeState::Starting
            | RuntimeState::Checking => {}
        }
    }

    /// The local proxy endpoint as an explicit URL, when known.
    ///
    /// Prefers the actual applied-session port; the explicit
    /// [`Self::set_local_proxy_port`] hook remains only as a test/override
    /// fallback and is never set by the production session path.
    pub fn local_proxy_url(&self) -> Option<String> {
        let applied_port = self
            .applied_session
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().and_then(|session| session.proxy_port));
        let port =
            applied_port.or_else(|| self.local_proxy_port.lock().ok().and_then(|guard| *guard));
        let scheme = self
            .apply_facts
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .map(|facts| facts.scheme.scheme())
            .unwrap_or("http");
        port.map(|port| format!("{scheme}://127.0.0.1:{port}"))
    }

    /// Resolve the monitor/API facts for an accepted plan. A full Custom config
    /// carries its real statistics/API port, listen address and Clash secret in
    /// the emitted JSON (RR-07 / R3-07). A Custom config that declares no API
    /// gets zero endpoints so the poller idles instead of probing the default
    /// statistics port.
    fn applied_facts_for(&self, plan: &RuntimePlan) -> AppliedFacts {
        let opts = self.runtime_codegen_options();
        let core = plan.target.core_type;
        let mut facts = AppliedFacts {
            core,
            state_port: opts.state_port.clamp(0, u16::MAX as i32) as u16,
            state_port2: opts.state_port2.clamp(0, u16::MAX as i32) as u16,
            scheme: runtime::ProxyProtocol::Mixed,
            api_kind: None,
            api_secret: None,
        };
        let is_custom = self
            .active_profile()
            .and_then(|id| self.profile_by_id(&id).ok().flatten())
            .map(|profile| profile.config_type == ConfigType::Custom)
            .unwrap_or(false);
        if is_custom {
            // Never fall back to a default statistics API for a Custom config:
            // the ports exist only if the config declares them.
            facts.state_port = 0;
            facts.state_port2 = 0;
            if let ConfigSource::Inline { body } = &plan.target.config {
                if let Ok(endpoints) = runtime::parse_custom_endpoints(core, body) {
                    if let Some(api) = &endpoints.api {
                        facts.api_kind = Some(api.kind);
                        facts.api_secret = api.secret.clone();
                        match api.kind {
                            runtime::ApiKind::XrayStats => facts.state_port = api.port,
                            runtime::ApiKind::ClashApi => facts.state_port2 = api.port,
                        }
                    }
                    if let Some(primary) = endpoints.primary() {
                        facts.scheme = primary.protocol;
                    }
                }
            }
        }
        facts
    }

    /// Whether the monitor pipeline should collect (`GuiItem.EnableStatistics`)
    /// and display real-time speed (`GuiItem.DisplayRealTimeSpeed`).
    pub fn monitor_settings(&self) -> (bool, bool) {
        let settings = self
            .settings
            .lock()
            .map(|guard| guard.settings.clone())
            .unwrap_or_default();
        (
            settings.gui_item.enable_statistics,
            settings.gui_item.display_real_time_speed,
        )
    }

    /// The monitor facts of the currently applied session, or `None` when no
    /// managed core is running. This consumes the FIX-07 [`AppliedSession`]
    /// fact; a desired-but-not-applied node is never reported.
    pub fn monitor_session(&self) -> Option<MonitorSession> {
        let applied = self.applied_session()?;
        let facts = self
            .apply_facts
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_else(|| {
                let opts = self.runtime_codegen_options();
                AppliedFacts {
                    core: CoreType::Xray,
                    state_port: opts.state_port.clamp(0, u16::MAX as i32) as u16,
                    state_port2: opts.state_port2.clamp(0, u16::MAX as i32) as u16,
                    scheme: runtime::ProxyProtocol::Mixed,
                    api_kind: None,
                    api_secret: None,
                }
            });
        Some(MonitorSession {
            core: facts.core,
            active_index_id: applied.active_index_id,
            proxy_port: applied.proxy_port,
            state_port: facts.state_port,
            state_port2: facts.state_port2,
            api_secret: facts.api_secret,
        })
    }

    /// A `ServerStatItem` store bound to this engine's data directory, or
    /// `None` for the in-memory engine. The bridge binds it to the monitor hub
    /// so real per-node rows persist and reload across restarts.
    pub fn traffic_store(&self) -> Option<Box<dyn crate::monitor::TrafficStore>> {
        let dir = self.data_dir.as_ref()?;
        let db = dir.join("guiNDB.db");
        let store = Store::open(&db).ok()?;
        Some(Box::new(SqliteTrafficStore::from_store(store)))
    }

    /// `list_sub_items` use case, ordered by `Sort`.
    pub fn list_sub_items(&self) -> Result<Vec<SubItem>, DomainError> {
        self.subs
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .list()
    }

    /// One subscription by id.
    pub fn get_sub_item(&self, id: &str) -> Result<Option<SubItem>, DomainError> {
        self.subs
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .get(id)
    }

    /// `save_sub_item` use case: validates, persists and bumps the revision.
    ///
    /// A new item without an id is assigned a stable id and the next `Sort`.
    pub fn save_sub_item(&self, mut item: SubItem) -> Result<SubItem, DomainError> {
        item.validate()?;
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut subs = self
            .subs
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        if item.id.trim().is_empty() {
            item.id = new_sub_id();
        }
        if subs.get(&item.id)?.is_none() && item.sort == 0 {
            item.sort = subs
                .list()?
                .iter()
                .map(|s| s.sort)
                .max()
                .map_or(1, |m| m + 1);
        }
        subs.upsert(item.clone())?;
        revisions.bump();
        drop(subs);
        self.persist_config(&revisions)?;
        Ok(item)
    }

    /// `delete_sub_items` use case. Returns the number of removed rows.
    ///
    /// Mirrors upstream `ConfigHandler.DeleteSubItem`: deleting a subscription
    /// also deletes every `ProfileItem` with that `subid`
    /// (`RemoveServersViaSubid(config, id, isSub: false)`). Profile rows are
    /// removed before the subscription row so a mid-way failure leaves the
    /// subscription intact rather than orphaning its nodes. (Upstream also
    /// deletes on-disk custom-config files for Custom/Outbound rows; this
    /// engine keeps those contents inline, so there is no file to delete.)
    pub fn delete_sub_items(&self, ids: &[String]) -> Result<u64, DomainError> {
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        // Remove dependent nodes first (upstream `RemoveServersViaSubid`).
        {
            let mut repo = self
                .repo
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            for id in ids {
                let existing = repo.query(
                    &ProfileFilter {
                        subid: Some(id.clone()),
                        ..ProfileFilter::default()
                    },
                    ProfileSort::IndexId,
                    PageRequest {
                        cursor: 0,
                        page_size: u32::MAX,
                    },
                )?;
                for profile in existing.items {
                    repo.remove(&profile.index_id)?;
                }
            }
        }
        let mut subs = self
            .subs
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut removed = 0u64;
        for id in ids {
            if subs.remove(id)? {
                removed += 1;
            }
        }
        if removed > 0 {
            revisions.bump();
            drop(subs);
            self.persist_config(&revisions)?;
        }
        Ok(removed)
    }

    /// `set_sub_enabled` use case.
    pub fn set_sub_enabled(&self, id: &str, enabled: bool) -> Result<SubItem, DomainError> {
        let mut item = self
            .get_sub_item(id)?
            .ok_or_else(|| DomainError::not_found("subscription", id))?;
        item.enabled = enabled;
        self.save_sub_item(item)
    }

    /// Persist a new `Sort` order. `ids` is the intended display order.
    pub fn reorder_sub_items(&self, ids: &[String]) -> Result<(), DomainError> {
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut subs = self
            .subs
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        for (index, id) in ids.iter().enumerate() {
            if let Some(mut item) = subs.get(id)? {
                item.sort = (index + 1) as i32;
                subs.upsert(item)?;
            }
        }
        revisions.bump();
        drop(subs);
        self.persist_config(&revisions)?;
        Ok(())
    }

    /// Select the subscriptions a request targets (enabled-only, non-empty urls).
    fn target_subs(&self, request: &SubUpdateRequest) -> Result<Vec<SubItem>, DomainError> {
        let all = self.list_sub_items()?;
        if request.sub_ids.is_empty() {
            return Ok(all);
        }
        Ok(all
            .into_iter()
            .filter(|s| request.sub_ids.contains(&s.id))
            .collect())
    }

    /// `refresh_subscriptions` use case (the F-SUB-003 update pipeline).
    ///
    /// For each targeted subscription: download (with `MoreUrl` merge), parse
    /// and filter into a candidate set, then atomically replace that `subid`'s
    /// nodes. A failed or empty download preserves the old nodes and surfaces a
    /// structured error. Returns a report and updates `UpdateTime` on success.
    pub async fn refresh_subscriptions(
        &self,
        request: SubUpdateRequest,
        cancellation: &CancellationToken,
        max_items: usize,
    ) -> SubUpdateReport {
        let mut report = SubUpdateReport::default();
        // Capture the epoch before any download: if a restore/import exchanges
        // the database while a download is in flight, `refresh_one`'s commit is
        // rejected instead of writing into the swapped database (R3-SET-03).
        let epoch = self.restore_epoch();
        let targets = match self.target_subs(&request) {
            Ok(items) => items,
            Err(_) => return report,
        };
        for item in targets {
            if cancellation.is_cancelled() {
                report.entries.push(SubUpdateEntry {
                    sub_id: item.id.clone(),
                    remarks: item.remarks.clone(),
                    outcome: SubUpdateOutcome::Cancelled,
                });
                break;
            }
            report.entries.push(
                self.refresh_one(&item, &request, cancellation, max_items, epoch)
                    .await,
            );
        }
        report
    }

    async fn refresh_one(
        &self,
        item: &SubItem,
        request: &SubUpdateRequest,
        cancellation: &CancellationToken,
        max_items: usize,
        epoch: u64,
    ) -> SubUpdateEntry {
        let entry = |outcome: SubUpdateOutcome| SubUpdateEntry {
            sub_id: item.id.clone(),
            remarks: item.remarks.clone(),
            outcome,
        };
        if item.url.trim().is_empty() {
            return entry(SubUpdateOutcome::Skipped {
                reason: "error.url_required".into(),
            });
        }
        if !item.enabled {
            return entry(SubUpdateOutcome::Skipped {
                reason: "error.sub_disabled".into(),
            });
        }
        if cancellation.is_cancelled() {
            return entry(SubUpdateOutcome::Cancelled);
        }
        let existing = self.profiles_by_subid(&item.id).unwrap_or_default();

        // Candidate-first: download and parse before touching storage.
        let content = match download_all(
            item,
            request.via_proxy,
            request.proxy_url.as_deref(),
            cancellation,
        )
        .await
        {
            Ok(content) => content,
            Err(subscriptions::SubError::Cancelled) => return entry(SubUpdateOutcome::Cancelled),
            Err(err) => return entry(sub_error_outcome(&err)),
        };

        let mut candidates = match build_candidates(item, &content, &existing, max_items) {
            Ok(profiles) => profiles,
            Err(err) => return entry(sub_error_outcome(&err)),
        };
        crate::subs::assign_candidate_ids(&mut candidates);

        // Atomic replace for this subid (upstream: remove-then-write).
        match self.replace_sub_profiles_at_epoch(&item.id, candidates, true, epoch) {
            Ok((added, removed)) => {
                let _ = self.touch_sub_update_time(&item.id, unix_now());
                SubUpdateEntry {
                    sub_id: item.id.clone(),
                    remarks: item.remarks.clone(),
                    outcome: SubUpdateOutcome::Updated { added, removed },
                }
            }
            Err(err) => entry(SubUpdateOutcome::Failed {
                code: err.code,
                message: err.message_key,
            }),
        }
    }

    /// All profiles belonging to a subscription, in stable order.
    pub fn profiles_by_subid(&self, subid: &str) -> Result<Vec<Profile>, DomainError> {
        let filter = ProfileFilter {
            subid: Some(subid.to_string()),
            ..ProfileFilter::default()
        };
        let page = self.query_profiles(
            filter,
            ProfileSort::IndexId,
            PageRequest {
                cursor: 0,
                page_size: u32::MAX,
            },
        )?;
        Ok(page.items)
    }

    /// Insert-or-replace records for a subid. Removes existing rows, then
    /// upserts the replacement set inside a single storage transaction.
    /// Returns `(added, removed)`; any mid-replace failure rolls back so the
    /// old nodes survive.
    pub fn replace_sub_profiles(
        &self,
        subid: &str,
        profiles: Vec<Profile>,
        remove_existing: bool,
    ) -> Result<(usize, usize), DomainError> {
        self.replace_sub_profiles_at_epoch(subid, profiles, remove_existing, self.restore_epoch())
    }

    /// [`Self::replace_sub_profiles`] guarded by the restore epoch that a task
    /// captured before the exchange. A mismatch means a restore/import replaced
    /// the database while the download was in flight, so the stale commit is
    /// explicitly rejected instead of clobbering the restored nodes.
    pub fn replace_sub_profiles_at_epoch(
        &self,
        subid: &str,
        profiles: Vec<Profile>,
        remove_existing: bool,
        expected_epoch: u64,
    ) -> Result<(usize, usize), DomainError> {
        if self.restore_epoch() != expected_epoch {
            return Err(
                DomainError::new(domain::codes::CONFLICT, "error.restore_in_progress")
                    .with_detail("subscription commit rejected: restore exchanged the database"),
            );
        }
        let mut profiles = profiles;
        for profile in &mut profiles {
            if profile.subid.is_empty() {
                profile.subid = subid.to_string();
            }
            if profile.index_id.trim().is_empty() {
                profile.index_id = crate::repository::new_index_id();
            }
        }
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let (added, removed) = repo.replace_for_sub(subid, profiles, remove_existing, false)?;
        revisions.bump();
        drop(repo);
        self.persist_config(&revisions)?;
        Ok((added, removed))
    }

    // -- SP-14 All纯预览批导入 --------------------------------------------------

    /// Register a parse-only preview: memory only, never SQLite/受控文件.
    ///
    /// Returns the `preview_token` binding this exact content; the later
    /// [`Self::commit_import_batch`] rejects any token that was not registered
    /// here or whose content digest drifted (second parse, new ids, retarget).
    pub fn register_import_preview(&self, text: &str, profiles: &[Profile]) -> String {
        let token = crate::import_batch::preview_token(text);
        let digest = crate::import_batch::content_digest(profiles);
        if let Ok(mut previews) = self.import_previews.lock() {
            if previews.len() >= 128 {
                if let Some(first) = previews.keys().next().cloned() {
                    previews.remove(&first);
                }
            }
            previews.insert(token.clone(), digest);
        }
        token
    }

    /// True when `token` was registered for exactly `profiles`.
    pub fn check_import_token(&self, profiles: &[Profile], token: &str) -> bool {
        let digest = crate::import_batch::content_digest(profiles);
        self.import_previews
            .lock()
            .ok()
            .and_then(|previews| previews.get(token).cloned())
            .is_some_and(|registered| registered == digest)
    }

    /// Fault injection for the SP-14 commit path (tests only). Production
    /// always leaves it `false`; the next commit fails after staging and rolls
    /// back staged files plus the DB batch.
    pub fn set_import_commit_fault(&self, fault: bool) {
        self.import_fault.store(fault, Ordering::Release);
    }

    /// Commit one previewed batch atomically (SP-14).
    ///
    /// Order: mutation replay check -> revision check -> token binding check ->
    /// stage Custom files -> single `replace_sub_profiles` transaction. Any
    /// failure before the DB commit writes nothing; a DB failure also deletes
    /// files staged by this commit so no orphan survives. A committed
    /// `mutation_id` replays the same receipt without rewriting.
    pub fn commit_import_batch(
        &self,
        commit: crate::import_batch::ImportCommit,
    ) -> Result<crate::import_batch::ImportReceipt, DomainError> {
        use crate::import_batch as batch;
        self.guard_storage()?;
        if commit.profiles.is_empty() {
            return Err(batch::empty_commit_error());
        }
        if let Some(cached) = self
            .import_commits
            .lock()
            .ok()
            .and_then(|commits| commits.get(&commit.mutation_id).cloned())
        {
            return Ok(cached);
        }
        let current = self.desired_revision();
        if commit.expected_revision != current {
            return Err(batch::stale_revision_error(
                commit.expected_revision,
                current,
            ));
        }
        if !self.check_import_token(&commit.profiles, &commit.preview_token) {
            return Err(batch::token_mismatch_error());
        }
        let target = commit.target_group.clone().unwrap_or_default();
        // Stage Custom/Outbound payloads before the DB transaction.
        let mut staged = commit.profiles.clone();
        let mut new_files: Vec<std::path::PathBuf> = Vec::new();
        let config_dir = self
            .data_dir()
            .map(|dir| dir.join("config"))
            .unwrap_or_else(std::env::temp_dir);
        for profile in staged.iter_mut() {
            if !matches!(
                profile.config_type,
                domain::ConfigType::Custom | domain::ConfigType::Outbound
            ) {
                continue;
            }
            let Some(raw) = subscriptions::take_raw_config(profile) else {
                continue;
            };
            let name = batch::staged_file_name(&raw);
            let existed = if self.data_dir().is_some() {
                config_dir.join(&name).exists()
            } else {
                std::env::temp_dir().join(&name).exists()
            };
            let (dir, stored) = if self.data_dir().is_some() {
                (config_dir.clone(), name.clone())
            } else {
                let dir = std::env::temp_dir();
                let absolute = dir.join(&name).to_string_lossy().into_owned();
                (dir, absolute)
            };
            let written = std::fs::create_dir_all(&dir).is_ok()
                && std::fs::write(dir.join(&name), &raw).is_ok();
            if !written {
                for path in new_files {
                    std::fs::remove_file(path).ok();
                }
                return Err(DomainError::new(domain::codes::INTERNAL, "error.storage")
                    .with_detail("import commit: failed to stage custom config"));
            }
            if !existed {
                new_files.push(dir.join(&name));
            }
            profile.address = stored;
        }
        if self.import_fault.swap(false, Ordering::AcqRel) {
            for path in new_files {
                std::fs::remove_file(path).ok();
            }
            return Err(DomainError::new(domain::codes::INTERNAL, "error.storage")
                .with_detail("injected import commit failure"));
        }
        let normalized: Vec<Profile> = staged
            .into_iter()
            .map(|p| batch::normalize_batch(p, &target))
            .collect();
        let count = normalized.len();
        let result = self.replace_sub_profiles(&target, normalized, false);
        match result {
            Ok(_) => {
                let new_revision = self.desired_revision();
                let receipt = batch::ImportReceipt {
                    ok: true,
                    imported: count as u32,
                    commit_id: crate::recoverable_commit::commit_id_for(&commit.mutation_id),
                    new_revision,
                };
                if let Ok(mut commits) = self.import_commits.lock() {
                    commits.insert(commit.mutation_id, receipt.clone());
                }
                Ok(receipt)
            }
            Err(error) => {
                for path in new_files {
                    std::fs::remove_file(path).ok();
                }
                Err(error)
            }
        }
    }

    /// Overwrite only `UpdateTime` for a subscription (scheduler bookkeeping).
    pub fn touch_sub_update_time(&self, id: &str, time: i64) -> Result<(), DomainError> {
        let mut item = self
            .get_sub_item(id)?
            .ok_or_else(|| DomainError::not_found("subscription", id))?;
        item.update_time = time;
        let mut subs = self
            .subs
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        subs.upsert(item)?;
        Ok(())
    }

    // -- T10 group / custom / template use cases -----------------------------

    /// Every profile keyed by stable id (group validation + child resolution).
    fn all_profiles_map_locked(
        &self,
    ) -> Result<std::collections::HashMap<String, Profile>, DomainError> {
        let repo = self
            .repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let page = repo.query(
            &ProfileFilter::default(),
            ProfileSort::IndexId,
            PageRequest {
                cursor: 0,
                page_size: u32::MAX,
            },
        )?;
        Ok(page
            .items
            .into_iter()
            .map(|p| (p.index_id.clone(), p))
            .collect())
    }

    /// Ordered, de-duplicated child profiles of a group/chain node: subscription
    /// matches first, then the explicit `ChildItems` order.
    pub fn group_children(&self, index_id: &str) -> Result<Vec<Profile>, DomainError> {
        let all = self.all_profiles_map_locked()?;
        let group = all
            .get(index_id)
            .ok_or_else(|| DomainError::not_found("profile", index_id))?;
        Ok(crate::groups::resolve_children(group, &all)
            .into_iter()
            .cloned()
            .collect())
    }

    /// Create the "all nodes of this subscription" policy group
    /// (upstream `ConfigHandler.AddGroupAllServer`).
    pub fn gen_group_all(&self, sub_id: &str) -> Result<Profile, DomainError> {
        let sub = self
            .get_sub_item(sub_id)?
            .ok_or_else(|| DomainError::not_found("subscription", sub_id))?;
        let remarks = format!("{} - PolicyGroup", sub.remarks);
        let draft = crate::groups::new_group_all(sub_id, remarks);
        let revision = self.desired_revision();
        Ok(self.save_profile(draft, DesiredRevision::new(revision))?.0)
    }

    /// Create one policy group per region with at least one matching node
    /// (upstream `ConfigHandler.AddGroupRegionServer`).
    pub fn gen_group_region(&self, sub_id: &str) -> Result<Vec<Profile>, DomainError> {
        let sub = self
            .get_sub_item(sub_id)?
            .ok_or_else(|| DomainError::not_found("subscription", sub_id))?;
        let nodes = self.profiles_by_subid(sub_id)?;
        let mut created = Vec::new();
        for (region, pattern) in crate::groups::REGION_FILTERS {
            let filter = crate::groups::region_filter(pattern);
            let matches = nodes.iter().any(|p| {
                crate::groups::is_eligible_child(p)
                    && crate::groups::remarks_match(Some(&filter), &p.remarks)
            });
            if !matches {
                continue;
            }
            let draft =
                crate::groups::new_group_region(sub_id, sub.remarks.clone(), region, pattern);
            let revision = self.desired_revision();
            created.push(self.save_profile(draft, DesiredRevision::new(revision))?.0);
        }
        Ok(created)
    }

    /// All full-config templates in stable order.
    pub fn list_templates(&self) -> Result<Vec<FullConfigTemplate>, DomainError> {
        self.templates
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))
    }

    /// The template row for `core`, if any.
    pub fn get_template_for_core(
        &self,
        core: CoreType,
    ) -> Result<Option<FullConfigTemplate>, DomainError> {
        Ok(self
            .list_templates()?
            .into_iter()
            .find(|t| t.core_type == core))
    }

    /// Validate, normalize and persist one template row.
    ///
    /// Upstream keeps one row per core; a draft without an id therefore
    /// updates the existing row for its core instead of adding a duplicate.
    pub fn save_template(
        &self,
        item: FullConfigTemplate,
    ) -> Result<FullConfigTemplate, DomainError> {
        crate::templates::validate(&item)?;
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut templates = self
            .templates
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut item = crate::templates::normalize(item, crate::repository::new_index_id());
        if item.id.trim().is_empty() {
            item.id = crate::repository::new_index_id();
        }
        if let Some(slot) = templates.iter_mut().find(|t| t.id == item.id) {
            *slot = item.clone();
        } else if let Some(slot) = templates.iter_mut().find(|t| t.core_type == item.core_type) {
            item.id = slot.id.clone();
            *slot = item.clone();
        } else {
            templates.push(item.clone());
        }
        templates.sort_by_key(|t| t.core_type.value());
        revisions.bump();
        drop(templates);
        self.persist_config(&revisions)?;
        Ok(item)
    }

    /// Delete one template row by id. Returns whether a row was removed.
    pub fn delete_template(&self, id: &str) -> Result<bool, DomainError> {
        let mut revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let mut templates = self
            .templates
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        let before = templates.len();
        templates.retain(|t| t.id != id);
        let removed = templates.len() != before;
        if removed {
            revisions.bump();
            drop(templates);
            self.persist_config(&revisions)?;
        }
        Ok(removed)
    }

    /// Assemble the pure generator input for `index_id` on `core`: the full
    /// profile set, inline custom/outbound contents, the stored template, and
    /// — since T11 — the real active routing profile, DNS rows, SimpleDNS,
    /// settings tree and routing mode.
    pub fn build_codegen_input(
        &self,
        index_id: &str,
        core: CoreType,
        opts: &crate::codegen::CodegenOptions,
    ) -> Result<config_codegen::input::CodegenInput, DomainError> {
        Ok(self
            .build_codegen_input_with_warnings(index_id, core, opts)?
            .0)
    }

    /// [`build_codegen_input`] plus the non-fatal diagnostics produced while
    /// wrapping the active node in its subscription-level `ProxyChain`
    /// (FIX-09C). Dangling `SubItem.PrevProfile`/`NextProfile` remarks are
    /// reported here and never fail the build. The stored `ProfileItem` rows
    /// are not modified; the chain exists only inside the returned input.
    pub fn build_codegen_input_with_warnings(
        &self,
        index_id: &str,
        core: CoreType,
        opts: &crate::codegen::CodegenOptions,
    ) -> Result<
        (
            config_codegen::input::CodegenInput,
            Vec<config_codegen::Diagnostic>,
        ),
        DomainError,
    > {
        let all = self.all_profiles_map_locked()?;
        let active = all
            .get(index_id)
            .ok_or_else(|| DomainError::not_found("profile", index_id))?
            .clone();
        let profiles: Vec<Profile> = all.values().cloned().collect();
        let mut outbound_contents = std::collections::BTreeMap::new();
        for profile in &profiles {
            if matches!(
                profile.config_type,
                ConfigType::Custom | ConfigType::Outbound
            ) {
                let text = crate::codegen::custom_config_text(profile)
                    .or_else(|| self.custom_file_text(profile));
                if let Some(text) = text {
                    outbound_contents.insert(profile.index_id.clone(), text);
                }
            }
        }
        let template = self
            .get_template_for_core(core)?
            .as_ref()
            .and_then(crate::codegen::template_for);
        // Active routing profile -> generator model (custom ruleset file is IO
        // done here; the generator only sees the parsed value).
        let routing = self.default_routing()?.map(|item| {
            let custom = if item.custom_ruleset_path4_singbox.trim().is_empty() {
                None
            } else {
                crate::routing::read_custom_ruleset(&item.custom_ruleset_path4_singbox)
            };
            crate::codegen::routing_to_codegen(&item, custom)
        });
        // DNS row for this core + SimpleDNS + read-only system hosts.
        let dns_row = self.get_dns_for_core(core).ok().flatten();
        let settings_snapshot = self
            .settings
            .lock()
            .map(|g| g.settings.clone())
            .unwrap_or_default();
        let system_hosts = domain::dns::read_system_hosts();
        let protect_domains: Vec<String> = profiles
            .iter()
            .filter_map(|p| {
                if is_domain_name(&p.address) {
                    Some(p.address.clone())
                } else {
                    None
                }
            })
            .collect();
        let dns = Some(crate::codegen::dns_to_codegen(
            dns_row.as_ref(),
            &settings_snapshot.simple_dns_item,
            system_hosts,
            protect_domains,
        ));
        let rule_mode = match self.rule_mode() {
            RuleMode::Global => Some("Global".to_string()),
            RuleMode::Direct => Some("Direct".to_string()),
            _ => None,
        };
        let mut input = crate::codegen::build_input_full(
            &active,
            &profiles,
            None,
            outbound_contents,
            template,
            opts,
            &settings_snapshot,
            routing,
            dns,
            rule_mode,
        );
        // FIX-16B: the SRS source really reaches generation. The stored
        // `SrsSourceUrl` (or the upstream built-in when unset) becomes the
        // `route.rule_set[].url` template emitted by the sing-box generator.
        input.settings.ruleset_url = Some(crate::dns::effective_srs_source(
            &settings_snapshot.const_item,
        ));
        // FIX-09C: upstream wraps a subscription node in a virtual `ProxyChain`
        // built from `SubItem.PrevProfile`/`NextProfile` remarks
        // (`CoreConfigContextBuilder.BuildSubscriptionChainNodeAsync`). The
        // synthesis stays generation-only and is never written back to storage.
        let mut warnings = Vec::new();
        if !active.subid.trim().is_empty() {
            let sub = self.get_sub_item(&active.subid)?;
            let (chain, chain_warnings) =
                crate::subs::build_subscription_chain_node(&active, &all, sub.as_ref());
            warnings = chain_warnings;
            if let Some(chain) = chain {
                let projected = crate::codegen::to_codegen_profile(&chain, None);
                input
                    .profiles
                    .insert(chain.index_id.clone(), projected.clone());
                input.profile = projected;
            }
        }
        Ok((input, warnings))
    }

    /// Base local port for the runtime codegen context.
    ///
    /// Upstream parity (`AppManager.GetLocalPort`): the base port is the
    /// `LocalPort` of the inbound row whose `Protocol` is `socks`
    /// (`FirstOrDefault(t => t.Protocol == "socks")`), falling back to the
    /// first row and then to `11808` (never the live `10808`).
    pub fn runtime_base_port(&self) -> i32 {
        let settings = self
            .settings
            .lock()
            .map(|guard| guard.settings.clone())
            .unwrap_or_default();
        Self::base_port_of(&settings)
    }

    fn base_port_of(settings: &AppSettings) -> i32 {
        settings
            .inbound
            .iter()
            .find(|item| item.protocol == domain::InboundProtocol::Socks)
            .or_else(|| settings.inbound.first())
            .map(|item| item.local_port)
            .unwrap_or(11808)
    }

    /// R3-03: which proxy cores emit a structured Xray/sing-box JSON config
    /// whose `inbounds` / statistics endpoints the runtime can resolve. Every
    /// other core receives its native config file passed through untouched
    /// (mihomo YAML, naive, tuic, mieru, ...).
    fn core_uses_json_endpoints(core: CoreType) -> bool {
        matches!(
            core,
            CoreType::Xray | CoreType::V2fly | CoreType::V2flyV5 | CoreType::SingBox
        )
    }

    /// SP-24 G-06: mihomo user-mixin file name (upstream
    /// `Global.ClashMixinConfigFileName`), resolved under `<data>/config/`.
    pub const MIHOMO_MIXIN_FILE_NAME: &str = "Mixin.yaml";

    /// SP-24 G-06: embedded mihomo TUN section, verbatim from upstream
    /// `ServiceLib/Sample/clash_tun_yaml` (`EmbedUtils.GetEmbedText` source).
    pub const MIHOMO_TUN_YAML: &str = "tun:\n  enable: true\n  stack: gvisor\n  dns-hijack:\n  - 0.0.0.0:53\n  auto-route: true\n  auto-detect-interface: true\n";

    fn mihomo_mixin_path(data_dir: Option<&Path>) -> Option<PathBuf> {
        data_dir.map(|dir| dir.join("config").join(Self::MIHOMO_MIXIN_FILE_NAME))
    }

    /// SP-24 G-06: read the user mixin text for one mihomo plan build (IO
    /// stays here; the generator only sees the text). `None` when
    /// `ClashUIItem.EnableMixinContent` is off (upstream `MixinContent` early
    /// return), when there is no data dir, or when the file is
    /// missing/unreadable/blank. Upstream logs a mixin failure and continues
    /// without the merge; a missing file likewise means "no mixin". Plan
    /// building stays side-effect free: unlike upstream startup, it never
    /// creates the file from the embedded default.
    fn read_mihomo_mixin_text(data_dir: Option<&Path>, mixin_enabled: bool) -> Option<String> {
        if !mixin_enabled {
            return None;
        }
        let path = Self::mihomo_mixin_path(data_dir)?;
        std::fs::read_to_string(path)
            .ok()
            .filter(|text| !text.trim().is_empty())
    }

    fn mihomo_mixin_text(&self, settings: &AppSettings) -> Option<String> {
        Self::read_mihomo_mixin_text(
            self.data_dir.as_deref(),
            settings.clash_ui_item.enable_mixin_content,
        )
    }

    /// SP-24 G-06: embedded TUN YAML text for one mihomo plan build, gated on
    /// `TunModeItem.EnableTun` (upstream `isTunEnabled` snapshot from
    /// `CoreConfigContextBuilder`).
    fn mihomo_tun_text(settings: &AppSettings) -> Option<&'static str> {
        settings
            .tun_mode_item
            .enable_tun
            .then_some(Self::MIHOMO_TUN_YAML)
    }

    /// The runtime codegen context derived from the persisted settings
    /// (`AppManager.GetLocalPort` + state-port offsets). No free-port probing
    /// here: the state ports are only emitted when the matching feature is on.
    pub fn runtime_codegen_options(&self) -> crate::codegen::CodegenOptions {
        let settings = self
            .settings
            .lock()
            .map(|guard| guard.settings.clone())
            .unwrap_or_default();
        let local_port = Self::base_port_of(&settings);
        let mut opts = crate::codegen::CodegenOptions {
            local_port,
            state_port: local_port + 4,
            state_port2: local_port + 5,
            speed_ping_test_url: settings.speed_test_item.speed_ping_test_url.clone(),
            ..Default::default()
        };
        if let Some(dir) = &self.data_dir {
            // The generated sing-box `cache.db` / Xray log paths resolve under
            // these directories; the core would fail to start when they are
            // missing (observed: `open bin/cache.db: ... cannot find the path`).
            let logs = dir.join("logs");
            let bin = dir.join("bin");
            let _ = std::fs::create_dir_all(&logs);
            let _ = std::fs::create_dir_all(&bin);
            opts.log_directory = logs.to_string_lossy().into_owned();
            opts.bin_directory = bin.to_string_lossy().into_owned();
        }
        opts
    }

    /// Resolve the core for a target profile (upstream `AppManager.GetCoreType`):
    /// explicit node core first, then the per-config-type `CoreTypeItem`
    /// binding, then a policy-group's first eligible child, else Xray.
    pub fn resolve_target_core(&self, profile: &Profile) -> Result<CoreType, DomainError> {
        if let Some(core) = profile.core_type {
            return Ok(core);
        }
        let settings = self
            .settings
            .lock()
            .map(|guard| guard.settings.clone())
            .unwrap_or_default();
        if let Some(bindings) = &settings.core_type_item {
            if let Some(binding) = bindings
                .iter()
                .find(|binding| binding.config_type == profile.config_type)
            {
                return Ok(binding.core_type);
            }
        }
        if crate::groups::is_group(profile.config_type) {
            let all = self.all_profiles_map_locked()?;
            if let Some(child) = crate::groups::resolve_children(profile, &all)
                .into_iter()
                .next()
            {
                if let Some(core) = child.core_type {
                    return Ok(core);
                }
            }
        }
        Ok(CoreType::Xray)
    }

    /// Pre-SOCKS sidecar decision mirroring upstream
    /// `ConfigHandler.GetPreSocksItem` (FLD-CFG-102 `EnableLegacyProtect`).
    ///
    /// - A non-`Custom` node on a non-sing-box core with TUN on and legacy
    ///   protect on needs a sing-box SOCKS sidecar on the runtime base port.
    /// - A `Custom` node with a valid `PreSocksPort` needs a SOCKS sidecar on
    ///   that port; its core is sing-box when legacy protect and TUN are both
    ///   on, otherwise the node's own resolved core.
    /// - Otherwise no sidecar is required (`None`).
    ///
    /// FIX-13 consumes this in `build_runtime_plan` to write the sidecar
    /// process node and start-order edge. RR-06 makes net-host execute the
    /// graph: the sidecar node is started in start order, the shared proxy port
    /// is awaited, then the main core starts; the sidecars stop in reverse
    /// order. The sidecar body is a real core config (a SOCKS listener).
    pub fn pre_socks_decision(&self, node: &Profile, core: CoreType) -> Option<PreSocksDecision> {
        let settings = self
            .settings
            .lock()
            .map(|guard| guard.settings.clone())
            .unwrap_or_default();
        Self::pre_socks_of(&settings, node, core)
    }

    fn pre_socks_of(
        settings: &AppSettings,
        node: &Profile,
        core: CoreType,
    ) -> Option<PreSocksDecision> {
        let tun = &settings.tun_mode_item;
        let legacy = tun.enable_legacy_protect;
        if node.config_type != ConfigType::Custom
            && core != CoreType::SingBox
            && tun.enable_tun
            && legacy
        {
            let port = Self::base_port_of(settings);
            if (1..=65535).contains(&port) {
                return Some(PreSocksDecision {
                    core: CoreType::SingBox,
                    address: "127.0.0.1".to_string(),
                    port: port as u16,
                });
            }
            return None;
        }
        if node.config_type == ConfigType::Custom {
            if let Some(port) = node.pre_socks_port {
                if (1..=65535).contains(&port) {
                    // Upstream `GetPreSocksItem` resolves the custom pre-core
                    // via `GetCoreType(null, EConfigType.Custom)` (the
                    // `CoreTypeItem` binding for `Custom`), not the node's own
                    // `CoreType`. With legacy protect + TUN the sidecar must be
                    // sing-box instead.
                    let sidecar_core = if legacy && tun.enable_tun {
                        CoreType::SingBox
                    } else {
                        Self::custom_default_core(settings)
                    };
                    return Some(PreSocksDecision {
                        core: sidecar_core,
                        address: "127.0.0.1".to_string(),
                        port: port as u16,
                    });
                }
            }
        }
        None
    }

    /// Upstream `AppManager.GetCoreType(null, EConfigType.Custom)`: the
    /// `CoreTypeItem` binding for the `Custom` config type, defaulting to Xray
    /// when unset (mirrors the frozen default run core).
    fn custom_default_core(settings: &AppSettings) -> CoreType {
        settings
            .core_type_item
            .as_ref()
            .and_then(|bindings| {
                bindings
                    .iter()
                    .find(|binding| binding.config_type == ConfigType::Custom)
            })
            .map(|binding| binding.core_type)
            .unwrap_or(CoreType::Xray)
    }

    /// Build the real immutable [`RuntimePlan`] for `target_id` from persisted
    /// state (T18-F03): the active node / expanded policy group, `AppSettings`,
    /// the active routing profile, the DNS row and the rule mode all flow into
    /// the pure generator. A missing target or a generator failure returns a
    /// structured error and never falls back to a hardcoded smoke config.
    pub fn build_runtime_plan(
        &self,
        target_id: &str,
        desired_revision: u64,
    ) -> Result<RuntimePlan, DomainError> {
        let hints = tun_plan::tun_hints_from_env();
        self.build_runtime_plan_with_hints(target_id, desired_revision, &hints)
    }

    /// Build the real immutable [`RuntimePlan`] with explicit TUN helper hints
    /// (adapter name + OS interface index). Used by isolated/dry-run runs and
    /// tests; the plain entry point reads the `V2RAYN_R_TUN_*` env hints.
    pub fn build_runtime_plan_with_hints(
        &self,
        target_id: &str,
        desired_revision: u64,
        tun_hints: &TunPlanHints,
    ) -> Result<RuntimePlan, DomainError> {
        let target = self
            .profile_by_id(target_id)?
            .ok_or_else(|| DomainError::not_found("profile", target_id))?;
        let core = self.resolve_target_core(&target)?;
        let opts = self.runtime_codegen_options();
        let (mut input, chain_warnings) =
            self.build_codegen_input_with_warnings(target_id, core, &opts)?;
        // TUN-A02: with the LegacyProtect topology the front sing-box service
        // owns the TUN device, so the main core must not generate a second tun
        // provider (upstream runs exactly one). Suppress the main config's tun
        // inbound only when a pre-SOCKS sidecar will carry it; a sing-box main
        // core without a sidecar keeps its own tun.
        let sidecar_owns_tun = {
            let settings = self
                .settings
                .lock()
                .map(|guard| guard.settings.clone())
                .unwrap_or_default();
            Self::pre_socks_of(&settings, &target, core).is_some()
        };
        if sidecar_owns_tun && input.settings.tun.enabled {
            input.settings.tun.enabled = false;
        }
        let mut generated = crate::codegen::generate(core, &input).map_err(|error| {
            DomainError::new(domain::codes::INVALID_PLAN, "error.codegen_failed")
                .with_detail(error.to_string())
        })?;
        generated.diagnostics.extend(chain_warnings);
        // R3-03: only Xray-family and sing-box `Custom` configs are structured
        // JSON whose proxy inbounds / statistics endpoints the runtime can
        // resolve. Every other proxy core receives its native config verbatim
        // (mihomo YAML, naive args file, mieru JSON delivered via
        // `MIERU_CONFIG_JSON_FILE`, ...): re-serialising it as JSON or parsing
        // it for `inbounds` would corrupt or reject it. Keep the raw text and
        // mark the endpoint state as native instead of failing.
        let native_custom =
            target.config_type == ConfigType::Custom && !Self::core_uses_json_endpoints(core);
        let body = if native_custom {
            let raw = input
                .profile
                .custom_config
                .clone()
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| serde_json::to_string(&generated.main).unwrap_or_default());
            // SP-24 G-06: a mihomo native custom plan merges the runtime
            // rewrites, the TUN section and the user mixin before it is
            // persisted (upstream
            // `CoreConfigClashService.GenerateClientCustomConfig`). Only the
            // mihomo path merges; every other native core keeps the verbatim
            // text. A merge failure (`FIELD_FORMAT`) propagates with `?`, so
            // the build fails before any plan exists and the caller keeps the
            // old plan: nothing below persists a body that failed to merge.
            if core == CoreType::Mihomo {
                let plan_settings = self
                    .settings
                    .lock()
                    .map(|guard| guard.settings.clone())
                    .unwrap_or_default();
                let mixin_text = self.mihomo_mixin_text(&plan_settings);
                let tun_text = Self::mihomo_tun_text(&plan_settings);
                crate::codegen::mihomo_body_for_plan(
                    &raw,
                    mixin_text.as_deref(),
                    tun_text,
                    &plan_settings,
                    &opts,
                )?
            } else {
                raw
            }
        } else {
            serde_json::to_string(&generated.main).map_err(|error| {
                DomainError::new(domain::codes::INTERNAL, "error.config_serialize_failed")
                    .with_detail(error.to_string())
            })?
        };

        // RR-07: a structured `Custom` config is emitted verbatim, so the plan
        // must wait on and publish the ports the config actually binds, not
        // `opts.local_port`. A native-format core exposes no parseable endpoint
        // here: the plan records that state (no `custom_endpoints`) rather than
        // rejecting the config.
        let custom_endpoints = if target.config_type == ConfigType::Custom && !native_custom {
            match runtime::parse_custom_endpoints(core, &body) {
                Ok(endpoints) => Some(endpoints),
                Err(detail) => {
                    return Err(DomainError::new(
                        domain::codes::INVALID_PLAN,
                        "error.custom_endpoint_parse_failed",
                    )
                    .with_field("customConfig")
                    .with_detail(detail));
                }
            }
        } else {
            None
        };

        let settings = self
            .settings
            .lock()
            .map(|guard| guard.settings.clone())
            .unwrap_or_default();
        let local_port = opts.local_port;
        if custom_endpoints.is_none() && !(1..=65535).contains(&local_port) {
            return Err(
                DomainError::new(domain::codes::FIELD_RANGE, "error.local_port_range")
                    .with_field("Inbound.LocalPort"),
            );
        }
        let port = local_port as u16;
        let config = ConfigSource::Inline { body: body.clone() };
        let config_sha256 = ContentHash::new(runtime::sha256_hex(body.as_bytes()));
        let mut ports = Vec::new();
        if let Some(endpoints) = &custom_endpoints {
            for resolved in &endpoints.inbounds {
                ports.push(PortRequest::tcp(resolved.port, "inbound"));
                if resolved.udp {
                    ports.push(PortRequest::udp(resolved.port, "inbound"));
                }
            }
            if let Some(api_port) = endpoints.api_port() {
                ports.push(PortRequest {
                    port: api_port,
                    transport: PortTransport::Tcp,
                    owner: "api".to_string(),
                    exclusive: false,
                });
            }
        } else {
            ports.push(PortRequest::tcp(port, "inbound"));
            let inbound = settings.inbound.first();
            if inbound.map(|item| item.udp_enabled).unwrap_or(true) {
                ports.push(PortRequest::udp(port, "inbound"));
            }
        }
        let privileges = vec![RequiredPrivilege::None];
        let mut graph = ProcessGraph::default();
        graph.add_process(ProcessNode {
            id: core.as_str().to_string(),
            core_type: core,
            config: config.clone(),
            ports: ports.clone(),
            privileges: privileges.clone(),
        });

        // FIX-13: pre-SOCKS / LegacyProtect sidecar topology (upstream
        // `CoreConfigContextBuilder` / `CoreManager`). Frozen order is main
        // core -> wait for proxy port -> front service, so the sidecar depends
        // on the main core. RR-06 writes the real process/port graph, a real
        // sidecar config body and the start order; net-host executes it.
        if let Some(decision) = Self::pre_socks_of(&settings, &target, core) {
            let sidecar_id = PRE_SOCKS_PROCESS_ID.to_string();
            // Upstream shares one user-facing SOCKS port between the main core
            // and the pre-service (the main core owns the listener; the
            // pre-service forwards/dials it). Record the port in the graph but
            // not as an exclusive claim, so the topology is visible without a
            // false hard conflict; net-host waits on it before the core.
            let sidecar_port = PortRequest {
                port: decision.port,
                transport: PortTransport::Tcp,
                owner: sidecar_id.clone(),
                exclusive: false,
            };
            // The sidecar body is a real core config (a SOCKS listener),
            // generated by the same frozen generators the main core uses, not a
            // plan descriptor. net-host executes this node from the graph.
            let sidecar_config = crate::codegen::generate_pre_socks_config(
                decision.core,
                &decision.address,
                decision.port,
                &opts,
                &settings,
                input.routing.clone(),
                input.dns.clone(),
            )
            .map_err(|error| {
                DomainError::new(
                    domain::codes::INVALID_PLAN,
                    "error.pre_socks_codegen_failed",
                )
                .with_detail(error.to_string())
            })?;
            let sidecar_body = serde_json::to_string(&sidecar_config.main).map_err(|error| {
                DomainError::new(domain::codes::INTERNAL, "error.config_serialize_failed")
                    .with_detail(error.to_string())
            })?;
            graph.add_process(ProcessNode {
                id: sidecar_id.clone(),
                core_type: decision.core,
                config: ConfigSource::Inline { body: sidecar_body },
                ports: vec![sidecar_port.clone()],
                privileges: vec![RequiredPrivilege::None],
            });
            graph.depends_on(&sidecar_id, core.as_str());
            ports.push(sidecar_port);
        }

        let mut plan = RuntimePlan {
            plan_id: format!("rt-{}-{}-{}", core.as_str(), target_id, desired_revision),
            desired_revision,
            target: RuntimeTarget {
                core_type: core,
                version: None,
                config,
                config_sha256,
            },
            process_graph: graph,
            outbound_graph: OutboundGraph::default(),
            ports,
            privileges,
            network_policy: NetworkPolicy {
                system_proxy: None,
                tun_enabled: false,
                bypass: Vec::new(),
            },
            resources: Vec::new(),
        };

        // FIX-13 / R3-04: attach the real TUN descriptor. With a known
        // interface the resolved descriptor goes in and net-host applies it
        // before starting the core. On a clean host the adapter does not exist
        // yet (the core's own tun inbound creates it): attach a *deferred*
        // descriptor instead of rejecting the plan, and let net-host discover
        // the interface after the core starts.
        if settings.tun_mode_item.enable_tun && tun_hints.interface_index == 0 {
            if let Some(spec) =
                tun_plan::tun_deferred_spec_from_settings(&settings.tun_mode_item, tun_hints)?
            {
                tun_plan::attach_deferred_tun_to_plan(&mut plan, &spec)?;
            }
        } else if let Some(spec) =
            tun_plan::tun_spec_from_settings(&settings.tun_mode_item, tun_hints)?
        {
            tun_plan::attach_tun_to_plan(&mut plan, &spec)?;
        }

        plan.validate()?;
        Ok(plan)
    }

    /// The JSON event payload for a refresh report.
    pub fn refresh_report_json(report: &SubUpdateReport) -> serde_json::Value {
        report_to_json(report)
    }
    /// Start the background subscription scheduler (idempotent).
    ///
    /// Starting it also starts the Geo resource task (R4-34), matching upstream
    /// `TaskManager` which drives both from one periodic loop. The existing
    /// subscription semantics are unchanged.
    pub fn start_sub_scheduler(&self, interval: std::time::Duration, max_items: usize) -> bool {
        {
            let mut guard = match self.sub_scheduler.lock() {
                Ok(guard) => guard,
                Err(_) => return false,
            };
            if guard.is_none() {
                *guard = Some(SubScheduler::start(self.clone(), interval, max_items));
            }
        }
        self.start_resource_scheduler(interval);
        true
    }

    /// Stop the schedulers gracefully. Never blocks process exit.
    pub fn stop_sub_scheduler(&self) {
        if let Ok(mut guard) = self.sub_scheduler.lock() {
            if let Some(scheduler) = guard.take() {
                scheduler.stop();
            }
        }
        if let Ok(mut guard) = self.resource_scheduler.lock() {
            if let Some(scheduler) = guard.take() {
                scheduler.stop();
            }
        }
    }

    pub fn sub_scheduler_running(&self) -> bool {
        self.sub_scheduler
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Start the Geo resource task (idempotent). Only a persisted engine with a
    /// data directory can download; an in-memory engine has nowhere to land.
    pub fn start_resource_scheduler(&self, tick: std::time::Duration) -> bool {
        let Some(bin_dir) = self.data_dir.as_ref().map(|dir| dir.join("bin")) else {
            return false;
        };
        match self.resource_scheduler.lock() {
            Ok(mut guard) => {
                if guard.is_some() {
                    return true;
                }
                *guard = Some(ResourceScheduler::start(self.clone(), bin_dir, tick));
                true
            }
            Err(_) => false,
        }
    }

    /// Stop the Geo resource task, waking the loop immediately.
    pub fn stop_resource_scheduler(&self) {
        if let Ok(mut guard) = self.resource_scheduler.lock() {
            if let Some(scheduler) = guard.take() {
                scheduler.stop();
            }
        }
    }

    pub fn resource_scheduler_running(&self) -> bool {
        self.resource_scheduler
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// One resource pass at a caller-supplied "hours since start" (upstream
    /// `TaskManager.UpdateTaskRunGeo`). The virtual clock is injected so the
    /// cadence is testable. Each file is staged to `<target>.part` and only
    /// renamed on success, so a failed download never clobbers the previous
    /// file and never reports success.
    pub async fn run_resource_pass(
        &self,
        bin_dir: &Path,
        now_hours: u64,
        cancellation: &CancellationToken,
    ) -> Result<ResourceUpdateReport, DomainError> {
        self.run_resource_pass_inner(bin_dir, now_hours, false, cancellation)
            .await
    }

    /// Force one Geo/SRS resource pass now (R4-34), ignoring the hourly
    /// cadence, and record the outcome for [`Self::last_resource_report`].
    /// Used by the manual "update resources now" entry point.
    pub async fn auto_update_now(&self) -> Result<ResourceUpdateReport, DomainError> {
        let bin_dir = self
            .data_dir
            .as_ref()
            .map(|dir| dir.join("bin"))
            .ok_or_else(|| {
                DomainError::new(domain::codes::UNAVAILABLE, "error.engine_not_persistent")
            })?;
        let cancellation = CancellationToken::new();
        self.run_resource_pass_inner(&bin_dir, 1, true, &cancellation)
            .await
    }

    /// The most recent resource pass outcome, or `None` when none ran yet.
    pub fn last_resource_report(&self) -> Option<ResourceUpdateReport> {
        self.last_resource_report
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    async fn run_resource_pass_inner(
        &self,
        bin_dir: &Path,
        now_hours: u64,
        force: bool,
        cancellation: &CancellationToken,
    ) -> Result<ResourceUpdateReport, DomainError> {
        self.guard_storage()?;
        let loaded = self.load_settings()?;
        let interval = loaded.settings.gui_item.auto_update_interval;
        if !force && (interval <= 0 || now_hours == 0 || !now_hours.is_multiple_of(interval as u64))
        {
            return Ok(ResourceUpdateReport::default());
        }
        let requests = build_resource_requests(&loaded.settings.const_item, bin_dir);
        let (via_proxy, proxy_url) = crate::subs::scheduler_proxy_choice(self.local_proxy_url());
        let options = DownloaderOptions {
            proxy: if via_proxy { proxy_url } else { None },
            ..DownloaderOptions::default()
        };
        let downloader = FileDownloader::new(options).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.resource_downloader")
                .with_detail(e.to_string())
        })?;
        let mut report = ResourceUpdateReport {
            due: true,
            attempted: requests.len(),
            ..Default::default()
        };
        for request in requests {
            if cancellation.is_cancelled() {
                break;
            }
            let staging = request.target.with_extension("part");
            let download = DownloadRequest::new(request.url.clone(), staging.clone());
            match downloader.download(&download, cancellation).await {
                Ok(done) => match std::fs::rename(&done.path, &request.target) {
                    Ok(()) => report
                        .downloaded
                        .push(request.target.to_string_lossy().into_owned()),
                    Err(error) => {
                        let _ = std::fs::remove_file(&staging);
                        report.failed.push(ResourceFailure {
                            url: request.url,
                            code: "error.resource_install".to_string(),
                            detail: error.to_string(),
                        });
                    }
                },
                Err(UpdateError::Cancelled) => {
                    let _ = std::fs::remove_file(&staging);
                    break;
                }
                Err(error) => {
                    let _ = std::fs::remove_file(&staging);
                    report.failed.push(ResourceFailure {
                        url: request.url,
                        code: "error.resource_download".to_string(),
                        detail: error.to_string(),
                    });
                }
            }
        }
        if let Ok(mut guard) = self.last_resource_report.lock() {
            *guard = Some(report.clone());
        }
        Ok(report)
    }

    /// Assemble the current snapshot.
    pub fn snapshot(&self) -> Result<Snapshot, DomainError> {
        self.guard_storage()?;
        let desired = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .desired();
        let runtime = self.runtime.snapshot()?;
        self.reconcile_applied_session(&runtime);
        let active = self.jobs.active();
        Ok(assemble(
            desired,
            &runtime,
            active,
            capability_table(),
            StartupRecovery {
                recovery_needed: false,
                stage: None,
                restored: 0,
                pending: 0,
            },
            self.profile_count(),
        ))
    }

    pub fn jobs(&self) -> &JobManager {
        &self.jobs
    }

    /// Test helper: mark the runtime as running at a revision.
    pub fn runtime_snapshot(&self) -> Result<RuntimeSnapshot, DomainError> {
        self.runtime.snapshot()
    }
}

impl Default for AppEngine {
    fn default() -> Self {
        Self::in_memory()
    }
}

fn lock_error() -> DomainError {
    DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned")
}

/// Split the compound `"<operation_id>:<job_id>"` correlation
/// [`AppEngine::apply_runtime`] returns (RUN-05). The runtime owns the
/// operation id and never sees the compound form; unknown shapes pass
/// through unsplit so a raw operation id keeps resolving.
fn split_operation_id(id: &str) -> (&str, Option<&str>) {
    match id.split_once(':') {
        Some((op, job)) if !op.is_empty() && !job.is_empty() => (op, Some(job)),
        _ => (id, None),
    }
}

/// Canonical `stable::OperationState` name for one [`domain::JobState`]
/// (SP-05: one mapped vocabulary, no parallel unmapped enums).
pub fn operation_state_name(state: domain::JobState) -> &'static str {
    stable_operation_name(state)
}

/// [`domain::JobState`] for one canonical `stable::OperationState` name, or
/// `None` when the name is not in the vocabulary (never guessed).
pub fn job_state_for_operation(name: &str) -> Option<domain::JobState> {
    job_state_from_stable_name(name)
}

/// Read `guiNConfig.json` from the data directory.
///
/// A missing file yields an empty object (first-run init, upstream
/// `LoadConfig` "not found" branch). A present-but-empty/whitespace file or
/// syntactically invalid JSON is a structured `error.config_corrupt` failure
/// (SP-01/CP-06): the caller fails closed and the source file is never
/// overwritten. Known-field type errors surface later in
/// [`read_settings_state`].
fn read_config(dir: &Path) -> Result<Value, DomainError> {
    let path = dir.join("guiNConfig.json");
    if !path.exists() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let text = std::fs::read_to_string(&path).map_err(storage_error)?;
    persistence::parse_config_text(&text).map_err(config_corrupt_error)
}

/// Map a `guiNConfig.json` text failure onto the stable corrupt-config
/// contract. The failure is retryable because a fresh read after an
/// out-of-band repair can succeed (re-reading is side-effect free); writes
/// stay blocked until a valid document loads.
fn config_corrupt_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::new(domain::codes::FIELD_FORMAT, "error.config_corrupt")
        .with_detail(error.to_string())
        .retryable()
}

/// Engine-owned keys that live next to the upstream `Config` tree. They are
/// stripped before parsing [`AppSettings`] so they never leak into `extra`.
pub const SETTINGS_META_KEYS: &[&str] = &[
    "desired_revision",
    "active_index_id",
    "applied_target",
    "runtime_intent_seq",
    "actual_generation",
    "dataset_epoch",
    "rule_mode",
    "full_config_templates",
    "settings_revision",
    "settings_group_revisions",
];

/// Engine-owned whole-dataset generation key in `guiNConfig.json` (plan
/// §3.1). Ordinary saves preserve it; a restore/import replacement advances
/// it so pre-restore requests stay rejected after reopen.
pub const DATASET_EPOCH_KEY: &str = "dataset_epoch";

/// Engine-owned frozen applied history in `guiNConfig.json` (SP-05 §3.1):
/// the submit-time target/plan/revision/operation/intent/generation record.
/// Stripped before settings parsing like every other engine-owned key.
pub const APPLIED_TARGET_KEY: &str = "applied_target";
/// Engine-owned submit sequence counter (plan §3.2 `intentSeq`).
pub const INTENT_SEQ_KEY: &str = "runtime_intent_seq";
/// Engine-owned actual generation counter (plan §3.1 `actualGeneration`).
pub const ACTUAL_GENERATION_KEY: &str = "actual_generation";

/// Canonical default resolution for one raw config tree: the engine mirror
/// `active_index_id` first (newest explicit choice), then the canonical
/// upstream `IndexId`. Blank values behave as absent.
fn canonical_active_from_config(config: &Value) -> Option<String> {
    let mirror = config
        .get("active_index_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let canonical = config
        .get("IndexId")
        .and_then(Value::as_str)
        .map(str::to_string);
    present_id(mirror.as_deref())
        .map(str::to_string)
        .or_else(|| present_id(canonical.as_deref()).map(str::to_string))
}

/// Persisted dataset epoch from one raw config tree; missing or malformed
/// values mean the pre-SP-03 generation zero.
fn dataset_epoch_from_config(config: &Value) -> DatasetEpoch {
    config
        .get(DATASET_EPOCH_KEY)
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// Parse the settings tree and its revision counters out of the raw config.
///
/// A known-field type error is a structured `error.config_corrupt` failure
/// (SP-01/CP-06), never a silent whole-tree default: the caller fails closed
/// so the damaged source file is preserved for recovery. Missing groups get
/// their `LoadConfig` defaults; engine-owned revision counters fall back to
/// zero when absent (they are regenerated, not user data).
fn read_settings_state(config: &Value) -> Result<SettingsState, DomainError> {
    let mut value = config.clone();
    if let Some(object) = value.as_object_mut() {
        for key in SETTINGS_META_KEYS {
            object.remove(*key);
        }
    }
    let mut settings = AppSettings::parse_strict(&value)?;
    settings.apply_load_defaults();
    let revision = config
        .get("settings_revision")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let group_revisions = config
        .get("settings_group_revisions")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    Ok(SettingsState {
        settings,
        revision,
        group_revisions,
    })
}

/// Atomically write `guiNConfig.json` (write temp + rename).
fn write_config(dir: &Path, value: &Value) -> Result<(), DomainError> {
    let path = dir.join("guiNConfig.json");
    let tmp = dir.join("guiNConfig.json.tmp");
    let text = serde_json::to_string_pretty(value).map_err(storage_error)?;
    std::fs::write(&tmp, text).map_err(storage_error)?;
    std::fs::rename(&tmp, &path).map_err(storage_error)?;
    Ok(())
}

/// Read the persisted full-config templates, seeding the two built-in rows
/// (Xray + sing-box) when the key is absent or empty.
fn read_templates(config: &Value) -> Vec<FullConfigTemplate> {
    let stored: Vec<FullConfigTemplate> = config
        .get("full_config_templates")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    if stored.is_empty() {
        crate::templates::builtins()
    } else {
        stored
    }
}

/// Static capability table derived from `compat/features.yaml`.
///
/// Values encode the T07/T08 support sets; only the two structured generators
/// are marked for structured generation in T02.
pub fn capability_table() -> Vec<CapabilityEntry> {
    use ConfigType::*;

    let xray_supported = vec![
        Vmess,
        Vless,
        Shadowsocks,
        Trojan,
        Hysteria2,
        WireGuard,
        Socks,
        Http,
    ];
    let singbox_supported = vec![
        Vmess,
        Vless,
        Shadowsocks,
        Trojan,
        Hysteria2,
        Tuic,
        Anytls,
        Naive,
        WireGuard,
        Socks,
        Http,
    ];
    vec![
        CapabilityEntry {
            core: CoreType::Xray,
            config_types: xray_supported,
            structured_generation: true,
            update_supported: true,
        },
        CapabilityEntry {
            core: CoreType::SingBox,
            config_types: singbox_supported,
            structured_generation: true,
            update_supported: true,
        },
        CapabilityEntry {
            core: CoreType::Mihomo,
            config_types: vec![Custom, Outbound],
            structured_generation: false,
            update_supported: true,
        },
        CapabilityEntry {
            core: CoreType::V2fly,
            config_types: vec![Vmess, Vless, Shadowsocks, Trojan, WireGuard, Socks, Http],
            structured_generation: false,
            update_supported: false,
        },
    ]
}

/// Convenience: an empty snapshot for bootstrapping.
pub fn empty_snapshot() -> Snapshot {
    let runtime = RuntimeSnapshot::default();
    assemble(
        DesiredRevision::ZERO,
        &runtime,
        Vec::new(),
        capability_table(),
        StartupRecovery {
            recovery_needed: false,
            stage: None,
            restored: 0,
            pending: 0,
        },
        0,
    )
}

/// Marker for the applied revision type used by tests.
pub type Applied = AppliedRevision;

/// Upstream `Utils.IsDomain` subset: non-IP, non-localhost, dotted hostname.
fn is_domain_name(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value == "localhost" || !value.contains('.') {
        return false;
    }
    if value.parse::<std::net::IpAddr>().is_ok() {
        return false;
    }
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Marker to keep `JobView` import used in public signatures.
pub type ActiveJob = JobView;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::synthetic_full_profile;
    use domain::*;

    // -- R4-34 resource task ------------------------------------------------

    /// Bind a loopback test port at `>= 11808` (never 10808).
    fn bind_test_listener() -> std::net::TcpListener {
        for port in 11808..11908u16 {
            if let Ok(listener) = std::net::TcpListener::bind(("127.0.0.1", port)) {
                return listener;
            }
        }
        panic!("no free test port >= 11808");
    }

    /// Serve `count` requests with a fixed body, then exit.
    fn serve_files(
        listener: std::net::TcpListener,
        count: usize,
        body: &'static str,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for _ in 0..count {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
        })
    }

    fn set_resource_sources(
        engine: &AppEngine,
        interval: i32,
        geo: Option<String>,
        srs: Option<String>,
    ) {
        let revision = engine.load_settings().unwrap().revision;
        let mut settings = engine.load_settings().unwrap().settings;
        settings.gui_item.auto_update_interval = interval;
        settings.const_item.geo_source_url = geo;
        settings.const_item.srs_source_url = srs;
        engine.save_settings(settings, revision).unwrap();
    }

    #[tokio::test]
    async fn resource_pass_skips_when_interval_disabled() {
        let engine = AppEngine::in_memory();
        let dir = tempfile::tempdir().unwrap();
        let report = engine
            .run_resource_pass(dir.path(), 1, &CancellationToken::new())
            .await
            .unwrap();
        assert!(!report.due);
        assert_eq!(report.attempted, 0);
    }

    #[tokio::test]
    async fn resource_pass_downloads_due_geo_from_local_endpoint() {
        let listener = bind_test_listener();
        let port = listener.local_addr().unwrap().port();
        let requests = 2 + DEFAULT_SRS_GEOSITE.len();
        let server = serve_files(listener, requests, "SYNTHETIC-GEO");

        let engine = AppEngine::in_memory();
        let dir = tempfile::tempdir().unwrap();
        set_resource_sources(
            &engine,
            1,
            Some(format!("http://127.0.0.1:{port}/{{0}}.dat")),
            Some(format!("http://127.0.0.1:{port}/rule-set/{{1}}.srs")),
        );

        let report = engine
            .run_resource_pass(dir.path(), 1, &CancellationToken::new())
            .await
            .unwrap();
        assert!(report.due, "now_hours=1 with interval=1 must fire");
        assert!(report.failed.is_empty(), "failures: {:?}", report.failed);
        assert_eq!(report.downloaded.len(), requests);
        server.join().unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("geoip.dat")).unwrap(),
            "SYNTHETIC-GEO"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("geosite.dat")).unwrap(),
            "SYNTHETIC-GEO"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("srss").join("geosite-google.srs")).unwrap(),
            "SYNTHETIC-GEO"
        );
    }

    #[tokio::test]
    async fn auto_update_now_forces_pass_and_records_status() {
        let listener = bind_test_listener();
        let port = listener.local_addr().unwrap().port();
        let requests = 2 + DEFAULT_SRS_GEOSITE.len();
        let server = serve_files(listener, requests, "SYNTHETIC-GEO");

        let dir = tempfile::tempdir().unwrap();
        let engine =
            AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).unwrap();
        assert!(engine.last_resource_report().is_none());
        // interval=0 would skip a scheduled pass; the manual "update now" must
        // still run (R4-34).
        set_resource_sources(
            &engine,
            0,
            Some(format!("http://127.0.0.1:{port}/{{0}}.dat")),
            Some(format!("http://127.0.0.1:{port}/rule-set/{{1}}.srs")),
        );
        let report = engine.auto_update_now().await.unwrap();
        assert!(report.due);
        assert!(report.ok(), "failures: {:?}", report.failed);
        assert_eq!(report.downloaded.len(), requests);
        server.join().unwrap();
        let last = engine
            .last_resource_report()
            .expect("status must record the last pass");
        assert_eq!(last.downloaded.len(), requests);
    }

    #[tokio::test]
    async fn resource_pass_not_due_on_non_multiple_hour() {
        let engine = AppEngine::in_memory();
        let dir = tempfile::tempdir().unwrap();
        set_resource_sources(
            &engine,
            3,
            Some("https://mirror.example/{0}.dat".to_string()),
            None,
        );
        let report = engine
            .run_resource_pass(dir.path(), 5, &CancellationToken::new())
            .await
            .unwrap();
        assert!(!report.due, "5 is not a multiple of 3");
    }

    #[tokio::test]
    async fn resource_pass_failure_is_structured_and_preserves_old_file() {
        let engine = AppEngine::in_memory();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("geoip.dat"), "OLD-GEO").unwrap();
        set_resource_sources(
            &engine,
            1,
            Some("http://127.0.0.1:9/{0}.dat".to_string()),
            Some("http://127.0.0.1:9/rule-set/{1}.srs".to_string()),
        );

        let report = engine
            .run_resource_pass(dir.path(), 1, &CancellationToken::new())
            .await
            .unwrap();
        assert!(report.due);
        assert!(!report.ok(), "a failed download must not report success");
        assert!(report.downloaded.is_empty());
        assert_eq!(report.failed.len(), report.attempted);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("geoip.dat")).unwrap(),
            "OLD-GEO",
            "a failed download must not clobber the previous file"
        );
        assert!(
            !dir.path().join("geoip.part").exists(),
            "staging file must be cleaned up"
        );
    }

    #[tokio::test]
    async fn resource_scheduler_stops_without_residue() {
        let engine = AppEngine::in_memory();
        let dir = tempfile::tempdir().unwrap();
        let scheduler =
            ResourceScheduler::start(engine, dir.path().to_path_buf(), Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(30));
        scheduler.stop();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !scheduler.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            scheduler.is_finished(),
            "resource scheduler must exit on stop"
        );
    }

    #[tokio::test]
    async fn starting_sub_scheduler_also_starts_resource_task() {
        let dir = tempfile::tempdir().unwrap();
        let engine = AppEngine::open(dir.path()).unwrap();
        assert!(!engine.resource_scheduler_running());
        engine.start_sub_scheduler(Duration::from_millis(5), 10);
        assert!(
            engine.resource_scheduler_running(),
            "the resource task must start with the subscription scheduler"
        );
        engine.stop_sub_scheduler();
        assert!(!engine.resource_scheduler_running());
        assert!(!engine.sub_scheduler_running());
    }

    fn tiny_plan() -> RuntimePlan {
        RuntimePlan {
            plan_id: "plan-test".into(),
            desired_revision: 0,
            target: RuntimeTarget {
                core_type: CoreType::Xray,
                version: Some("1.0.0".into()),
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new("00"),
            },
            process_graph: ProcessGraph::default(),
            outbound_graph: OutboundGraph::default(),
            ports: vec![],
            privileges: vec![],
            network_policy: NetworkPolicy::default(),
            resources: vec![],
        }
    }

    #[test]
    fn save_profile_rejects_stale_revision() {
        let engine = AppEngine::in_memory();
        let p = synthetic_full_profile(1);
        // First save at revision 0 succeeds and bumps to 1.
        engine
            .save_profile(p.clone(), DesiredRevision::ZERO)
            .unwrap();
        // Reusing the stale revision 0 is rejected.
        let err = engine.save_profile(p, DesiredRevision::ZERO).unwrap_err();
        assert_eq!(err.code, domain::codes::REVISION_STALE);
    }

    #[test]
    fn save_profile_validates_field() {
        let engine = AppEngine::in_memory();
        let mut p = synthetic_full_profile(1);
        p.port = 0;
        let err = engine.save_profile(p, DesiredRevision::ZERO).unwrap_err();
        assert_eq!(err.code, domain::codes::FIELD_RANGE);
        assert_eq!(err.field_path.as_deref(), Some("port"));
    }

    #[test]
    fn storage_unavailable_engine_fails_closed() {
        // R4-27 / D20: a production storage-open failure must not fall back to
        // an in-memory database or a NullRuntime fake `Accepted`.
        let error = DomainError::new("E_PERSIST_SQLITE", "error.persist_sqlite")
            .with_detail("database disk image is malformed")
            .retryable();
        let engine = AppEngine::storage_unavailable(
            Some(std::path::PathBuf::from("C:/nonexistent/r4_27")),
            error,
        );
        let code = "E_PERSIST_SQLITE";
        assert_eq!(
            engine.storage_failure().map(|e| e.code),
            Some(code.to_string())
        );

        assert_eq!(engine.snapshot().unwrap_err().code, code);
        assert_eq!(
            engine
                .query_profiles(
                    ProfileFilter::default(),
                    ProfileSort::IndexId,
                    PageRequest {
                        cursor: 0,
                        page_size: 10,
                    },
                )
                .unwrap_err()
                .code,
            code
        );
        // A null-runtime engine must never fake an accepted apply.
        assert_eq!(
            engine
                .apply_runtime(tiny_plan(), DesiredRevision::new(engine.desired_revision()),)
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(engine.set_active(None).unwrap_err().code, code);
        assert_eq!(engine.profile_ex_all().unwrap_err().code, code);
    }

    #[test]
    fn regional_preset_writes_upstream_sources_into_settings() {
        let engine = AppEngine::in_memory();
        let (pending, _) = engine
            .apply_regional_preset(crate::dns::RegionalPreset::RussiaOffline)
            .unwrap();
        let settings = engine.load_settings().unwrap().settings;
        let sources = crate::dns::region_sources(&crate::dns::RegionalPreset::RussiaOffline)
            .expect("russia sources");
        assert_eq!(
            settings.const_item.geo_source_url.as_deref(),
            Some(sources.geo_source)
        );
        assert_eq!(
            settings.const_item.srs_source_url.as_deref(),
            Some(sources.srs_source)
        );
        assert_eq!(
            settings
                .const_item
                .route_rules_template_source_url
                .as_deref(),
            Some(sources.routing_rules_source)
        );
        assert!(!pending.is_empty(), "remote DNS templates stay pending");

        // Default resets the three sources.
        engine
            .apply_regional_preset(crate::dns::RegionalPreset::Default)
            .unwrap();
        let settings = engine.load_settings().unwrap().settings;
        assert!(settings.const_item.geo_source_url.is_none());
        assert!(settings.const_item.srs_source_url.is_none());
        assert!(settings
            .const_item
            .route_rules_template_source_url
            .is_none());
    }

    #[test]
    fn build_codegen_input_uses_settings_srs_source() {
        let engine = AppEngine::in_memory();
        let profile = synthetic_full_profile(1);
        let id = profile.index_id.clone();
        engine.save_profile(profile, DesiredRevision::ZERO).unwrap();

        // Unset settings -> upstream built-in SRS template.
        let opts = engine.runtime_codegen_options();
        let input = engine
            .build_codegen_input(&id, CoreType::Xray, &opts)
            .unwrap();
        assert_eq!(
            input.settings.ruleset_url.as_deref(),
            Some(crate::dns::BUILTIN_SRS_URL)
        );

        // A regional preset sets the source; generation must emit that source.
        engine
            .apply_regional_preset(crate::dns::RegionalPreset::RussiaOffline)
            .unwrap();
        let stored = engine
            .load_settings()
            .unwrap()
            .settings
            .const_item
            .srs_source_url;
        let opts = engine.runtime_codegen_options();
        let input = engine
            .build_codegen_input(&id, CoreType::Xray, &opts)
            .unwrap();
        assert_eq!(input.settings.ruleset_url, stored);
    }

    #[test]
    fn apply_runtime_submits_plan_and_returns_operation() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let op = engine
            .apply_runtime(tiny_plan(), DesiredRevision::new(engine.desired_revision()))
            .unwrap();
        assert!(op.contains("plan-test"));
        assert!(runtime.last_plan().is_some());
    }

    #[test]
    fn apply_runtime_rejects_stale_revision() {
        let engine = AppEngine::in_memory();
        let err = engine
            .apply_runtime(tiny_plan(), DesiredRevision::new(9))
            .unwrap_err();
        assert_eq!(err.code, domain::codes::REVISION_STALE);
    }

    #[test]
    fn cancel_job_is_idempotent() {
        let engine = AppEngine::in_memory();
        let job = engine.jobs().start("x");
        assert_eq!(engine.cancel_job(&job.job_id), CancelOutcome::Requested);
        assert_eq!(engine.cancel_job(&job.job_id), CancelOutcome::Compensating);
    }

    #[test]
    fn snapshot_reports_saved_not_applied() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        engine
            .save_profile(synthetic_full_profile(1), DesiredRevision::ZERO)
            .unwrap();
        let snap = engine.snapshot().unwrap();
        assert_eq!(snap.revisions.desired, DesiredRevision::new(1));
        assert_eq!(snap.revisions.applied, AppliedRevision::ZERO);
        assert_eq!(snap.revision_state, RevisionState::Pending);
        assert_eq!(snap.profile_count, 1);
    }

    #[test]
    fn capability_table_matches_upstream_sets() {
        let table = capability_table();
        let xray = table.iter().find(|c| c.core == CoreType::Xray).unwrap();
        assert!(xray.config_types.contains(&ConfigType::WireGuard));
        assert!(!xray.config_types.contains(&ConfigType::Tuic));
        let sbox = table.iter().find(|c| c.core == CoreType::SingBox).unwrap();
        for t in [ConfigType::Tuic, ConfigType::Anytls, ConfigType::Naive] {
            assert!(sbox.config_types.contains(&t));
        }
    }

    #[test]
    fn runtime_marking_reconciles_snapshot() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        engine
            .save_profile(synthetic_full_profile(1), DesiredRevision::ZERO)
            .unwrap();
        runtime.mark_running(AppliedRevision::new(1));
        let snap = engine.snapshot().unwrap();
        assert_eq!(snap.revision_state, RevisionState::InSync);
        assert_eq!(snap.runtime_state, RuntimeState::Running);
    }

    #[test]
    fn active_set_is_idempotent_and_switches() {
        let engine = AppEngine::in_memory();
        let a = synthetic_full_profile(1);
        let b = synthetic_full_profile(2);
        engine.seed(vec![a.clone(), b.clone()]);
        engine.set_active(Some(a.index_id.clone())).unwrap();
        // Setting the same active node again must not clear or change it.
        engine.set_active(Some(a.index_id.clone())).unwrap();
        assert_eq!(engine.active_profile(), Some(a.index_id.clone()));
        // Setting a different node switches.
        engine.set_active(Some(b.index_id.clone())).unwrap();
        assert_eq!(engine.active_profile(), Some(b.index_id.clone()));
    }

    #[test]
    fn active_set_bumps_desired_only_when_the_id_changes() {
        let engine = AppEngine::in_memory();
        let a = synthetic_full_profile(1);
        let b = synthetic_full_profile(2);
        engine.seed(vec![a.clone(), b.clone()]);
        assert_eq!(engine.desired_revision(), 0);
        engine.set_active(Some(a.index_id.clone())).unwrap();
        assert_eq!(engine.desired_revision(), 1, "a new default bumps desired");
        // Re-selecting the same default is upstream-idempotent: no bump.
        engine.set_active(Some(a.index_id.clone())).unwrap();
        assert_eq!(engine.desired_revision(), 1);
        engine.set_active(Some(b.index_id.clone())).unwrap();
        assert_eq!(engine.desired_revision(), 2, "switching bumps desired");
    }

    #[test]
    fn active_set_persist_failure_does_not_leave_a_fake_active() {
        let dir = tempfile::tempdir().unwrap();
        let engine =
            AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).unwrap();
        let a = synthetic_full_profile(1);
        engine.seed(vec![a.clone()]);
        // Force the atomic config write to fail: the temp path is a directory.
        std::fs::create_dir(dir.path().join("guiNConfig.json.tmp")).unwrap();
        let error = engine.set_active(Some(a.index_id.clone())).unwrap_err();
        assert_eq!(error.code, domain::codes::INTERNAL);
        assert_eq!(error.message_key, "error.storage");
        assert_eq!(engine.active_profile(), None, "no fake in-memory active");
        assert_eq!(engine.desired_revision(), 0, "revision rolled back");
    }

    #[test]
    fn desired_active_does_not_publish_endpoint_until_running() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime);
        let p = synthetic_full_profile(1);
        engine.seed(vec![p.clone()]);
        engine.set_active(Some(p.index_id.clone())).unwrap();
        // Reconcile while stopped: a desired active node is not an endpoint.
        engine.snapshot().unwrap();
        assert!(engine.applied_session().is_none());
        assert!(engine.local_proxy_url().is_none());
    }

    #[test]
    fn failed_candidate_does_not_publish_endpoint() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let p = synthetic_full_profile(1);
        engine.seed(vec![p.clone()]);
        engine.set_active(Some(p.index_id.clone())).unwrap();
        engine
            .apply_runtime(tiny_plan(), DesiredRevision::new(engine.desired_revision()))
            .unwrap();
        // The apply failed: net-host reports Degraded with no usable port.
        runtime.set_state(RuntimeState::Degraded);
        engine.snapshot().unwrap();
        assert!(engine.applied_session().is_none());
        assert!(engine.local_proxy_url().is_none());
    }

    #[test]
    fn running_session_publishes_actual_endpoint_and_stop_withdraws() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let p = synthetic_full_profile(1);
        engine.seed(vec![p.clone()]);
        engine.set_active(Some(p.index_id.clone())).unwrap();
        engine
            .apply_runtime(tiny_plan(), DesiredRevision::new(engine.desired_revision()))
            .unwrap();
        runtime.mark_running_with("s-1", vec![11810], AppliedRevision::new(0));
        engine.snapshot().unwrap();

        let applied = engine.applied_session().expect("running session published");
        assert_eq!(applied.proxy_port, Some(11810));
        assert_eq!(applied.session_id.as_deref(), Some("s-1"));
        assert_eq!(applied.active_index_id, Some(p.index_id.clone()));
        assert_eq!(
            engine.local_proxy_url().as_deref(),
            Some("http://127.0.0.1:11810")
        );

        // Stopping the core withdraws the endpoint.
        engine.stop_runtime().unwrap();
        engine.snapshot().unwrap();
        assert!(engine.applied_session().is_none());
        assert!(engine.local_proxy_url().is_none());
    }

    #[test]
    fn monitor_session_consumes_applied_session_facts() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let p = synthetic_full_profile(1);
        engine.seed(vec![p.clone()]);
        engine.set_active(Some(p.index_id.clone())).unwrap();
        // Desired active but not applied: the monitor reports no session.
        engine.snapshot().unwrap();
        assert!(engine.monitor_session().is_none());

        engine
            .apply_runtime(tiny_plan(), DesiredRevision::new(engine.desired_revision()))
            .unwrap();
        runtime.mark_running_with("s-1", vec![11810], AppliedRevision::new(0));
        engine.snapshot().unwrap();
        let session = engine.monitor_session().expect("running session");
        assert_eq!(session.core, CoreType::Xray);
        assert_eq!(session.active_index_id, Some(p.index_id.clone()));
        assert_eq!(session.proxy_port, Some(11810));
        // Default inbound port 10808 -> Xray stats 10812 / Clash 10813.
        assert_eq!(session.state_port, 10812);
        assert_eq!(session.state_port2, 10813);

        // Stopping the core withdraws the monitor session too.
        engine.stop_runtime().unwrap();
        engine.snapshot().unwrap();
        assert!(engine.monitor_session().is_none());
    }

    #[test]
    fn custom_config_publishes_actual_api_port_and_socks_scheme() {
        // RR-07: a Custom Xray config's real metrics port reaches the monitor
        // session, and a SOCKS-only config yields a socks5 download URL.
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let mut custom = synthetic_full_profile(1);
        custom.config_type = ConfigType::Custom;
        custom.core_type = Some(CoreType::Xray);
        custom.address = "custom.json".into();
        custom.proto_extra.extra.insert(
            crate::codegen::CUSTOM_CONFIG_KEY.to_string(),
            serde_json::json!(
                r#"{"inbounds":[{"port":11950,"protocol":"socks"}],"metrics":{"listen":"127.0.0.1:11955"},"outbounds":[{"protocol":"freedom"}]}"#
            ),
        );
        engine.seed(vec![custom.clone()]);
        engine.set_active(Some(custom.index_id.clone())).unwrap();
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan(&custom.index_id, revision)
            .unwrap();
        engine
            .apply_runtime(plan, DesiredRevision::new(revision))
            .unwrap();
        runtime.mark_running_with("s-c", vec![11950], AppliedRevision::new(revision));
        engine.snapshot().unwrap();

        let session = engine.monitor_session().expect("running custom session");
        assert_eq!(session.state_port, 11955, "actual metrics port");
        assert_eq!(
            engine.local_proxy_url().as_deref(),
            Some("socks5://127.0.0.1:11950")
        );
    }

    #[test]
    fn custom_singbox_clash_secret_reaches_monitor_session() {
        // R3-07: a sing-box Custom config's Clash API port, listen address and
        // secret reach the monitor session through the normal applied-session
        // path (not only the dedicated `monitor_configure` hook). The secret is
        // returned as data; this test never logs it.
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let mut custom = synthetic_full_profile(1);
        custom.config_type = ConfigType::Custom;
        custom.core_type = Some(CoreType::SingBox);
        custom.address = "custom.json".into();
        custom.proto_extra.extra.insert(
            crate::codegen::CUSTOM_CONFIG_KEY.to_string(),
            serde_json::json!(
                r#"{"inbounds":[{"type":"mixed","listen":"127.0.0.1","listen_port":11960,"users":[{"username":"u","password":"p"}]}],"experimental":{"clash_api":{"external_controller":"127.0.0.1:11965","secret":"synthetic-secret"}},"outbounds":[]}"#
            ),
        );
        engine.seed(vec![custom.clone()]);
        engine.set_active(Some(custom.index_id.clone())).unwrap();
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan(&custom.index_id, revision)
            .unwrap();
        engine
            .apply_runtime(plan, DesiredRevision::new(revision))
            .unwrap();
        runtime.mark_running_with("s-c", vec![11960], AppliedRevision::new(revision));
        engine.snapshot().unwrap();

        let session = engine.monitor_session().expect("running sing-box custom");
        assert_eq!(session.core, CoreType::SingBox);
        assert_eq!(session.state_port, 0);
        assert_eq!(session.state_port2, 11965, "clash api port");
        assert_eq!(session.api_secret.as_deref(), Some("synthetic-secret"));
        assert_eq!(
            engine.local_proxy_url().as_deref(),
            Some("http://127.0.0.1:11960")
        );
    }

    #[test]
    fn custom_without_api_publishes_zero_stats_ports() {
        // R3-07: a Custom config that declares no statistics/API listener must
        // not fall back to the generic/default statistics port; the poller must
        // idle instead of probing an API the config does not expose.
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let mut custom = synthetic_full_profile(1);
        custom.config_type = ConfigType::Custom;
        custom.core_type = Some(CoreType::Xray);
        custom.address = "custom.json".into();
        custom.proto_extra.extra.insert(
            crate::codegen::CUSTOM_CONFIG_KEY.to_string(),
            serde_json::json!(r#"{"inbounds":[{"port":11970,"protocol":"socks"}],"outbounds":[]}"#),
        );
        engine.seed(vec![custom.clone()]);
        engine.set_active(Some(custom.index_id.clone())).unwrap();
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan(&custom.index_id, revision)
            .unwrap();
        engine
            .apply_runtime(plan, DesiredRevision::new(revision))
            .unwrap();
        runtime.mark_running_with("s-c", vec![11970], AppliedRevision::new(revision));
        engine.snapshot().unwrap();

        let session = engine.monitor_session().expect("running custom session");
        assert_eq!(session.state_port, 0, "no default statistics API");
        assert_eq!(session.state_port2, 0, "no default Clash API");
        assert!(session.api_secret.is_none());
    }

    #[test]
    fn pre_socks_legacy_sidecar_uses_base_port_and_real_socks_config() {
        // RR-06: a non-sing-box node under TUN + LegacyProtect gets a sing-box
        // pre-SOCKS sidecar on the runtime base port, and the graph node body
        // is a real SOCKS listener config (not a plan descriptor).
        let engine = AppEngine::in_memory();
        let mut node = synthetic_full_profile(1);
        node.core_type = Some(CoreType::Xray);
        engine.seed(vec![node.clone()]);
        let mut settings = engine.load_settings().unwrap().settings;
        settings.tun_mode_item.enable_tun = true;
        settings.tun_mode_item.enable_legacy_protect = true;
        if let Some(inbound) = settings.inbound.first_mut() {
            inbound.local_port = 11808;
        }
        engine
            .save_settings(settings, engine.load_settings().unwrap().revision)
            .unwrap();

        let decision = engine.pre_socks_decision(&node, CoreType::Xray).unwrap();
        assert_eq!(decision.core, CoreType::SingBox);
        let base = engine.runtime_base_port();
        assert_eq!(decision.port as i32, base);

        let revision = engine.desired_revision();
        // TUN is enabled but no real device exists: pass explicit hints so the
        // plan builds without touching the OS (the sidecar graph is the subject).
        let mut hints = tun_plan::TunPlanHints {
            interface_index: 9,
            ..Default::default()
        };
        hints.adapter_name = tun_plan::DEFAULT_TUN_ADAPTER.to_string();
        let plan = engine
            .build_runtime_plan_with_hints(&node.index_id, revision, &hints)
            .unwrap();
        let sidecar = plan
            .process_graph
            .nodes
            .iter()
            .find(|candidate| candidate.id == PRE_SOCKS_PROCESS_ID)
            .expect("pre-socks node");
        let body = match &sidecar.config {
            ConfigSource::Inline { body } => body.clone(),
            _ => panic!("inline sidecar config"),
        };
        assert!(
            body.contains("\"socks\""),
            "sidecar body is a real socks config: {body}"
        );
        assert!(!body.contains("presocks.plan.v1"));
        // TUN-A02: exactly one TUN provider. The sidecar carries the tun
        // inbound; the main Xray config must not declare a second one.
        let sidecar_json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(
            sidecar_json["inbounds"]
                .as_array()
                .expect("sidecar inbounds")
                .iter()
                .any(|entry| entry["type"] == "tun"),
            "sidecar owns the tun inbound"
        );
        let main_node = plan
            .process_graph
            .nodes
            .iter()
            .find(|candidate| candidate.id == CoreType::Xray.as_str())
            .expect("main core node");
        let main_body = match &main_node.config {
            ConfigSource::Inline { body } => body.clone(),
            _ => panic!("inline main config"),
        };
        let main_json: serde_json::Value = serde_json::from_str(&main_body).unwrap();
        let main_has_tun = main_json["inbounds"]
            .as_array()
            .map(|inbounds| {
                inbounds
                    .iter()
                    .any(|entry| entry["type"] == "tun" || entry["protocol"] == "tun")
            })
            .unwrap_or(false);
        assert!(
            !main_has_tun,
            "the main core must not declare a second tun provider: {main_body}"
        );
    }

    #[test]
    fn first_tun_without_interface_builds_a_deferred_plan() {
        // R3-04: on a clean host the adapter does not exist yet. The plan must
        // be built with a deferred descriptor, not rejected.
        let engine = AppEngine::in_memory();
        let node = synthetic_full_profile(1);
        engine.seed(vec![node.clone()]);
        let mut settings = engine.load_settings().unwrap().settings;
        settings.tun_mode_item.enable_tun = true;
        if let Some(inbound) = settings.inbound.first_mut() {
            inbound.local_port = 11808;
        }
        engine
            .save_settings(settings, engine.load_settings().unwrap().revision)
            .unwrap();

        let revision = engine.desired_revision();
        let hints = tun_plan::TunPlanHints {
            interface_index: 0,
            ..Default::default()
        };
        let plan = engine
            .build_runtime_plan_with_hints(&node.index_id, revision, &hints)
            .expect("first TUN without an interface must not be rejected");
        assert!(plan.network_policy.tun_enabled);
        assert!(plan
            .process_graph
            .nodes
            .iter()
            .any(|candidate| candidate.id == tun_plan::TUN_DEFERRED_PROCESS_ID));
        assert!(!plan
            .process_graph
            .nodes
            .iter()
            .any(|candidate| candidate.id == runtime::TUN_PROCESS_ID));
    }

    #[test]
    fn pre_socks_custom_default_core_uses_core_type_binding() {
        // RR-06: a Custom node's pre-SOCKS core mirrors upstream
        // `GetCoreType(null, EConfigType.Custom)` (the CoreTypeItem binding),
        // not the node's own CoreType.
        let engine = AppEngine::in_memory();
        let mut node = synthetic_full_profile(1);
        node.config_type = ConfigType::Custom;
        node.core_type = Some(CoreType::Xray);
        node.pre_socks_port = Some(11888);
        engine.seed(vec![node.clone()]);
        let mut settings = engine.load_settings().unwrap().settings;
        settings.tun_mode_item.enable_tun = false;
        settings.tun_mode_item.enable_legacy_protect = false;
        settings.core_type_item = Some(vec![domain::entities::CoreTypeBinding::new(
            ConfigType::Custom,
            CoreType::Mihomo,
        )]);
        engine
            .save_settings(settings, engine.load_settings().unwrap().revision)
            .unwrap();

        let decision = engine.pre_socks_decision(&node, CoreType::Xray).unwrap();
        assert_eq!(decision.core, CoreType::Mihomo);
        assert_eq!(decision.port, 11888);
    }

    #[test]
    fn pre_socks_custom_falls_back_to_xray_without_binding() {
        let engine = AppEngine::in_memory();
        let mut node = synthetic_full_profile(1);
        node.config_type = ConfigType::Custom;
        node.core_type = Some(CoreType::Xray);
        node.pre_socks_port = Some(11888);
        let mut settings = engine.load_settings().unwrap().settings;
        settings.tun_mode_item.enable_tun = false;
        settings.tun_mode_item.enable_legacy_protect = false;
        settings.core_type_item = None;
        engine
            .save_settings(settings, engine.load_settings().unwrap().revision)
            .unwrap();
        let decision = engine.pre_socks_decision(&node, CoreType::Xray).unwrap();
        assert_eq!(decision.core, CoreType::Xray);
    }

    #[test]
    fn applied_session_reports_apply_target_not_later_selection() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let a = synthetic_full_profile(1);
        let b = synthetic_full_profile(2);
        engine.seed(vec![a.clone(), b.clone()]);
        engine.set_active(Some(a.index_id.clone())).unwrap();
        engine
            .apply_runtime(tiny_plan(), DesiredRevision::new(engine.desired_revision()))
            .unwrap();
        runtime.mark_running_with("s-1", vec![11811], AppliedRevision::new(0));
        engine.snapshot().unwrap();

        // Desired active moves to B, but the running config is still A's.
        engine.set_active(Some(b.index_id.clone())).unwrap();
        let applied = engine.applied_session().unwrap();
        assert_eq!(applied.active_index_id, Some(a.index_id.clone()));
        assert_eq!(applied.proxy_port, Some(11811));
    }

    #[test]
    fn applied_inbound_facts_report_protocol_and_port_only_when_running() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let p = synthetic_full_profile(1);
        engine.seed(vec![p.clone()]);
        engine.set_active(Some(p.index_id.clone())).unwrap();
        // Nothing applied yet: no fabricated protocol/port.
        assert_eq!(engine.applied_inbound_protocol(), None);
        assert_eq!(engine.applied_inbound_port(), None);
        engine
            .apply_runtime(tiny_plan(), DesiredRevision::new(engine.desired_revision()))
            .unwrap();
        runtime.mark_running_with("s-inbound", vec![11813], AppliedRevision::new(0));
        engine.snapshot().unwrap();
        assert_eq!(engine.applied_inbound_protocol().as_deref(), Some("mixed"));
        assert_eq!(engine.applied_inbound_port(), Some(11813));
        // A stop withdraws both facts.
        runtime.set_state(RuntimeState::Stopped);
        engine.snapshot().unwrap();
        assert_eq!(engine.applied_inbound_protocol(), None);
        assert_eq!(engine.applied_inbound_port(), None);
    }

    #[test]
    fn busy_restart_keeps_previous_applied_endpoint() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let p = synthetic_full_profile(1);
        engine.seed(vec![p.clone()]);
        engine.set_active(Some(p.index_id.clone())).unwrap();
        engine
            .apply_runtime(tiny_plan(), DesiredRevision::new(engine.desired_revision()))
            .unwrap();
        runtime.mark_running_with("s-old", vec![11812], AppliedRevision::new(0));
        engine.snapshot().unwrap();
        assert_eq!(engine.applied_session().unwrap().proxy_port, Some(11812));
        // An in-place restart is busy, not stopped: keep the proven endpoint
        // until the new session is actually running.
        runtime.set_state(RuntimeState::Starting);
        engine.snapshot().unwrap();
        assert_eq!(engine.applied_session().unwrap().proxy_port, Some(11812));
    }

    #[test]
    fn settings_round_trip_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let engine =
            AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).unwrap();
        let loaded = engine.load_settings().unwrap();
        assert_eq!(loaded.revision, 0);
        let mut settings = loaded.settings;
        settings.gui_item.enable_statistics = true;
        settings.core_basic_item.loglevel = Some("debug".to_string());
        let outcome = engine.save_settings(settings, 0).unwrap();
        assert_eq!(outcome.new_revision, 1);
        assert!(outcome
            .restart_app_fields
            .contains(&"GuiItem.EnableStatistics".to_string()));
        assert!(outcome
            .restart_core_fields
            .contains(&"CoreBasicItem.Loglevel".to_string()));

        let reopened =
            AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).unwrap();
        let loaded2 = reopened.load_settings().unwrap();
        assert_eq!(loaded2.revision, 1);
        assert!(loaded2.settings.gui_item.enable_statistics);
        assert_eq!(
            loaded2.settings.core_basic_item.loglevel.as_deref(),
            Some("debug")
        );
    }

    #[test]
    fn settings_save_rejects_stale_and_keeps_old_value() {
        let engine = AppEngine::in_memory();
        let mut settings = engine.load_settings().unwrap().settings;
        settings.ui_item.double_click2_activate = true;
        engine.save_settings(settings.clone(), 0).unwrap();
        let err = engine.save_settings(settings, 0).unwrap_err();
        assert_eq!(err.code, domain::codes::REVISION_STALE);
        assert_eq!(engine.settings_revision(), 1);
        assert!(
            engine
                .load_settings()
                .unwrap()
                .settings
                .ui_item
                .double_click2_activate
        );
    }

    #[test]
    fn settings_invalid_port_does_not_change_old_value() {
        let engine = AppEngine::in_memory();
        let mut settings = engine.load_settings().unwrap().settings;
        settings.inbound[0].local_port = 0;
        let err = engine.save_settings(settings, 0).unwrap_err();
        assert_eq!(err.code, domain::codes::FIELD_RANGE);
        assert_eq!(engine.settings_revision(), 0);
        assert_eq!(
            engine.load_settings().unwrap().settings.inbound[0].local_port,
            10808
        );
    }

    #[test]
    fn settings_group_save_is_scoped_and_revision_checked() {
        let engine = AppEngine::in_memory();
        let before = engine.load_settings().unwrap();
        let group_rev = before.group_revisions.get("GuiItem").copied().unwrap_or(0);
        let outcome = engine
            .save_settings_group(
                "GuiItem",
                serde_json::json!({"EnableStatistics": true}),
                group_rev,
            )
            .unwrap();
        assert!(outcome
            .restart_app_fields
            .contains(&"GuiItem.EnableStatistics".to_string()));
        let after = engine.load_settings().unwrap();
        assert!(after.settings.gui_item.enable_statistics);
        assert_eq!(
            after.settings.core_basic_item,
            before.settings.core_basic_item
        );
        assert_eq!(
            after.group_revisions.get("GuiItem").copied().unwrap_or(0),
            group_rev + 1
        );

        let stale = engine
            .save_settings_group(
                "GuiItem",
                serde_json::json!({"EnableStatistics": false}),
                group_rev,
            )
            .unwrap_err();
        assert_eq!(stale.code, domain::codes::REVISION_STALE);
        assert!(
            engine
                .load_settings()
                .unwrap()
                .settings
                .gui_item
                .enable_statistics
        );
    }

    #[test]
    fn delete_sub_items_removes_orphan_nodes() {
        use crate::synthetic::synthetic_full_profile;
        let engine = AppEngine::in_memory();
        let item = crate::subs::SubItem {
            id: "s-del".into(),
            remarks: "del".into(),
            url: "https://example.com/s".into(),
            ..Default::default()
        };
        engine.subs.lock().unwrap().upsert(item).unwrap();
        let mut seed = vec![synthetic_full_profile(1), synthetic_full_profile(2)];
        for p in &mut seed {
            p.subid = "s-del".to_string();
        }
        let mut other = vec![synthetic_full_profile(3)];
        for p in &mut other {
            p.subid = "s-keep".to_string();
        }
        engine.replace_sub_profiles("s-del", seed, true).unwrap();
        engine.replace_sub_profiles("s-keep", other, true).unwrap();
        assert_eq!(engine.profiles_by_subid("s-del").unwrap().len(), 2);
        let removed = engine.delete_sub_items(&["s-del".to_string()]).unwrap();
        assert_eq!(removed, 1);
        assert!(engine.profiles_by_subid("s-del").unwrap().is_empty());
        // Unrelated subscriptions are untouched.
        assert_eq!(engine.profiles_by_subid("s-keep").unwrap().len(), 1);
    }

    #[test]
    fn replace_sub_profiles_failure_keeps_old_nodes() {
        use crate::synthetic::synthetic_full_profile;
        let engine = AppEngine::in_memory();
        let mut seed = vec![synthetic_full_profile(1), synthetic_full_profile(2)];
        for p in &mut seed {
            p.subid = "s-tx".to_string();
        }
        let (added, _) = engine.replace_sub_profiles("s-tx", seed, true).unwrap();
        assert_eq!(added, 2);
        // Inject a mid-replace failure after the deletes; the old set must
        // survive intact.
        let mut replacement = vec![synthetic_full_profile(3)];
        for p in &mut replacement {
            p.subid = "s-tx".to_string();
        }
        {
            let mut repo = engine.repo.lock().unwrap();
            let err = repo
                .replace_for_sub("s-tx", replacement, true, true)
                .unwrap_err();
            assert_eq!(err.code, domain::codes::INTERNAL);
        }
        let kept = engine.profiles_by_subid("s-tx").unwrap();
        assert_eq!(kept.len(), 2);
        // A successful replace still works after the aborted attempt.
        let mut next = vec![synthetic_full_profile(4)];
        for p in &mut next {
            p.subid = "s-tx".to_string();
        }
        let (added, removed) = engine.replace_sub_profiles("s-tx", next, true).unwrap();
        assert_eq!((added, removed), (1, 2));
    }

    #[test]
    fn settings_unlinked_actions_report_not_restart() {
        // A UI-only change must not be reported as needing a restart.
        let engine = AppEngine::in_memory();
        let mut settings = engine.load_settings().unwrap().settings;
        settings.ui_item.current_theme = Some("Dark".to_string());
        settings.ui_item.double_click2_activate = true;
        let outcome = engine.save_settings(settings, 0).unwrap();
        assert!(outcome.restart_core_fields.is_empty());
        assert!(outcome.restart_app_fields.is_empty());
        assert!(outcome.next_launch_fields.is_empty());
    }

    fn chain_leaf(index: u32, remarks: &str) -> Profile {
        let mut profile = synthetic_full_profile(index);
        profile.config_type = ConfigType::Vless;
        profile.password = "11111111-2222-3333-4444-555555555555".into();
        profile.remarks = remarks.to_string();
        profile
    }

    fn chain_test_opts() -> crate::codegen::CodegenOptions {
        crate::codegen::CodegenOptions {
            local_port: 11818,
            state_port: 11819,
            state_port2: 11820,
            ..Default::default()
        }
    }

    fn seed_chain_subscription(
        engine: &AppEngine,
        with_prev: bool,
        with_next: bool,
    ) -> (Profile, Profile, Profile) {
        let prev = chain_leaf(2, "chain-prev");
        let active = chain_leaf(1, "chain-active");
        let next = chain_leaf(3, "chain-next");
        engine
            .save_sub_item(SubItem {
                id: "s-chain".into(),
                remarks: "chain-sub".into(),
                url: "https://example.com/sub".into(),
                prev_profile: with_prev.then(|| "chain-prev".to_string()),
                next_profile: with_next.then(|| "chain-next".to_string()),
                ..Default::default()
            })
            .unwrap();
        engine
            .replace_sub_profiles(
                "s-chain",
                vec![prev.clone(), active.clone(), next.clone()],
                true,
            )
            .unwrap();
        (prev, active, next)
    }

    #[test]
    fn subscription_chain_synthesizes_prev_active_next() {
        let engine = AppEngine::in_memory();
        let (prev, active, next) = seed_chain_subscription(&engine, true, true);
        let opts = chain_test_opts();
        let (input, warnings) = engine
            .build_codegen_input_with_warnings(&active.index_id, CoreType::Xray, &opts)
            .unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            input.profile.config_type,
            config_codegen::input::ConfigType::ProxyChain
        );
        assert!(input.profile.index_id.starts_with("inner-"));
        assert_eq!(input.profile.remarks, "chain-active");
        assert_eq!(
            input.profile.proto_extra.group_type.as_deref(),
            Some("ProxyChain")
        );
        let expected = format!("{},{},{}", prev.index_id, active.index_id, next.index_id);
        assert_eq!(
            input.profile.proto_extra.child_items.as_deref(),
            Some(expected.as_str())
        );
        assert!(input.profiles.contains_key(&input.profile.index_id));

        // Generated Xray config: reverse order, `proxy` entry dials the next hop.
        let generated = crate::codegen::generate(CoreType::Xray, &input).unwrap();
        let outbounds = generated.main["outbounds"].as_array().unwrap();
        assert_eq!(outbounds[0]["tag"], serde_json::json!("proxy"));
        assert_eq!(
            outbounds[0]["settings"]["address"],
            serde_json::json!(next.address)
        );
        assert!(outbounds[0]["streamSettings"]["sockopt"]
            .get("dialerProxy")
            .is_some());

        // Generated sing-box config: same reverse chain via `detour`.
        let sbox = crate::codegen::generate(CoreType::SingBox, &input).unwrap();
        let sbox_out = sbox.main["outbounds"].as_array().unwrap();
        assert_eq!(sbox_out[0]["tag"], serde_json::json!("proxy"));
        assert!(sbox_out[0].get("detour").is_some());

        // Stored rows stay untouched: no ProfileItem rewrite at generation time.
        let stored = engine.profile_by_id(&active.index_id).unwrap().unwrap();
        assert_eq!(stored.config_type, ConfigType::Vless);
        assert!(stored.proto_extra.child_items.is_none());
        assert_eq!(stored.remarks, "chain-active");
    }

    #[test]
    fn subscription_chain_dangling_reference_warns_and_falls_back() {
        let engine = AppEngine::in_memory();
        let (_, active, next) = seed_chain_subscription(&engine, true, true);
        let mut sub = engine.get_sub_item("s-chain").unwrap().unwrap();
        sub.prev_profile = Some("missing-prev".into());
        engine.save_sub_item(sub).unwrap();

        let opts = chain_test_opts();
        let (input, warnings) = engine
            .build_codegen_input_with_warnings(&active.index_id, CoreType::Xray, &opts)
            .unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].code,
            "error.subscription_prev_profile_not_found"
        );
        assert_eq!(
            warnings[0].field_path.as_deref(),
            Some("SubItem.PrevProfile")
        );
        // The node and the still-resolvable next node keep chaining.
        assert_eq!(
            input.profile.config_type,
            config_codegen::input::ConfigType::ProxyChain
        );
        let expected = format!("{},{}", active.index_id, next.index_id);
        assert_eq!(
            input.profile.proto_extra.child_items.as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn subscription_chain_missing_both_does_not_wrap() {
        let engine = AppEngine::in_memory();
        let (_, active, _) = seed_chain_subscription(&engine, false, false);
        let opts = chain_test_opts();
        let (input, warnings) = engine
            .build_codegen_input_with_warnings(&active.index_id, CoreType::Xray, &opts)
            .unwrap();
        assert!(warnings.is_empty());
        assert_eq!(
            input.profile.config_type,
            config_codegen::input::ConfigType::Vless
        );
    }

    #[test]
    fn subscription_chain_excludes_custom_and_unsubscribed_nodes() {
        let engine = AppEngine::in_memory();
        let (_, active, _) = seed_chain_subscription(&engine, true, true);
        let opts = chain_test_opts();

        // No subscription id -> no synthesis.
        let mut plain = active.clone();
        plain.subid.clear();
        engine.seed(vec![plain.clone()]);
        let (input, warnings) = engine
            .build_codegen_input_with_warnings(&plain.index_id, CoreType::Xray, &opts)
            .unwrap();
        assert!(warnings.is_empty());
        assert_eq!(
            input.profile.config_type,
            config_codegen::input::ConfigType::Vless
        );

        // `Custom` is excluded even when it carries a subscription id.
        let mut custom = active.clone();
        custom.config_type = ConfigType::Custom;
        engine.seed(vec![custom.clone()]);
        let (input, warnings) = engine
            .build_codegen_input_with_warnings(&custom.index_id, CoreType::Xray, &opts)
            .unwrap();
        assert!(warnings.is_empty());
        assert_eq!(
            input.profile.config_type,
            config_codegen::input::ConfigType::Custom
        );
    }

    #[test]
    fn replace_sub_profiles_rejects_stale_restore_epoch() {
        let engine = AppEngine::in_memory();
        engine
            .save_sub_item(SubItem {
                id: "s1".into(),
                remarks: "s1".into(),
                url: "https://example.invalid/sub".into(),
                ..SubItem::default()
            })
            .unwrap();
        let epoch = engine.restore_epoch();
        let candidate = synthetic_full_profile(1);
        // A restore bumps the epoch; the stale task's commit is rejected.
        engine.bump_restore_epoch();
        let err = engine
            .replace_sub_profiles_at_epoch("s1", vec![candidate.clone()], true, epoch)
            .unwrap_err();
        assert_eq!(err.code, domain::codes::CONFLICT);
        assert!(engine.profiles_by_subid("s1").unwrap().is_empty());
        // The current epoch commits normally.
        let current = engine.restore_epoch();
        let (added, _) = engine
            .replace_sub_profiles_at_epoch("s1", vec![candidate], true, current)
            .unwrap();
        assert_eq!(added, 1);
        assert_eq!(engine.profiles_by_subid("s1").unwrap().len(), 1);
    }

    #[test]
    fn cancel_and_drain_sub_tasks_times_out_on_stuck_job() {
        let engine = AppEngine::in_memory();
        let job = engine.jobs().start("update_subscription");
        let err = engine
            .cancel_and_drain_sub_tasks(std::time::Duration::from_millis(40))
            .unwrap_err();
        assert_eq!(err.code, domain::codes::UNAVAILABLE);
        // Once the worker reports terminal, the drain succeeds.
        engine.jobs().finish(&job.job_id, JobState::Cancelled, None);
        assert_eq!(
            engine
                .cancel_and_drain_sub_tasks(std::time::Duration::from_millis(40))
                .unwrap(),
            0
        );
    }

    #[test]
    fn prepare_restore_bumps_epoch_before_exchange() {
        let engine = AppEngine::in_memory();
        let before = engine.restore_epoch();
        engine.prepare_restore().unwrap();
        assert_eq!(engine.restore_epoch(), before + 1);
    }

    // -- SP-24 G-06 mihomo native-plan wiring -------------------------------

    const SP24_G06_BASE_YAML: &str =
        "port: 7890\nmode: direct\nsecret: hunter2\nrules:\n  - DOMAIN,example.com,DIRECT\n";

    fn sp24_g06_mihomo_node(yaml: &str) -> Profile {
        let mut node = synthetic_full_profile(7);
        node.config_type = ConfigType::Custom;
        node.core_type = Some(CoreType::Mihomo);
        node.address = "mihomo-custom.yaml".into();
        node.proto_extra.extra.insert(
            crate::codegen::CUSTOM_CONFIG_KEY.to_string(),
            serde_json::json!(yaml),
        );
        node
    }

    fn sp24_g06_plan_body(plan: &domain::runtime_plan::RuntimePlan) -> String {
        match &plan.target.config {
            ConfigSource::Inline { body } => body.clone(),
            other => panic!("expected inline plan body, got {other:?}"),
        }
    }

    fn sp24_g06_settings(engine: &AppEngine, tun: bool, mixin: bool, ipv6: bool) {
        let revision = engine.load_settings().unwrap().revision;
        let mut settings = engine.load_settings().unwrap().settings;
        settings.tun_mode_item.enable_tun = tun;
        settings.clash_ui_item.enable_mixin_content = mixin;
        settings.clash_ui_item.enable_ipv6 = ipv6;
        if let Some(inbound) = settings.inbound.first_mut() {
            inbound.local_port = 11808;
        }
        engine.save_settings(settings, revision).unwrap();
    }

    fn sp24_g06_hints() -> tun_plan::TunPlanHints {
        tun_plan::TunPlanHints {
            interface_index: 0,
            ..Default::default()
        }
    }

    #[test]
    fn sp24_g06_mixin_file_name_matches_upstream() {
        // Upstream `Global.ClashMixinConfigFileName` is `Mixin.yaml` under the
        // config dir; the engine resolves the same name.
        assert_eq!(AppEngine::MIHOMO_MIXIN_FILE_NAME, "Mixin.yaml");
    }

    #[test]
    fn sp24_g06_read_mixin_text_gated_by_switch_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("Mixin.yaml"), "mode: rule\n").unwrap();
        assert_eq!(
            AppEngine::read_mihomo_mixin_text(Some(dir.path()), true).as_deref(),
            Some("mode: rule\n")
        );
        assert!(AppEngine::read_mihomo_mixin_text(Some(dir.path()), false).is_none());
        assert!(AppEngine::read_mihomo_mixin_text(None, true).is_none());
        let empty = tempfile::tempdir().unwrap();
        assert!(AppEngine::read_mihomo_mixin_text(Some(empty.path()), true).is_none());
    }

    #[test]
    fn sp24_g06_mihomo_native_plan_merges_rewrites_and_tun() {
        // G-06: a Mihomo native custom plan passes through
        // `mihomo_body_for_plan` before it is persisted: runtime rewrites,
        // the embedded TUN section (TunModeItem on) and ipv6 land in the body.
        let engine = AppEngine::in_memory();
        let node = sp24_g06_mihomo_node(SP24_G06_BASE_YAML);
        engine.seed(vec![node.clone()]);
        sp24_g06_settings(&engine, true, false, true);
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan_with_hints(&node.index_id, revision, &sp24_g06_hints())
            .unwrap();
        let body = sp24_g06_plan_body(&plan);
        assert!(
            body.contains("mixed-port: 11808"),
            "runtime rewrite: {body}"
        );
        assert!(body.contains("ipv6: true"), "ipv6 switch: {body}");
        assert!(!body.contains("hunter2"), "secret removed: {body}");
        assert!(!body.contains("10808"), "never the live proxy port: {body}");
        assert!(
            body.contains("auto-route: true"),
            "embedded TUN section: {body}"
        );
    }

    #[test]
    fn sp24_g06_mihomo_native_plan_tun_gate_off_skips_tun_section() {
        let engine = AppEngine::in_memory();
        let node = sp24_g06_mihomo_node(SP24_G06_BASE_YAML);
        engine.seed(vec![node.clone()]);
        sp24_g06_settings(&engine, false, false, false);
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan_with_hints(&node.index_id, revision, &sp24_g06_hints())
            .unwrap();
        let body = sp24_g06_plan_body(&plan);
        assert!(body.contains("ipv6: false"), "ipv6 rewritten off: {body}");
        assert!(
            !body.contains("auto-route"),
            "no TUN section when disabled: {body}"
        );
    }

    #[test]
    fn sp24_g06_mihomo_merge_failure_keeps_old_plan() {
        // G-06: a bad base YAML is FIELD_FORMAT and the build fails before any
        // plan exists, so the caller keeps the old plan (no half-written body
        // is persisted).
        let engine = AppEngine::in_memory();
        let node = sp24_g06_mihomo_node("- just\n- a\n- list\n");
        engine.seed(vec![node.clone()]);
        sp24_g06_settings(&engine, false, false, false);
        let revision = engine.desired_revision();
        let err = engine
            .build_runtime_plan_with_hints(&node.index_id, revision, &sp24_g06_hints())
            .unwrap_err();
        assert_eq!(err.code, domain::codes::FIELD_FORMAT);
        assert!(engine.applied_session().is_none(), "no plan applied");
        assert_eq!(engine.desired_revision(), revision, "revision untouched");
    }

    #[test]
    fn sp24_g06_non_mihomo_native_custom_stays_verbatim() {
        // G-06 only touches the Mihomo native path: any other native core
        // (here naiveproxy) keeps its verbatim text even with TUN/switches on.
        let engine = AppEngine::in_memory();
        let mut node = synthetic_full_profile(7);
        node.config_type = ConfigType::Custom;
        node.core_type = Some(CoreType::NaiveProxy);
        node.address = "naive-custom.txt".into();
        node.proto_extra.extra.insert(
            crate::codegen::CUSTOM_CONFIG_KEY.to_string(),
            serde_json::json!(SP24_G06_BASE_YAML),
        );
        engine.seed(vec![node.clone()]);
        sp24_g06_settings(&engine, true, true, true);
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan_with_hints(&node.index_id, revision, &sp24_g06_hints())
            .unwrap();
        assert_eq!(sp24_g06_plan_body(&plan), SP24_G06_BASE_YAML);
    }

    #[test]
    fn sp24_g06_mihomo_native_plan_merges_mixin_file() {
        // G-06 end to end: synthetic mixin text from `<data>/config/Mixin.yaml`
        // merges into the Mihomo native plan when EnableMixinContent is on;
        // with the switch off the same file is ignored.
        let dir = tempfile::tempdir().unwrap();
        let engine =
            AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new())).unwrap();
        let node = sp24_g06_mihomo_node(SP24_G06_BASE_YAML);
        engine.seed(vec![node.clone()]);
        sp24_g06_settings(&engine, false, true, false);
        let config = dir.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join(AppEngine::MIHOMO_MIXIN_FILE_NAME),
            "unknown-kept: 42\nappend-rules:\n  - MATCH,DIRECT\n",
        )
        .unwrap();
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan_with_hints(&node.index_id, revision, &sp24_g06_hints())
            .unwrap();
        let body = sp24_g06_plan_body(&plan);
        assert!(
            body.contains("unknown-kept"),
            "unknown keys retained: {body}"
        );
        assert!(body.contains("MATCH,DIRECT"), "mixin list merge: {body}");

        sp24_g06_settings(&engine, false, false, false);
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan_with_hints(&node.index_id, revision, &sp24_g06_hints())
            .unwrap();
        let body = sp24_g06_plan_body(&plan);
        assert!(
            !body.contains("unknown-kept"),
            "gate off skips merge: {body}"
        );
    }
}
