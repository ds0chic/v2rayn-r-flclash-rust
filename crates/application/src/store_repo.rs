//! SQLite-backed [`ProfileRepository`] plus a small enum that lets the engine
//! run either against real storage or the in-memory store used by tests.
//!
//! The mapping mirrors `compat/domain-map.yaml` (`ProfileItem` 40 columns +
//! `ProtoExtra` 30 + `TransportExtra` 11). Legacy columns that upstream keeps
//! only for migration are preserved in `Profile::extra` so a round-trip never
//! drops them.

use domain::{
    codes, CancellationToken, ConfigType, CoreType, DomainError, Profile, SecurityParams,
    TrafficStats,
};
use persistence::mapping::map_traffic;
use persistence::rows::RawRow;
use persistence::{ProtocolExtraBlob, Store, TransportExtraBlob};
use rusqlite::types::ToSql;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

use crate::repository::{
    InMemoryProfileRepository, PageRequest, ProfileFilter, ProfilePage, ProfileRepository,
    ProfileSort, SubRepository,
};
use crate::subs::SubItem;

/// Columns retained verbatim in `Profile::extra` (superseded by the JSON
/// blobs, but still present in an imported database).
const LEGACY_COLUMNS: &[&str] = &[
    "HeaderType",
    "RequestHost",
    "Path",
    "Extra",
    "Ports",
    "AlterId",
    "Flow",
    "Id",
    "Security",
];

/// Map a persistence failure onto the shared error contract.
pub fn storage_error(error: impl std::fmt::Display) -> DomainError {
    DomainError::new(codes::INTERNAL, "error.storage").with_detail(error.to_string())
}

/// Classified storage failure: preserves the persistence layer's stable code
/// and message key so the UI can distinguish a corrupt database, a read-only
/// directory, a full disk or a lock conflict instead of a single opaque
/// `error.storage` (R4-27). Transient IO/SQLite failures are retryable.
pub fn persistence_storage_error(error: persistence::PersistenceError) -> DomainError {
    let retryable = matches!(
        error,
        persistence::PersistenceError::Io(_) | persistence::PersistenceError::Sqlite(_)
    );
    let domain = DomainError::new(error.code(), error.message_key()).with_detail(error.to_string());
    if retryable {
        domain.retryable()
    } else {
        domain
    }
}

// ---- SP-21 async paged query data layer (prep; full UI wiring waits on
// SP-16, the FRB `QueryProfilesPageAsync` entry on the SP-00 integrator) ----

/// Upper bound for one async page: a caller only ever waits on a bounded
/// window, never on a full-table read.
pub const ASYNC_PAGE_MAX_SIZE: u32 = 2000;

/// One frozen async page request. `filter`/`sort`/`cursor` are captured at
/// call time; `expected_revision == 0 && cursor == 0` is a fresh query that
/// adopts the current dataset revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncPageRequest {
    pub filter: ProfileFilter,
    pub sort: ProfileSort,
    pub cursor: usize,
    pub page_size: u32,
    pub expected_revision: u64,
    pub generation: u64,
}

/// One bounded page plus the cursor/revision contract (PLAN §3.5:
/// `items/nextCursor/datasetRevision/total`).
#[derive(Debug, Clone, PartialEq)]
pub struct AsyncPage {
    pub items: Vec<Profile>,
    pub next_cursor: Option<usize>,
    pub dataset_revision: u64,
    pub total: usize,
    pub generation: u64,
    /// Thread that executed the query (proof the worker path leaves the
    /// caller thread free).
    pub worker_thread: std::thread::ThreadId,
}

/// Failure modes of an async page query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncPageError {
    /// Cooperative cancel won: no page is delivered, nothing was committed.
    Cancelled,
    /// A newer generation superseded this request; the late result is
    /// dropped instead of overwriting the latest view.
    Superseded,
    /// The cursor belongs to an older dataset revision; restart from zero.
    StaleCursor {
        expected: u64,
        actual: u64,
    },
    /// The bounded wait expired before the worker answered.
    Timeout,
    Storage(DomainError),
}

impl AsyncPageError {
    /// Map onto the shared error contract for future FRB wiring.
    pub fn to_domain_error(&self) -> DomainError {
        match self {
            AsyncPageError::Cancelled => DomainError::new(codes::CANCELLED, "error.cancelled"),
            AsyncPageError::Superseded => {
                DomainError::new(codes::CONFLICT, "error.page_superseded")
            }
            AsyncPageError::StaleCursor { expected, actual } => {
                DomainError::stale_revision(*expected, *actual)
            }
            AsyncPageError::Timeout => DomainError::new(codes::TIMEOUT, "error.page_timeout"),
            AsyncPageError::Storage(error) => error.clone(),
        }
    }
}

/// Revision + generation orchestration shared by the writer side and the
/// background reader. The dataset revision is process-local per handle and
/// starts at zero; every successful profile mutation must call
/// [`Self::notify_mutated`] so cursors from before the write fail loudly
/// instead of skipping or repeating rows.
#[derive(Debug, Default)]
pub struct ProfilePageQuery {
    revision: AtomicU64,
    generation: AtomicU64,
}

impl ProfilePageQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dataset_revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    /// Record a committed profile mutation (upsert/remove/replace). Lock-free.
    pub fn notify_mutated(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    /// Open a new request generation; in-flight results carrying an older one
    /// are dropped by [`Self::query_page`].
    pub fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }

    /// Run one bounded page with cancel/revision/generation guards. The call
    /// itself is blocking SQLite IO; production callers run it on
    /// [`AsyncPageWorker`], never on the UI thread.
    pub fn query_page(
        &self,
        repo: &impl ProfileRepository,
        request: &AsyncPageRequest,
        cancel: &CancellationToken,
    ) -> Result<AsyncPage, AsyncPageError> {
        cancel.check().map_err(|_| AsyncPageError::Cancelled)?;
        let revision = self.dataset_revision();
        if (request.cursor != 0 || request.expected_revision != 0)
            && request.expected_revision != revision
        {
            return Err(AsyncPageError::StaleCursor {
                expected: request.expected_revision,
                actual: revision,
            });
        }
        if !self.is_current(request.generation) {
            return Err(AsyncPageError::Superseded);
        }
        let page_size = request.page_size.clamp(1, ASYNC_PAGE_MAX_SIZE);
        let page = repo
            .query(
                &request.filter,
                request.sort,
                PageRequest {
                    cursor: request.cursor,
                    page_size,
                },
            )
            .map_err(AsyncPageError::Storage)?;
        // Late-result guards: a cancel or a newer generation that landed
        // while the read was in flight discards this page.
        cancel.check().map_err(|_| AsyncPageError::Cancelled)?;
        if !self.is_current(request.generation) {
            return Err(AsyncPageError::Superseded);
        }
        // A write racing the read invalidates the page just produced.
        let after = self.dataset_revision();
        if after != revision {
            return Err(AsyncPageError::StaleCursor {
                expected: revision,
                actual: after,
            });
        }
        Ok(AsyncPage {
            items: page.items,
            next_cursor: page.next_cursor,
            dataset_revision: revision,
            total: page.total,
            generation: request.generation,
            worker_thread: std::thread::current().id(),
        })
    }
}

struct WorkerJob {
    request: AsyncPageRequest,
    cancel: CancellationToken,
    respond_to: mpsc::Sender<Result<AsyncPage, AsyncPageError>>,
}

/// Background reader for async pages. It owns a dedicated read-only SQLite
/// connection to the same file (WAL mode admits concurrent readers), so a
/// long page never holds the writer connection and the caller only blocks on
/// a bounded-page rendezvous. `:memory:` databases cannot be shared across
/// connections; the worker requires a file path.
pub struct AsyncPageWorker {
    jobs: mpsc::Sender<WorkerJob>,
}

impl AsyncPageWorker {
    pub fn open_readonly(
        path: impl AsRef<std::path::Path>,
        shared: Arc<ProfilePageQuery>,
    ) -> Result<Self, DomainError> {
        let store = Store::open_readonly(path).map_err(persistence_storage_error)?;
        let repo = SqliteProfileRepository::from_store(store);
        let (jobs, inbox) = mpsc::channel::<WorkerJob>();
        std::thread::Builder::new()
            .name("sp21-page-worker".to_string())
            .spawn(move || {
                while let Ok(job) = inbox.recv() {
                    let outcome = shared.query_page(&repo, &job.request, &job.cancel);
                    // The caller cancelled and dropped its receiver: discard
                    // the late result instead of leaking it.
                    let _ = job.respond_to.send(outcome);
                }
            })
            .map_err(storage_error)?;
        Ok(Self { jobs })
    }

    /// Enqueue one page without blocking on the read itself.
    pub fn submit(&self, request: AsyncPageRequest, cancel: CancellationToken) -> QueryHandle {
        let (respond_to, inbox) = mpsc::channel();
        let accepted = self
            .jobs
            .send(WorkerJob {
                request,
                cancel: cancel.clone(),
                respond_to,
            })
            .is_ok();
        QueryHandle {
            cancel,
            inbox: accepted.then_some(inbox),
        }
    }
}

/// Handle to one in-flight page. `cancel` is synchronous and lock-free: it
/// returns immediately and the worker drops the late result at its next safe
/// point (or the send fails because the receiver is gone).
pub struct QueryHandle {
    cancel: CancellationToken,
    inbox: Option<mpsc::Receiver<Result<AsyncPage, AsyncPageError>>>,
}

impl QueryHandle {
    pub fn cancel(&mut self) {
        self.cancel.cancel();
        self.inbox.take();
    }

    pub fn wait(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Result<AsyncPage, AsyncPageError>, AsyncPageError> {
        let Some(inbox) = self.inbox.take() else {
            return Err(AsyncPageError::Cancelled);
        };
        match inbox.recv_timeout(timeout) {
            Ok(outcome) => Ok(outcome),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.inbox = Some(inbox);
                Err(AsyncPageError::Timeout)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(AsyncPageError::Cancelled),
        }
    }
}

/// A SQLite-backed profile repository.
pub struct SqliteProfileRepository {
    store: Store,
}
impl SqliteProfileRepository {
    /// Open (creating if needed) the database at `path`.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, DomainError> {
        let store = Store::create(path).map_err(persistence_storage_error)?;
        Ok(Self { store })
    }

    /// Wrap an already-open store (used by import/migration tests).
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Atomically replace one subscription's *subscription-sourced* nodes in a
    /// single SQLite transaction: delete the existing `Subid = subid AND
    /// IsSub = 1` rows, then upsert the replacement set. Manual nodes in the
    /// same group (`IsSub = 0`, e.g. a batch import) are preserved, matching
    /// upstream `RemoveServersViaSubid(config, subid, isSub: true)`
    /// (`ConfigHandler.cs:2246-2259`). Any failure rolls the transaction back
    /// so the old nodes survive. `fail_after_delete` is a test-only injection
    /// that errors after the deletes (before commit) to prove the rollback.
    ///
    /// After the node upsert the matched `ServerStatItem` rows are cloned from
    /// the old identity to the new one inside the same transaction (upstream
    /// `CloneServerStatItem`, `ConfigHandler.cs:2120-2131`), so traffic totals
    /// follow a node whose stable id changed while its transport identity did
    /// not. `(added, removed)` reports the replacement set size and the number
    /// of replaced subscription rows, not the group's total (manual nodes are
    /// not "added" by an update).
    pub fn replace_for_sub(
        &self,
        subid: &str,
        profiles: Vec<Profile>,
        remove_existing: bool,
        fail_after_delete: bool,
    ) -> Result<(usize, usize), DomainError> {
        let old_profiles: Vec<Profile> = self
            .store
            .query_rows(
                "SELECT * FROM \"ProfileItem\" WHERE \"Subid\" = ?1",
                &[&subid],
            )
            .map_err(persistence_storage_error)?
            .iter()
            .map(profile_from_row)
            .collect();
        let to_delete: Vec<String> = if remove_existing {
            old_profiles
                .iter()
                .filter(|p| p.is_sub)
                .map(|p| p.index_id.clone())
                .collect()
        } else {
            Vec::new()
        };
        let removed = to_delete.len();
        let tx = self.store.begin().map_err(persistence_storage_error)?;
        for id in &to_delete {
            tx.execute(
                "DELETE FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
                rusqlite::params![id],
            )
            .map_err(|error| persistence_storage_error(error.into()))?;
        }
        if fail_after_delete {
            return Err(DomainError::new(codes::INTERNAL, "error.storage")
                .with_detail("injected failure after delete"));
        }
        for profile in &profiles {
            let row = row_from_profile(profile);
            self.store
                .upsert_row(&tx, &row)
                .map_err(persistence_storage_error)?;
        }
        if remove_existing {
            for profile in &profiles {
                let Some(old) = crate::subs::find_matched_profile(&old_profiles, profile) else {
                    continue;
                };
                if old.index_id == profile.index_id {
                    continue;
                }
                let stat = self
                    .store
                    .query_rows(
                        "SELECT * FROM \"ServerStatItem\" WHERE \"IndexId\" = ?1",
                        &[&old.index_id],
                    )
                    .map_err(persistence_storage_error)?;
                for mut cloned in stat {
                    cloned.table = "ServerStatItem".to_string();
                    cloned.set("IndexId", json!(profile.index_id.clone()));
                    self.store
                        .upsert_row(&tx, &cloned)
                        .map_err(persistence_storage_error)?;
                }
            }
        }
        tx.commit()
            .map_err(|error| persistence_storage_error(error.into()))?;
        Ok((profiles.len(), removed))
    }
}

impl ProfileRepository for SqliteProfileRepository {
    fn get(&self, index_id: &str) -> Result<Option<Profile>, DomainError> {
        let rows = self
            .store
            .query_rows(
                "SELECT * FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
                &[&index_id],
            )
            .map_err(persistence_storage_error)?;
        Ok(rows.first().map(profile_from_row))
    }

    fn upsert(&mut self, profile: Profile) -> Result<(), DomainError> {
        let row = row_from_profile(&profile);
        self.store
            .upsert_row(self.store.connection(), &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, index_id: &str) -> Result<bool, DomainError> {
        let affected = self
            .store
            .execute(
                "DELETE FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
                &[&index_id],
            )
            .map_err(persistence_storage_error)?;
        Ok(affected > 0)
    }

    fn query(
        &self,
        filter: &ProfileFilter,
        sort: ProfileSort,
        page: PageRequest,
    ) -> Result<ProfilePage, DomainError> {
        let (where_sql, params) = build_where(filter);

        let count_sql = format!("SELECT COUNT(*) FROM \"ProfileItem\"{where_sql}");
        let count_params: Vec<&dyn ToSql> = params.iter().map(|s| s as &dyn ToSql).collect();
        let total = self
            .store
            .count_query(&count_sql, &count_params)
            .map_err(persistence_storage_error)? as usize;

        let order_sql = match sort {
            ProfileSort::Remarks => " ORDER BY \"Remarks\" COLLATE NOCASE ASC, \"IndexId\" ASC",
            ProfileSort::Address => " ORDER BY \"Address\" ASC, \"IndexId\" ASC",
            ProfileSort::Delay => {
                " ORDER BY (SELECT \"Delay\" FROM \"ProfileExItem\" e WHERE e.\"IndexId\" = \
                 \"ProfileItem\".\"IndexId\") ASC, \"IndexId\" ASC"
            }
            ProfileSort::IndexId => " ORDER BY \"IndexId\" ASC",
        };
        let limit = page.page_size.max(1) as i64;
        let offset = page.cursor as i64;
        let limit_index = params.len() + 1;
        let offset_index = params.len() + 2;
        let sql = format!(
            "SELECT * FROM \"ProfileItem\"{where_sql}{order_sql} LIMIT ?{limit_index} OFFSET ?{offset_index}"
        );
        let mut all_params: Vec<&dyn ToSql> = params.iter().map(|s| s as &dyn ToSql).collect();
        all_params.push(&limit);
        all_params.push(&offset);

        let rows = self
            .store
            .query_rows(&sql, &all_params)
            .map_err(persistence_storage_error)?;
        let items: Vec<Profile> = rows.iter().map(profile_from_row).collect();
        let end = page.cursor + items.len();
        let next_cursor = if end < total { Some(end) } else { None };
        Ok(ProfilePage {
            items,
            total,
            next_cursor,
        })
    }

    fn count(&self) -> usize {
        self.store.count_rows("ProfileItem").unwrap_or(0) as usize
    }
}

/// A SQLite-backed subscription repository sharing the profile store's `Store`.
pub struct SqliteSubRepository {
    store: Store,
}

impl SqliteSubRepository {
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }
}

impl SubRepository for SqliteSubRepository {
    fn list(&self) -> Result<Vec<SubItem>, DomainError> {
        let rows = self
            .store
            .read_rows("SubItem")
            .map_err(persistence_storage_error)?;
        let mut items: Vec<SubItem> = rows.iter().map(SubItem::from_row).collect();
        items.sort_by_key(|s| s.sort);
        Ok(items)
    }

    fn get(&self, id: &str) -> Result<Option<SubItem>, DomainError> {
        let rows = self
            .store
            .query_rows("SELECT * FROM \"SubItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(rows.first().map(SubItem::from_row))
    }

    fn upsert(&mut self, item: SubItem) -> Result<(), DomainError> {
        let row = item.to_row();
        self.store
            .upsert_row(self.store.connection(), &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        let affected = self
            .store
            .execute("DELETE FROM \"SubItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(affected > 0)
    }

    fn count(&self) -> usize {
        self.store.count_rows("SubItem").unwrap_or(0) as usize
    }
}

/// A SQLite-backed routing repository sharing the profile store's `Store`.
pub struct SqliteRoutingRepository {
    store: Store,
}

impl SqliteRoutingRepository {
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }
}

impl crate::routing::RoutingRepository for SqliteRoutingRepository {
    fn list(&self) -> Result<Vec<domain::RoutingProfile>, DomainError> {
        let rows = self
            .store
            .read_rows("RoutingItem")
            .map_err(persistence_storage_error)?;
        let mut items: Vec<domain::RoutingProfile> =
            rows.iter().map(crate::routing::routing_from_row).collect();
        items.sort_by_key(|r| r.sort);
        Ok(items)
    }

    fn get(&self, id: &str) -> Result<Option<domain::RoutingProfile>, DomainError> {
        let rows = self
            .store
            .query_rows("SELECT * FROM \"RoutingItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(rows.first().map(crate::routing::routing_from_row))
    }

    fn upsert(&mut self, item: domain::RoutingProfile) -> Result<(), DomainError> {
        let row = crate::routing::routing_to_row(&item);
        self.store
            .upsert_row(self.store.connection(), &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        let affected = self
            .store
            .execute("DELETE FROM \"RoutingItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(affected > 0)
    }

    fn count(&self) -> usize {
        self.store.count_rows("RoutingItem").unwrap_or(0) as usize
    }
}

/// A SQLite-backed DNS repository sharing the profile store's `Store`.
pub struct SqliteDnsRepository {
    store: Store,
}

impl SqliteDnsRepository {
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }
}

impl crate::dns::DnsRepository for SqliteDnsRepository {
    fn list(&self) -> Result<Vec<domain::DnsProfile>, DomainError> {
        let rows = self
            .store
            .read_rows("DNSItem")
            .map_err(persistence_storage_error)?;
        let mut items: Vec<domain::DnsProfile> =
            rows.iter().map(crate::dns::dns_from_row).collect();
        items.sort_by(|a, b| {
            a.core_type
                .value()
                .cmp(&b.core_type.value())
                .then_with(|| a.remarks.cmp(&b.remarks))
        });
        Ok(items)
    }

    fn get(&self, id: &str) -> Result<Option<domain::DnsProfile>, DomainError> {
        let rows = self
            .store
            .query_rows("SELECT * FROM \"DNSItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(rows.first().map(crate::dns::dns_from_row))
    }

    fn upsert(&mut self, item: domain::DnsProfile) -> Result<(), DomainError> {
        let row = crate::dns::dns_to_row(&item);
        self.store
            .upsert_row(self.store.connection(), &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        let affected = self
            .store
            .execute("DELETE FROM \"DNSItem\" WHERE \"Id\" = ?1", &[&id])
            .map_err(persistence_storage_error)?;
        Ok(affected > 0)
    }

    fn count(&self) -> usize {
        self.store.count_rows("DNSItem").unwrap_or(0) as usize
    }
}

fn build_where(filter: &ProfileFilter) -> (String, Vec<String>) {
    let mut parts: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();
    if let Some(text) = &filter.text {
        let needle = format!("%{text}%");
        parts.push("(\"Remarks\" LIKE ? OR \"Address\" LIKE ?)".to_string());
        params.push(needle.clone());
        params.push(needle);
    }
    if !filter.config_types.is_empty() {
        let list = filter
            .config_types
            .iter()
            .map(|c| c.value().to_string())
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("\"ConfigType\" IN ({list})"));
    }
    if let Some(subid) = &filter.subid {
        parts.push("\"Subid\" = ?".to_string());
        params.push(subid.clone());
    }
    let where_sql = if parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", parts.join(" AND "))
    };
    (where_sql, params)
}

/// Build a `ProfileItem` row from a domain profile.
pub fn row_from_profile(profile: &Profile) -> RawRow {
    let mut row = RawRow::new("ProfileItem");
    row.set("IndexId", json!(profile.index_id));
    row.set("ConfigType", json!(profile.config_type.value()));
    row.set(
        "CoreType",
        profile.core_type.map_or(Value::Null, |c| json!(c.value())),
    );
    row.set("ConfigVersion", json!(profile.config_version));
    row.set("Subid", json!(profile.subid));
    row.set("IsSub", json!(i64::from(profile.is_sub)));
    row.set(
        "PreSocksPort",
        profile.pre_socks_port.map_or(Value::Null, |v| json!(v)),
    );
    row.set("DisplayLog", json!(i64::from(profile.display_log)));
    row.set("Remarks", json!(profile.remarks));
    row.set("Address", json!(profile.address));
    row.set("Port", json!(profile.port));
    row.set("Password", json!(profile.password));
    row.set("Username", json!(profile.username));
    row.set("Network", json!(profile.network));
    row.set(
        "StreamSecurity",
        opt_json(profile.security.stream_security.as_deref()),
    );
    row.set(
        "AllowInsecure",
        opt_json(profile.security.allow_insecure.as_deref()),
    );
    row.set("Sni", opt_json(profile.security.sni.as_deref()));
    row.set("Alpn", opt_json(profile.security.alpn.as_deref()));
    row.set(
        "Fingerprint",
        opt_json(profile.security.fingerprint.as_deref()),
    );
    row.set(
        "PublicKey",
        opt_json(profile.security.public_key.as_deref()),
    );
    row.set("ShortId", opt_json(profile.security.short_id.as_deref()));
    row.set("SpiderX", opt_json(profile.security.spider_x.as_deref()));
    row.set(
        "Mldsa65Verify",
        opt_json(profile.security.mldsa65_verify.as_deref()),
    );
    row.set("Cert", opt_json(profile.security.cert.as_deref()));
    row.set("CertSha", opt_json(profile.security.cert_sha.as_deref()));
    row.set(
        "EchConfigList",
        opt_json(profile.security.ech_config_list.as_deref()),
    );
    row.set(
        "VerifyPeerCertByName",
        opt_json(profile.security.verify_peer_cert_by_name.as_deref()),
    );
    row.set("Finalmask", opt_json(profile.finalmask.as_deref()));
    row.set(
        "MuxEnabled",
        profile
            .mux_enabled
            .map_or(Value::Null, |v| json!(i64::from(v))),
    );
    row.set(
        "ProtoExtra",
        json!(ProtocolExtraBlob::from_domain(&profile.proto_extra)
            .to_json()
            .unwrap_or_else(|_| "{}".to_string())),
    );
    row.set(
        "TransportExtra",
        json!(TransportExtraBlob::from_domain(&profile.transport_extra)
            .to_json()
            .unwrap_or_else(|_| "{}".to_string())),
    );

    // Preserve any legacy columns carried in `extra`; drop unknown top-level
    // keys (upstream has no column for them).
    for column in LEGACY_COLUMNS {
        if let Some(value) = profile.extra.get(*column) {
            row.set(column, value.clone());
        } else {
            row.set(column, Value::Null);
        }
    }
    row
}

/// Build a domain profile from a `ProfileItem` row.
pub fn profile_from_row(row: &RawRow) -> Profile {
    let config_type = row
        .opt_i64("ConfigType")
        .and_then(|v| ConfigType::from_value(v as i32))
        .unwrap_or(ConfigType::Vmess);
    let core_type = row
        .opt_i64("CoreType")
        .and_then(|v| CoreType::from_value(v as i32));

    let mut extra = domain::ExtraMap::new();
    for &column in LEGACY_COLUMNS {
        if let Some(value) = row.get(column) {
            if !value.is_null() {
                let empty_string = value.as_str().is_some_and(str::is_empty);
                if !empty_string {
                    extra.insert((*column).to_string(), value.clone());
                }
            }
        }
    }

    let proto_extra = ProtocolExtraBlob::parse(row.opt_string("ProtoExtra").as_deref())
        .map(|b| b.to_domain())
        .unwrap_or_default();
    let transport_extra = TransportExtraBlob::parse(row.opt_string("TransportExtra").as_deref())
        .map(|b| b.to_domain())
        .unwrap_or_default();

    Profile {
        index_id: row.string("IndexId"),
        config_type,
        core_type,
        config_version: row.opt_i64("ConfigVersion").unwrap_or(4) as i32,
        subid: row.string("Subid"),
        is_sub: row.bool("IsSub"),
        pre_socks_port: row.opt_i64("PreSocksPort").map(|v| v as i32),
        display_log: row.bool("DisplayLog"),
        remarks: row.string("Remarks"),
        address: row.string("Address"),
        port: row.opt_i64("Port").unwrap_or(0) as i32,
        password: row.string("Password"),
        username: row.string("Username"),
        network: row.string("Network"),
        mux_enabled: row.opt_i64("MuxEnabled").map(|v| v != 0),
        finalmask: row.opt_string("Finalmask"),
        outbound_tag: None,
        security: SecurityParams {
            stream_security: row.opt_string("StreamSecurity"),
            allow_insecure: row.opt_string("AllowInsecure"),
            sni: row.opt_string("Sni"),
            alpn: row.opt_string("Alpn"),
            fingerprint: row.opt_string("Fingerprint"),
            public_key: row.opt_string("PublicKey"),
            short_id: row.opt_string("ShortId"),
            spider_x: row.opt_string("SpiderX"),
            mldsa65_verify: row.opt_string("Mldsa65Verify"),
            cert: row.opt_string("Cert"),
            cert_sha: row.opt_string("CertSha"),
            ech_config_list: row.opt_string("EchConfigList"),
            verify_peer_cert_by_name: row.opt_string("VerifyPeerCertByName"),
        },
        proto_extra,
        transport_extra,
        extra,
    }
}

fn opt_json(value: Option<&str>) -> Value {
    match value {
        Some(v) if !v.is_empty() => json!(v),
        _ => Value::Null,
    }
}

/// Storage backend selected at engine construction.
pub enum ProfileStore {
    Memory(InMemoryProfileRepository),
    Sqlite(SqliteProfileRepository),
}

impl ProfileStore {
    /// Restore exactly the profile rows touched by a mutation. SQLite applies
    /// the compensation as one transaction so a failed config write cannot
    /// leave a partially restored row set.
    pub fn restore_profiles(
        &mut self,
        snapshot: &[(String, Option<Profile>)],
    ) -> Result<(), DomainError> {
        match self {
            ProfileStore::Memory(repo) => {
                for (id, profile) in snapshot {
                    match profile {
                        Some(profile) => repo.upsert(profile.clone())?,
                        None => {
                            repo.remove(id)?;
                        }
                    }
                }
                Ok(())
            }
            ProfileStore::Sqlite(repo) => {
                let tx = repo.store.begin().map_err(persistence_storage_error)?;
                for (id, profile) in snapshot {
                    match profile {
                        Some(profile) => repo
                            .store
                            .upsert_row(&tx, &row_from_profile(profile))
                            .map_err(persistence_storage_error)?,
                        None => {
                            tx.execute(
                                "DELETE FROM \"ProfileItem\" WHERE \"IndexId\" = ?1",
                                rusqlite::params![id],
                            )
                            .map_err(|error| persistence_storage_error(error.into()))?;
                        }
                    }
                }
                tx.commit()
                    .map_err(|error| persistence_storage_error(error.into()))
            }
        }
    }

    /// Transactional replace for one subscription (see
    /// [`SqliteProfileRepository::replace_for_sub`]). The in-memory backend
    /// snapshots the affected rows and restores them on failure so the
    /// "old data survives a mid-replace error" contract holds for tests.
    pub fn replace_for_sub(
        &mut self,
        subid: &str,
        profiles: Vec<Profile>,
        remove_existing: bool,
        fail_after_delete: bool,
    ) -> Result<(usize, usize), DomainError> {
        match self {
            ProfileStore::Memory(repo) => {
                let snapshot = repo.snapshot_for_sub(subid);
                let added_count = profiles.len();
                let mut removed = 0usize;
                let result: Result<(usize, usize), DomainError> = (|| {
                    if remove_existing {
                        // Only subscription-sourced rows are replaced; manual
                        // `IsSub = false` nodes in the same group are kept
                        // (upstream `RemoveServersViaSubid(..., isSub: true)`).
                        for id in snapshot
                            .iter()
                            .filter(|p| p.is_sub)
                            .map(|p| p.index_id.clone())
                            .collect::<Vec<_>>()
                        {
                            if repo.remove(&id)? {
                                removed += 1;
                            }
                        }
                    }
                    if fail_after_delete {
                        return Err(DomainError::new(codes::INTERNAL, "error.storage")
                            .with_detail("injected failure after delete"));
                    }
                    for profile in &profiles {
                        repo.upsert(profile.clone())?;
                    }
                    Ok((added_count, removed))
                })();
                if result.is_err() {
                    repo.restore_snapshot(subid, snapshot);
                }
                result
            }
            ProfileStore::Sqlite(repo) => {
                repo.replace_for_sub(subid, profiles, remove_existing, fail_after_delete)
            }
        }
    }
}

impl ProfileRepository for ProfileStore {
    fn get(&self, index_id: &str) -> Result<Option<Profile>, DomainError> {
        match self {
            ProfileStore::Memory(repo) => repo.get(index_id),
            ProfileStore::Sqlite(repo) => repo.get(index_id),
        }
    }

    fn upsert(&mut self, profile: Profile) -> Result<(), DomainError> {
        match self {
            ProfileStore::Memory(repo) => repo.upsert(profile),
            ProfileStore::Sqlite(repo) => repo.upsert(profile),
        }
    }

    fn remove(&mut self, index_id: &str) -> Result<bool, DomainError> {
        match self {
            ProfileStore::Memory(repo) => repo.remove(index_id),
            ProfileStore::Sqlite(repo) => repo.remove(index_id),
        }
    }

    fn query(
        &self,
        filter: &ProfileFilter,
        sort: ProfileSort,
        page: PageRequest,
    ) -> Result<ProfilePage, DomainError> {
        match self {
            ProfileStore::Memory(repo) => repo.query(filter, sort, page),
            ProfileStore::Sqlite(repo) => repo.query(filter, sort, page),
        }
    }

    fn count(&self) -> usize {
        match self {
            ProfileStore::Memory(repo) => repo.count(),
            ProfileStore::Sqlite(repo) => repo.count(),
        }
    }
}

/// Storage backend for subscriptions, selected at engine construction.
pub enum SubStore {
    Memory(crate::repository::InMemorySubRepository),
    Sqlite(SqliteSubRepository),
}

impl SubRepository for SubStore {
    fn list(&self) -> Result<Vec<SubItem>, DomainError> {
        match self {
            SubStore::Memory(repo) => repo.list(),
            SubStore::Sqlite(repo) => repo.list(),
        }
    }

    fn get(&self, id: &str) -> Result<Option<SubItem>, DomainError> {
        match self {
            SubStore::Memory(repo) => repo.get(id),
            SubStore::Sqlite(repo) => repo.get(id),
        }
    }

    fn upsert(&mut self, item: SubItem) -> Result<(), DomainError> {
        match self {
            SubStore::Memory(repo) => repo.upsert(item),
            SubStore::Sqlite(repo) => repo.upsert(item),
        }
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        match self {
            SubStore::Memory(repo) => repo.remove(id),
            SubStore::Sqlite(repo) => repo.remove(id),
        }
    }

    fn count(&self) -> usize {
        match self {
            SubStore::Memory(repo) => repo.count(),
            SubStore::Sqlite(repo) => repo.count(),
        }
    }
}

/// Storage backend for routing profiles, selected at engine construction.
pub enum RoutingStore {
    Memory(crate::routing::InMemoryRoutingRepository),
    Sqlite(SqliteRoutingRepository),
}

impl crate::routing::RoutingRepository for RoutingStore {
    fn list(&self) -> Result<Vec<domain::RoutingProfile>, DomainError> {
        match self {
            RoutingStore::Memory(repo) => repo.list(),
            RoutingStore::Sqlite(repo) => repo.list(),
        }
    }

    fn get(&self, id: &str) -> Result<Option<domain::RoutingProfile>, DomainError> {
        match self {
            RoutingStore::Memory(repo) => repo.get(id),
            RoutingStore::Sqlite(repo) => repo.get(id),
        }
    }

    fn upsert(&mut self, item: domain::RoutingProfile) -> Result<(), DomainError> {
        match self {
            RoutingStore::Memory(repo) => repo.upsert(item),
            RoutingStore::Sqlite(repo) => repo.upsert(item),
        }
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        match self {
            RoutingStore::Memory(repo) => repo.remove(id),
            RoutingStore::Sqlite(repo) => repo.remove(id),
        }
    }

    fn count(&self) -> usize {
        match self {
            RoutingStore::Memory(repo) => repo.count(),
            RoutingStore::Sqlite(repo) => repo.count(),
        }
    }
}

/// Storage backend for DNS profiles, selected at engine construction.
pub enum DnsStore {
    Memory(crate::dns::InMemoryDnsRepository),
    Sqlite(SqliteDnsRepository),
}

impl crate::dns::DnsRepository for DnsStore {
    fn list(&self) -> Result<Vec<domain::DnsProfile>, DomainError> {
        match self {
            DnsStore::Memory(repo) => repo.list(),
            DnsStore::Sqlite(repo) => repo.list(),
        }
    }

    fn get(&self, id: &str) -> Result<Option<domain::DnsProfile>, DomainError> {
        match self {
            DnsStore::Memory(repo) => repo.get(id),
            DnsStore::Sqlite(repo) => repo.get(id),
        }
    }

    fn upsert(&mut self, item: domain::DnsProfile) -> Result<(), DomainError> {
        match self {
            DnsStore::Memory(repo) => repo.upsert(item),
            DnsStore::Sqlite(repo) => repo.upsert(item),
        }
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        match self {
            DnsStore::Memory(repo) => repo.remove(id),
            DnsStore::Sqlite(repo) => repo.remove(id),
        }
    }

    fn count(&self) -> usize {
        match self {
            DnsStore::Memory(repo) => repo.count(),
            DnsStore::Sqlite(repo) => repo.count(),
        }
    }
}

/// SQLite-backed `ServerStatItem` store for the T15a statistics pipeline.
pub struct SqliteTrafficStore {
    store: Store,
}

impl SqliteTrafficStore {
    pub fn from_store(store: Store) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }
}

impl crate::monitor::TrafficStore for SqliteTrafficStore {
    fn load(&self) -> Result<Vec<TrafficStats>, DomainError> {
        let rows = self
            .store
            .read_rows("ServerStatItem")
            .map_err(persistence_storage_error)?;
        Ok(rows.iter().map(map_traffic).collect())
    }

    fn upsert(&mut self, stat: &TrafficStats) -> Result<(), DomainError> {
        let mut row = RawRow::new("ServerStatItem");
        row.set("IndexId", json!(&stat.index_id));
        row.set("TotalUp", json!(stat.total_up));
        row.set("TotalDown", json!(stat.total_down));
        row.set("TodayUp", json!(stat.today_up));
        row.set("TodayDown", json!(stat.today_down));
        row.set("DateNow", json!(stat.date_now));
        let conn = self.store.connection();
        self.store
            .upsert_row(conn, &row)
            .map_err(persistence_storage_error)
    }

    fn remove(&mut self, index_id: &str) -> Result<(), DomainError> {
        self.store
            .execute(
                "DELETE FROM \"ServerStatItem\" WHERE \"IndexId\" = ?1",
                &[&index_id],
            )
            .map_err(persistence_storage_error)?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DomainError> {
        self.store
            .execute("DELETE FROM \"ServerStatItem\"", &[])
            .map_err(persistence_storage_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::synthetic_full_profile;

    #[test]
    fn sqlite_replace_for_sub_rolls_back_on_injected_failure() {
        use crate::synthetic::synthetic_full_profile;
        let repo = SqliteProfileRepository::open(":memory:").unwrap();
        let mut seed = vec![synthetic_full_profile(1), synthetic_full_profile(2)];
        for p in &mut seed {
            p.subid = "s-sql".to_string();
        }
        let (added, _) = repo.replace_for_sub("s-sql", seed, true, false).unwrap();
        assert_eq!(added, 2);
        let mut replacement = vec![synthetic_full_profile(3)];
        for p in &mut replacement {
            p.subid = "s-sql".to_string();
        }
        let err = repo
            .replace_for_sub("s-sql", replacement, true, true)
            .unwrap_err();
        assert_eq!(err.code, domain::codes::INTERNAL);
        let kept: Vec<_> = repo
            .query(
                &crate::repository::ProfileFilter {
                    subid: Some("s-sql".to_string()),
                    ..Default::default()
                },
                crate::repository::ProfileSort::IndexId,
                crate::repository::PageRequest {
                    cursor: 0,
                    page_size: u32::MAX,
                },
            )
            .unwrap()
            .items;
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn round_trips_all_extras_through_sqlite() {
        let mut repo = SqliteProfileRepository::open(":memory:").unwrap();
        let mut profile = synthetic_full_profile(1);
        profile.proto_extra.ss_method = Some("aes-256-gcm".into());
        profile.proto_extra.flow = Some("xtls-rprx-vision".into());
        profile.transport_extra.path = Some("/ws".into());
        profile.transport_extra.host = Some("example.com".into());
        profile.security.sni = Some("sni.example.com".into());
        profile.extra.insert("HeaderType".into(), json!("none"));
        repo.upsert(profile.clone()).unwrap();

        let loaded = repo.get(&profile.index_id).unwrap().unwrap();
        assert_eq!(loaded.proto_extra.ss_method.as_deref(), Some("aes-256-gcm"));
        assert_eq!(loaded.proto_extra.flow.as_deref(), Some("xtls-rprx-vision"));
        assert_eq!(loaded.transport_extra.path.as_deref(), Some("/ws"));
        assert_eq!(loaded.transport_extra.host.as_deref(), Some("example.com"));
        assert_eq!(loaded.security.sni.as_deref(), Some("sni.example.com"));
        assert_eq!(loaded.extra.get("HeaderType"), Some(&json!("none")));
    }

    #[test]
    fn subscription_replace_keeps_manual_nodes_and_clones_stats() {
        // R4-17 / D04+D05: a subscription replace only removes `IsSub = 1`
        // rows of the group, keeps a manual `IsSub = 0` node, and clones the
        // matched `ServerStatItem` row onto the new identity.
        let mut repo = SqliteProfileRepository::open(":memory:").unwrap();
        let mut old_sub = synthetic_full_profile(10);
        old_sub.subid = "s-A".into();
        old_sub.is_sub = true;
        let old_id = old_sub.index_id.clone();
        let mut manual = synthetic_full_profile(11);
        manual.subid = "s-A".into();
        manual.is_sub = false;
        manual.address = "198.51.100.99".into();
        let manual_id = manual.index_id.clone();
        repo.upsert(old_sub.clone()).unwrap();
        repo.upsert(manual.clone()).unwrap();

        let mut stat = RawRow::new("ServerStatItem");
        stat.set("IndexId", json!(old_id.clone()));
        stat.set("TotalUp", json!(111));
        stat.set("TotalDown", json!(222));
        stat.set("TodayUp", json!(1));
        stat.set("TodayDown", json!(2));
        stat.set("DateNow", json!(0));
        repo.store()
            .upsert_row(repo.store().connection(), &stat)
            .unwrap();

        let mut new_sub = old_sub.clone();
        new_sub.index_id = "new-sub-id".into();
        let (added, removed) = repo
            .replace_for_sub("s-A", vec![new_sub.clone()], true, false)
            .unwrap();
        assert_eq!(added, 1, "only the replacement set counts as added");
        assert_eq!(removed, 1, "only the IsSub=1 row is replaced");

        assert!(repo.get(&manual_id).unwrap().is_some(), "manual node kept");
        assert!(repo.get(&old_id).unwrap().is_none(), "old sub node removed");
        assert!(
            repo.get("new-sub-id").unwrap().is_some(),
            "new node present"
        );

        let cloned = repo
            .store()
            .query_rows(
                "SELECT * FROM \"ServerStatItem\" WHERE \"IndexId\" = ?1",
                &[&"new-sub-id"],
            )
            .unwrap();
        assert_eq!(cloned.len(), 1, "stat cloned to the new identity");
        assert_eq!(cloned[0].i64("TotalUp"), 111);
        assert_eq!(cloned[0].i64("TotalDown"), 222);
        // Upstream `CloneServerStatItem` copies; the old row stays.
        let old_stat = repo
            .store()
            .query_rows(
                "SELECT * FROM \"ServerStatItem\" WHERE \"IndexId\" = ?1",
                &[&old_id],
            )
            .unwrap();
        assert_eq!(old_stat.len(), 1);
    }

    #[test]
    fn paging_follows_cursor_over_ten_thousand_rows() {
        // R4-09 / D09: a store larger than one page must be read through the
        // real cursor without truncation or a fabricated end-of-list.
        let repo = SqliteProfileRepository::open(":memory:").unwrap();
        let total = 10_000usize;
        let profiles: Vec<Profile> = (0..total)
            .map(|i| {
                let mut p = synthetic_full_profile(i as u32 + 1);
                p.subid = "bulk".to_string();
                p
            })
            .collect();
        let (added, _) = repo.replace_for_sub("bulk", profiles, true, false).unwrap();
        assert_eq!(added, total);

        let page_size = 500u32;
        let mut cursor = 0usize;
        let mut seen = 0usize;
        let mut pages = 0usize;
        loop {
            let page = repo
                .query(
                    &crate::repository::ProfileFilter::default(),
                    crate::repository::ProfileSort::IndexId,
                    crate::repository::PageRequest { cursor, page_size },
                )
                .unwrap();
            assert!(page.items.len() <= page_size as usize, "page stays bounded");
            seen += page.items.len();
            pages += 1;
            match page.next_cursor {
                Some(next) => {
                    assert!(next > cursor, "cursor must advance");
                    cursor = next;
                }
                None => break,
            }
        }
        assert_eq!(seen, total, "every row is read, none truncated");
        assert!(pages > 1, "large store is read in bounded pages");
    }

    // ---- SP-21 async paged query data layer (red contracts) ----

    fn sp21_rows() -> usize {
        std::env::var("SP21_ROWS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5_000)
    }

    #[test]
    fn sp21_index_id_sort_is_stable_by_id_not_insertion_order() {
        // Cursor contract: ProfileSort::IndexId must order by IndexId so a
        // page walk is stable regardless of insertion order.
        let mut repo = SqliteProfileRepository::open(":memory:").unwrap();
        for i in [3u32, 1, 2] {
            let mut p = synthetic_full_profile(i);
            p.index_id = format!("sp21-stable-{i:05}");
            p.subid = "sp21".to_string();
            repo.upsert(p).unwrap();
        }
        let page = repo
            .query(
                &crate::repository::ProfileFilter::default(),
                crate::repository::ProfileSort::IndexId,
                crate::repository::PageRequest {
                    cursor: 0,
                    page_size: 10,
                },
            )
            .unwrap();
        let ids: Vec<_> = page.items.iter().map(|p| p.index_id.clone()).collect();
        assert_eq!(
            ids,
            vec![
                "sp21-stable-00001".to_string(),
                "sp21-stable-00002".to_string(),
                "sp21-stable-00003".to_string(),
            ]
        );
    }

    #[test]
    fn sp21_async_pages_cover_full_dataset_without_dup_or_miss() {
        use super::{AsyncPageRequest, ProfilePageQuery};
        use crate::repository::{ProfileFilter, ProfileSort};
        let total = sp21_rows();
        let pager = ProfilePageQuery::new();
        let repo = SqliteProfileRepository::open(":memory:").unwrap();
        let profiles: Vec<Profile> = (0..total)
            .map(|i| {
                let mut p = synthetic_full_profile(i as u32 + 1);
                p.index_id = format!("sp21-walk-{i:07}");
                p.subid = "sp21-walk".to_string();
                p
            })
            .collect();
        let (added, _) = repo
            .replace_for_sub("sp21-walk", profiles, true, false)
            .unwrap();
        assert_eq!(added, total);
        pager.notify_mutated();

        let cancel = domain::CancellationToken::new();
        let generation = pager.next_generation();
        let revision = pager.dataset_revision();
        let mut cursor = 0usize;
        let mut seen: Vec<String> = Vec::with_capacity(total);
        let mut pages = 0usize;
        let timer = std::time::Instant::now();
        loop {
            let request = AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::Remarks,
                cursor,
                page_size: 500,
                expected_revision: revision,
                generation,
            };
            let page = pager.query_page(&repo, &request, &cancel).unwrap();
            assert!(page.items.len() <= 500, "page stays bounded");
            assert_eq!(page.dataset_revision, revision);
            assert_eq!(page.generation, generation);
            seen.extend(page.items.iter().map(|p| p.index_id.clone()));
            pages += 1;
            match page.next_cursor {
                Some(next) => {
                    assert!(next > cursor, "cursor must advance");
                    cursor = next;
                }
                None => break,
            }
        }
        let elapsed = timer.elapsed();
        println!(
            "SP21 walk rows={total} pages={pages} elapsed_ms={}",
            elapsed.as_millis()
        );
        assert_eq!(seen.len(), total, "every row is read exactly once");
        let mut sorted = seen.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), total, "no duplicate rows across pages");
        assert!(pages > 1, "large store is read in bounded pages");
    }

    #[test]
    fn sp21_async_sort_uses_id_tiebreak_across_pages() {
        use super::{AsyncPageRequest, ProfilePageQuery};
        use crate::repository::{ProfileFilter, ProfileSort};
        let pager = ProfilePageQuery::new();
        let repo = SqliteProfileRepository::open(":memory:").unwrap();
        let profiles: Vec<Profile> = (0..120)
            .map(|i| {
                let mut p = synthetic_full_profile(i + 1);
                p.index_id = format!("sp21-tie-{i:05}");
                p.remarks = "same-remarks".to_string();
                p.subid = "sp21-tie".to_string();
                p
            })
            .collect();
        repo.replace_for_sub("sp21-tie", profiles, true, false)
            .unwrap();
        pager.notify_mutated();

        let cancel = domain::CancellationToken::new();
        let generation = pager.next_generation();
        let revision = pager.dataset_revision();
        let mut cursor = 0usize;
        let mut ids: Vec<String> = Vec::new();
        loop {
            let page = pager
                .query_page(
                    &repo,
                    &AsyncPageRequest {
                        filter: ProfileFilter::default(),
                        sort: ProfileSort::Remarks,
                        cursor,
                        page_size: 50,
                        expected_revision: revision,
                        generation,
                    },
                    &cancel,
                )
                .unwrap();
            ids.extend(page.items.iter().map(|p| p.index_id.clone()));
            match page.next_cursor {
                Some(next) => cursor = next,
                None => break,
            }
        }
        let mut expected = ids.clone();
        expected.sort();
        assert_eq!(ids, expected, "equal keys fall back to IndexId order");
    }

    #[test]
    fn sp21_async_cursor_is_invalidated_by_dataset_revision_change() {
        use super::{AsyncPageError, AsyncPageRequest, ProfilePageQuery};
        use crate::repository::{ProfileFilter, ProfileSort};
        let pager = ProfilePageQuery::new();
        let mut repo = SqliteProfileRepository::open(":memory:").unwrap();
        let profiles: Vec<Profile> = (0..300)
            .map(|i| {
                let mut p = synthetic_full_profile(i + 1);
                p.index_id = format!("sp21-rev-{i:05}");
                p.subid = "sp21-rev".to_string();
                p
            })
            .collect();
        repo.replace_for_sub("sp21-rev", profiles, true, false)
            .unwrap();
        pager.notify_mutated();

        let cancel = domain::CancellationToken::new();
        let generation = pager.next_generation();
        let revision = pager.dataset_revision();
        let first = pager
            .query_page(
                &repo,
                &AsyncPageRequest {
                    filter: ProfileFilter::default(),
                    sort: ProfileSort::IndexId,
                    cursor: 0,
                    page_size: 100,
                    expected_revision: revision,
                    generation,
                },
                &cancel,
            )
            .unwrap();
        let cursor = first.next_cursor.expect("more pages remain");

        // A write bumps the dataset revision; the old cursor must not be
        // silently reused (no skipped/repeated rows).
        let mut extra = synthetic_full_profile(999_001);
        extra.index_id = "sp21-rev-added".to_string();
        extra.subid = "sp21-rev".to_string();
        repo.upsert(extra).unwrap();
        pager.notify_mutated();

        let stale = pager.query_page(
            &repo,
            &AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor,
                page_size: 100,
                expected_revision: revision,
                generation,
            },
            &cancel,
        );
        match stale {
            Err(AsyncPageError::StaleCursor { expected, actual }) => {
                assert_eq!(expected, revision);
                assert_eq!(actual, pager.dataset_revision());
            }
            other => panic!("expected StaleCursor, got {other:?}"),
        }
    }

    #[test]
    fn sp21_async_cancel_is_immediate_and_drops_the_late_result() {
        use super::{AsyncPageError, AsyncPageRequest, ProfilePageQuery};
        use crate::repository::{ProfileFilter, ProfileSort};
        let pager = ProfilePageQuery::new();
        let repo = SqliteProfileRepository::open(":memory:").unwrap();
        let profiles: Vec<Profile> = (0..1_000)
            .map(|i| {
                let mut p = synthetic_full_profile(i + 1);
                p.index_id = format!("sp21-cancel-{i:05}");
                p.subid = "sp21-cancel".to_string();
                p
            })
            .collect();
        repo.replace_for_sub("sp21-cancel", profiles, true, false)
            .unwrap();
        pager.notify_mutated();

        let cancel = domain::CancellationToken::new();
        cancel.cancel();
        // Cancelling before the query must surface immediately, not after a
        // full read.
        let outcome = pager.query_page(
            &repo,
            &AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor: 0,
                page_size: 500,
                expected_revision: pager.dataset_revision(),
                generation: pager.next_generation(),
            },
            &cancel,
        );
        assert_eq!(outcome.unwrap_err(), AsyncPageError::Cancelled);
    }

    #[test]
    fn sp21_async_superseded_generation_is_dropped() {
        use super::{AsyncPageError, AsyncPageRequest, ProfilePageQuery};
        use crate::repository::{ProfileFilter, ProfileSort};
        let pager = ProfilePageQuery::new();
        let repo = SqliteProfileRepository::open(":memory:").unwrap();
        let profiles: Vec<Profile> = (0..200)
            .map(|i| {
                let mut p = synthetic_full_profile(i + 1);
                p.index_id = format!("sp21-gen-{i:05}");
                p.subid = "sp21-gen".to_string();
                p
            })
            .collect();
        repo.replace_for_sub("sp21-gen", profiles, true, false)
            .unwrap();
        pager.notify_mutated();

        let cancel = domain::CancellationToken::new();
        let old_generation = pager.next_generation();
        let revision = pager.dataset_revision();
        // A newer request supersedes the in-flight one; its late result must
        // be dropped instead of overwriting the latest view.
        let _new_generation = pager.next_generation();
        let late = pager.query_page(
            &repo,
            &AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor: 0,
                page_size: 100,
                expected_revision: revision,
                generation: old_generation,
            },
            &cancel,
        );
        assert_eq!(late.unwrap_err(), AsyncPageError::Superseded);
    }

    #[test]
    fn sp21_async_worker_runs_off_caller_thread_and_cancel_returns_fast() {
        use super::{AsyncPageError, AsyncPageRequest, AsyncPageWorker, ProfilePageQuery};
        use crate::repository::{ProfileFilter, ProfileSort};
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sp21-worker.db");
        let repo = SqliteProfileRepository::open(&path).unwrap();
        let total = 5_000usize;
        let profiles: Vec<Profile> = (0..total)
            .map(|i| {
                let mut p = synthetic_full_profile(i as u32 + 1);
                p.index_id = format!("sp21-worker-{i:07}");
                p.subid = "sp21-worker".to_string();
                p
            })
            .collect();
        repo.replace_for_sub("sp21-worker", profiles, true, false)
            .unwrap();

        let pager = Arc::new(ProfilePageQuery::new());
        pager.notify_mutated();
        let worker = AsyncPageWorker::open_readonly(&path, pager.clone()).unwrap();
        let caller = std::thread::current().id();
        let revision = pager.dataset_revision();
        let generation = pager.next_generation();

        // The query executes on the worker thread: the caller blocks only on
        // a bounded page rendezvous, never on a full-table read.
        let mut handle = worker.submit(
            AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor: 0,
                page_size: 200,
                expected_revision: revision,
                generation,
            },
            domain::CancellationToken::new(),
        );
        let page = handle
            .wait(std::time::Duration::from_secs(30))
            .expect("bounded page arrives")
            .expect("page query succeeds");
        assert_eq!(page.items.len(), 200);
        assert_ne!(page.worker_thread, caller, "work ran off the caller thread");
        assert_eq!(page.dataset_revision, revision);

        // Cancel returns immediately (sync) and the late result is discarded.
        let cancel = domain::CancellationToken::new();
        let mut late = worker.submit(
            AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor: 0,
                page_size: 200,
                expected_revision: revision,
                generation,
            },
            cancel.clone(),
        );
        let timer = std::time::Instant::now();
        late.cancel();
        assert!(
            timer.elapsed() < std::time::Duration::from_secs(1),
            "cancel stays immediately responsive"
        );
        drop(worker);
        match late.wait(std::time::Duration::from_secs(30)) {
            Err(AsyncPageError::Cancelled) | Ok(_) => {}
            other => panic!("cancelled query must not deliver a live page, got {other:?}"),
        }
    }

    // ---- SP-21 continuation: real-SQLite file-backed scale proof ----
    //
    // The prep tests above run against `:memory:` through `query_page`
    // directly. These tests prove the real end-to-end path: a file-backed
    // SQLite database (WAL, synthetic rows only, no network) read through
    // the background `AsyncPageWorker` connection at 10k and 100k rows,
    // covering page/cursor walks, ID tie ordering, revision-stale cursors,
    // cancel, and cross-page select-all (the full id set, exactly once).

    fn sp21_seed_file_db(path: &std::path::Path, total: usize, same_remarks: bool) -> Vec<String> {
        let repo = SqliteProfileRepository::open(path).unwrap();
        let profiles: Vec<Profile> = (0..total)
            .map(|i| {
                let mut p = synthetic_full_profile(i as u32 + 1);
                p.index_id = format!("sp21-scale-{i:07}");
                if same_remarks {
                    p.remarks = "sp21-same-remarks".to_string();
                }
                p.subid = "sp21-scale".to_string();
                p
            })
            .collect();
        let expected: Vec<String> = profiles.iter().map(|p| p.index_id.clone()).collect();
        let (added, _) = repo
            .replace_for_sub("sp21-scale", profiles, true, false)
            .unwrap();
        assert_eq!(added, total);
        expected
    }

    fn sp21_worker_walk_all(
        worker: &AsyncPageWorker,
        revision: u64,
        generation: u64,
        page_size: u32,
    ) -> (Vec<String>, usize) {
        use super::AsyncPageRequest;
        use crate::repository::{ProfileFilter, ProfileSort};
        let mut cursor = 0usize;
        let mut seen: Vec<String> = Vec::new();
        let mut pages = 0usize;
        loop {
            let mut handle = worker.submit(
                AsyncPageRequest {
                    filter: ProfileFilter::default(),
                    sort: ProfileSort::Remarks,
                    cursor,
                    page_size,
                    expected_revision: revision,
                    generation,
                },
                CancellationToken::new(),
            );
            let page = handle
                .wait(std::time::Duration::from_secs(120))
                .expect("bounded page arrives")
                .expect("page query succeeds");
            assert!(page.items.len() <= page_size as usize, "page stays bounded");
            assert_eq!(page.dataset_revision, revision);
            assert_eq!(page.generation, generation);
            seen.extend(page.items.iter().map(|p| p.index_id.clone()));
            pages += 1;
            match page.next_cursor {
                Some(next) => {
                    assert!(next > cursor, "cursor must advance");
                    cursor = next;
                }
                None => break,
            }
        }
        (seen, pages)
    }

    #[test]
    fn sp21_real_sqlite_10k_worker_walk_covers_every_row_once() {
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sp21-scale-10k.db");
        let total = 10_000usize;
        let mut expected = sp21_seed_file_db(&path, total, false);
        expected.sort();

        let pager = Arc::new(ProfilePageQuery::new());
        pager.notify_mutated();
        let worker = AsyncPageWorker::open_readonly(&path, pager.clone()).unwrap();
        let revision = pager.dataset_revision();
        let generation = pager.next_generation();

        let timer = std::time::Instant::now();
        let (seen, pages) = sp21_worker_walk_all(&worker, revision, generation, 500);
        let elapsed = timer.elapsed();
        println!(
            "SP21 real-sqlite 10k worker walk rows={total} pages={pages} elapsed_ms={}",
            elapsed.as_millis()
        );

        // Cross-page select-all: the walked id set is exactly the store,
        // every row once, none missing, none repeated.
        let mut seen_sorted = seen;
        seen_sorted.sort();
        assert_eq!(seen_sorted, expected, "every row exactly once");
        assert!(pages > 1, "large store is read in bounded pages");
        drop(worker);
    }

    #[test]
    fn sp21_real_sqlite_100k_worker_walk_covers_every_row_once() {
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sp21-scale-100k.db");
        let total = 100_000usize;
        let mut expected = sp21_seed_file_db(&path, total, false);
        expected.sort();

        let pager = Arc::new(ProfilePageQuery::new());
        pager.notify_mutated();
        let worker = AsyncPageWorker::open_readonly(&path, pager.clone()).unwrap();
        let revision = pager.dataset_revision();
        let generation = pager.next_generation();

        let timer = std::time::Instant::now();
        let (seen, pages) = sp21_worker_walk_all(&worker, revision, generation, 500);
        let elapsed = timer.elapsed();
        println!(
            "SP21 real-sqlite 100k worker walk rows={total} pages={pages} elapsed_ms={}",
            elapsed.as_millis()
        );

        let mut seen_sorted = seen;
        seen_sorted.sort();
        assert_eq!(seen_sorted, expected, "every row exactly once");
        assert!(pages > 1, "large store is read in bounded pages");
        drop(worker);
    }

    #[test]
    fn sp21_real_sqlite_id_tiebreak_is_stable_across_worker_pages() {
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sp21-scale-tie.db");
        // Equal sort keys: page order must fall back to `IndexId` on every
        // worker page, not just within one page.
        let expected = sp21_seed_file_db(&path, 5_000, true);

        let pager = Arc::new(ProfilePageQuery::new());
        pager.notify_mutated();
        let worker = AsyncPageWorker::open_readonly(&path, pager.clone()).unwrap();
        let (seen, _) = sp21_worker_walk_all(
            &worker,
            pager.dataset_revision(),
            pager.next_generation(),
            500,
        );
        let mut sorted = expected.clone();
        sorted.sort();
        assert_eq!(seen, sorted, "equal keys fall back to IndexId order");
        drop(worker);
    }

    #[test]
    fn sp21_real_sqlite_worker_stale_revision_and_cancel() {
        use super::{AsyncPageError, AsyncPageRequest};
        use crate::repository::{ProfileFilter, ProfileSort};
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sp21-scale-rev.db");
        sp21_seed_file_db(&path, 2_000, false);

        let pager = Arc::new(ProfilePageQuery::new());
        pager.notify_mutated();
        let worker = AsyncPageWorker::open_readonly(&path, pager.clone()).unwrap();
        let revision = pager.dataset_revision();
        let generation = pager.next_generation();

        // First worker page succeeds and reports a cursor.
        let mut first = worker.submit(
            AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor: 0,
                page_size: 500,
                expected_revision: revision,
                generation,
            },
            CancellationToken::new(),
        );
        let page = first
            .wait(std::time::Duration::from_secs(120))
            .expect("bounded page arrives")
            .expect("first page succeeds");
        let cursor = page.next_cursor.expect("more pages remain");

        // A committed write bumps the dataset revision; the old cursor must
        // fail loudly on the real file instead of skipping/repeating rows.
        let mut writer = SqliteProfileRepository::open(&path).unwrap();
        let mut extra = synthetic_full_profile(999_001);
        extra.index_id = "sp21-scale-added".to_string();
        extra.subid = "sp21-scale".to_string();
        writer.upsert(extra).unwrap();
        drop(writer);
        pager.notify_mutated();

        let mut stale = worker.submit(
            AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor,
                page_size: 500,
                expected_revision: revision,
                generation,
            },
            CancellationToken::new(),
        );
        match stale
            .wait(std::time::Duration::from_secs(120))
            .expect("worker answers")
        {
            Err(AsyncPageError::StaleCursor { expected, actual }) => {
                assert_eq!(expected, revision);
                assert_eq!(actual, pager.dataset_revision());
            }
            other => panic!("expected StaleCursor, got {other:?}"),
        }

        // A pre-cancelled token surfaces without a page on the real file.
        let cancelled_token = CancellationToken::new();
        cancelled_token.cancel();
        let mut cancelled = worker.submit(
            AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor: 0,
                page_size: 500,
                expected_revision: pager.dataset_revision(),
                generation,
            },
            cancelled_token,
        );
        assert_eq!(
            cancelled
                .wait(std::time::Duration::from_secs(120))
                .expect("worker answers"),
            Err(AsyncPageError::Cancelled)
        );

        // `cancel()` on the handle is synchronous and the late result is
        // discarded instead of delivered.
        let mut late = worker.submit(
            AsyncPageRequest {
                filter: ProfileFilter::default(),
                sort: ProfileSort::IndexId,
                cursor: 0,
                page_size: 500,
                expected_revision: pager.dataset_revision(),
                generation,
            },
            CancellationToken::new(),
        );
        let timer = std::time::Instant::now();
        late.cancel();
        assert!(
            timer.elapsed() < std::time::Duration::from_secs(1),
            "cancel stays immediately responsive"
        );
        drop(worker);
        match late.wait(std::time::Duration::from_secs(30)) {
            Err(AsyncPageError::Cancelled) | Ok(_) => {}
            other => panic!("cancelled query must not deliver a live page, got {other:?}"),
        }
    }

    #[test]
    fn sp21_async_page_size_is_bounded() {
        use super::{AsyncPageRequest, ProfilePageQuery, ASYNC_PAGE_MAX_SIZE};
        use crate::repository::{ProfileFilter, ProfileSort};
        let pager = ProfilePageQuery::new();
        let repo = SqliteProfileRepository::open(":memory:").unwrap();
        let profiles: Vec<Profile> = (0..100)
            .map(|i| {
                let mut p = synthetic_full_profile(i + 1);
                p.index_id = format!("sp21-bound-{i:05}");
                p.subid = "sp21-bound".to_string();
                p
            })
            .collect();
        repo.replace_for_sub("sp21-bound", profiles, true, false)
            .unwrap();
        pager.notify_mutated();

        let cancel = domain::CancellationToken::new();
        let page = pager
            .query_page(
                &repo,
                &AsyncPageRequest {
                    filter: ProfileFilter::default(),
                    sort: ProfileSort::IndexId,
                    cursor: 0,
                    page_size: u32::MAX,
                    expected_revision: pager.dataset_revision(),
                    generation: pager.next_generation(),
                },
                &cancel,
            )
            .unwrap();
        assert!(
            page.items.len() <= ASYNC_PAGE_MAX_SIZE as usize,
            "page is clamped to the bounded size"
        );
    }
}
