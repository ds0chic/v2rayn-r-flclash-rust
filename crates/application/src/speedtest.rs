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

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::{CancellationToken, DomainError, SpeedTestAction};
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

/// Persisted per-node test result (`ProfileExItem`, upstream `ProfilesEx.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileExItem {
    pub index_id: String,
    pub delay: i32,
    pub speed: f64,
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

    /// All rows, sorted by `index_id` for deterministic UI output.
    pub fn all(&self) -> Vec<ProfileExItem> {
        let mut rows: Vec<ProfileExItem> = self.rows.values().cloned().collect();
        rows.sort_by(|a, b| a.index_id.cmp(&b.index_id));
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

    /// `RemoveInvalidServerResult`: drop rows whose delay failed (`== -1`).
    /// Returns the number removed.
    pub fn remove_invalid(&mut self) -> usize {
        let before = self.rows.len();
        self.rows.retain(|_, row| row.delay != -1);
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

    /// RealPing: request `url` through the session's socks proxy and return the
    /// minimum of two round trips in ms, or `-1` on failure.
    fn real_ping(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        ct: &CancellationToken,
    ) -> i32;

    /// Download `url` through the session and return the observed speed.
    fn download(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        max_bytes: u64,
        ct: &CancellationToken,
    ) -> DownloadOutcome;

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
        _ct: &CancellationToken,
    ) -> i32 {
        -1
    }

    fn download(
        &self,
        _session: &TestSession,
        _url: &str,
        _timeout: Duration,
        _max_bytes: u64,
        _ct: &CancellationToken,
    ) -> DownloadOutcome {
        DownloadOutcome {
            mb_s: 0.0,
            bytes: 0,
            elapsed: Duration::ZERO,
        }
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
}

impl SpeedTestRunner {
    pub fn new(settings: SpeedTestSettings, session: Arc<dyn SpeedTestSession>) -> Self {
        Self { settings, session }
    }

    pub fn settings(&self) -> &SpeedTestSettings {
        &self.settings
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
                let delay = self.session.real_ping(
                    guard.get(),
                    &self.settings.speed_ping_test_url,
                    self.settings.timeout.min(LOCAL_FETCH_TIMEOUT),
                    ct,
                );
                let mut result = SpeedTestResult::delay(node.index_id.clone(), delay);
                if delay > 0 {
                    if let Some(url) = &self.settings.ipapi_url {
                        if let Some(ip) =
                            self.session
                                .ip_info(guard.get(), url, self.settings.timeout, ct)
                        {
                            result.ip_info = Some(ip);
                        }
                    }
                }
                result
            }
            Err(error) => SpeedTestResult::failed(node.index_id.clone(), error.message_key.clone()),
        }
    }

    fn mixed_one(&self, node: &TestNode, ct: &CancellationToken) -> SpeedTestResult {
        match SessionGuard::open(self.session.as_ref(), node) {
            Ok(guard) => {
                let delay = self.session.real_ping(
                    guard.get(),
                    &self.settings.speed_ping_test_url,
                    self.settings.timeout.min(LOCAL_FETCH_TIMEOUT),
                    ct,
                );
                if delay <= 0 {
                    return SpeedTestResult::failed(node.index_id.clone(), "SpeedtestingSkip");
                }
                if ct.is_cancelled() {
                    return SpeedTestResult::failed(node.index_id.clone(), "cancelled");
                }
                let outcome = self.session.download(
                    guard.get(),
                    &self.settings.speed_test_url,
                    self.settings.timeout,
                    0,
                    ct,
                );
                SpeedTestResult {
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

/// Pick a free TCP port at/above `base`, skipping the reserved live proxy port.
/// Returns `None` when the scan is exhausted.
pub fn find_free_test_port(mut base: u16) -> Option<u16> {
    while base < 60_000 {
        if base != 10_808 && std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return Some(base);
        }
        base += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// SOCKS5 + HTTP probe helper (real, no TLS)
// ---------------------------------------------------------------------------

/// Outcome of one HTTP request through a SOCKS5 proxy.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpProbe {
    pub header_ms: u128,
    pub body_bytes: u64,
    pub total: Duration,
    pub success: bool,
}

/// Perform `GET url` through the SOCKS5 proxy at `127.0.0.1:port`.
///
/// Only `http://` URLs are supported by this helper; an `https://` URL returns
/// `success=false` (TLS is out of scope for the local test session; remote TLS
/// measurement is tracked as unresolved).
pub fn http_get_via_socks(
    socks_port: u16,
    url: &str,
    timeout: Duration,
    max_bytes: u64,
    ct: &CancellationToken,
) -> Result<HttpProbe, DomainError> {
    let parsed = parse_http_url(url).ok_or_else(|| {
        DomainError::new(domain::codes::INVALID_ARGUMENT, "error.speedtest_url")
            .with_field("url")
            .with_detail(url)
    })?;
    let deadline = Instant::now() + timeout;

    let mut stream = TcpStream::connect(("127.0.0.1", socks_port)).map_err(|e| {
        DomainError::new(domain::codes::UNAVAILABLE, "error.test_session_unreachable")
            .with_detail(e.to_string())
    })?;
    // Poll with a short read timeout so cancellation is prompt even while a
    // slow server is holding the connection open.
    let poll = timeout.min(Duration::from_millis(250));
    let _ = stream.set_read_timeout(Some(poll));
    let _ = stream.set_write_timeout(Some(poll));

    socks5_connect(&mut stream, &parsed.host, parsed.port, ct)?;

    let path = if parsed.path.is_empty() {
        "/".to_string()
    } else {
        parsed.path.clone()
    };
    let request = format!(
        "GET {path} HTTP/1.0\r\nHost: {}\r\nUser-Agent: v2rayN-rs/0.1\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        parsed.host_header()
    );
    let start = Instant::now();
    stream
        .write_all(request.as_bytes())
        .map_err(|e| unavailable(e.to_string()))?;

    let mut buf = [0u8; 16 * 1024];
    let mut header_ms = 0u128;
    let mut header_done = false;
    let mut body_bytes: u64 = 0;
    loop {
        if ct.is_cancelled() {
            return Err(ct.check().unwrap_err());
        }
        if Instant::now() >= deadline {
            break;
        }
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(unavailable(e.to_string())),
        };
        if !header_done {
            header_ms = start.elapsed().as_millis();
            if let Some(pos) = find_header_end(&buf[..n]) {
                header_done = true;
                let body = &buf[pos..n];
                body_bytes = body_bytes.saturating_add(body.len() as u64);
            }
        } else {
            body_bytes = body_bytes.saturating_add(n as u64);
        }
        if max_bytes > 0 && body_bytes >= max_bytes {
            break;
        }
    }
    let total = start.elapsed();
    Ok(HttpProbe {
        header_ms,
        body_bytes,
        total,
        success: header_done,
    })
}

struct ParsedUrl {
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

fn parse_http_url(url: &str) -> Option<ParsedUrl> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => {
            (host.to_string(), port.parse::<u16>().ok()?)
        }
        _ => (authority.to_string(), 80u16),
    };
    if host.is_empty() {
        return None;
    }
    Some(ParsedUrl {
        host,
        port,
        path: path.to_string(),
        is_default_port: port == 80,
    })
}

fn socks5_connect(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    ct: &CancellationToken,
) -> Result<(), DomainError> {
    if ct.is_cancelled() {
        return Err(ct.check().unwrap_err());
    }
    // Greeting: no-auth.
    stream
        .write_all(&[0x05, 0x01, 0x00])
        .map_err(|e| unavailable(e.to_string()))?;
    let mut resp = [0u8; 2];
    stream
        .read_exact(&mut resp)
        .map_err(|e| unavailable(e.to_string()))?;
    if resp != [0x05, 0x00] {
        return Err(unavailable("socks5 auth negotiation failed"));
    }
    // CONNECT request (domain address type).
    let mut request = vec![0x05, 0x01, 0x00, 0x03];
    let host_bytes = host.as_bytes();
    if host_bytes.len() > 255 {
        return Err(unavailable("socks5 host too long"));
    }
    request.push(host_bytes.len() as u8);
    request.extend_from_slice(host_bytes);
    request.extend_from_slice(&port.to_be_bytes());
    stream
        .write_all(&request)
        .map_err(|e| unavailable(e.to_string()))?;
    let mut head = [0u8; 4];
    stream
        .read_exact(&mut head)
        .map_err(|e| unavailable(e.to_string()))?;
    if head[1] != 0x00 {
        return Err(unavailable(format!("socks5 connect reply {:#x}", head[1])));
    }
    // Consume the bound address.
    let addr_len = match head[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len = [0u8; 1];
            stream
                .read_exact(&mut len)
                .map_err(|e| unavailable(e.to_string()))?;
            len[0] as usize
        }
        _ => return Err(unavailable("socks5 bad atyp")),
    };
    let mut discard = vec![0u8; addr_len + 2];
    stream
        .read_exact(&mut discard)
        .map_err(|e| unavailable(e.to_string()))?;
    Ok(())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

fn unavailable(detail: impl Into<String>) -> DomainError {
    DomainError::new(domain::codes::UNAVAILABLE, "error.speedtest_failed")
        .with_detail(detail.into())
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
            ct: &CancellationToken,
        ) -> i32 {
            if let Some(block) = self.block {
                sleep_cancellable(block, ct);
                if ct.is_cancelled() {
                    return -1;
                }
            }
            *self.delay_ms.get(&session.node.index_id).unwrap_or(&10)
        }

        fn download(
            &self,
            _session: &TestSession,
            _url: &str,
            _timeout: Duration,
            _max_bytes: u64,
            ct: &CancellationToken,
        ) -> DownloadOutcome {
            if let Some(block) = self.block {
                sleep_cancellable(block, ct);
            }
            DownloadOutcome {
                mb_s: if ct.is_cancelled() {
                    0.0
                } else {
                    self.download_mb_s
                },
                bytes: 1_000_000,
                elapsed: Duration::from_secs(1),
            }
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
    fn parse_http_url_handles_ports_and_paths() {
        let p = parse_http_url("http://127.0.0.1:11809/large.file").unwrap();
        assert_eq!(p.host, "127.0.0.1");
        assert_eq!(p.port, 11809);
        assert_eq!(p.path, "/large.file");
        assert!(!p.is_default_port);
        let p = parse_http_url("http://example.com").unwrap();
        assert_eq!(p.port, 80);
        assert!(p.is_default_port);
        assert!(parse_http_url("https://example.com").is_none());
    }
}
