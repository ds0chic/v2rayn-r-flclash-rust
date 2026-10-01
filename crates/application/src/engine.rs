//! In-memory AppEngine skeleton.
//!
//! Wires the repository, revision store, job manager and runtime client into
//! the T02 use cases the FRB layer exposes. No database, process or network
//! access: T03 replaces [`NullRuntimeClient`] with the net-host client and T04
//! replaces [`InMemoryProfileRepository`] with SQLite.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use domain::{
    AppliedRevision, CancelOutcome, ConfigType, CoreType, DesiredRevision, DomainError, JobId,
    Profile, RuntimePlan,
};
use serde_json::Value;

use crate::jobs::{JobManager, JobView};
use crate::net_host_client::NetHostClient;
use crate::repository::{
    InMemoryProfileRepository, PageRequest, ProfileFilter, ProfilePage, ProfileRepository,
    ProfileSort, RevisionStore,
};
use crate::runtime_client::{
    ApplyOutcome, EventSink, NullRuntimeClient, RuntimeClient, RuntimeSnapshot,
};
use crate::snapshot::{assemble, CapabilityEntry, Snapshot, StartupRecovery};
use crate::store_repo::{storage_error, ProfileStore, SqliteProfileRepository};

/// Application data directory override.
pub const DATA_DIR_ENV: &str = "V2RAYN_R_DATA_DIR";

/// Shared engine handle. Cloning shares all state.
#[derive(Clone)]
pub struct AppEngine {
    repo: Arc<Mutex<ProfileStore>>,
    revisions: Arc<Mutex<RevisionStore>>,
    active: Arc<Mutex<Option<String>>>,
    data_dir: Option<PathBuf>,
    jobs: JobManager,
    runtime: Arc<dyn RuntimeClient>,
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
        Self {
            repo: Arc::new(Mutex::new(ProfileStore::Memory(
                InMemoryProfileRepository::new(),
            ))),
            revisions: Arc::new(Mutex::new(RevisionStore::new())),
            active: Arc::new(Mutex::new(None)),
            data_dir: None,
            jobs: JobManager::new(),
            runtime,
        }
    }

    /// Open a real SQLite-backed engine rooted at `data_dir` (creating
    /// `guiNDB.db` / `guiNConfig.json` as needed). Runtime events go to the
    /// real net-host client.
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, DomainError> {
        Self::open_with_runtime(data_dir, Arc::new(NetHostClient::new()))
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

        Ok(Self {
            repo: Arc::new(Mutex::new(ProfileStore::Sqlite(sqlite))),
            revisions: Arc::new(Mutex::new(RevisionStore::with_desired(
                DesiredRevision::new(desired),
            ))),
            active: Arc::new(Mutex::new(active)),
            data_dir: Some(data_dir),
            jobs: JobManager::new(),
            runtime,
        })
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
    /// and bumps the desired revision.
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
        draft.validate()?;

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

    fn persist_config(&self, revisions: &RevisionStore) -> Result<(), DomainError> {
        let Some(dir) = &self.data_dir else {
            return Ok(());
        };
        let active = self.active.lock().ok().and_then(|guard| guard.clone());
        let value = serde_json::json!({
            "desired_revision": revisions.desired().get(),
            "active_index_id": active,
        });
        write_config(dir, &value)?;
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

    /// Assemble the current snapshot.
    pub fn snapshot(&self) -> Result<Snapshot, DomainError> {
        let desired = self
            .revisions
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?
            .desired();
        let runtime = self.runtime.snapshot()?;
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

/// Atomically write `guiNConfig.json` (write temp + rename).
fn write_config(dir: &Path, value: &Value) -> Result<(), DomainError> {
    let path = dir.join("guiNConfig.json");
    let tmp = dir.join("guiNConfig.json.tmp");
    let text = serde_json::to_string_pretty(value).map_err(storage_error)?;
    std::fs::write(&tmp, text).map_err(storage_error)?;
    std::fs::rename(&tmp, &path).map_err(storage_error)?;
    Ok(())
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
}
