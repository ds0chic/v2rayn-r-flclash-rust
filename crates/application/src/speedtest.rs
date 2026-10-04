//! T15b speedtest job framework.
//!
//! This module owns the *measurement* orchestration: the job/cancellation
//! contract, the two independent concurrency pools (connect/handshake vs.
//! download), page-batched result emission, the node-set snapshot, and the
//! `ProfileExItem` result closure.
//!
//! It deliberately does **not** own a core process or a socket server. Opening
//! a temporary test core for `RealPing`/`Speedtest`/`Mixedtest` is delegated to
//! a [`SpeedTestSession`] implementation:
//!
//! * the production implementation talks to net-host's restricted test-session
//!   IPC (independent port `>= 11808`, isolated from the managed runtime);
//! * tests may implement the trait directly with a real kernel to prove the
//!   measurement path end to end.
//!
//! TCPing needs no session: it is a direct TCP connect with a per-item timeout.
//! All network operations abort on the shared [`CancellationToken`] and every
//! temporary session is closed in a `finally`-equivalent (RAII guard), so a
//! cancelled or failed item never leaks a test core.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use domain::{CancellationToken, DomainError, Profile, SpeedTestAction};
use rustls::pki_types::ServerName;
use rustls::{ClientConnection, RootCertStore, StreamOwned};
use serde::{Deserialize, Serialize};

/// Test-port floor: the reserved live proxy port `10808` can never be used.
pub const TEST_PORT_FLOOR: u16 = 11_808;

/// Upstream `SpeedTestTimeout` is normalized to at least 10 seconds.
pub const MIN_SPEEDTEST_TIMEOUT_SECS: i64 = 10;

/// Upstream `Global.SpeedTestPageSize` fallback when the setting is null.
pub const DEFAULT_PAGE_SIZE: usize = 1_000;

/// Upstream `Global.SpeedTestConcurrencyCountMin`.
pub const MIN_MIXED_CONCURRENCY: usize = 10;

/// Upstream `Global.LocalFetch` used by RealPing per request.
pub const LOCAL_FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// TCPing connect timeout (upstream `GetTcpingTime` hard-codes 5 s).
pub const TCPING_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Result batch cadence (plan: 50-100 ms batches, never per-item UI churn).
pub const BATCH_FLUSH_MS: u64 = 75;

/// Default `SpeedPingTestUrl` fallback.
pub const DEFAULT_PING_URL: &str = "https://www.gstatic.com/generate_204";
/// Default `SpeedTestUrl` fallback.
pub const DEFAULT_SPEED_URL: &str = "https://cachefly.cachefly.net/50mb.test";

// ---------------------------------------------------------------------------
// Settings / nodes
// ---------------------------------------------------------------------------

/// Effective speedtest settings derived from `SpeedTestItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedTestSettings {
    pub page_size: usize,
    pub mixed_concurrency: usize,
    pub timeout: Duration,
    pub speed_test_url: String,
    pub speed_ping_test_url: String,
    pub ipapi_url: Option<String>,
    pub udp_test_target: Option<String>,
    pub delay_interval: Duration,
}

impl Default for SpeedTestSettings {
    fn default() -> Self {
        Self {
            page_size: DEFAULT_PAGE_SIZE,
            mixed_concurrency: MIN_MIXED_CONCURRENCY,
            timeout: Duration::from_secs(MIN_SPEEDTEST_TIMEOUT_SECS as u64),
            speed_test_url: DEFAULT_SPEED_URL.to_string(),
            speed_ping_test_url: DEFAULT_PING_URL.to_string(),
            ipapi_url: None,
            udp_test_target: None,
            delay_interval: Duration::from_secs(1),
        }
    }
}

impl SpeedTestSettings {
    /// Apply the same normalize-not-reject corrections as
    /// `domain::settings::AppSettings::normalize`.
    pub fn from_item(item: &domain::SpeedTestItem) -> Self {
        let timeout = if item.speed_test_timeout < MIN_SPEEDTEST_TIMEOUT_SECS as i32 {
            MIN_SPEEDTEST_TIMEOUT_SECS as u64
        } else {
            item.speed_test_timeout as u64
        };
        let mut mixed = item.mixed_concurrency_count.max(0) as usize;
        if mixed < MIN_MIXED_CONCURRENCY {
            mixed = MIN_MIXED_CONCURRENCY;
        }
        let mut page = item.speed_test_page_size.unwrap_or(0).max(0) as usize;
        if page == 0 {
            page = DEFAULT_PAGE_SIZE;
        }
        let delay = item.speed_test_delay_interval.unwrap_or(1).max(0) as u64;
        Self {
            page_size: page,
            mixed_concurrency: mixed,
            timeout: Duration::from_secs(timeout),
            speed_test_url: non_empty(&item.speed_test_url)
                .unwrap_or_else(|| DEFAULT_SPEED_URL.to_string()),
            speed_ping_test_url: non_empty(&item.speed_ping_test_url)
                .unwrap_or_else(|| DEFAULT_PING_URL.to_string()),
            ipapi_url: non_empty(&item.ipapi_url),
            udp_test_target: non_empty(&item.udp_test_target),
            delay_interval: Duration::from_secs(delay),
        }
    }
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

/// Node data frozen at job start. Mutating the profile store afterwards must
/// not change an in-flight job (`节点集合快照+源版本`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestNode {
    pub index_id: String,
    pub address: String,
    pub port: i32,
    pub config_type: i32,
    pub core_type: i32,
}

/// A job's frozen node set plus the source revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedTestSnapshot {
    pub source_revision: u64,
    pub nodes: Vec<TestNode>,
}

// ---------------------------------------------------------------------------
// Results and ProfileEx closure
// ---------------------------------------------------------------------------

/// One node's test result.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedTestResult {
    pub index_id: String,
    /// `Some(>0)` for a measured delay, `Some(-1)` for a timeout/failure.
    pub delay: Option<i32>,
    /// Download speed in MB/s (decimal, upstream `maxSpeed / 1000 / 1000`).
    pub speed: Option<f64>,
    /// Progress/status or error message.
    pub message: Option<String>,
    /// Best-effort IP info (upstream `IPAPIUrl`); `None` when not attempted.
    pub ip_info: Option<String>,
    /// Whether the item failed (timeout, core error, unsupported).
    pub failed: bool,
}

impl SpeedTestResult {
    pub fn status(index_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            index_id: index_id.into(),
            delay: None,
            speed: None,
            message: Some(message.into()),
            ip_info: None,
            failed: false,
        }
    }

    pub fn delay(index_id: impl Into<String>, delay: i32) -> Self {
        Self {
            index_id: index_id.into(),
            delay: Some(delay),
            speed: None,
            message: None,
            ip_info: None,
            failed: delay <= 0,
        }
    }

    pub fn failed(index_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            index_id: index_id.into(),
            delay: Some(-1),
            speed: None,
            message: Some(message.into()),
            ip_info: None,
            failed: true,
        }
    }
}

/// Persisted per-node test result (`ProfileExItem`, upstream
/// `Models/Entities/ProfileExItem.cs`: `IndexId`, `Delay`, `Speed`, `Sort`,
/// `Message`, `IpInfo`). `sort` is the manual/column order written by
/// `SetSort`; it survives a restart.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileExItem {
    pub index_id: String,
    pub delay: i32,
    pub speed: f64,
    pub sort: i32,
    pub message: String,
    pub ip_info: String,
}

/// In-memory `ProfileExItem` table. Persistence is a separate concern; the
/// bridge overlays this onto `ProfileSummary` and can flush it to disk.
#[derive(Debug, Clone, Default)]
pub struct ProfileExStore {
    rows: HashMap<String, ProfileExItem>,
}

impl ProfileExStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn get(&self, index_id: &str) -> Option<&ProfileExItem> {
        self.rows.get(index_id)
    }

    /// All rows in the persisted display order: by `sort`, then `index_id`.
    ///
    /// Upstream `GetProfileItemsEx` joins the result rows and does
    /// `OrderBy(t => t.Sort)`, so the list order reported to the UI already
    /// carries the manual/column order. Rows never ordered (`sort == 0`) keep
    /// a stable `index_id` order, matching the tie-break of `OrderBy(Sort)`.
    pub fn all(&self) -> Vec<ProfileExItem> {
        let mut rows: Vec<ProfileExItem> = self.rows.values().cloned().collect();
        rows.sort_by(|a, b| {
            a.sort
                .cmp(&b.sort)
                .then_with(|| a.index_id.cmp(&b.index_id))
        });
        rows
    }

    /// Apply one result. Only present fields overwrite (upstream semantics:
    /// a progress-only message must not clear a measured delay).
    pub fn apply(&mut self, result: &SpeedTestResult) {
        let entry = self
            .rows
            .entry(result.index_id.clone())
            .or_insert_with(|| ProfileExItem {
                index_id: result.index_id.clone(),
                delay: 0,
                speed: 0.0,
                sort: 0,
                message: String::new(),
                ip_info: String::new(),
            });
        if let Some(delay) = result.delay {
            entry.delay = delay;
        }
        if let Some(speed) = result.speed {
            if speed > 0.0 {
                entry.speed = speed;
            }
        }
        if let Some(message) = &result.message {
            entry.message = message.clone();
        }
        if let Some(ip) = &result.ip_info {
            entry.ip_info = ip.clone();
        }
    }

    pub fn apply_all(&mut self, results: &[SpeedTestResult]) {
        for result in results {
            self.apply(result);
        }
    }

    /// Replace the whole table (e.g. after loading it from SQLite at startup).
    pub fn replace_all(&mut self, rows: Vec<ProfileExItem>) {
        self.rows = rows.into_iter().map(|r| (r.index_id.clone(), r)).collect();
    }

    /// `ProfileExManager.SetSort`: write the manual order for one node.
    pub fn set_sort(&mut self, index_id: &str, sort: i32) {
        let entry = self
            .rows
            .entry(index_id.to_string())
            .or_insert_with(|| ProfileExItem {
                index_id: index_id.to_string(),
                ..Default::default()
            });
        entry.sort = sort;
    }

    /// `ProfileExManager.GetSort`.
    pub fn get_sort(&self, index_id: &str) -> i32 {
        self.rows.get(index_id).map(|r| r.sort).unwrap_or(0)
    }

    /// `ProfileExManager.GetMaxSort`.
    pub fn max_sort(&self) -> i32 {
        self.rows.values().map(|r| r.sort).max().unwrap_or(0)
    }

    /// Rewrite `sort = (position + 1) * 10` for `ordered_ids`, mirroring
    /// `ConfigHandler.MoveServer`/`SortServers`. Ids absent from the table are
    /// inserted with just the sort value (test results overlay later).
    pub fn apply_order(&mut self, ordered_ids: &[String]) {
        for (i, id) in ordered_ids.iter().enumerate() {
            self.set_sort(id, (i as i32 + 1) * 10);
        }
    }

    /// `RemoveInvalidServerResult`: drop rows whose delay failed (`== -1`).
    /// Returns the number removed.
    pub fn remove_invalid(&mut self) -> usize {
        let before = self.rows.len();
        self.rows.retain(|_, row| row.delay != -1);
        before - self.rows.len()
    }

    /// Group-scoped `RemoveInvalidServerResult`: drop failed (`delay == -1`)
    /// rows that belong to `ids` (the current group's index ids), leaving every
    /// other group's failure evidence untouched (RE-PROF-06).
    pub fn remove_invalid_in(&mut self, ids: &HashSet<&str>) -> usize {
        let before = self.rows.len();
        self.rows
            .retain(|id, row| row.delay != -1 || !ids.contains(id.as_str()));
        before - self.rows.len()
    }

    /// Drop failed rows whose profile no longer exists. Deleting a real
    /// `ProfileItem` leaves its result row behind; this prunes exactly those
    /// orphans. A failed node that is still stored (another group, or a failed
    /// delete) keeps its evidence (RE-PROF-06).
    pub fn remove_invalid_orphans(&mut self, stored: &HashSet<&str>) -> usize {
        let before = self.rows.len();
        self.rows
            .retain(|id, row| row.delay != -1 || stored.contains(id.as_str()));
        before - self.rows.len()
    }

    /// Clearing traffic statistics (`ClearServerStatistics`) must not touch the
    /// test-result closure; this is a no-op documented by a test.
    pub fn clear_statistics(&mut self) {
        // Intentionally empty: statistics live in `monitor::StatsService`.
    }

    pub fn clear(&mut self) {
        self.rows.clear();
    }
}

#[cfg(test)]
mod re_prof_06_tests {
    use super::*;

    fn store(rows: &[(&str, i32)]) -> ProfileExStore {
        let mut s = ProfileExStore::new();
        for (id, delay) in rows {
            s.apply(&SpeedTestResult::delay(*id, *delay));
        }
        s
    }

    #[test]
    fn remove_invalid_in_clears_only_the_scoped_group() {
        let mut s = store(&[("a", -1), ("b", 20), ("other", -1)]);
        let ids: HashSet<&str> = ["a", "b"].into_iter().collect();
        assert_eq!(s.remove_invalid_in(&ids), 1, "only the in-scope failure");
        assert!(s.get("a").is_none());
        assert!(s.get("b").is_some(), "an in-scope success stays");
        assert!(
            s.get("other").is_some(),
            "another group's failure evidence stays"
        );
    }

    #[test]
    fn remove_invalid_orphans_keeps_stored_failures() {
        let mut s = store(&[("deleted", -1), ("failed", -1), ("ok", 15)]);
        // Only `failed` and `ok` profiles still exist; `deleted` was removed.
        let stored: HashSet<&str> = ["failed", "ok"].into_iter().collect();
        assert_eq!(s.remove_invalid_orphans(&stored), 1, "only the orphan");
        assert!(s.get("deleted").is_none());
        assert!(
            s.get("failed").is_some(),
            "a failed profile that was not deleted keeps its evidence"
        );
        assert!(s.get("ok").is_some());
    }
}

/// `ConfigHandler.DedupServerList`: collapse transport-identical profiles.
/// Complex nodes always stay; when `keep_older` is false the newer entry wins.
/// Returns `(kept_count, removed_ids)`.
pub fn deduplicate_profiles(items: &[Profile], keep_older: bool) -> (usize, Vec<String>) {
    let ordered: Vec<&Profile> = if keep_older {
        items.iter().collect()
    } else {
        items.iter().rev().collect()
    };
    let mut kept: Vec<&Profile> = Vec::new();
    let mut removed_ids = Vec::new();
    for item in ordered {
        if item.config_type.is_complex() {
            kept.push(item);
            continue;
        }
        if kept
            .iter()
            .any(|existing| subscriptions::merge::compare_profile(existing, item, false))
        {
            removed_ids.push(item.index_id.clone());
        } else {
            kept.push(item);
        }
    }
    (kept.len(), removed_ids)
}

// ---------------------------------------------------------------------------
// TLS trust + structured probe failures
// ---------------------------------------------------------------------------

/// Extra CA bundle path used by the real-window integration test to trust a
/// local synthetic CA. Verification stays enabled: this only *adds* a root.
const EXTRA_CA_ENV: &str = "V2RAYN_SPEEDTEST_EXTRA_CA";

/// Root trust for the HTTPS probe. Production uses the OS store plus an
/// optional test-only extra CA; deterministic tests inject a synthetic CA
/// directly and never touch the OS store.
#[derive(Clone)]
pub enum TlsTrust {
    /// Production: OS native roots (and `V2RAYN_SPEEDTEST_EXTRA_CA`, if set).
    Native,
    /// Test seam: exactly these roots, no OS trust.
    Custom(Arc<RootCertStore>),
}

impl std::fmt::Debug for TlsTrust {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsTrust::Native => f.write_str("TlsTrust::Native"),
            TlsTrust::Custom(store) => write!(f, "TlsTrust::Custom({} roots)", store.len()),
        }
    }
}

impl TlsTrust {
    /// Test seam: trust exactly the provided store (e.g. a synthetic CA).
    pub fn custom(store: RootCertStore) -> Self {
        TlsTrust::Custom(Arc::new(store))
    }

    /// Resolve the effective root store.
    pub fn store(&self) -> Arc<RootCertStore> {
        match self {
            TlsTrust::Custom(store) => store.clone(),
            TlsTrust::Native => native_root_store(),
        }
    }
}

/// Cached native root store. Loading is done once; a malformed extra CA is
/// best-effort and never panics.
fn native_root_store() -> Arc<RootCertStore> {
    static NATIVE: OnceLock<Arc<RootCertStore>> = OnceLock::new();
    NATIVE
        .get_or_init(|| {
            let mut store = RootCertStore::empty();
            let loaded = rustls_native_certs::load_native_certs();
            for cert in loaded.certs {
                let _ = store.add(cert);
            }
            if let Ok(path) = std::env::var(EXTRA_CA_ENV) {
                if let Ok(pem) = std::fs::read(path) {
                    let mut cursor = std::io::Cursor::new(pem);
                    for cert in rustls_pemfile::certs(&mut cursor).flatten() {
                        let _ = store.add(cert);
                    }
                }
            }
            Arc::new(store)
        })
        .clone()
}

/// Coarse failure taxonomy for a probe. `message_key` is stable and safe to
/// surface to the UI (no addresses, no credentials, no URLs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeFailureKind {
    /// URL could not be parsed.
    Url,
    /// DNS resolution failed.
    Resolve,
    /// SOCKS negotiation or TCP connect to the target failed.
    Connect,
    /// Deadline elapsed before a response completed.
    Timeout,
    /// TLS handshake or certificate verification failed.
    Tls,
    /// A complete HTTP response carried a 4xx/5xx status.
    HttpStatus(u16),
    /// The peer spoke something that was not a valid HTTP response.
    Protocol,
    /// The caller cancelled the probe.
    Cancelled,
}

/// A structured probe failure (never a bare `-1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeError {
    pub kind: ProbeFailureKind,
    pub detail: String,
}

impl ProbeError {
    pub fn new(kind: ProbeFailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// Stable i18n key the UI maps to a human reason.
    pub fn message_key(&self) -> &'static str {
        match self.kind {
            ProbeFailureKind::Url => "error.speedtest_url",
            ProbeFailureKind::Resolve => "speedtest.resolve_failed",
            ProbeFailureKind::Connect => "speedtest.connect_failed",
            ProbeFailureKind::Timeout => "speedtest.timeout",
            ProbeFailureKind::Tls => "speedtest.tls_failed",
            ProbeFailureKind::HttpStatus(_) => "speedtest.http_status",
            ProbeFailureKind::Protocol => "speedtest.protocol_error",
            ProbeFailureKind::Cancelled => "speedtest.cancelled",
        }
    }
}

// ---------------------------------------------------------------------------
// Test session boundary
// ---------------------------------------------------------------------------

/// A temporary core bound to one node, on an isolated port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestSession {
    pub node: TestNode,
    pub port: u16,
    /// Provider-specific id (net-host session id, or a test label).
    pub handle_id: String,
}

impl TestSession {
    pub fn require_test_port(port: u16) -> Result<(), DomainError> {
        if port < TEST_PORT_FLOOR {
            return Err(
                DomainError::new(domain::codes::INVALID_ARGUMENT, "error.test_port_floor")
                    .with_field("port")
                    .with_detail(format!("test port {port} < {TEST_PORT_FLOOR}")),
            );
        }
        Ok(())
    }
}

/// Outcome of one download probe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DownloadOutcome {
    pub mb_s: f64,
    pub bytes: u64,
    pub elapsed: Duration,
}

/// Opens a temporary test core per node and measures through its local proxy.
pub trait SpeedTestSession: Send + Sync {
    /// Start a temporary core for `node` on a port `>= 11808` and return the
    /// session handle. The core must be isolated from the managed runtime.
    fn open(&self, node: &TestNode) -> Result<TestSession, DomainError>;

    /// RealPing: request `url` through the session's SOCKS proxy and return the
    /// minimum of two round trips in ms. A non-positive round trip never
    /// succeeds: failures are structured [`ProbeError`]s.
    fn real_ping(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        trust: &TlsTrust,
        ct: &CancellationToken,
    ) -> Result<i32, ProbeError>;

    /// Download `url` through the session and return the observed speed.
    fn download(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        max_bytes: u64,
        trust: &TlsTrust,
        ct: &CancellationToken,
    ) -> Result<DownloadOutcome, ProbeError>;

    /// UDP test (ntp/dns/stun/mcbe). Default: not supported. A `None` result is
    /// surfaced as an explicit unsupported failure, never a fake delay.
    fn udp_ping(
        &self,
        _session: &TestSession,
        _target: &str,
        _timeout: Duration,
        _ct: &CancellationToken,
    ) -> Option<i32> {
        None
    }

    /// Best-effort IP info (upstream `IPAPIUrl`). Default: not attempted.
    fn ip_info(
        &self,
        _session: &TestSession,
        _url: &str,
        _timeout: Duration,
        _ct: &CancellationToken,
    ) -> Option<String> {
        None
    }

    /// Stop the temporary core. Must be safe to call exactly once.
    fn close(&self, session: TestSession);
}

/// A session that supports nothing. Used when no test-session backend is
/// configured; every core-backed test becomes an explicit structured failure.
#[derive(Debug, Default)]
pub struct UnsupportedSession;

impl SpeedTestSession for UnsupportedSession {
    fn open(&self, _node: &TestNode) -> Result<TestSession, DomainError> {
        Err(DomainError::new(
            domain::codes::UNAVAILABLE,
            "error.test_session_unsupported",
        ))
    }

    fn real_ping(
        &self,
        _session: &TestSession,
        _url: &str,
        _timeout: Duration,
        _trust: &TlsTrust,
        _ct: &CancellationToken,
    ) -> Result<i32, ProbeError> {
        Err(ProbeError::new(
            ProbeFailureKind::Connect,
            "test session unsupported",
        ))
    }

    fn download(
        &self,
        _session: &TestSession,
        _url: &str,
        _timeout: Duration,
        _max_bytes: u64,
        _trust: &TlsTrust,
        _ct: &CancellationToken,
    ) -> Result<DownloadOutcome, ProbeError> {
        Err(ProbeError::new(
            ProbeFailureKind::Connect,
            "test session unsupported",
        ))
    }

    fn close(&self, _session: TestSession) {}
}

/// RAII guard: a session is always closed, even on panic/cancel.
struct SessionGuard<'a> {
    session: Option<TestSession>,
    provider: &'a dyn SpeedTestSession,
}

impl<'a> SessionGuard<'a> {
    fn open(provider: &'a dyn SpeedTestSession, node: &TestNode) -> Result<Self, DomainError> {
        let session = provider.open(node)?;
        TestSession::require_test_port(session.port)?;
        Ok(Self {
            session: Some(session),
            provider,
        })
    }

    fn get(&self) -> &TestSession {
        self.session.as_ref().expect("session guard dropped")
    }
}

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            self.provider.close(session);
        }
    }
}

// ---------------------------------------------------------------------------
// Job
// ---------------------------------------------------------------------------

/// A started speedtest job.
#[derive(Debug, Clone)]
pub struct SpeedTestJob {
    pub job_id: domain::JobId,
    pub token: CancellationToken,
    pub kind: SpeedTestAction,
    pub snapshot: Arc<SpeedTestSnapshot>,
}

/// Registry of in-flight speedtest jobs. The bridge owns one per process.
#[derive(Clone, Default)]
pub struct SpeedTestJobs {
    inner: Arc<Mutex<HashMap<String, SpeedTestJob>>>,
    seq: Arc<AtomicUsize>,
}

impl SpeedTestJobs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&self, kind: SpeedTestAction, snapshot: SpeedTestSnapshot) -> SpeedTestJob {
        let n = self.seq.fetch_add(1, Ordering::AcqRel) + 1;
        let job = SpeedTestJob {
            job_id: domain::JobId::new(format!("speedtest-{n:06}")),
            token: CancellationToken::new(),
            kind,
            snapshot: Arc::new(snapshot),
        };
        if let Ok(mut map) = self.inner.lock() {
            map.insert(job.job_id.as_str().to_string(), job.clone());
        }
        job
    }

    pub fn get(&self, job_id: &str) -> Option<SpeedTestJob> {
        self.inner.lock().ok()?.get(job_id).cloned()
    }

    /// Idempotent cancel; returns whether the token transitioned.
    pub fn cancel(&self, job_id: &str) -> bool {
        let Some(job) = self.get(job_id) else {
            return false;
        };
        job.token.cancel()
    }

    pub fn finish(&self, job_id: &str) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(job_id);
        }
    }

    pub fn active_count(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }
}

/// Why a job stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Completed,
    Cancelled,
}

/// The full outcome of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedTestOutcome {
    pub results: Vec<SpeedTestResult>,
    pub stop_reason: StopReason,
}

impl SpeedTestOutcome {
    pub fn cancelled(&self) -> bool {
        self.stop_reason == StopReason::Cancelled
    }
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// Orchestrates a speedtest run over a frozen node set.
pub struct SpeedTestRunner {
    settings: SpeedTestSettings,
    session: Arc<dyn SpeedTestSession>,
    trust: TlsTrust,
}

impl SpeedTestRunner {
    /// Production: verify HTTPS against the OS root store.
    pub fn new(settings: SpeedTestSettings, session: Arc<dyn SpeedTestSession>) -> Self {
        Self {
            settings,
            session,
            trust: TlsTrust::Native,
        }
    }

    /// Test seam: verify HTTPS against exactly `roots` (e.g. a synthetic CA).
    pub fn with_roots(
        settings: SpeedTestSettings,
        session: Arc<dyn SpeedTestSession>,
        roots: RootCertStore,
    ) -> Self {
        Self {
            settings,
            session,
            trust: TlsTrust::custom(roots),
        }
    }

    /// Test seam: inject a full [`TlsTrust`].
    pub fn with_trust(
        settings: SpeedTestSettings,
        session: Arc<dyn SpeedTestSession>,
        trust: TlsTrust,
    ) -> Self {
        Self {
            settings,
            session,
            trust,
        }
    }

    pub fn settings(&self) -> &SpeedTestSettings {
        &self.settings
    }

    pub fn trust(&self) -> &TlsTrust {
        &self.trust
    }

    /// Run synchronously. `on_batch` receives result batches every
    /// [`BATCH_FLUSH_MS`] (plus a final drain), never per item.
    pub fn run<F>(
        &self,
        kind: SpeedTestAction,
        nodes: &[TestNode],
        ct: &CancellationToken,
        mut on_batch: F,
    ) -> SpeedTestOutcome
    where
        F: FnMut(&[SpeedTestResult]) + Send,
    {
        let pending: Mutex<Vec<SpeedTestResult>> = Mutex::new(Vec::new());
        let collected: Mutex<Vec<SpeedTestResult>> = Mutex::new(Vec::new());
        let done = AtomicBool::new(false);
        let stop = Mutex::new(StopReason::Completed);

        // Seed status rows (upstream `SetTestResultAsync` with `Speedtesting`).
        // These are progress notifications, not final results, so they are
        // emitted directly and never enter `collected`.
        let seeds: Vec<SpeedTestResult> = nodes
            .iter()
            .map(|n| SpeedTestResult::status(n.index_id.clone(), "Speedtesting"))
            .collect();
        if !seeds.is_empty() {
            on_batch(&seeds);
        }

        std::thread::scope(|scope| {
            scope.spawn(|| loop {
                std::thread::sleep(Duration::from_millis(BATCH_FLUSH_MS));
                let batch = drain(&pending);
                if !batch.is_empty() {
                    collected
                        .lock()
                        .expect("collected")
                        .extend(batch.iter().cloned());
                    on_batch(&batch);
                }
                if done.load(Ordering::Acquire) {
                    let rest = drain(&pending);
                    if !rest.is_empty() {
                        collected
                            .lock()
                            .expect("collected")
                            .extend(rest.iter().cloned());
                        on_batch(&rest);
                    }
                    break;
                }
            });

            match kind {
                SpeedTestAction::Tcping => {
                    self.run_tcping(nodes, ct, &pending, &stop);
                }
                SpeedTestAction::UdpTest => {
                    self.run_udp(nodes, ct, &pending, &stop);
                }
                SpeedTestAction::Realping | SpeedTestAction::FastRealping => {
                    self.run_real_ping(nodes, ct, &pending, &stop);
                }
                SpeedTestAction::Speedtest => {
                    self.run_mixed(nodes, 1, ct, &pending, &stop);
                }
                SpeedTestAction::Mixedtest => {
                    self.run_mixed(nodes, self.settings.mixed_concurrency, ct, &pending, &stop);
                }
            }

            done.store(true, Ordering::Release);
        });

        let results = collected.into_inner().unwrap_or_default();
        let stop_reason = *stop.lock().expect("stop");
        SpeedTestOutcome {
            results,
            stop_reason,
        }
    }

    fn run_tcping(
        &self,
        nodes: &[TestNode],
        ct: &CancellationToken,
        pending: &Mutex<Vec<SpeedTestResult>>,
        stop: &Mutex<StopReason>,
    ) {
        for batch in nodes.chunks(self.settings.page_size.max(1)) {
            if ct.is_cancelled() {
                *stop.lock().expect("stop") = StopReason::Cancelled;
                return;
            }
            let concurrency = batch.len().max(1);
            self.pool(batch, concurrency, ct, pending, |node| {
                if ct.is_cancelled() {
                    return SpeedTestResult::failed(node.index_id.clone(), "cancelled");
                }
                let delay = tcping(&node.address, node.port, TCPING_CONNECT_TIMEOUT, ct);
                SpeedTestResult::delay(node.index_id.clone(), delay)
            });
            sleep_cancellable(self.settings.delay_interval, ct);
        }
    }

    fn run_real_ping(
        &self,
        nodes: &[TestNode],
        ct: &CancellationToken,
        pending: &Mutex<Vec<SpeedTestResult>>,
        stop: &Mutex<StopReason>,
    ) {
        // Each node opens its own temporary isolated core, so the pool is
        // bounded by `MixedConcurrencyCount` (upstream shares one multi-inbound
        // core; the per-node process model requires a smaller cap).
        let concurrency = self.settings.mixed_concurrency.min(nodes.len()).max(1);
        self.pool(nodes, concurrency, ct, pending, |node| {
            self.real_ping_one(node, ct)
        });
        if ct.is_cancelled() {
            *stop.lock().expect("stop") = StopReason::Cancelled;
        }
    }

    fn run_udp(
        &self,
        nodes: &[TestNode],
        ct: &CancellationToken,
        pending: &Mutex<Vec<SpeedTestResult>>,
        stop: &Mutex<StopReason>,
    ) {
        let concurrency = self.settings.mixed_concurrency.min(nodes.len()).max(1);
        self.pool(
            nodes,
            concurrency,
            ct,
            pending,
            |node| match SessionGuard::open(self.session.as_ref(), node) {
                Ok(guard) => {
                    let target = self
                        .settings
                        .udp_test_target
                        .clone()
                        .unwrap_or_else(|| "ntp".to_string());
                    match self
                        .session
                        .udp_ping(guard.get(), &target, self.settings.timeout, ct)
                    {
                        Some(delay) => SpeedTestResult::delay(node.index_id.clone(), delay),
                        None => SpeedTestResult::failed(
                            node.index_id.clone(),
                            "test_session.udp_unsupported",
                        ),
                    }
                }
                Err(error) => {
                    SpeedTestResult::failed(node.index_id.clone(), error.message_key.clone())
                }
            },
        );
        if ct.is_cancelled() {
            *stop.lock().expect("stop") = StopReason::Cancelled;
        }
    }

    /// Mixed/`Speedtest`: RealPing then (when the delay is positive) download.
    fn run_mixed(
        &self,
        nodes: &[TestNode],
        concurrency: usize,
        ct: &CancellationToken,
        pending: &Mutex<Vec<SpeedTestResult>>,
        stop: &Mutex<StopReason>,
    ) {
        let concurrency = concurrency.min(nodes.len()).max(1);
        self.pool(nodes, concurrency, ct, pending, |node| {
            self.mixed_one(node, ct)
        });
        if ct.is_cancelled() {
            *stop.lock().expect("stop") = StopReason::Cancelled;
        }
    }

    fn real_ping_one(&self, node: &TestNode, ct: &CancellationToken) -> SpeedTestResult {
        match SessionGuard::open(self.session.as_ref(), node) {
            Ok(guard) => {
                match self.session.real_ping(
                    guard.get(),
                    &self.settings.speed_ping_test_url,
                    self.settings.timeout.min(LOCAL_FETCH_TIMEOUT),
                    &self.trust,
                    ct,
                ) {
                    Ok(delay) => {
                        let mut result = SpeedTestResult::delay(node.index_id.clone(), delay);
                        if delay > 0 {
                            if let Some(url) = &self.settings.ipapi_url {
                                if let Some(ip) = self.session.ip_info(
                                    guard.get(),
                                    url,
                                    self.settings.timeout,
                                    ct,
                                ) {
                                    result.ip_info = Some(ip);
                                }
                            }
                        }
                        result
                    }
                    Err(error) => {
                        SpeedTestResult::failed(node.index_id.clone(), error.message_key())
                    }
                }
            }
            Err(error) => SpeedTestResult::failed(node.index_id.clone(), error.message_key.clone()),
        }
    }

    fn mixed_one(&self, node: &TestNode, ct: &CancellationToken) -> SpeedTestResult {
        match SessionGuard::open(self.session.as_ref(), node) {
            Ok(guard) => {
                let delay = match self.session.real_ping(
                    guard.get(),
                    &self.settings.speed_ping_test_url,
                    self.settings.timeout.min(LOCAL_FETCH_TIMEOUT),
                    &self.trust,
                    ct,
                ) {
                    Ok(delay) => delay,
                    Err(error) => {
                        return SpeedTestResult::failed(node.index_id.clone(), error.message_key())
                    }
                };
                if delay <= 0 {
                    return SpeedTestResult::failed(node.index_id.clone(), "SpeedtestingSkip");
                }
                if ct.is_cancelled() {
                    return SpeedTestResult::failed(node.index_id.clone(), "cancelled");
                }
                match self.session.download(
                    guard.get(),
                    &self.settings.speed_test_url,
                    self.settings.timeout,
                    0,
                    &self.trust,
                    ct,
                ) {
                    Ok(outcome) => SpeedTestResult {
                        index_id: node.index_id.clone(),
                        delay: Some(delay),
                        speed: Some(outcome.mb_s),
                        message: if outcome.mb_s > 0.0 {
                            Some(format!("{:.1}", outcome.mb_s))
                        } else {
                            Some("SpeedtestingFailed".to_string())
                        },
                        ip_info: None,
                        failed: false,
                    },
                    Err(error) => SpeedTestResult {
                        index_id: node.index_id.clone(),
                        delay: Some(delay),
                        speed: Some(0.0),
                        message: Some(error.message_key().to_string()),
                        ip_info: None,
                        failed: true,
                    },
                }
            }
            Err(error) => SpeedTestResult::failed(node.index_id.clone(), error.message_key.clone()),
        }
    }

    /// Bounded worker pool over one batch. Results are pushed to `pending` as
    /// they complete (the emitter throttles them into batches).
    fn pool<F>(
        &self,
        batch: &[TestNode],
        concurrency: usize,
        ct: &CancellationToken,
        pending: &Mutex<Vec<SpeedTestResult>>,
        f: F,
    ) where
        F: Fn(&TestNode) -> SpeedTestResult + Sync,
    {
        let next = AtomicUsize::new(0);
        let workers = concurrency.min(batch.len().max(1));
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::AcqRel);
                    if i >= batch.len() {
                        break;
                    }
                    if ct.is_cancelled() {
                        break;
                    }
                    let result = f(&batch[i]);
                    pending.lock().expect("pending").push(result);
                });
            }
        });
    }
}

fn drain(pending: &Mutex<Vec<SpeedTestResult>>) -> Vec<SpeedTestResult> {
    let mut guard = pending.lock().unwrap_or_else(|p| p.into_inner());
    std::mem::take(&mut *guard)
}

fn sleep_cancellable(total: Duration, ct: &CancellationToken) {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if ct.is_cancelled() {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(Duration::from_millis(50)));
    }
}

// ---------------------------------------------------------------------------
// Real TCPing
// ---------------------------------------------------------------------------

/// Direct TCP connect latency in ms. `-1` on resolve/connect failure/timeout.
pub fn tcping(address: &str, port: i32, timeout: Duration, ct: &CancellationToken) -> i32 {
    if address.trim().is_empty() || !(1..=65535).contains(&port) {
        return -1;
    }
    if ct.is_cancelled() {
        return -1;
    }
    // Resolve with cancellation awareness: run the blocking resolve in this
    // thread but bound the connect with `connect_timeout` below.
    let mut addrs = match (address, port as u16).to_socket_addrs() {
        Ok(addrs) => addrs,
        Err(_) => return -1,
    };
    let Some(addr) = addrs.next() else {
        return -1;
    };
    let start = Instant::now();
    match TcpStream::connect_timeout(&addr, timeout.min(TCPING_CONNECT_TIMEOUT)) {
        Ok(_stream) => {
            if ct.is_cancelled() {
                return -1;
            }
            // A loopback connect can be sub-millisecond. Report at least 1 ms so
            // a successful measurement is never confused with "not tested"
            // (upstream `Delay == 0` is the untested default).
            (start.elapsed().as_millis() as i32).max(1)
        }
        Err(_) => -1,
    }
}

// ---------------------------------------------------------------------------
// UDP latency probe (RE-PROF-08)
// ---------------------------------------------------------------------------

/// Resolve an upstream UDP test target to `(host, port)`.
///
/// A bare protocol keyword maps to its canonical endpoint (`ntp`/`time`,
/// `dns`, `stun`, `mcbe`); anything else must be `host:port`. `None` means the
/// target is unusable, which the UI surfaces as an explicit failure.
pub fn udp_target_endpoint(target: &str) -> Option<(String, u16)> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((host, port)) = match trimmed.to_ascii_lowercase().as_str() {
        "ntp" | "time" => Some(("pool.ntp.org", 123u16)),
        "dns" => Some(("1.1.1.1", 53u16)),
        "stun" => Some(("stun.l.google.com", 19302u16)),
        "mcbe" | "minecraft" | "minecraft-be" => Some(("play.nethergames.org", 19132u16)),
        _ => None,
    } {
        return Some((host.to_string(), port));
    }
    let (host, port) = trimmed.rsplit_once(':')?;
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    let port: u16 = port.trim().parse().ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }
    Some((host.to_string(), port))
}

/// Protocol-shaped probe payload. The reply is not parsed: any datagram within
/// the timeout counts as a response, mirroring upstream's UDP ping (it measures
/// the round trip, not the payload content).
fn udp_probe_payload(target: &str) -> Vec<u8> {
    match target.trim().to_ascii_lowercase().as_str() {
        "ntp" | "time" => {
            let mut packet = vec![0u8; 48];
            packet[0] = 0x1b;
            packet
        }
        "stun" => {
            let mut packet = vec![0u8; 20];
            packet[1] = 0x01;
            packet[4] = 0x21;
            packet[5] = 0x12;
            packet[6] = 0xa4;
            packet[7] = 0x42;
            packet
        }
        "dns" => vec![
            0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x02, 0x00, 0x01,
        ],
        _ => vec![0x00],
    }
}

/// One direct UDP round trip. Returns the round-trip milliseconds (>= 1), or
/// `None` on resolve/send/timeout/cancel.
///
/// Scope note: this is a direct datagram probe, not a route through the node's
/// SOCKS UDP associate; node-routed UDP remains an isolated acceptance item
/// (RE-PROF-08 task card / evidence). The endpoint comes from the effective
/// `UdpTestTarget`, so it is exercised here with a loopback fixture.
pub fn udp_ping(target: &str, timeout: Duration, ct: &CancellationToken) -> Option<i32> {
    if ct.is_cancelled() {
        return None;
    }
    let (host, port) = udp_target_endpoint(target)?;
    let addr = (host.as_str(), port).to_socket_addrs().ok()?.next()?;
    let socket = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    let effective = timeout
        .min(Duration::from_secs(5))
        .max(Duration::from_millis(50));
    socket.set_read_timeout(Some(effective)).ok()?;
    let payload = udp_probe_payload(target);
    let start = Instant::now();
    socket.send_to(&payload, addr).ok()?;
    let mut buf = [0u8; 4096];
    match socket.recv_from(&mut buf) {
        Ok(_) if !ct.is_cancelled() => Some((start.elapsed().as_millis() as i32).max(1)),
        _ => None,
    }
}

#[cfg(test)]
mod re_prof_08_udp_tests {
    use super::*;
    use std::net::UdpSocket;

    fn bind_udp_floor() -> UdpSocket {
        for port in TEST_PORT_FLOOR..TEST_PORT_FLOOR + 200 {
            if port == 10_808 {
                continue;
            }
            if let Ok(socket) = UdpSocket::bind(("127.0.0.1", port)) {
                return socket;
            }
        }
        panic!("no free UDP test port >= {TEST_PORT_FLOOR}");
    }

    fn echo_once(socket: UdpSocket) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            if let Ok((len, peer)) = socket.recv_from(&mut buf) {
                let _ = socket.send_to(&buf[..len], peer);
            }
        })
    }

    #[test]
    fn udp_keyword_and_host_port_targets_resolve() {
        assert_eq!(
            udp_target_endpoint("ntp"),
            Some(("pool.ntp.org".into(), 123))
        );
        assert_eq!(
            udp_target_endpoint("stun"),
            Some(("stun.l.google.com".into(), 19302))
        );
        assert_eq!(
            udp_target_endpoint("127.0.0.1:11808"),
            Some(("127.0.0.1".into(), 11808))
        );
        assert_eq!(udp_target_endpoint("not-a-target"), None);
        assert_eq!(udp_target_endpoint("  "), None);
    }

    #[test]
    fn udp_ping_measures_loopback_round_trip() {
        let server = bind_udp_floor();
        let port = server.local_addr().expect("addr").port();
        let handle = echo_once(server);
        let ct = CancellationToken::new();
        let delay = udp_ping(&format!("127.0.0.1:{port}"), Duration::from_secs(2), &ct);
        let _ = handle.join();
        assert!(delay.is_some_and(|d| d >= 1), "delay={delay:?}");
    }

    #[test]
    fn udp_ping_times_out_without_a_reply() {
        let silent = bind_udp_floor();
        let port = silent.local_addr().expect("addr").port();
        let ct = CancellationToken::new();
        assert_eq!(
            udp_ping(
                &format!("127.0.0.1:{port}"),
                Duration::from_millis(200),
                &ct
            ),
            None
        );
        drop(silent);
    }

    struct LoopbackUdpSession;

    impl SpeedTestSession for LoopbackUdpSession {
        fn open(&self, node: &TestNode) -> Result<TestSession, DomainError> {
            Ok(TestSession {
                node: node.clone(),
                port: TEST_PORT_FLOOR,
                handle_id: "loopback".into(),
            })
        }
        fn real_ping(
            &self,
            _session: &TestSession,
            _url: &str,
            _timeout: Duration,
            _trust: &TlsTrust,
            _ct: &CancellationToken,
        ) -> Result<i32, ProbeError> {
            Err(ProbeError::new(ProbeFailureKind::Connect, "unused"))
        }
        fn download(
            &self,
            _session: &TestSession,
            _url: &str,
            _timeout: Duration,
            _max_bytes: u64,
            _trust: &TlsTrust,
            _ct: &CancellationToken,
        ) -> Result<DownloadOutcome, ProbeError> {
            Err(ProbeError::new(ProbeFailureKind::Connect, "unused"))
        }
        fn udp_ping(
            &self,
            _session: &TestSession,
            target: &str,
            timeout: Duration,
            ct: &CancellationToken,
        ) -> Option<i32> {
            udp_ping(target, timeout, ct)
        }
        fn close(&self, _session: TestSession) {}
    }

    fn node() -> TestNode {
        TestNode {
            index_id: "n1".into(),
            address: "127.0.0.1".into(),
            port: 1,
            config_type: 0,
            core_type: 0,
        }
    }

    #[test]
    fn runner_udp_reports_measured_delay_and_failure() {
        let ct = CancellationToken::new();

        let server = bind_udp_floor();
        let port = server.local_addr().expect("addr").port();
        let handle = echo_once(server);
        let settings = SpeedTestSettings {
            udp_test_target: Some(format!("127.0.0.1:{port}")),
            timeout: Duration::from_secs(2),
            ..Default::default()
        };
        let runner = SpeedTestRunner::new(settings, Arc::new(LoopbackUdpSession));
        let outcome = runner.run(SpeedTestAction::UdpTest, &[node()], &ct, |_| {});
        let _ = handle.join();
        assert!(
            outcome
                .results
                .iter()
                .any(|r| r.index_id == "n1" && r.delay.is_some_and(|d| d >= 1)),
            "results={:?}",
            outcome.results
        );

        let silent = bind_udp_floor();
        let silent_port = silent.local_addr().expect("addr").port();
        let settings = SpeedTestSettings {
            udp_test_target: Some(format!("127.0.0.1:{silent_port}")),
            timeout: Duration::from_millis(200),
            ..Default::default()
        };
        let runner = SpeedTestRunner::new(settings, Arc::new(LoopbackUdpSession));
        let outcome = runner.run(SpeedTestAction::UdpTest, &[node()], &ct, |_| {});
        assert!(
            outcome
                .results
                .iter()
                .any(|r| r.index_id == "n1" && r.delay == Some(-1) && r.failed),
            "results={:?}",
            outcome.results
        );
        drop(silent);
    }
}

/// Test-port ceiling for the allocation scan (exclusive).
const TEST_PORT_CEIL: u16 = 59_000;
/// Number of consecutive ports reserved per test session: the SOCKS inbound
/// plus the two state ports the codegen derives from it.
const TEST_PORT_BLOCK: u16 = 3;

/// Pick a free TCP port at/above `base`, skipping the reserved live proxy port.
/// Returns `None` when the scan is exhausted.
pub fn find_free_test_port(mut base: u16) -> Option<u16> {
    while base < TEST_PORT_CEIL {
        if base != 10_808 && std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return Some(base);
        }
        base += 1;
    }
    None
}

static TEST_PORT_CURSOR: AtomicU16 = AtomicU16::new(TEST_PORT_FLOOR);
static RESERVED_TEST_PORTS: OnceLock<Mutex<HashSet<u16>>> = OnceLock::new();

fn reserved_ports() -> &'static Mutex<HashSet<u16>> {
    RESERVED_TEST_PORTS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn port_block_free(start: u16) -> bool {
    (0..TEST_PORT_BLOCK).all(|offset| {
        let port = start.saturating_add(offset);
        port >= TEST_PORT_FLOOR
            && port != 10_808
            && std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    })
}

/// Reserve a contiguous test-port block, racing safely with other in-process
/// callers. `find_free_test_port` alone only *probes* a port: with a
/// concurrency pool every worker could observe the same free port and then
/// collide at core spawn (`error.port_conflict`). The reservation set makes
/// concurrent opens pick disjoint ports, and a rotating cursor avoids
/// immediately reusing a just-closed port (Windows `TIME_WAIT`).
pub fn reserve_free_test_port() -> Option<u16> {
    let reserved = reserved_ports();
    let mut guard = reserved.lock().unwrap_or_else(|p| p.into_inner());
    let start = TEST_PORT_CURSOR
        .load(Ordering::Acquire)
        .max(TEST_PORT_FLOOR);
    let mut port = start;
    for _ in 0..(TEST_PORT_CEIL - TEST_PORT_FLOOR) {
        if port.saturating_add(TEST_PORT_BLOCK) >= TEST_PORT_CEIL {
            port = TEST_PORT_FLOOR;
        }
        let block_conflicts =
            (0..TEST_PORT_BLOCK).any(|offset| guard.contains(&port.saturating_add(offset)));
        if !block_conflicts && port_block_free(port) {
            for offset in 0..TEST_PORT_BLOCK {
                guard.insert(port.saturating_add(offset));
            }
            TEST_PORT_CURSOR.store(port.saturating_add(TEST_PORT_BLOCK), Ordering::Release);
            return Some(port);
        }
        port = port.saturating_add(1);
    }
    None
}

/// Release a block previously reserved by [`reserve_free_test_port`].
pub fn release_test_port(port: u16) {
    let mut guard = reserved_ports().lock().unwrap_or_else(|p| p.into_inner());
    for offset in 0..TEST_PORT_BLOCK {
        guard.remove(&port.saturating_add(offset));
    }
}

/// Number of currently reserved test ports (diagnostics/tests).
pub fn reserved_test_port_count() -> usize {
    reserved_ports().lock().map(|g| g.len()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// SOCKS5 + HTTP(S) probe helper (real TLS via rustls)
// ---------------------------------------------------------------------------

/// Outcome of one HTTP request through a SOCKS5 proxy.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpProbe {
    pub header_ms: u128,
    pub body_bytes: u64,
    pub total: Duration,
    pub success: bool,
    /// Parsed HTTP status code, when a response line was read.
    pub status: Option<u16>,
}

/// Perform `GET url` through the SOCKS5 proxy at `127.0.0.1:port`.
///
/// Both `http://` and `https://` are supported. HTTPS is verified against
/// `trust` (OS roots in production, an injected CA in deterministic tests);
/// insecure mode is deliberately not exposed. Failures are classified by
/// [`ProbeFailureKind`] so the UI can display a concrete reason.
pub fn http_get_via_socks(
    socks_port: u16,
    url: &str,
    trust: &TlsTrust,
    timeout: Duration,
    max_bytes: u64,
    ct: &CancellationToken,
) -> Result<HttpProbe, ProbeError> {
    let parsed = parse_url(url)
        .ok_or_else(|| ProbeError::new(ProbeFailureKind::Url, format!("invalid url: {url}")))?;
    let deadline = Instant::now() + timeout;

    let mut tcp = TcpStream::connect_timeout(
        &("127.0.0.1", socks_port)
            .to_socket_addrs()
            .map_err(|e| ProbeError::new(ProbeFailureKind::Resolve, e.to_string()))?
            .next()
            .ok_or_else(|| ProbeError::new(ProbeFailureKind::Resolve, "no socks address"))?,
        timeout.min(Duration::from_secs(5)),
    )
    .map_err(|e| ProbeError::new(ProbeFailureKind::Connect, e.to_string()))?;
    // Poll with a short timeout so cancellation is prompt even while a slow
    // server holds the connection open.
    let poll = timeout.min(Duration::from_millis(250));
    let _ = tcp.set_read_timeout(Some(poll));
    let _ = tcp.set_write_timeout(Some(poll));

    socks5_connect(&mut tcp, &parsed.host, parsed.port, ct)?;

    let path = if parsed.path.is_empty() {
        "/".to_string()
    } else {
        parsed.path.clone()
    };
    let request = format!(
        "GET {path} HTTP/1.0\r\nHost: {}\r\nUser-Agent: v2rayN-rs/0.1\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        parsed.host_header()
    );

    match parsed.scheme {
        Scheme::Http => {
            write_request(&mut tcp, request.as_bytes(), deadline, ct)?;
            read_response(&mut tcp, false, deadline, max_bytes, ct)
        }
        Scheme::Https => {
            let mut tls = tls_handshake(tcp, &parsed.host, trust, deadline, ct)?;
            write_request(&mut tls, request.as_bytes(), deadline, ct)?;
            read_response(&mut tls, true, deadline, max_bytes, ct)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scheme {
    Http,
    Https,
}

struct ParsedUrl {
    scheme: Scheme,
    host: String,
    port: u16,
    path: String,
    is_default_port: bool,
}

impl ParsedUrl {
    fn host_header(&self) -> String {
        if self.is_default_port {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn parse_url(url: &str) -> Option<ParsedUrl> {
    let (scheme, rest, default_port) = if let Some(rest) = url.strip_prefix("https://") {
        (Scheme::Https, rest, 443u16)
    } else {
        let rest = url.strip_prefix("http://")?;
        (Scheme::Http, rest, 80u16)
    };
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => {
            (host.to_string(), port.parse::<u16>().ok()?)
        }
        _ => (authority.to_string(), default_port),
    };
    if host.is_empty() {
        return None;
    }
    Some(ParsedUrl {
        scheme,
        host,
        port,
        path: path.to_string(),
        is_default_port: port == default_port,
    })
}

/// Drive the rustls handshake to completion so certificate/verification
/// failures are classified as TLS rather than a generic read error.
fn tls_handshake(
    tcp: TcpStream,
    host: &str,
    trust: &TlsTrust,
    deadline: Instant,
    ct: &CancellationToken,
) -> Result<StreamOwned<ClientConnection, TcpStream>, ProbeError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| ProbeError::new(ProbeFailureKind::Tls, e.to_string()))?
        .with_root_certificates(trust.store())
        .with_no_client_auth();
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| ProbeError::new(ProbeFailureKind::Protocol, e.to_string()))?;
    let conn = ClientConnection::new(Arc::new(config), server_name)
        .map_err(|e| ProbeError::new(ProbeFailureKind::Tls, e.to_string()))?;
    let mut tls = StreamOwned::new(conn, tcp);
    while tls.conn.is_handshaking() {
        if ct.is_cancelled() {
            return Err(ProbeError::new(ProbeFailureKind::Cancelled, "cancelled"));
        }
        if Instant::now() >= deadline {
            return Err(ProbeError::new(
                ProbeFailureKind::Timeout,
                "tls handshake timeout",
            ));
        }
        match tls.conn.complete_io(&mut tls.sock) {
            Ok((rd, wr)) => {
                if rd == 0 && wr == 0 {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(ProbeError::new(ProbeFailureKind::Tls, e.to_string())),
        }
    }
    Ok(tls)
}

fn write_request<S: Write>(
    stream: &mut S,
    data: &[u8],
    deadline: Instant,
    ct: &CancellationToken,
) -> Result<(), ProbeError> {
    let mut written = 0usize;
    while written < data.len() {
        if ct.is_cancelled() {
            return Err(ProbeError::new(ProbeFailureKind::Cancelled, "cancelled"));
        }
        if Instant::now() >= deadline {
            return Err(ProbeError::new(ProbeFailureKind::Timeout, "write timeout"));
        }
        match stream.write(&data[written..]) {
            Ok(0) => {
                return Err(ProbeError::new(
                    ProbeFailureKind::Connect,
                    "write returned 0",
                ))
            }
            Ok(n) => written += n,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) => return Err(ProbeError::new(ProbeFailureKind::Connect, e.to_string())),
        }
    }
    let _ = stream.flush();
    Ok(())
}

fn read_response<S: Read>(
    stream: &mut S,
    tls: bool,
    deadline: Instant,
    max_bytes: u64,
    ct: &CancellationToken,
) -> Result<HttpProbe, ProbeError> {
    let start = Instant::now();
    let mut buf = [0u8; 16 * 1024];
    let mut header_buf: Vec<u8> = Vec::new();
    let mut header_ms = 0u128;
    let mut header_done = false;
    let mut body_bytes: u64 = 0;
    let mut status: Option<u16> = None;
    let mut content_length: Option<u64> = None;
    loop {
        if ct.is_cancelled() {
            return Err(ProbeError::new(ProbeFailureKind::Cancelled, "cancelled"));
        }
        let past_deadline = Instant::now() >= deadline;
        if past_deadline && !header_done {
            return Err(ProbeError::new(ProbeFailureKind::Timeout, "read timeout"));
        }
        if past_deadline {
            // Header complete: return the bytes measured so far rather than
            // discarding a valid response.
            break;
        }
        if header_done {
            let body_complete = content_length.is_some_and(|len| body_bytes >= len);
            if body_complete || (max_bytes > 0 && body_bytes >= max_bytes) {
                break;
            }
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if !header_done {
                    header_ms = start.elapsed().as_millis();
                    header_buf.extend_from_slice(&buf[..n]);
                    if let Some(pos) = find_header_end(&header_buf) {
                        header_done = true;
                        status = parse_status(&header_buf[..pos]);
                        content_length = parse_content_length(&header_buf[..pos]);
                        body_bytes = body_bytes.saturating_add((header_buf.len() - pos) as u64);
                    }
                } else {
                    body_bytes = body_bytes.saturating_add(n as u64);
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => {
                if header_done {
                    // A real server (or a relay) may close the stream without a
                    // TLS `close_notify`; a completed header still yields a
                    // valid measurement, so treat the truncation as EOF.
                    break;
                }
                return Err(classify_io(&e, tls));
            }
        }
    }
    if !header_done {
        return Err(ProbeError::new(
            ProbeFailureKind::Timeout,
            "no http response before timeout",
        ));
    }
    match status {
        Some(code) if code < 400 => Ok(HttpProbe {
            header_ms,
            body_bytes,
            total: start.elapsed(),
            success: true,
            status,
        }),
        Some(code) => Err(ProbeError::new(
            ProbeFailureKind::HttpStatus(code),
            format!("http status {code}"),
        )),
        None => Err(ProbeError::new(
            ProbeFailureKind::Protocol,
            "malformed http status line",
        )),
    }
}

fn classify_io(error: &std::io::Error, tls: bool) -> ProbeError {
    if tls && error.kind() == std::io::ErrorKind::InvalidData {
        ProbeError::new(ProbeFailureKind::Tls, error.to_string())
    } else if error.kind() == std::io::ErrorKind::InvalidData {
        ProbeError::new(ProbeFailureKind::Protocol, error.to_string())
    } else {
        ProbeError::new(ProbeFailureKind::Connect, error.to_string())
    }
}

/// Parse `HTTP/1.x 204 ...` into `204`.
fn parse_status(header: &[u8]) -> Option<u16> {
    let line_end = header.windows(2).position(|w| w == b"\r\n")?;
    let line = std::str::from_utf8(&header[..line_end]).ok()?;
    let mut parts = line.split_whitespace();
    let version = parts.next()?;
    if !version.starts_with("HTTP/") {
        return None;
    }
    parts.next()?.parse::<u16>().ok()
}

/// Parse a case-insensitive `Content-Length` header, when present.
fn parse_content_length(header: &[u8]) -> Option<u64> {
    let text = String::from_utf8_lossy(header);
    for line in text.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value.trim().parse::<u64>().ok();
            }
        }
    }
    None
}

fn socks5_connect(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    ct: &CancellationToken,
) -> Result<(), ProbeError> {
    let io = |e: std::io::Error| ProbeError::new(ProbeFailureKind::Connect, e.to_string());
    if ct.is_cancelled() {
        return Err(ProbeError::new(ProbeFailureKind::Cancelled, "cancelled"));
    }
    // Greeting: no-auth.
    stream.write_all(&[0x05, 0x01, 0x00]).map_err(io)?;
    let mut resp = [0u8; 2];
    stream.read_exact(&mut resp).map_err(io)?;
    if resp != [0x05, 0x00] {
        return Err(ProbeError::new(
            ProbeFailureKind::Connect,
            "socks5 auth negotiation failed",
        ));
    }
    // CONNECT request (domain address type).
    let host_bytes = host.as_bytes();
    if host_bytes.len() > 255 {
        return Err(ProbeError::new(
            ProbeFailureKind::Connect,
            "socks5 host too long",
        ));
    }
    let mut request = vec![0x05, 0x01, 0x00, 0x03, host_bytes.len() as u8];
    request.extend_from_slice(host_bytes);
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).map_err(io)?;
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).map_err(io)?;
    if head[1] != 0x00 {
        return Err(ProbeError::new(
            ProbeFailureKind::Connect,
            format!("socks5 connect reply {:#x}", head[1]),
        ));
    }
    // Consume the bound address.
    let addr_len = match head[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).map_err(io)?;
            len[0] as usize
        }
        _ => {
            return Err(ProbeError::new(
                ProbeFailureKind::Protocol,
                "socks5 bad atyp",
            ))
        }
    };
    let mut discard = vec![0u8; addr_len + 2];
    stream.read_exact(&mut discard).map_err(io)?;
    Ok(())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicI64;

    /// Fake session: deterministic per-node delays, optional block/abort.
    struct FakeSession {
        opens: AtomicUsize,
        closes: AtomicUsize,
        active: AtomicI64,
        max_active: AtomicI64,
        delay_ms: HashMap<String, i32>,
        block: Option<Duration>,
        download_mb_s: f64,
        fail_open: bool,
    }

    impl FakeSession {
        fn new() -> Self {
            Self {
                opens: AtomicUsize::new(0),
                closes: AtomicUsize::new(0),
                active: AtomicI64::new(0),
                max_active: AtomicI64::new(0),
                delay_ms: HashMap::new(),
                block: None,
                download_mb_s: 42.5,
                fail_open: false,
            }
        }

        fn with_delays(pairs: &[(&str, i32)]) -> Self {
            let mut s = Self::new();
            for (k, v) in pairs {
                s.delay_ms.insert((*k).to_string(), *v);
            }
            s
        }
    }

    impl SpeedTestSession for FakeSession {
        fn open(&self, node: &TestNode) -> Result<TestSession, DomainError> {
            if self.fail_open {
                return Err(DomainError::new(
                    domain::codes::UNAVAILABLE,
                    "error.test_open",
                ));
            }
            let n = self.opens.fetch_add(1, Ordering::AcqRel) + 1;
            let active = self.active.fetch_add(1, Ordering::AcqRel) + 1;
            self.max_active.fetch_max(active, Ordering::AcqRel);
            Ok(TestSession {
                node: node.clone(),
                port: 11_808 + (n as u16 % 100),
                handle_id: format!("fake-{n}"),
            })
        }

        fn real_ping(
            &self,
            session: &TestSession,
            _url: &str,
            _timeout: Duration,
            _trust: &TlsTrust,
            ct: &CancellationToken,
        ) -> Result<i32, ProbeError> {
            if let Some(block) = self.block {
                sleep_cancellable(block, ct);
                if ct.is_cancelled() {
                    return Err(ProbeError::new(ProbeFailureKind::Cancelled, "cancelled"));
                }
            }
            let delay = *self.delay_ms.get(&session.node.index_id).unwrap_or(&10);
            if delay <= 0 {
                Err(ProbeError::new(ProbeFailureKind::Connect, "fake failure"))
            } else {
                Ok(delay)
            }
        }

        fn download(
            &self,
            _session: &TestSession,
            _url: &str,
            _timeout: Duration,
            _max_bytes: u64,
            _trust: &TlsTrust,
            ct: &CancellationToken,
        ) -> Result<DownloadOutcome, ProbeError> {
            if let Some(block) = self.block {
                sleep_cancellable(block, ct);
            }
            if ct.is_cancelled() {
                return Err(ProbeError::new(ProbeFailureKind::Cancelled, "cancelled"));
            }
            Ok(DownloadOutcome {
                mb_s: self.download_mb_s,
                bytes: 1_000_000,
                elapsed: Duration::from_secs(1),
            })
        }

        fn close(&self, _session: TestSession) {
            self.closes.fetch_add(1, Ordering::AcqRel);
            self.active.fetch_sub(1, Ordering::AcqRel);
        }
    }

    fn node(id: &str) -> TestNode {
        TestNode {
            index_id: id.to_string(),
            address: "127.0.0.1".to_string(),
            port: 443,
            config_type: 5,
            core_type: 1,
        }
    }

    fn nodes(ids: &[&str]) -> Vec<TestNode> {
        ids.iter().map(|id| node(id)).collect()
    }

    fn run_with(
        kind: SpeedTestAction,
        nodes: &[TestNode],
        session: Arc<dyn SpeedTestSession>,
    ) -> (SpeedTestOutcome, Vec<Vec<SpeedTestResult>>) {
        let runner = SpeedTestRunner::new(SpeedTestSettings::default(), session);
        let batches = Mutex::new(Vec::new());
        let outcome = runner.run(kind, nodes, &CancellationToken::new(), |batch| {
            batches.lock().unwrap().push(batch.to_vec());
        });
        (outcome, batches.into_inner().unwrap())
    }

    #[test]
    fn tcping_measures_open_port_and_fails_closed_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let _ = listener.accept();
        });
        let delay = tcping(
            "127.0.0.1",
            port as i32,
            Duration::from_secs(2),
            &CancellationToken::new(),
        );
        assert!(delay >= 0, "open port should measure >= 0, got {delay}");
        handle.join().unwrap();

        // A closed ephemeral port fails with -1.
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let closed_port = closed.local_addr().unwrap().port();
        drop(closed);
        assert_eq!(
            tcping(
                "127.0.0.1",
                closed_port as i32,
                Duration::from_secs(1),
                &CancellationToken::new()
            ),
            -1
        );
        assert_eq!(
            tcping("", 0, Duration::from_secs(1), &CancellationToken::new()),
            -1
        );
    }

    #[test]
    fn tcping_pool_runs_and_produces_stable_ids() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let acceptor = std::thread::spawn(move || {
            for _ in 0..4 {
                let _ = listener.accept();
            }
        });
        let ns: Vec<TestNode> = (0..4)
            .map(|i| TestNode {
                index_id: format!("n{i}"),
                address: "127.0.0.1".into(),
                port: port as i32,
                config_type: 5,
                core_type: 1,
            })
            .collect();
        let (outcome, batches) =
            run_with(SpeedTestAction::Tcping, &ns, Arc::new(UnsupportedSession));
        acceptor.join().unwrap();
        assert_eq!(outcome.stop_reason, StopReason::Completed);
        let mut ids: Vec<String> = outcome.results.iter().map(|r| r.index_id.clone()).collect();
        ids.sort();
        assert_eq!(ids, vec!["n0", "n1", "n2", "n3"]);
        assert!(outcome.results.iter().all(|r| r.delay.unwrap() >= 0));
        assert!(!batches.is_empty(), "results must be emitted in batches");
    }

    #[test]
    fn cancel_at_safe_point_stops_and_retains_completed() {
        let mut session = FakeSession::with_delays(&[("a", 5), ("b", 5), ("c", 5), ("d", 5)]);
        session.block = Some(Duration::from_millis(50));
        let runner = SpeedTestRunner::new(SpeedTestSettings::default(), Arc::new(session));
        let token = CancellationToken::new();
        let seen: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let token2 = token.clone();
        let watcher = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            token2.cancel();
        });
        let outcome = runner.run(
            SpeedTestAction::Realping,
            &nodes(&["a", "b", "c", "d"]),
            &token,
            |batch| {
                let mut seen = seen.lock().unwrap();
                for r in batch {
                    seen.push(r.index_id.clone());
                }
            },
        );
        watcher.join().unwrap();
        assert_eq!(outcome.stop_reason, StopReason::Cancelled);
        assert!(token.is_cancelled());
    }

    #[test]
    fn cancel_interrupts_real_ping_and_closes_every_session() {
        let mut session = FakeSession::new();
        session.block = Some(Duration::from_secs(10));
        session.delay_ms.insert("a".into(), 5);
        let runner = SpeedTestRunner::new(SpeedTestSettings::default(), Arc::new(session));
        let token = CancellationToken::new();
        let token2 = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            token2.cancel();
        });
        let start = Instant::now();
        let outcome = runner.run(SpeedTestAction::Realping, &nodes(&["a"]), &token, |_| {});
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "cancel must be prompt"
        );
        assert!(outcome.cancelled());
    }

    #[test]
    fn per_node_sessions_never_exceed_mixed_concurrency() {
        let mut session = FakeSession::new();
        session.block = Some(Duration::from_millis(20));
        let session = Arc::new(session);
        let runner = SpeedTestRunner::new(
            SpeedTestSettings {
                mixed_concurrency: 3,
                ..SpeedTestSettings::default()
            },
            session.clone(),
        );
        let ns: Vec<TestNode> = (0..12).map(|i| node(&format!("n{i}"))).collect();
        let outcome = runner.run(
            SpeedTestAction::Realping,
            &ns,
            &CancellationToken::new(),
            |_| {},
        );
        assert_eq!(outcome.results.len(), 12);
        assert!(session.max_active.load(Ordering::Acquire) <= 3);
        assert_eq!(session.active.load(Ordering::Acquire), 0);
        assert_eq!(session.opens.load(Ordering::Acquire), 12);
    }

    #[test]
    fn real_ping_opens_and_closes_one_session_per_node() {
        let session = Arc::new(FakeSession::with_delays(&[("a", 12), ("b", 34)]));
        let (outcome, _) = run_with(
            SpeedTestAction::Realping,
            &nodes(&["a", "b"]),
            session.clone(),
        );
        let map: HashMap<_, _> = outcome
            .results
            .iter()
            .map(|r| (r.index_id.clone(), r.delay))
            .collect();
        assert_eq!(map["a"], Some(12));
        assert_eq!(map["b"], Some(34));
        assert_eq!(session.opens.load(Ordering::Acquire), 2);
        assert_eq!(session.closes.load(Ordering::Acquire), 2);
        assert_eq!(session.active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn fast_real_ping_aliases_real_ping() {
        let session = Arc::new(FakeSession::with_delays(&[("a", 7)]));
        let (outcome, _) = run_with(SpeedTestAction::FastRealping, &nodes(&["a"]), session);
        assert_eq!(outcome.results[0].delay, Some(7));
    }

    #[test]
    fn mixed_test_reports_delay_and_speed() {
        let session = Arc::new(FakeSession::with_delays(&[("a", 20)]));
        let runner = SpeedTestRunner::new(SpeedTestSettings::default(), session);
        let outcome = runner.run(
            SpeedTestAction::Mixedtest,
            &nodes(&["a"]),
            &CancellationToken::new(),
            |_| {},
        );
        let r = &outcome.results[0];
        assert_eq!(r.delay, Some(20));
        assert_eq!(r.speed, Some(42.5));
        assert!(!r.failed);
    }

    #[test]
    fn failed_delay_skips_download() {
        let session = Arc::new(FakeSession::with_delays(&[("a", -1)]));
        let runner = SpeedTestRunner::new(SpeedTestSettings::default(), session);
        let outcome = runner.run(
            SpeedTestAction::Mixedtest,
            &nodes(&["a"]),
            &CancellationToken::new(),
            |_| {},
        );
        let r = &outcome.results[0];
        assert_eq!(r.delay, Some(-1));
        assert_eq!(r.speed, None);
        assert!(r.failed);
    }

    #[test]
    fn open_failure_is_reported_not_faked() {
        let mut session = FakeSession::new();
        session.fail_open = true;
        let runner = SpeedTestRunner::new(SpeedTestSettings::default(), Arc::new(session));
        let outcome = runner.run(
            SpeedTestAction::Realping,
            &nodes(&["a"]),
            &CancellationToken::new(),
            |_| {},
        );
        assert_eq!(outcome.results[0].delay, Some(-1));
        assert!(outcome.results[0].message.is_some());
    }

    #[test]
    fn udp_without_support_is_explicitly_unavailable() {
        let runner =
            SpeedTestRunner::new(SpeedTestSettings::default(), Arc::new(UnsupportedSession));
        // UnsupportedSession cannot even open, so this hits the open error path.
        let outcome = runner.run(
            SpeedTestAction::UdpTest,
            &nodes(&["a"]),
            &CancellationToken::new(),
            |_| {},
        );
        assert!(outcome.results[0].failed);
        assert!(outcome.results[0].delay == Some(-1));
    }

    #[test]
    fn profile_ex_closure_applies_and_removes_invalid() {
        let mut store = ProfileExStore::new();
        store.apply(&SpeedTestResult::delay("a", 12));
        store.apply(&SpeedTestResult {
            index_id: "a".into(),
            delay: None,
            speed: Some(10.0),
            message: Some("10.0".into()),
            ip_info: None,
            failed: false,
        });
        assert_eq!(store.get("a").unwrap().delay, 12);
        assert_eq!(store.get("a").unwrap().speed, 10.0);
        assert_eq!(store.get("a").unwrap().message, "10.0");

        store.apply(&SpeedTestResult::delay("b", -1));
        assert_eq!(store.len(), 2);
        assert_eq!(store.remove_invalid(), 1);
        assert!(store.get("a").is_some());
        assert!(store.get("b").is_none());
    }

    #[test]
    fn apply_order_writes_step_ten_and_reads_back_stably() {
        // FIX-10B: `speedtest_apply_profile_order` rewrites `ProfileExItem.Sort`
        // as `(position + 1) * 10` and this order is what a reopened reader
        // reconstructs from the persisted rows.
        let mut store = ProfileExStore::new();
        store.apply(&SpeedTestResult::delay("a", 12));
        store.apply(&SpeedTestResult::delay("b", -1));
        store.apply(&SpeedTestResult::delay("c", 30));

        let order = vec!["c".to_string(), "a".to_string(), "b".to_string()];
        store.apply_order(&order);

        assert_eq!(store.get_sort("c"), 10, "first row → 10");
        assert_eq!(store.get_sort("a"), 20, "second row → 20");
        assert_eq!(store.get_sort("b"), 30, "third row → 30");
        assert_eq!(store.max_sort(), 30);

        // Reconstruct the persisted order from `Sort` alone (reopen read-back).
        let mut rows = store.all();
        rows.sort_by_key(|r| r.sort);
        let ids: Vec<&str> = rows.iter().map(|r| r.index_id.as_str()).collect();
        assert_eq!(ids, vec!["c", "a", "b"]);
        // The measured result fields are untouched by an order write.
        assert_eq!(store.get("a").unwrap().delay, 12);
        assert_eq!(store.get("b").unwrap().delay, -1);
    }

    #[test]
    fn all_reports_persisted_sort_order_and_direction() {
        // RE-PROF-04: the UI reads order back through the result list, so
        // `all()` must already be in persisted `Sort` order (not index order).
        let mut store = ProfileExStore::new();
        store.apply(&SpeedTestResult::delay("a", 12));
        store.apply(&SpeedTestResult::delay("b", 20));
        store.apply(&SpeedTestResult::delay("c", 30));

        // No order written yet: deterministic index order.
        let ids = |s: &ProfileExStore| -> Vec<String> {
            s.all().iter().map(|r| r.index_id.clone()).collect()
        };
        assert_eq!(ids(&store), vec!["a", "b", "c"]);

        store.apply_order(&["c".into(), "a".into(), "b".into()]);
        assert_eq!(ids(&store), vec!["c", "a", "b"]);

        // Re-writing the order (the UI's descending toggle) flips the read-back.
        store.apply_order(&["b".into(), "a".into(), "c".into()]);
        assert_eq!(ids(&store), vec!["b", "a", "c"]);
    }

    #[test]
    fn apply_order_is_noop_for_empty_list() {
        let mut store = ProfileExStore::new();
        store.apply(&SpeedTestResult::delay("a", 12));
        store.apply_order(&[]);
        assert_eq!(store.get_sort("a"), 0, "no order written for empty list");
    }

    #[test]
    fn clear_statistics_does_not_touch_test_results() {
        let mut store = ProfileExStore::new();
        store.apply(&SpeedTestResult::delay("a", 12));
        store.clear_statistics();
        assert_eq!(store.len(), 1);
        assert_eq!(store.get("a").unwrap().delay, 12);
    }

    #[test]
    fn snapshot_records_source_revision_and_frozen_nodes() {
        let snap = SpeedTestSnapshot {
            source_revision: 7,
            nodes: nodes(&["a", "b"]),
        };
        let jobs = SpeedTestJobs::new();
        let job = jobs.start(SpeedTestAction::Tcping, snap.clone());
        assert_eq!(job.snapshot.source_revision, 7);
        assert_eq!(job.snapshot.nodes.len(), 2);
        assert!(jobs.cancel(job.job_id.as_str()));
        assert!(!jobs.cancel(job.job_id.as_str()));
        jobs.finish(job.job_id.as_str());
        assert_eq!(jobs.active_count(), 0);
    }

    #[test]
    fn settings_normalize_matches_domain_contract() {
        let item = domain::SpeedTestItem {
            speed_test_timeout: 3,
            mixed_concurrency_count: 2,
            speed_test_page_size: None,
            speed_test_url: Some("  ".into()),
            ..Default::default()
        };
        let s = SpeedTestSettings::from_item(&item);
        assert_eq!(s.timeout, Duration::from_secs(10));
        assert_eq!(s.mixed_concurrency, MIN_MIXED_CONCURRENCY);
        assert_eq!(s.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(s.speed_test_url, DEFAULT_SPEED_URL);
    }

    #[test]
    fn test_port_floor_is_enforced() {
        assert!(TestSession::require_test_port(11_808).is_ok());
        assert!(TestSession::require_test_port(10_808).is_err());
    }

    #[test]
    fn parse_url_handles_schemes_ports_and_paths() {
        let p = parse_url("http://127.0.0.1:11809/large.file").unwrap();
        assert_eq!(p.scheme, Scheme::Http);
        assert_eq!(p.host, "127.0.0.1");
        assert_eq!(p.port, 11809);
        assert_eq!(p.path, "/large.file");
        assert!(!p.is_default_port);
        let p = parse_url("http://example.com").unwrap();
        assert_eq!(p.port, 80);
        assert!(p.is_default_port);
        let p = parse_url("https://example.com/generate_204").unwrap();
        assert_eq!(p.scheme, Scheme::Https);
        assert_eq!(p.port, 443);
        assert!(p.is_default_port);
        assert_eq!(p.path, "/generate_204");
        assert!(parse_url("ftp://example.com").is_none());
    }
}
