//! In-memory AppEngine skeleton.
//!
//! Wires the repository, revision store, job manager and runtime client into
//! the T02 use cases the FRB layer exposes. No database, process or network
//! access: T03 replaces [`NullRuntimeClient`] with the net-host client and T04
//! replaces [`InMemoryProfileRepository`] with SQLite.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use domain::runtime_plan::{
    ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, PortRequest, PortTransport,
    ProcessGraph, ProcessNode, RequiredPrivilege, RuntimePlan, RuntimeTarget,
};
use domain::{
    AppSettings, AppliedRevision, CancelOutcome, CancellationToken, ConfigType, CoreType,
    DesiredRevision, DnsProfile, DomainError, FullConfigTemplate, JobId, Profile, RoutingProfile,
    RuleMode, RuntimeState,
};
use serde_json::Value;

use crate::jobs::{JobManager, JobView};
use crate::net_host_client::NetHostClient;
use crate::repository::{
    InMemoryProfileRepository, InMemorySubRepository, PageRequest, ProfileFilter, ProfilePage,
    ProfileRepository, ProfileSort, RevisionStore, SubRepository,
};
use crate::runtime_client::{
    AppliedSession, ApplyOutcome, EventSink, NullRuntimeClient, RuntimeClient, RuntimeSnapshot,
};
use crate::settings::{
    apply_group_patch, normalize_for_save, validate_settings, LoadedSettings, SaveSettingsOutcome,
    SettingsState,
};
use crate::snapshot::{assemble, CapabilityEntry, Snapshot, StartupRecovery};
use persistence::Store;

use crate::dns::{DnsRepository, InMemoryDnsRepository};
use crate::routing::{InMemoryRoutingRepository, RoutingRepository};
use crate::store_repo::{
    storage_error, DnsStore, ProfileStore, RoutingStore, SqliteDnsRepository,
    SqliteProfileRepository, SqliteRoutingRepository, SqliteSubRepository, SqliteTrafficStore,
    SubStore,
};
use crate::subs::{
    build_candidates, download_all, new_sub_id, report_to_json, sub_error_outcome, unix_now,
    SubItem, SubScheduler, SubUpdateEntry, SubUpdateOutcome, SubUpdateReport, SubUpdateRequest,
};
use crate::tun_plan::{self, TunPlanHints};

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
}

/// Core + statistics/API ports and proxy scheme captured when an apply was
/// accepted. For a full Custom config the values come from the emitted JSON
/// (RR-07); otherwise they are the generated-config expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AppliedFacts {
    core: CoreType,
    state_port: u16,
    state_port2: u16,
    scheme: runtime::ProxyProtocol,
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
    /// The local socks/mixed port of the running session, when known.
    local_proxy_port: Arc<Mutex<Option<u16>>>,
    /// The applied-session fact (FIX-07): published only while net-host
    /// reports a `Running` session, withdrawn on stop/failure.
    applied_session: Arc<Mutex<Option<AppliedSession>>>,
    /// The active node id captured when an apply was accepted, so the applied
    /// session reports the node the running config was built for, not the
    /// current desired selection.
    apply_target: Arc<Mutex<Option<String>>>,
    /// Core + statistics/API ports captured when an apply was accepted, so the
    /// monitor pipeline polls the running core instead of re-deriving facts
    /// from a desired (possibly changed) plan.
    apply_facts: Arc<Mutex<Option<AppliedFacts>>>,
}

impl AppEngine {
    /// Build an engine with a Null runtime client (T02 default, used by tests).
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
            local_proxy_port: Arc::new(Mutex::new(None)),
            applied_session: Arc::new(Mutex::new(None)),
            apply_target: Arc::new(Mutex::new(None)),
            apply_facts: Arc::new(Mutex::new(None)),
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
        let active = config
            .get("active_index_id")
            .and_then(Value::as_str)
            .map(str::to_string);

        let sub_store =
            SqliteSubRepository::from_store(Store::open(&db_path).map_err(storage_error)?);
        let routing_store =
            SqliteRoutingRepository::from_store(Store::open(&db_path).map_err(storage_error)?);
        let dns_store =
            SqliteDnsRepository::from_store(Store::open(&db_path).map_err(storage_error)?);
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
            settings: Arc::new(Mutex::new(read_settings_state(&config))),
            data_dir: Some(data_dir),
            jobs: JobManager::new(),
            runtime,
            sub_scheduler: Arc::new(Mutex::new(None)),
            local_proxy_port: Arc::new(Mutex::new(None)),
            applied_session: Arc::new(Mutex::new(None)),
            apply_target: Arc::new(Mutex::new(None)),
            apply_facts: Arc::new(Mutex::new(None)),
        };
        engine.ensure_builtin_routing_dns();
        Ok(engine)
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
        let subs = SqliteSubRepository::from_store(Store::open(&db_path).map_err(storage_error)?);
        let routing =
            SqliteRoutingRepository::from_store(Store::open(&db_path).map_err(storage_error)?);
        let dns = SqliteDnsRepository::from_store(Store::open(&db_path).map_err(storage_error)?);
        let config = read_config(&dir)?;
        let desired = config
            .get("desired_revision")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let active = config
            .get("active_index_id")
            .and_then(Value::as_str)
            .map(str::to_string);
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
        *self.settings.lock().map_err(|_| lock_error())? = read_settings_state(&config);
        self.ensure_builtin_routing_dns();
        Ok(())
    }

    /// Stop the subscription scheduler and wait (bounded) for an in-flight
    /// tick to finish, so no scheduler pass can write to the database while a
    /// restore/import exchanges the live files. `None` when no scheduler runs.
    pub fn stop_sub_scheduler_blocking(&self, timeout: std::time::Duration) {
        let scheduler = self
            .sub_scheduler
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(scheduler) = scheduler else {
            return;
        };
        scheduler.stop();
        let deadline = std::time::Instant::now() + timeout;
        while !scheduler.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// SR-03 restore lifecycle, storage half: stop the managed runtime session
    /// and the subscription scheduler, then quiesce the SQLite handles so a
    /// restore/import can exchange the live files without a competing session
    /// or timer.
    ///
    /// The scheduler is stopped and drained unconditionally. A runtime that
    /// reports a live session is stopped through the idempotent boundary and a
    /// failure is returned *before* any file is touched; an idle or unreachable
    /// runtime is left alone so a backup-only session can still restore.
    /// Quiesce failures are propagated for the same reason.
    pub fn prepare_restore(&self) -> Result<(), DomainError> {
        self.stop_sub_scheduler_blocking(std::time::Duration::from_millis(2000));
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
        self.repo
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .query(&filter, sort, page)
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
            // (RE-PROF-07).
            draft = crate::custom::normalize_server(draft);
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
            // on this path.
            draft = crate::custom::normalize_server(draft);
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
                    .map_err(storage_error)?;
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
                .map_err(storage_error)?;
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
    pub fn set_active(&self, id: Option<String>) -> Result<(), DomainError> {
        let revisions = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        if let Some(target) = &id {
            let repo = self
                .repo
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            if repo.get(target)?.is_none() {
                return Err(DomainError::not_found("profile", target));
            }
        }
        *self
            .active
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))? = id;
        self.persist_config(&revisions)?;
        Ok(())
    }

    /// Currently active profile id, if any.
    pub fn active_profile(&self) -> Option<String> {
        self.active.lock().ok().and_then(|guard| guard.clone())
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
    pub fn save_settings(
        &self,
        settings: AppSettings,
        expected_revision: u64,
    ) -> Result<SaveSettingsOutcome, DomainError> {
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
        let next = normalize_for_save(settings);
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
    pub fn save_settings_group(
        &self,
        group: &str,
        patch: Value,
        expected_revision: u64,
    ) -> Result<SaveSettingsOutcome, DomainError> {
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
        let next = apply_group_patch(&guard.settings, group, patch)?;
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

    /// `apply_runtime` use case.
    ///
    /// Checks `expected_revision`, submits the immutable plan to the runtime
    /// client and returns an operation id. Results flow on the event stream.
    pub fn apply_runtime(
        &self,
        plan: RuntimePlan,
        expected_revision: DesiredRevision,
    ) -> Result<String, DomainError> {
        {
            let revisions = self
                .revisions
                .lock()
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            revisions.check(expected_revision)?;
        }
        plan.validate()?;

        match self.runtime.apply(&plan)? {
            ApplyOutcome::Accepted { operation_id } => {
                // Remember which node this apply targets so the applied-session
                // fact reports it even if the desired selection changes later.
                if let Ok(mut guard) = self.apply_target.lock() {
                    *guard = self.active_profile();
                }
                // Record the core + statistics/API ports of the accepted plan
                // so the monitor pipeline can poll the applied session without
                // re-deriving facts from a later desired plan.
                let facts = self.applied_facts_for(&plan);
                if let Ok(mut guard) = self.apply_facts.lock() {
                    *guard = Some(facts);
                }
                let job = self.jobs.start("apply_runtime");
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

    /// `stop_runtime` use case: ask net-host to stop the managed core.
    pub fn stop_runtime(&self) -> Result<(), DomainError> {
        self.runtime.stop()
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

    /// Reconcile the applied-session fact from a fresh runtime snapshot.
    ///
    /// Only a `Running` session with a bound port publishes an endpoint; a
    /// stopped/degraded/rolling-back runtime withdraws it. Busy states keep the
    /// previous fact so an in-place restart does not drop the old endpoint
    /// before the new one is proven.
    fn reconcile_applied_session(&self, snapshot: &RuntimeSnapshot) {
        match snapshot.state {
            RuntimeState::Running => {
                let Some(port) = snapshot.ports.first().copied() else {
                    return;
                };
                let active = self
                    .apply_target
                    .lock()
                    .ok()
                    .and_then(|guard| guard.clone())
                    .or_else(|| self.active_profile());
                if let Ok(mut guard) = self.applied_session.lock() {
                    *guard = Some(AppliedSession {
                        session_id: snapshot.session_id.clone(),
                        active_index_id: active,
                        proxy_port: Some(port),
                        applied_revision: snapshot.applied_revision,
                    });
                }
                self.set_local_proxy_port(Some(port));
            }
            RuntimeState::Stopped | RuntimeState::Degraded | RuntimeState::RollingBack => {
                if let Ok(mut guard) = self.applied_session.lock() {
                    *guard = None;
                }
                self.set_local_proxy_port(None);
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
            .and_then(|guard| *guard)
            .map(|facts| facts.scheme.scheme())
            .unwrap_or("http");
        port.map(|port| format!("{scheme}://127.0.0.1:{port}"))
    }

    /// Resolve the monitor/API facts for an accepted plan. A full Custom config
    /// carries its real statistics/API port in the emitted JSON (RR-07).
    fn applied_facts_for(&self, plan: &RuntimePlan) -> AppliedFacts {
        let opts = self.runtime_codegen_options();
        let core = plan.target.core_type;
        let mut facts = AppliedFacts {
            core,
            state_port: opts.state_port.clamp(0, u16::MAX as i32) as u16,
            state_port2: opts.state_port2.clamp(0, u16::MAX as i32) as u16,
            scheme: runtime::ProxyProtocol::Mixed,
        };
        let is_custom = self
            .active_profile()
            .and_then(|id| self.profile_by_id(&id).ok().flatten())
            .map(|profile| profile.config_type == ConfigType::Custom)
            .unwrap_or(false);
        if is_custom {
            if let ConfigSource::Inline { body } = &plan.target.config {
                if let Ok(endpoints) = runtime::parse_custom_endpoints(core, body) {
                    if core == CoreType::Xray {
                        if let Some(api) = endpoints.api_port {
                            facts.state_port = api;
                        }
                    } else if let Some(api) = endpoints.api_port {
                        facts.state_port2 = api;
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
            .and_then(|guard| *guard)
            .unwrap_or_else(|| {
                let opts = self.runtime_codegen_options();
                AppliedFacts {
                    core: CoreType::Xray,
                    state_port: opts.state_port.clamp(0, u16::MAX as i32) as u16,
                    state_port2: opts.state_port2.clamp(0, u16::MAX as i32) as u16,
                    scheme: runtime::ProxyProtocol::Mixed,
                }
            });
        Some(MonitorSession {
            core: facts.core,
            active_index_id: applied.active_index_id,
            proxy_port: applied.proxy_port,
            state_port: facts.state_port,
            state_port2: facts.state_port2,
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
                self.refresh_one(&item, &request, cancellation, max_items)
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
        match self.replace_sub_profiles(&item.id, candidates, true) {
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
    /// process node and start-order edge; net-host still does not *execute* the
    /// second core (FIX-13B), so only the plan topology is emitted today.
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
                    let sidecar_core = if legacy && tun.enable_tun {
                        CoreType::SingBox
                    } else {
                        node.core_type.unwrap_or(CoreType::Xray)
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
        let (input, chain_warnings) =
            self.build_codegen_input_with_warnings(target_id, core, &opts)?;
        let mut generated = crate::codegen::generate(core, &input).map_err(|error| {
            DomainError::new(domain::codes::INVALID_PLAN, "error.codegen_failed")
                .with_detail(error.to_string())
        })?;
        generated.diagnostics.extend(chain_warnings);
        let body = serde_json::to_string(&generated.main).map_err(|error| {
            DomainError::new(domain::codes::INTERNAL, "error.config_serialize_failed")
                .with_detail(error.to_string())
        })?;

        // RR-07: a full `Custom` config is emitted verbatim, so the plan must
        // wait on and publish the ports the config actually binds, not
        // `opts.local_port`. A config with no usable inbound is a hard error:
        // never pretend the core is ready.
        let custom_endpoints = if target.config_type == ConfigType::Custom {
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
            if let Some(api_port) = endpoints.api_port {
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
        // on the main core. Executing the sidecar is FIX-13B; this card writes
        // the real process/port graph and start order into the plan.
        if let Some(decision) = Self::pre_socks_of(&settings, &target, core) {
            let sidecar_id = PRE_SOCKS_PROCESS_ID.to_string();
            // Upstream shares one user-facing SOCKS port between the main core
            // and the pre-service (the main core owns the listener; the
            // pre-service forwards/dials it). Record the port in the graph but
            // not as an exclusive claim, so the topology is visible without a
            // false hard conflict; FIX-13B owns the actual listener handoff.
            let sidecar_port = PortRequest {
                port: decision.port,
                transport: PortTransport::Tcp,
                owner: sidecar_id.clone(),
                exclusive: false,
            };
            graph.add_process(ProcessNode {
                id: sidecar_id.clone(),
                core_type: decision.core,
                config: ConfigSource::Inline {
                    body: format!(
                        "{{\"kind\":\"v2rayn.presocks.plan.v1\",\"address\":\"{}\",\"port\":{}}}",
                        decision.address, decision.port
                    ),
                },
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

        // FIX-13: attach the real TUN descriptor only when one can be built.
        // `tun_spec_from_settings` returns `Ok(None)` when TUN is off and a
        // structured error when it is on but the interface hints are missing;
        // a plan never claims `tun_enabled` without a matching `tun` node.
        if let Some(spec) = tun_plan::tun_spec_from_settings(&settings.tun_mode_item, tun_hints)? {
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
    pub fn start_sub_scheduler(&self, interval: std::time::Duration, max_items: usize) -> bool {
        let mut guard = match self.sub_scheduler.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        if guard.is_some() {
            return true;
        }
        *guard = Some(SubScheduler::start(self.clone(), interval, max_items));
        true
    }

    /// Stop the scheduler gracefully. Never blocks process exit.
    pub fn stop_sub_scheduler(&self) {
        if let Ok(mut guard) = self.sub_scheduler.lock() {
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

    /// Assemble the current snapshot.
    pub fn snapshot(&self) -> Result<Snapshot, DomainError> {
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

/// Read `guiNConfig.json` from the data directory (empty object when absent).
fn read_config(dir: &Path) -> Result<Value, DomainError> {
    let path = dir.join("guiNConfig.json");
    if !path.exists() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let text = std::fs::read_to_string(&path).map_err(storage_error)?;
    if text.trim().is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    serde_json::from_str(&text).map_err(storage_error)
}

/// Engine-owned keys that live next to the upstream `Config` tree. They are
/// stripped before parsing [`AppSettings`] so they never leak into `extra`.
pub const SETTINGS_META_KEYS: &[&str] = &[
    "desired_revision",
    "active_index_id",
    "rule_mode",
    "full_config_templates",
    "settings_revision",
    "settings_group_revisions",
];

/// Parse the settings tree and its revision counters out of the raw config.
fn read_settings_state(config: &Value) -> SettingsState {
    let mut value = config.clone();
    if let Some(object) = value.as_object_mut() {
        for key in SETTINGS_META_KEYS {
            object.remove(*key);
        }
    }
    let mut settings: AppSettings = serde_json::from_value(value).unwrap_or_default();
    settings.apply_load_defaults();
    let revision = config
        .get("settings_revision")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let group_revisions = config
        .get("settings_group_revisions")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    SettingsState {
        settings,
        revision,
        group_revisions,
    }
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
            .apply_runtime(tiny_plan(), DesiredRevision::ZERO)
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
            .apply_runtime(tiny_plan(), DesiredRevision::ZERO)
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
            .apply_runtime(tiny_plan(), DesiredRevision::ZERO)
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
            .apply_runtime(tiny_plan(), DesiredRevision::ZERO)
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
    fn applied_session_reports_apply_target_not_later_selection() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let a = synthetic_full_profile(1);
        let b = synthetic_full_profile(2);
        engine.seed(vec![a.clone(), b.clone()]);
        engine.set_active(Some(a.index_id.clone())).unwrap();
        engine
            .apply_runtime(tiny_plan(), DesiredRevision::ZERO)
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
    fn busy_restart_keeps_previous_applied_endpoint() {
        let runtime = std::sync::Arc::new(NullRuntimeClient::new());
        let engine = AppEngine::with_runtime(runtime.clone());
        let p = synthetic_full_profile(1);
        engine.seed(vec![p.clone()]);
        engine.set_active(Some(p.index_id.clone())).unwrap();
        engine
            .apply_runtime(tiny_plan(), DesiredRevision::ZERO)
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
}
