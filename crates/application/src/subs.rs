//! Subscription use cases (T09 wiring).
//!
//! This module owns the 17-field `SubItem` model, its persistence mapping, the
//! candidate-first refresh pipeline and the periodic scheduler. It builds on the
//! pure `subscriptions` crate (`parse_content` / `refresh` / `download`) and the
//! `Store`-backed repositories, so a failed or empty download never clears the
//! existing nodes (plan §15, F-SUB-011).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use domain::{CancellationToken, DomainError, Profile};
use persistence::RawRow;
use serde_json::{json, Map, Value};
use subscriptions::{
    build_client, parse_content, ContentHint, DownloadOptions, MergeOptions, ParseOptions,
    ParsedFormat, ProxyConfig, RefreshOutcome, SubError,
};

use crate::repository::new_index_id;

static SUB_ID_SEQ: AtomicU64 = AtomicU64::new(0);

/// A fresh stable subscription id (never a row number).
pub fn new_sub_id() -> String {
    let seq = SUB_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("s-{nanos:x}-{seq:x}")
}

/// The 17 upstream `SubItem` columns (`compat/fields.entities.yaml` FLD-ENT-082..098).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubItem {
    pub id: String,
    pub remarks: String,
    pub url: String,
    /// Comma-separated additional subscription URLs.
    pub more_url: String,
    pub enabled: bool,
    pub user_agent: String,
    /// JSON object text (`{ "Name": "value" }`) or `None`.
    pub request_headers: Option<String>,
    pub sort: i32,
    pub filter: Option<String>,
    /// Minutes; `0` or negative disables the periodic update.
    pub auto_update_interval: i32,
    /// Unix seconds of the last update attempt.
    pub update_time: i64,
    pub convert_target: Option<String>,
    pub prev_profile: Option<String>,
    pub next_profile: Option<String>,
    pub pre_socks_port: Option<i32>,
    pub memo: Option<String>,
    /// Numeric `ECoreType` or `None`.
    pub custom_core_type: Option<i32>,
}

impl Default for SubItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            remarks: String::new(),
            url: String::new(),
            more_url: String::new(),
            enabled: true,
            user_agent: String::new(),
            request_headers: None,
            sort: 0,
            filter: None,
            auto_update_interval: 0,
            update_time: 0,
            convert_target: None,
            prev_profile: None,
            next_profile: None,
            pre_socks_port: None,
            memo: None,
            custom_core_type: None,
        }
    }
}

impl SubItem {
    pub fn to_row(&self) -> RawRow {
        let mut row = RawRow::new("SubItem");
        row.set("Id", json!(self.id));
        row.set("Remarks", json!(self.remarks));
        row.set("Url", json!(self.url));
        row.set("MoreUrl", json!(self.more_url));
        row.set("Enabled", json!(i64::from(self.enabled)));
        row.set("UserAgent", json!(self.user_agent));
        row.set(
            "RequestHeaders",
            self.request_headers
                .clone()
                .map_or(Value::Null, Value::String),
        );
        row.set("Sort", json!(self.sort));
        row.set(
            "Filter",
            self.filter.clone().map_or(Value::Null, Value::String),
        );
        row.set("AutoUpdateInterval", json!(self.auto_update_interval));
        row.set("UpdateTime", json!(self.update_time));
        row.set(
            "ConvertTarget",
            self.convert_target
                .clone()
                .map_or(Value::Null, Value::String),
        );
        row.set(
            "PrevProfile",
            self.prev_profile.clone().map_or(Value::Null, Value::String),
        );
        row.set(
            "NextProfile",
            self.next_profile.clone().map_or(Value::Null, Value::String),
        );
        row.set(
            "PreSocksPort",
            self.pre_socks_port.map_or(Value::Null, |v| json!(v)),
        );
        row.set("Memo", self.memo.clone().map_or(Value::Null, Value::String));
        row.set(
            "CustomCoreType",
            self.custom_core_type.map_or(Value::Null, |v| json!(v)),
        );
        row
    }

    pub fn from_row(row: &RawRow) -> Self {
        Self {
            id: row.string("Id"),
            remarks: row.string("Remarks"),
            url: row.string("Url"),
            more_url: row.string("MoreUrl"),
            enabled: row.bool("Enabled"),
            user_agent: row.string("UserAgent"),
            request_headers: non_empty(row.opt_string("RequestHeaders")),
            sort: row.opt_i64("Sort").unwrap_or(0) as i32,
            filter: non_empty(row.opt_string("Filter")),
            auto_update_interval: row.opt_i64("AutoUpdateInterval").unwrap_or(0) as i32,
            update_time: row.opt_i64("UpdateTime").unwrap_or(0),
            convert_target: non_empty(row.opt_string("ConvertTarget")),
            prev_profile: non_empty(row.opt_string("PrevProfile")),
            next_profile: non_empty(row.opt_string("NextProfile")),
            pre_socks_port: row.opt_i64("PreSocksPort").map(|v| v as i32),
            memo: non_empty(row.opt_string("Memo")),
            custom_core_type: row.opt_i64("CustomCoreType").map(|v| v as i32),
        }
    }

    /// Validate before persisting (F-SUB-002). Returns the first field error.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.remarks.trim().is_empty() {
            return Err(
                DomainError::new(domain::codes::FIELD_REQUIRED, "error.remarks_required")
                    .with_field("remarks"),
            );
        }
        // An empty URL is a plain group (upstream `SubEditViewModel` only
        // validates the URL when non-empty); only a non-empty URL must be a
        // valid http(s) address. Refreshes skip plain groups instead.
        if !self.url.trim().is_empty() && !is_http_url(&self.url) {
            return Err(
                DomainError::new(domain::codes::FIELD_FORMAT, "error.url_invalid")
                    .with_field("url"),
            );
        }
        parse_request_headers(self.request_headers.as_deref())?;
        if let Some(port) = self.pre_socks_port {
            if !(1..=65535).contains(&port) {
                return Err(
                    DomainError::new(domain::codes::FIELD_RANGE, "error.port_range")
                        .with_field("pre_socks_port"),
                );
            }
        }
        if let Some(core) = self.custom_core_type {
            if core < 0 {
                return Err(
                    DomainError::new(domain::codes::FIELD_RANGE, "error.core_type_range")
                        .with_field("custom_core_type"),
                );
            }
        }
        Ok(())
    }

    /// The additional URLs, trimmed and with empty entries dropped.
    pub fn more_urls(&self) -> Vec<String> {
        self.more_url
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

fn is_http_url(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

/// Parse and validate a `RequestHeaders` JSON object (`HttpRequestHeadersHelper`).
pub fn parse_request_headers(json: Option<&str>) -> Result<Vec<(String, String)>, DomainError> {
    let Some(text) = json.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(Vec::new());
    };
    let value: Value = serde_json::from_str(text).map_err(|_| {
        DomainError::new(domain::codes::FIELD_FORMAT, "error.sub_headers_invalid")
            .with_field("request_headers")
    })?;
    let Value::Object(map) = value else {
        return Err(
            DomainError::new(domain::codes::FIELD_FORMAT, "error.sub_headers_invalid")
                .with_field("request_headers"),
        );
    };
    let mut headers = Vec::new();
    // Upstream `HttpRequestHeadersHelper.TryParse` rejects duplicate header
    // names regardless of case (`Dictionary(StringComparer.OrdinalIgnoreCase)`).
    let mut seen = std::collections::HashSet::new();
    for (name, value) in map {
        let Value::String(value) = value else {
            return Err(
                DomainError::new(domain::codes::FIELD_FORMAT, "error.sub_headers_invalid")
                    .with_field("request_headers"),
            );
        };
        if name.is_empty()
            || name.chars().any(|c| c.is_control())
            || value.chars().any(|c| c.is_control() && c != '\t')
        {
            return Err(
                DomainError::new(domain::codes::FIELD_FORMAT, "error.sub_headers_invalid")
                    .with_field("request_headers"),
            );
        }
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(
                DomainError::new(domain::codes::FIELD_FORMAT, "error.sub_headers_invalid")
                    .with_field("request_headers"),
            );
        }
        headers.push((name, value));
    }
    Ok(headers)
}

/// Per-subscription refresh result (structured; never carries payload text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubUpdateOutcome {
    Updated { added: usize, removed: usize },
    PreservedEmpty { existing: usize },
    PreservedError { code: String, message: String },
    Skipped { reason: String },
    Cancelled,
    Failed { code: String, message: String },
}

/// One entry of a refresh run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubUpdateEntry {
    pub sub_id: String,
    pub remarks: String,
    pub outcome: SubUpdateOutcome,
}

/// A whole refresh run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubUpdateReport {
    pub entries: Vec<SubUpdateEntry>,
}

impl SubUpdateReport {
    pub fn success_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e.outcome, SubUpdateOutcome::Updated { .. }))
            .count()
    }

    pub fn cancelled(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e.outcome, SubUpdateOutcome::Cancelled))
    }
}

/// Parameters for a refresh run.
#[derive(Debug, Clone, Default)]
pub struct SubUpdateRequest {
    /// Explicit sub ids; empty means every enabled subscription.
    pub sub_ids: Vec<String>,
    /// Whether to attempt the download through the running local proxy first.
    pub via_proxy: bool,
    /// Explicit local proxy endpoint (`http://127.0.0.1:PORT`), if known.
    pub proxy_url: Option<String>,
}

pub fn sub_error_outcome(err: &SubError) -> SubUpdateOutcome {
    let domain = err.to_domain();
    SubUpdateOutcome::PreservedError {
        code: domain.code,
        message: domain.message_key,
    }
}

/// Parse the downloaded subscription text into profiles.
///
/// Mirrors the `AddBatchServers` detection order through the shared parser; a
/// structural type only found by the caller (e.g. a custom core) narrows the
/// hint. An empty vector means "nothing usable".
pub fn parse_subscription(
    content: &str,
    custom_core_type: Option<i32>,
    subid: &str,
    max_items: usize,
) -> subscriptions::ParseResult {
    let hint = core_hint(custom_core_type);
    let opts = ParseOptions {
        subid: subid.to_string(),
        max_items,
        ..ParseOptions::default()
    };
    parse_content(content, hint, &opts)
}

fn core_hint(custom_core_type: Option<i32>) -> ContentHint {
    use domain::{ConfigType, CoreType};
    let _ = ConfigType::Vmess;
    match custom_core_type.and_then(CoreType::from_value) {
        Some(CoreType::SingBox) => ContentHint::Singbox,
        Some(CoreType::Xray) | Some(CoreType::V2fly) | Some(CoreType::V2flyV5) => ContentHint::Xray,
        _ => ContentHint::Auto,
    }
}

/// Merge options derived from a subscription (`Filter`, always keep-older).
pub fn merge_options(item: &SubItem) -> MergeOptions {
    MergeOptions {
        keep_older: true,
        filter: item.filter.clone(),
        ..MergeOptions::default()
    }
}

/// Download the main URL plus `MoreUrl` entries, mirroring
/// `DownloadAllSubscriptions`/`DownloadAdditionalSubscriptions` ordering.
///
/// The main body is base64-decoded first when it is a base64 list, then each
/// additional body is appended (decoded when it is itself base64).
pub async fn download_all(
    item: &SubItem,
    via_proxy: bool,
    proxy_url: Option<&str>,
    cancellation: &CancellationToken,
) -> Result<String, SubError> {
    let headers = parse_request_headers(item.request_headers.as_deref())
        .map_err(|_| SubError::HeaderInvalid("request headers".into()))?;
    if via_proxy && proxy_url.map(str::trim).filter(|s| !s.is_empty()).is_none() {
        return Err(SubError::ProxyUnavailable);
    }
    let proxy = if via_proxy {
        proxy_url.map(ProxyConfig::new)
    } else {
        None
    };
    let options = DownloadOptions {
        headers,
        user_agent: non_empty(Some(item.user_agent.clone())),
        proxy,
        ..DownloadOptions::default()
    };
    let downloader = build_client(&options)?;

    let main = download_with_fallback(&downloader, &options, &item.url, cancellation).await?;
    if item.convert_target.as_deref().unwrap_or("").is_empty() && !item.more_url.trim().is_empty() {
        let mut result = main;
        if subscriptions::util::is_base64_string(&result) {
            if let Ok(decoded) = subscriptions::util::base64_decode(&result) {
                result = decoded;
            }
        }
        for url in item.more_urls() {
            let extra = download_with_fallback(&downloader, &options, &url, cancellation).await;
            match extra {
                Ok(body) if !body.is_empty() => {
                    result.push('\n');
                    if subscriptions::util::is_base64_string(&body) {
                        if let Ok(decoded) = subscriptions::util::base64_decode(&body) {
                            result.push_str(&decoded);
                            continue;
                        }
                    }
                    result.push_str(&body);
                }
                _ => continue,
            }
        }
        return Ok(result);
    }
    Ok(main)
}

async fn download_with_fallback(
    downloader: &subscriptions::Downloader,
    options: &DownloadOptions,
    url: &str,
    cancellation: &CancellationToken,
) -> Result<String, SubError> {
    match downloader.download(url, cancellation).await {
        Ok(downloaded) if !downloaded.body.is_empty() => Ok(downloaded.body),
        first => {
            // Upstream `DownloadSubscriptionContent`: retry direct when the
            // proxied attempt yields nothing, and build a no-proxy client.
            if options.proxy.is_some() {
                let direct = DownloadOptions {
                    proxy: None,
                    ..options.clone()
                };
                let client = build_client(&direct)?;
                if let Ok(downloaded) = client.download(url, cancellation).await {
                    return Ok(downloaded.body);
                }
            }
            match first {
                Ok(downloaded) => Ok(downloaded.body),
                Err(err) => Err(err),
            }
        }
    }
}

/// Compute the candidate set for a subscription from downloaded content.
///
/// Returns `Ok(profiles)` with the filter/dedup applied, `Err` when parsing
/// produced no usable node (the caller then preserves the old set).
pub fn build_candidates(
    item: &SubItem,
    content: &str,
    existing: &[Profile],
    max_items: usize,
) -> Result<Vec<Profile>, SubError> {
    if content.trim().is_empty() {
        return Err(SubError::Empty);
    }
    let parsed = parse_subscription(content, item.custom_core_type, &item.id, max_items);
    if parsed.profiles.is_empty() {
        return Err(SubError::Unsupported(
            parsed
                .detected
                .map(ParsedFormat::as_str)
                .unwrap_or("unknown")
                .to_string(),
        ));
    }
    match subscriptions::refresh(existing, &parsed.profiles, &merge_options(item)) {
        RefreshOutcome::Replaced(result) => {
            if result.profiles.is_empty() {
                Err(SubError::Empty)
            } else {
                Ok(result.profiles)
            }
        }
        RefreshOutcome::PreservedOnEmpty { .. } | RefreshOutcome::PreservedOnError(..) => {
            Err(SubError::Empty)
        }
    }
}

/// Assign a stable index id to every unparsed candidate profile.
pub fn assign_candidate_ids(profiles: &mut [Profile]) {
    for profile in profiles.iter_mut() {
        if profile.index_id.trim().is_empty() {
            profile.index_id = new_index_id();
        }
    }
}

/// A minimal JSON representation of the structured report for event payloads.
pub fn report_to_json(report: &SubUpdateReport) -> Value {
    let entries: Vec<Value> = report
        .entries
        .iter()
        .map(|e| {
            let (status, added, existing, code, message) = match &e.outcome {
                SubUpdateOutcome::Updated { added, removed } => {
                    ("updated", Some(*added), Some(*removed), None, None)
                }
                SubUpdateOutcome::PreservedEmpty { existing } => {
                    ("preserved_empty", None, Some(*existing), None, None)
                }
                SubUpdateOutcome::PreservedError { code, message } => (
                    "preserved_error",
                    None,
                    None,
                    Some(code.clone()),
                    Some(message.clone()),
                ),
                SubUpdateOutcome::Skipped { reason } => {
                    ("skipped", None, None, None, Some(reason.clone()))
                }
                SubUpdateOutcome::Cancelled => ("cancelled", None, None, None, None),
                SubUpdateOutcome::Failed { code, message } => (
                    "failed",
                    None,
                    None,
                    Some(code.clone()),
                    Some(message.clone()),
                ),
            };
            let mut map = Map::new();
            map.insert("sub_id".into(), json!(e.sub_id));
            map.insert("remarks".into(), json!(e.remarks));
            map.insert("status".into(), json!(status));
            if let Some(added) = added {
                map.insert("added".into(), json!(added));
            }
            if let Some(existing) = existing {
                map.insert("existing".into(), json!(existing));
            }
            if let Some(code) = code {
                map.insert("code".into(), json!(code));
            }
            if let Some(message) = message {
                map.insert("message".into(), json!(message));
            }
            Value::Object(map)
        })
        .collect();
    json!({
        "success": report.success_count(),
        "cancelled": report.cancelled(),
        "entries": entries,
    })
}

/// Whether a subscription is due for a periodic update at `now` (Unix seconds).
pub fn is_due(item: &SubItem, now: i64) -> bool {
    item.enabled
        && item.auto_update_interval > 0
        && now - item.update_time >= i64::from(item.auto_update_interval) * 60
}

/// Cooperative periodic scheduler handle.
///
/// The loop runs on its own OS thread with a current-thread tokio runtime, so
/// it never needs an ambient runtime (the FRB sync entry point has none), never
/// blocks process exit, and [`Self::stop`] flips a flag and wakes the loop for a
/// prompt, graceful shutdown that leaves no timer behind.
pub struct SubScheduler {
    stop: Arc<AtomicBool>,
    wake: Arc<tokio::sync::Notify>,
    finished: Arc<AtomicBool>,
}

impl SubScheduler {
    /// Spawn the scheduler for `engine`, checking every `interval`.
    pub fn start(engine: crate::engine::AppEngine, interval: Duration, max_items: usize) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(tokio::sync::Notify::new());
        let finished = Arc::new(AtomicBool::new(false));
        let stop_loop = stop.clone();
        let wake_loop = wake.clone();
        let finished_loop = finished.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let Ok(runtime) = runtime else {
                finished_loop.store(true, Ordering::Release);
                return;
            };
            runtime.block_on(async move {
                loop {
                    if stop_loop.load(Ordering::Acquire) {
                        break;
                    }
                    run_scheduler_tick(&engine, max_items).await;
                    tokio::select! {
                        _ = tokio::time::sleep(interval) => {},
                        _ = wake_loop.notified() => {},
                    }
                }
            });
            finished_loop.store(true, Ordering::Release);
        });
        Self {
            stop,
            wake,
            finished,
        }
    }

    /// Request a graceful stop and wake the loop immediately.
    ///
    /// `notify_one` (not `notify_waiters`) stores a permit, so a stop that races
    /// the loop's `select!` is never lost and the loop exits at its next check.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.wake.notify_one();
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    /// Whether the scheduler loop has fully exited (test/teardown aid).
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

/// A single scheduler pass at a controlled clock.
///
/// Extracted from the timer loop so tests can inject a fake clock and a local
/// synthetic endpoint and observe the structured report. The combined report is
/// returned so a failed or endpoint-unavailable pass is explicit rather than
/// silently swallowed (upstream `TaskManager` logs the failure).
pub async fn run_scheduler_pass(
    engine: &crate::engine::AppEngine,
    max_items: usize,
    now: i64,
) -> SubUpdateReport {
    let due: Vec<SubItem> = match engine.list_sub_items() {
        Ok(items) => items.into_iter().filter(|item| is_due(item, now)).collect(),
        Err(_) => return SubUpdateReport::default(),
    };
    if due.is_empty() {
        return SubUpdateReport::default();
    }
    let (via_proxy, proxy_url) = scheduler_proxy_choice(engine.local_proxy_url());
    let mut report = SubUpdateReport::default();
    for item in due {
        let request = SubUpdateRequest {
            sub_ids: vec![item.id.clone()],
            via_proxy,
            proxy_url: proxy_url.clone(),
        };
        let item_report = engine
            .refresh_subscriptions(request, &CancellationToken::new(), max_items)
            .await;
        report.entries.extend(item_report.entries);
    }
    report
}

/// The background scheduler's proxy preference.
///
/// Upstream `TaskManager.UpdateTaskRunSubscription` (7d6a967) calls
/// `SubscriptionHandler.UpdateProcess(..., blProxy: true)` and
/// `DownloadSubscriptionContent` retries a direct connection when the proxied
/// attempt yields nothing. Our bridge `update_subscriptions` guard treats an
/// *explicit* `via_proxy` with no recorded endpoint as `E_PROXY_UNAVAILABLE`
/// (the FIX-09 user semantics we must not change), so the scheduler prefers the
/// recorded endpoint when one exists and otherwise downloads directly: a due
/// background pass must not fail wholesale merely because no core is running.
pub fn scheduler_proxy_choice(proxy_url: Option<String>) -> (bool, Option<String>) {
    match proxy_url {
        Some(url) if !url.trim().is_empty() => (true, Some(url)),
        _ => (false, None),
    }
}

async fn run_scheduler_tick(engine: &crate::engine::AppEngine, max_items: usize) {
    let report = run_scheduler_pass(engine, max_items, unix_now()).await;
    for entry in &report.entries {
        if let SubUpdateOutcome::PreservedError { code, message }
        | SubUpdateOutcome::Failed { code, message } = &entry.outcome
        {
            eprintln!(
                "[subs] scheduled update failed for {}: {code} {message}",
                entry.sub_id
            );
        }
    }
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_headers_must_be_string_object() {
        assert!(parse_request_headers(None).unwrap().is_empty());
        assert!(parse_request_headers(Some("  ")).unwrap().is_empty());
        let parsed = parse_request_headers(Some(r#"{"X-A":"1"}"#)).unwrap();
        assert_eq!(parsed, vec![("X-A".into(), "1".into())]);
        assert!(parse_request_headers(Some("[]")).is_err());
        assert!(parse_request_headers(Some(r#"{"X":1}"#)).is_err());
        assert!(parse_request_headers(Some("not json")).is_err());
    }

    #[test]
    fn request_headers_reject_case_insensitive_duplicates() {
        // Upstream `HttpRequestHeadersHelper` keys headers with
        // `StringComparer.OrdinalIgnoreCase`, so `X-Test` / `x-test` collide.
        // (Exact duplicate JSON keys are already collapsed by serde_json
        // before this function sees the object.)
        let error = parse_request_headers(Some(r#"{"X-Test":"1","x-test":"2"}"#)).unwrap_err();
        assert_eq!(error.code, domain::codes::FIELD_FORMAT);
        assert_eq!(error.field_path.as_deref(), Some("request_headers"));
        assert!(parse_request_headers(Some(r#"{"Content-Type":"a","content-type":"b"}"#)).is_err());
        // Distinct names survive, including ones that differ only in case.
        let ok = parse_request_headers(Some(r#"{"X-A":"1","X-B":"2"}"#)).unwrap();
        assert_eq!(ok.len(), 2);
    }

    #[test]
    fn validate_rejects_bad_url_and_headers() {
        let mut item = SubItem {
            remarks: "sub".into(),
            url: "ftp://example.com".into(),
            ..SubItem::default()
        };
        assert_eq!(
            item.validate().unwrap_err().field_path.as_deref(),
            Some("url")
        );
        item.url = "https://example.com/sub".into();
        item.request_headers = Some("{}x".into());
        assert_eq!(
            item.validate().unwrap_err().field_path.as_deref(),
            Some("request_headers")
        );
        item.request_headers = None;
        assert!(item.validate().is_ok());
    }

    #[test]
    fn validate_allows_empty_url_plain_group() {
        // FIX-06 / SET-01: an empty URL is a plain group and saves without a
        // URL error; only `remarks` is required.
        let item = SubItem {
            remarks: "普通分组".into(),
            url: String::new(),
            ..SubItem::default()
        };
        assert!(item.validate().is_ok());

        // A whitespace-only URL is treated the same as empty.
        let blank = SubItem {
            remarks: "普通分组".into(),
            url: "   ".into(),
            ..SubItem::default()
        };
        assert!(blank.validate().is_ok());

        // The remarks requirement is unchanged.
        let no_remarks = SubItem::default();
        assert_eq!(
            no_remarks.validate().unwrap_err().field_path.as_deref(),
            Some("remarks")
        );
    }

    #[test]
    fn validate_still_rejects_invalid_nonempty_url() {
        let item = SubItem {
            remarks: "sub".into(),
            url: "not a url".into(),
            ..SubItem::default()
        };
        let error = item.validate().unwrap_err();
        assert_eq!(error.code, domain::codes::FIELD_FORMAT);
        assert_eq!(error.field_path.as_deref(), Some("url"));
    }

    #[test]
    fn build_candidates_honors_custom_core_hint() {
        // F-SUB-007: the persisted `custom_core_type` narrows the parse hint
        // (here Xray, so a full Xray JSON document resolves to candidates).
        // Remote conversion itself is out of scope; the hint path is what
        // ships, and `convert_target` persistence is covered by
        // `subitem_crud_survives_reopen`.
        let item = SubItem {
            id: "s-hint".into(),
            remarks: "hint".into(),
            url: "https://example.com/s".into(),
            custom_core_type: Some(domain::CoreType::Xray.value()),
            ..SubItem::default()
        };
        let content = r#"{"inbounds":[{"port":1080,"protocol":"socks"}],"outbounds":[{"protocol":"vmess","tag":"proxy","settings":{},"streamSettings":{"network":"ws"}}]}"#;
        let out = build_candidates(&item, content, &[], 100).unwrap();
        assert_eq!(out.len(), 1);
        // The hint path leaves `subid` for `replace_sub_profiles` to fill.
        assert_eq!(out[0].core_type, Some(domain::CoreType::Xray));
    }

    #[tokio::test]
    async fn via_proxy_without_endpoint_returns_proxy_unavailable() {
        let item = SubItem {
            url: "https://example.com/sub".into(),
            ..SubItem::default()
        };
        let err = download_all(&item, true, None, &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(err, SubError::ProxyUnavailable);
        assert_eq!(err.code(), domain::codes::PROXY_UNAVAILABLE);
        // Direct mode must not raise the proxy error (it proceeds to network;
        // here we only assert the guard does not fire for via_proxy=false by
        // checking a header-validation failure fires first instead).
        let mut bad = item.clone();
        bad.request_headers = Some("not json".into());
        let err = download_all(&bad, false, None, &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(err, SubError::HeaderInvalid(_)));
    }

    #[tokio::test]
    async fn via_proxy_with_endpoint_is_not_reported_unavailable() {
        let item = SubItem {
            url: "https://example.com/sub".into(),
            ..SubItem::default()
        };
        // A whitespace-only endpoint counts as "no endpoint".
        let err = download_all(&item, true, Some("   "), &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(err, SubError::ProxyUnavailable);
        // A real endpoint bypasses the guard. A pre-cancelled token
        // short-circuits before any network I/O, so the surfaced error is
        // cancellation rather than `E_PROXY_UNAVAILABLE`.
        let cancelled = CancellationToken::new();
        assert!(cancelled.cancel());
        let err = download_all(&item, true, Some("http://127.0.0.1:9"), &cancelled)
            .await
            .unwrap_err();
        assert_ne!(err, SubError::ProxyUnavailable);
        assert_eq!(err, SubError::Cancelled);
    }

    #[test]
    fn is_due_respects_interval_and_enabled() {
        let base = SubItem {
            enabled: true,
            auto_update_interval: 10,
            update_time: 1000,
            ..SubItem::default()
        };
        assert!(!is_due(&base, 1000 + 599));
        assert!(is_due(&base, 1000 + 600));
        let disabled = SubItem {
            auto_update_interval: 0,
            ..base.clone()
        };
        assert!(!is_due(&disabled, 100000));
        let off = SubItem {
            enabled: false,
            ..base
        };
        assert!(!is_due(&off, 100000));
    }

    #[test]
    fn more_urls_split_and_trim() {
        let item = SubItem {
            more_url: " https://a/1 , ,https://b/2 ".into(),
            ..SubItem::default()
        };
        assert_eq!(item.more_urls(), vec!["https://a/1", "https://b/2"]);
    }

    #[test]
    fn row_roundtrip_keeps_all_fields() {
        let item = SubItem {
            id: "s1".into(),
            remarks: "r".into(),
            url: "https://example.com/s".into(),
            more_url: "https://a/1".into(),
            enabled: false,
            user_agent: "ua".into(),
            request_headers: Some(r#"{"X":"y"}"#.into()),
            sort: 7,
            filter: Some("Tokyo".into()),
            auto_update_interval: 30,
            update_time: 4242,
            convert_target: Some("clash".into()),
            prev_profile: Some("p1".into()),
            next_profile: Some("p2".into()),
            pre_socks_port: Some(11808),
            memo: Some("m".into()),
            custom_core_type: Some(3),
        };
        let row = item.to_row();
        assert_eq!(SubItem::from_row(&row), item);
    }

    // -- FIX-09D scheduler: due trigger, download, explicit failure, cleanup ---

    async fn bind_test_listener() -> tokio::net::TcpListener {
        for port in 11808..11950u16 {
            if let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                return listener;
            }
        }
        panic!("no free loopback port in 11808..11950");
    }

    /// A one-shot loopback HTTP server returning `body`, bound at `>= 11808`.
    async fn spawn_sub_server(body: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = bind_test_listener().await;
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let mut bytes = header.into_bytes();
                bytes.extend_from_slice(body.as_bytes());
                let _ = socket.write_all(&bytes).await;
                let _ = socket.shutdown().await;
            }
        });
        format!("http://127.0.0.1:{port}/sub")
    }

    fn synthetic_sub(id: &str, url: String) -> SubItem {
        SubItem {
            id: id.into(),
            remarks: "合成订阅".into(),
            url,
            enabled: true,
            auto_update_interval: 10,
            update_time: 0,
            ..SubItem::default()
        }
    }

    #[test]
    fn scheduler_proxy_choice_prefers_endpoint_else_direct() {
        assert_eq!(scheduler_proxy_choice(None), (false, None));
        assert_eq!(scheduler_proxy_choice(Some("   ".into())), (false, None));
        assert_eq!(
            scheduler_proxy_choice(Some("http://127.0.0.1:12000".into())),
            (true, Some("http://127.0.0.1:12000".into()))
        );
    }

    #[tokio::test]
    async fn scheduler_pass_skips_subscriptions_not_yet_due() {
        let engine = crate::engine::AppEngine::in_memory();
        let saved = engine
            .save_sub_item(synthetic_sub("s-notdue", "http://127.0.0.1:9/sub".into()))
            .expect("save");
        // `now - update_time < AutoUpdateInterval * 60`: no download attempted.
        let report = run_scheduler_pass(&engine, 100, 0).await;
        assert!(report.entries.is_empty());
        assert!(engine.profiles_by_subid(&saved.id).unwrap().is_empty());
    }

    #[tokio::test]
    async fn scheduler_pass_downloads_due_subscription_from_local_endpoint() {
        let vless =
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?encryption=none#sched";
        let url = spawn_sub_server(vless).await;
        let engine = crate::engine::AppEngine::in_memory();
        let saved = engine
            .save_sub_item(synthetic_sub("s-due", url))
            .expect("save");
        let report = run_scheduler_pass(&engine, 100, 1_000_000).await;
        assert_eq!(report.success_count(), 1, "{:?}", report.entries);
        assert_eq!(engine.profiles_by_subid(&saved.id).unwrap().len(), 1);
        let reread = engine.get_sub_item(&saved.id).unwrap().unwrap();
        assert!(
            reread.update_time > 0,
            "UpdateTime must be touched on success"
        );
    }

    #[tokio::test]
    async fn scheduler_pass_reports_unavailable_endpoint_not_fake_success() {
        // Probe a free port, then release it so the TCP connect is refused.
        let listener = bind_test_listener().await;
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let engine = crate::engine::AppEngine::in_memory();
        let saved = engine
            .save_sub_item(synthetic_sub(
                "s-dead",
                format!("http://127.0.0.1:{port}/sub"),
            ))
            .expect("save");
        let report = run_scheduler_pass(&engine, 100, 1_000_000).await;
        assert_eq!(report.success_count(), 0);
        assert_eq!(report.entries.len(), 1);
        assert!(
            matches!(
                &report.entries[0].outcome,
                SubUpdateOutcome::PreservedError { .. } | SubUpdateOutcome::Failed { .. }
            ),
            "unavailable endpoint must surface a structured failure, got {:?}",
            report.entries[0].outcome
        );
        assert!(engine.profiles_by_subid(&saved.id).unwrap().is_empty());
    }

    #[tokio::test]
    async fn scheduler_stop_exits_loop_without_residual_timer() {
        let engine = crate::engine::AppEngine::in_memory();
        let scheduler = SubScheduler::start(engine, Duration::from_millis(10), 100);
        scheduler.stop();
        assert!(scheduler.is_stopped());
        for _ in 0..100 {
            if scheduler.is_finished() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            scheduler.is_finished(),
            "scheduler loop did not exit on stop"
        );
    }
}
