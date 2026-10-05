//! Subscription use cases (T09 wiring).
//!
//! This module owns the 17-field `SubItem` model, its persistence mapping, the
//! candidate-first refresh pipeline and the periodic scheduler. It builds on the
//! pure `subscriptions` crate (`parse_content` / `refresh` / `download`) and the
//! `Store`-backed repositories, so a failed or empty download never clears the
//! existing nodes (plan §15, F-SUB-011).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use domain::{CancellationToken, ConfigType, DomainError, Profile};
use persistence::RawRow;
use serde_json::{json, Map, Value};
use subscriptions::{
    build_client, build_convert_url, parse_content, punycode_url, ContentHint, DownloadOptions,
    MergeOptions, ParseOptions, ParsedFormat, ProxyConfig, RefreshOutcome, SubError,
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

/// Resolve a subscription-level chain reference by exact Remarks (`first` on
/// index order, mirroring upstream `AppManager.GetProfileItemViaRemarks`).
fn resolve_remarks<'a>(all: &'a HashMap<String, Profile>, remarks: &str) -> Option<&'a Profile> {
    let mut matches: Vec<&Profile> = all.values().filter(|p| p.remarks == remarks).collect();
    matches.sort_by(|a, b| a.index_id.cmp(&b.index_id));
    matches.into_iter().next()
}

/// Build the virtual `ProxyChain` node that wraps `active` with the owning
/// subscription's `PrevProfile`/`NextProfile` remarks (upstream
/// `CoreConfigContextBuilder.BuildSubscriptionChainNodeAsync`).
///
/// The synthesis is generation-only: the stored `ProfileItem` rows are never
/// modified. `None` means "no chain" — the node carries no subscription, is
/// `Custom`, the subscription row is gone, or neither reference resolved.
/// Missing references are reported as non-fatal warnings and excluded
/// (upstream `MsgSubscriptionPrevProfileNotFound` /
/// `MsgSubscriptionNextProfileNotFound`); the chain then falls back to the
/// node and whichever reference did resolve.
pub fn build_subscription_chain_node(
    active: &Profile,
    all: &HashMap<String, Profile>,
    sub: Option<&SubItem>,
) -> (Option<Profile>, Vec<config_codegen::Diagnostic>) {
    let mut warnings: Vec<config_codegen::Diagnostic> = Vec::new();
    // Upstream guard: `node.Subid.IsNullOrEmpty() || ConfigType == Custom`.
    if active.subid.trim().is_empty() || active.config_type == ConfigType::Custom {
        return (None, warnings);
    }
    let Some(sub) = sub else {
        return (None, warnings);
    };

    let mut prev: Option<Profile> = None;
    if let Some(remarks) = sub.prev_profile.as_deref().filter(|raw| !raw.is_empty()) {
        match resolve_remarks(all, remarks) {
            Some(profile) => prev = Some(profile.clone()),
            None => warnings.push(config_codegen::Diagnostic::warning(
                "error.subscription_prev_profile_not_found",
                format!("subscription PrevProfile `{remarks}` does not resolve to a profile"),
                Some("SubItem.PrevProfile"),
            )),
        }
    }
    let mut next: Option<Profile> = None;
    if let Some(remarks) = sub.next_profile.as_deref().filter(|raw| !raw.is_empty()) {
        match resolve_remarks(all, remarks) {
            Some(profile) => next = Some(profile.clone()),
            None => warnings.push(config_codegen::Diagnostic::warning(
                "error.subscription_next_profile_not_found",
                format!("subscription NextProfile `{remarks}` does not resolve to a profile"),
                Some("SubItem.NextProfile"),
            )),
        }
    }
    if prev.is_none() && next.is_none() {
        return (None, warnings);
    }

    // Upstream `ChildItems = [prev?.IndexId, node.IndexId, next?.IndexId]`,
    // de-duplicated by the traversal's global visited set.
    let mut seen = HashSet::new();
    let mut child_ids: Vec<String> = Vec::new();
    for candidate in [prev.as_ref(), Some(active), next.as_ref()]
        .into_iter()
        .flatten()
    {
        if seen.insert(candidate.index_id.clone()) {
            child_ids.push(candidate.index_id.clone());
        }
    }
    let mut chain = Profile {
        index_id: format!("inner-{}", new_index_id()),
        config_type: ConfigType::ProxyChain,
        core_type: active.core_type,
        remarks: active.remarks.clone(),
        ..Default::default()
    };
    chain.proto_extra.group_type = Some(ConfigType::ProxyChain.as_str().to_string());
    chain.proto_extra.child_items = Some(child_ids.join(","));
    (Some(chain), warnings)
}

/// Built-in subscription-conversion config (upstream
/// `Global.SubConvertConfig.FirstOrDefault()`).
pub const BUILTIN_SUB_CONVERT_CONFIG: &str =
    "https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/config/ACL4SSR_Online.ini";

/// The effective converter request parameters for one refresh run.
///
/// Upstream resolves these once from `ConstItem.SubConvertUrl` (falling back to
/// `Global.SubConvertUrls.First()`) and `Global.SubConvertConfig.First()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertContext {
    pub template: String,
    pub config: String,
}

impl ConvertContext {
    /// The upstream built-ins (`Global.SubConvertUrls[0]` + config).
    pub fn builtin() -> Self {
        Self {
            template: crate::dns::BUILTIN_SUB_CONVERT_URL.to_string(),
            config: BUILTIN_SUB_CONVERT_CONFIG.to_string(),
        }
    }

    /// Resolve the configured converter service from persisted settings.
    pub fn from_const_item(const_item: &domain::ConstItem) -> Self {
        Self {
            template: crate::dns::effective_sub_convert_url(const_item),
            config: BUILTIN_SUB_CONVERT_CONFIG.to_string(),
        }
    }
}

/// The URL to download for `item`'s main body.
///
/// Plain subscriptions use the punycoded URL; a non-empty `convert_target`
/// routes the source through the converter service (`DownloadMainSubscription`).
pub fn subscription_request_url(item: &SubItem, convert: &ConvertContext) -> String {
    let source = punycode_url(item.url.trim());
    match item
        .convert_target
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(target) => build_convert_url(&convert.template, &source, target, &convert.config),
        None => source,
    }
}

fn has_convert_target(item: &SubItem) -> bool {
    item.convert_target
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty())
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
    download_all_configured(item, via_proxy, proxy_url, None, cancellation).await
}

/// Download the main body (optionally through the converter service) plus the
/// `MoreUrl` entries when no conversion is in play.
///
/// `convert` carries the effective `SubConvertUrl`/`SubConvertConfig` resolved
/// from settings; when it is `None` the upstream built-ins are used. The
/// ordering matches `DownloadAllSubscriptions`: `MoreUrl` is *skipped* whenever
/// `ConvertTarget` is set, and the main body is base64-decoded before the
/// additional bodies are appended otherwise.
pub async fn download_all_configured(
    item: &SubItem,
    via_proxy: bool,
    proxy_url: Option<&str>,
    convert: Option<&ConvertContext>,
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

    let converted = has_convert_target(item);
    let builtin = ConvertContext::builtin();
    let request_url = subscription_request_url(item, convert.unwrap_or(&builtin));
    let main = download_with_fallback(&downloader, &options, &request_url, cancellation).await?;

    if !converted && !item.more_url.trim().is_empty() {
        let mut result = main;
        if subscriptions::util::is_base64_string(&result) {
            if let Ok(decoded) = subscriptions::util::base64_decode(&result) {
                result = decoded;
            }
        }
        for url in item.more_urls() {
            let url = punycode_url(&url);
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

/// Mark every candidate as subscription-sourced (upstream
/// `AddBatchServersCommon` sets `profileItem.IsSub = isSub`, and the refresh
/// path always passes `isSub: true`). Manual/import nodes keep their source
/// flag; only the refresh candidate set is forced here.
pub fn mark_subscription_candidates(profiles: &mut [Profile]) {
    for profile in profiles.iter_mut() {
        profile.is_sub = true;
    }
}

/// Upstream `ConfigHandler.CompareProfileItem` (`ConfigHandler.cs:1258-1304`):
/// whether two nodes share the same transport identity. Empty and absent
/// string options compare equal. `remarks` optionally participates.
pub fn profiles_match(o: &Profile, n: &Profile, remarks: bool) -> bool {
    fn opt_eq(a: &Option<String>, b: &Option<String>) -> bool {
        let norm = |v: &Option<String>| v.clone().filter(|s| !s.is_empty());
        norm(a) == norm(b)
    }
    fn str_eq(a: &str, b: &str) -> bool {
        a == b || (a.is_empty() && b.is_empty())
    }
    let oe = &o.proto_extra;
    let ne = &n.proto_extra;
    let ot = &o.transport_extra;
    let nt = &n.transport_extra;
    if o.config_type != n.config_type
        || !str_eq(&o.address, &n.address)
        || o.port != n.port
        || !str_eq(&o.password, &n.password)
        || !str_eq(&o.username, &n.username)
        || !opt_eq(&oe.vless_encryption, &ne.vless_encryption)
        || !opt_eq(&oe.ss_method, &ne.ss_method)
        || !opt_eq(&oe.vmess_security, &ne.vmess_security)
        || !str_eq(&o.network, &n.network)
        || !opt_eq(&ot.raw_header_type, &nt.raw_header_type)
        || !opt_eq(&ot.host, &nt.host)
        || !opt_eq(&ot.path, &nt.path)
        || !opt_eq(&ot.xhttp_mode, &nt.xhttp_mode)
        || !opt_eq(&ot.xhttp_extra, &nt.xhttp_extra)
        || !opt_eq(&ot.grpc_authority, &nt.grpc_authority)
        || !opt_eq(&ot.grpc_service_name, &nt.grpc_service_name)
        || !opt_eq(&ot.grpc_mode, &nt.grpc_mode)
        || !opt_eq(&ot.kcp_header_type, &nt.kcp_header_type)
        || !opt_eq(&ot.kcp_seed, &nt.kcp_seed)
    {
        return false;
    }
    // Trojans ignore StreamSecurity; every other type must match it.
    if o.config_type != ConfigType::Trojan
        && !opt_eq(&o.security.stream_security, &n.security.stream_security)
    {
        return false;
    }
    opt_eq(&oe.flow, &ne.flow)
        && opt_eq(&oe.salamander_pass, &ne.salamander_pass)
        && opt_eq(&o.security.sni, &n.security.sni)
        && opt_eq(&o.security.alpn, &n.security.alpn)
        && opt_eq(&o.security.fingerprint, &n.security.fingerprint)
        && opt_eq(&o.security.public_key, &n.security.public_key)
        && opt_eq(&o.security.short_id, &n.security.short_id)
        && opt_eq(&o.finalmask, &n.finalmask)
        && (!remarks || o.remarks == n.remarks)
}

/// Upstream `ConfigHandler.FindMatchedProfileItem` (`ConfigHandler.cs:1317-1355`):
/// full identity match (remarks included), then remarks, then
/// address+port+password. `source` is the candidate set, `target` the node the
/// caller is trying to re-locate after a replace.
pub fn find_matched_profile<'a>(source: &'a [Profile], target: &Profile) -> Option<&'a Profile> {
    if let Some(found) = source.iter().find(|p| profiles_match(p, target, true)) {
        return Some(found);
    }
    if !target.remarks.is_empty() {
        if let Some(found) = source.iter().find(|p| p.remarks == target.remarks) {
            return Some(found);
        }
    }
    if !target.address.is_empty() && target.port > 0 && !target.password.is_empty() {
        return source.iter().find(|p| {
            p.address.eq_ignore_ascii_case(&target.address)
                && p.port == target.port
                && p.password.eq_ignore_ascii_case(&target.password)
        });
    }
    None
}

/// Re-map the persisted active node after a subscription replace, mirroring
/// upstream `AddBatchServers` (`ConfigHandler.cs:2109-2117`): find the new node
/// matching the old active identity and write it back as the default. When the
/// old active did not belong to this subscription, or has no match, the active
/// stays untouched (upstream's later `SetDefaultServer` fallback is out of this
/// card's scope). Returns the matched new id when one was written.
pub fn remap_active_after_replace(
    engine: &crate::engine::AppEngine,
    old_profiles: &[Profile],
    new_profiles: &[Profile],
) -> Result<Option<String>, DomainError> {
    let Some(active_id) = engine.active_profile() else {
        return Ok(None);
    };
    let Some(active) = old_profiles.iter().find(|p| p.index_id == active_id) else {
        return Ok(None);
    };
    let Some(matched) = find_matched_profile(new_profiles, active) else {
        return Ok(None);
    };
    let matched_id = matched.index_id.clone();
    if matched_id != active_id {
        engine.set_active(Some(matched_id.clone()))?;
    }
    Ok(Some(matched_id))
}

/// F-SUB-007 converted refresh: the candidate-first pipeline with the
/// subscription-conversion service applied to the main URL.
///
/// Equivalent to [`crate::engine::AppEngine::refresh_subscriptions`] but
/// resolves `ConstItem.SubConvertUrl` / `Global.SubConvertConfig` and routes
/// `ConvertTarget` subscriptions through the converter (and therefore skips
/// `MoreUrl`). The candidate-first guarantee is unchanged: a failed/empty
/// download preserves the old group and reports a structured error.
pub async fn refresh_subscriptions_with_convert(
    engine: &crate::engine::AppEngine,
    request: SubUpdateRequest,
    cancellation: &CancellationToken,
    max_items: usize,
) -> SubUpdateReport {
    let convert = engine
        .load_settings()
        .map(|loaded| ConvertContext::from_const_item(&loaded.settings.const_item))
        .unwrap_or_else(|_| ConvertContext::builtin());
    let mut report = SubUpdateReport::default();
    // Capture the epoch before any download so a commit that races a
    // restore/import is rejected (R3-SET-03).
    let epoch = engine.restore_epoch();
    let targets = match engine.list_sub_items() {
        Ok(all) if request.sub_ids.is_empty() => all,
        Ok(all) => all
            .into_iter()
            .filter(|s| request.sub_ids.contains(&s.id))
            .collect(),
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
            refresh_one_with_convert(
                engine,
                &item,
                &request,
                &convert,
                cancellation,
                max_items,
                epoch,
            )
            .await,
        );
    }
    report
}

#[allow(clippy::too_many_arguments)]
async fn refresh_one_with_convert(
    engine: &crate::engine::AppEngine,
    item: &SubItem,
    request: &SubUpdateRequest,
    convert: &ConvertContext,
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
    let existing = engine.profiles_by_subid(&item.id).unwrap_or_default();

    // Candidate-first: download and parse before touching storage.
    let content = match download_all_configured(
        item,
        request.via_proxy,
        request.proxy_url.as_deref(),
        Some(convert),
        cancellation,
    )
    .await
    {
        Ok(content) => content,
        Err(SubError::Cancelled) => return entry(SubUpdateOutcome::Cancelled),
        Err(err) => return entry(sub_error_outcome(&err)),
    };

    let mut candidates = match build_candidates(item, &content, &existing, max_items) {
        Ok(profiles) => profiles,
        Err(err) => return entry(sub_error_outcome(&err)),
    };
    assign_candidate_ids(&mut candidates);
    mark_subscription_candidates(&mut candidates);

    match engine.replace_sub_profiles_at_epoch(&item.id, candidates, true, epoch) {
        Ok((added, removed)) => {
            // Upstream `AddBatchServers` re-points the persisted default to the
            // new node matching the old active identity (`ConfigHandler.cs`
            // `:2109-2117`); stats transfer is done in the storage transaction.
            if let Ok(new_profiles) = engine.profiles_by_subid(&item.id) {
                let _ = remap_active_after_replace(engine, &existing, &new_profiles);
            }
            let _ = engine.touch_sub_update_time(&item.id, unix_now());
            entry(SubUpdateOutcome::Updated { added, removed })
        }
        Err(err) => entry(SubUpdateOutcome::Failed {
            code: err.code,
            message: err.message_key,
        }),
    }
}

/// A minimal JSON representation of the structured report for event payloads.
pub fn report_to_json(report: &SubUpdateReport) -> Value {
    let entries: Vec<Value> = report
        .entries
        .iter()
        .map(|e| {
            let (status, added, existing, removed, code, message) = match &e.outcome {
                // A group that downloaded and merged: `added` is the real new
                // row count and `removed` the real removed count, never a net
                // growth.
                SubUpdateOutcome::Updated { added, removed } => {
                    ("updated", Some(*added), None, Some(*removed), None, None)
                }
                SubUpdateOutcome::PreservedEmpty { existing } => {
                    ("preserved_empty", None, Some(*existing), None, None, None)
                }
                SubUpdateOutcome::PreservedError { code, message } => (
                    "preserved_error",
                    None,
                    None,
                    None,
                    Some(code.clone()),
                    Some(message.clone()),
                ),
                SubUpdateOutcome::Skipped { reason } => {
                    ("skipped", None, None, None, None, Some(reason.clone()))
                }
                SubUpdateOutcome::Cancelled => ("cancelled", None, None, None, None, None),
                SubUpdateOutcome::Failed { code, message } => (
                    "failed",
                    None,
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
            if let Some(removed) = removed {
                map.insert("removed".into(), json!(removed));
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
    /// Cancels the in-flight download(s) of the current tick so a stop does not
    /// wait for a slow network response (R3-SET-03).
    cancel: CancellationToken,
}

impl SubScheduler {
    /// Spawn the scheduler for `engine`, checking every `interval`.
    pub fn start(engine: crate::engine::AppEngine, interval: Duration, max_items: usize) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(tokio::sync::Notify::new());
        let finished = Arc::new(AtomicBool::new(false));
        let cancel = CancellationToken::new();
        let stop_loop = stop.clone();
        let wake_loop = wake.clone();
        let finished_loop = finished.clone();
        let cancel_loop = cancel.clone();
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
                    run_scheduler_tick(&engine, max_items, &cancel_loop).await;
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
            cancel,
        }
    }

    /// Request a graceful stop and wake the loop immediately.
    ///
    /// `notify_one` (not `notify_waiters`) stores a permit, so a stop that races
    /// the loop's `select!` is never lost and the loop exits at its next check.
    /// The cancellation token is flipped first so a download already in flight
    /// aborts at its next safe point.
    pub fn stop(&self) {
        self.cancel.cancel();
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
    cancellation: &CancellationToken,
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
        if cancellation.is_cancelled() {
            report.entries.push(SubUpdateEntry {
                sub_id: item.id.clone(),
                remarks: item.remarks.clone(),
                outcome: SubUpdateOutcome::Cancelled,
            });
            break;
        }
        let request = SubUpdateRequest {
            sub_ids: vec![item.id.clone()],
            via_proxy,
            proxy_url: proxy_url.clone(),
        };
        let item_report =
            refresh_subscriptions_with_convert(engine, request, cancellation, max_items).await;
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

async fn run_scheduler_tick(
    engine: &crate::engine::AppEngine,
    max_items: usize,
    cancellation: &CancellationToken,
) {
    let report = run_scheduler_pass(engine, max_items, unix_now(), cancellation).await;
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

    /// A port that is guaranteed to be refused: bound, then released. The scan
    /// starts above the range `bind_test_listener` uses so a parallel test
    /// cannot immediately re-bind the released port and turn the "dead"
    /// endpoint alive (observed as a cross-test flake).
    async fn dead_endpoint_port() -> u16 {
        for port in 21000..21100u16 {
            if let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                let resolved = listener.local_addr().unwrap().port();
                drop(listener);
                return resolved;
            }
        }
        panic!("no free loopback port in 21000..21100");
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
        let report = run_scheduler_pass(&engine, 100, 0, &CancellationToken::new()).await;
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
        let report = run_scheduler_pass(&engine, 100, 1_000_000, &CancellationToken::new()).await;
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
        // A port released from a disjoint high range so the TCP connect is
        // reliably refused even with parallel tests binding 11808..11950.
        let port = dead_endpoint_port().await;
        let engine = crate::engine::AppEngine::in_memory();
        let saved = engine
            .save_sub_item(synthetic_sub(
                "s-dead",
                format!("http://127.0.0.1:{port}/sub"),
            ))
            .expect("save");
        let report = run_scheduler_pass(&engine, 100, 1_000_000, &CancellationToken::new()).await;
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
    async fn refresh_report_is_per_group_success_failure_and_empty_url() {
        // SR-01: A succeeds, B cannot be downloaded, C is an enabled plain
        // (empty URL) group. The report must keep the three outcomes distinct
        // and expose the real `added` count for A instead of inferring "all
        // succeeded" from success_count > 0.
        let vless =
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?encryption=none#sr01";
        let ok_url = spawn_sub_server(vless).await;
        let dead_port = dead_endpoint_port().await;
        let engine = crate::engine::AppEngine::in_memory();
        engine
            .save_sub_item(SubItem {
                id: "s-a".into(),
                remarks: "A".into(),
                url: ok_url,
                enabled: true,
                ..SubItem::default()
            })
            .expect("save A");
        engine
            .save_sub_item(SubItem {
                id: "s-b".into(),
                remarks: "B".into(),
                url: format!("http://127.0.0.1:{dead_port}/sub"),
                enabled: true,
                ..SubItem::default()
            })
            .expect("save B");
        engine
            .save_sub_item(SubItem {
                id: "s-c".into(),
                remarks: "C 普通分组".into(),
                url: String::new(),
                enabled: true,
                ..SubItem::default()
            })
            .expect("save C");

        let report = refresh_subscriptions_with_convert(
            &engine,
            SubUpdateRequest::default(),
            &CancellationToken::new(),
            100,
        )
        .await;
        assert_eq!(report.success_count(), 1, "{:?}", report.entries);
        let entry = |id: &str| {
            report
                .entries
                .iter()
                .find(|e| e.sub_id == id)
                .unwrap_or_else(|| panic!("missing entry {id}"))
        };
        assert!(
            matches!(
                entry("s-a").outcome,
                SubUpdateOutcome::Updated { added: 1, .. }
            ),
            "A must report the real added count, got {:?}",
            entry("s-a").outcome
        );
        assert!(
            matches!(
                entry("s-b").outcome,
                SubUpdateOutcome::PreservedError { .. } | SubUpdateOutcome::Failed { .. }
            ),
            "B must surface a structured failure, got {:?}",
            entry("s-b").outcome
        );
        assert!(
            matches!(entry("s-c").outcome, SubUpdateOutcome::Skipped { .. }),
            "empty-URL group must be skipped, not counted as success, got {:?}",
            entry("s-c").outcome
        );

        let json = report_to_json(&report);
        assert_eq!(json["success"], 1);
        let entries = json["entries"].as_array().expect("entries array");
        let a = entries
            .iter()
            .find(|e| e["sub_id"] == "s-a")
            .expect("A json entry");
        assert_eq!(a["status"], "updated");
        assert_eq!(a["added"], 1);
        assert!(a.get("removed").is_some(), "report must carry removed");
        let b = entries
            .iter()
            .find(|e| e["sub_id"] == "s-b")
            .expect("B json entry");
        assert_ne!(b["status"], "updated");
        let c = entries
            .iter()
            .find(|e| e["sub_id"] == "s-c")
            .expect("C json entry");
        assert_eq!(c["status"], "skipped");
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

    #[tokio::test]
    async fn scheduler_stop_cancels_slow_download_before_commit() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        // A slow one-shot server: holds the response for 800ms so the tick is
        // still in flight when `stop` is called.
        let listener = bind_test_listener().await;
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await;
                tokio::time::sleep(Duration::from_millis(800)).await;
                let body = "vless://11111111-1111-1111-1111-111111111111@example.com:443?encryption=none#slow";
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
        let engine = crate::engine::AppEngine::in_memory();
        let saved = engine
            .save_sub_item(synthetic_sub(
                "s-slow",
                format!("http://127.0.0.1:{port}/sub"),
            ))
            .expect("save");
        let scheduler = SubScheduler::start(engine.clone(), Duration::from_secs(3600), 100);
        // Let the first tick start the download.
        tokio::time::sleep(Duration::from_millis(120)).await;
        let started = std::time::Instant::now();
        scheduler.stop();
        for _ in 0..200 {
            if scheduler.is_finished() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(scheduler.is_finished(), "cancelled tick must end the loop");
        assert!(
            started.elapsed() < Duration::from_millis(700),
            "stop must abort the slow download, not wait for the body"
        );
        // Candidate-first: the aborted download never replaced the node set.
        assert!(
            engine.profiles_by_subid(&saved.id).unwrap().is_empty(),
            "an aborted scheduler download must not commit profiles"
        );
    }

    // -- FIX-09B: subscription conversion target ----------------------------

    #[test]
    fn subscription_request_url_builds_convert_request_for_target() {
        let item = SubItem {
            url: "https://example.com/s".into(),
            convert_target: Some("clash".into()),
            ..SubItem::default()
        };
        let ctx = ConvertContext {
            template: "https://c/sub?url={0}".into(),
            config: "cfg.ini".into(),
        };
        assert_eq!(
            subscription_request_url(&item, &ctx),
            "https://c/sub?url=https%3A%2F%2Fexample.com%2Fs&target=clash&config=cfg.ini"
        );
        // No target: the plain punycoded URL is used.
        let plain = SubItem {
            url: "https://example.com/s".into(),
            ..SubItem::default()
        };
        assert_eq!(
            subscription_request_url(&plain, &ctx),
            "https://example.com/s"
        );
        // `Global.SubConvertUrls[0]` fallback is used when settings are unset.
        let builtin = ConvertContext::builtin();
        assert_eq!(builtin.template, crate::dns::BUILTIN_SUB_CONVERT_URL);
        assert_eq!(builtin.config, BUILTIN_SUB_CONVERT_CONFIG);
    }

    /// A loopback server that captures every request and answers with a fixed
    /// status/body. Bound at `>= 11808`.
    async fn spawn_capture_server(
        status: &'static str,
        body: &'static str,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = bind_test_listener().await;
        let port = listener.local_addr().unwrap().port();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let cap = captured.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = vec![0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                cap.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).into_owned());
                let header = format!(
                    "{status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let mut bytes = header.into_bytes();
                bytes.extend_from_slice(body.as_bytes());
                let _ = socket.write_all(&bytes).await;
                let _ = socket.shutdown().await;
            }
        });
        (format!("http://127.0.0.1:{port}/sub"), captured)
    }

    fn set_convert_url(engine: &crate::engine::AppEngine, template: String) {
        let revision = engine.settings_revision();
        let mut loaded = engine.load_settings().expect("settings");
        loaded.settings.const_item.sub_convert_url = Some(template);
        engine
            .save_settings(loaded.settings, revision)
            .expect("save settings");
    }

    #[tokio::test]
    async fn download_configured_uses_converter_and_skips_more_url() {
        let vless = "vless://11111111-1111-1111-1111-111111111111@example.com:443?encryption=none#converted";
        let (base, captured) = spawn_capture_server("HTTP/1.1 200 OK", vless).await;
        let item = SubItem {
            url: "https://example.com/actual?token=abc".into(),
            more_url: format!("{base}/more"),
            convert_target: Some("mixed".into()),
            user_agent: "V2RayN-Test/1.0".into(),
            request_headers: Some(r#"{"X-Test":"1"}"#.into()),
            ..SubItem::default()
        };
        let convert = ConvertContext {
            template: format!("{base}?url={{0}}"),
            config: "https://cfg.example/ACL4SSR_Online.ini".into(),
        };
        let content = download_all_configured(
            &item,
            false,
            None,
            Some(&convert),
            &CancellationToken::new(),
        )
        .await
        .expect("converted download");
        assert!(content.contains("vless://"), "{content}");

        let requests = captured.lock().unwrap().clone();
        assert_eq!(
            requests.len(),
            1,
            "MoreUrl must be skipped when converting: {requests:?}"
        );
        let request = &requests[0];
        assert!(
            request.contains("url=https%3A%2F%2Fexample.com%2Factual%3Ftoken%3Dabc"),
            "source must be URL-encoded: {request}"
        );
        assert!(request.contains("target=mixed"), "{request}");
        assert!(request.contains("config=http"), "{request}");
        assert!(request.contains("ACL4SSR_Online.ini"), "{request}");
        assert!(
            request.to_ascii_lowercase().contains("x-test: 1"),
            "custom header missing: {request}"
        );
        assert!(
            request.to_ascii_lowercase().contains("v2rayn-test/1.0"),
            "user agent missing: {request}"
        );
    }

    #[tokio::test]
    async fn converted_pipeline_replaces_group_with_converted_nodes() {
        let vless = "vless://22222222-2222-2222-2222-222222222222@converted.example:443?encryption=none#conv";
        let (base, captured) = spawn_capture_server("HTTP/1.1 200 OK", vless).await;
        let engine = crate::engine::AppEngine::in_memory();
        set_convert_url(&engine, format!("{base}?url={{0}}"));
        let saved = engine
            .save_sub_item(SubItem {
                remarks: "conv".into(),
                url: "https://example.com/origin".into(),
                more_url: format!("{base}/more"),
                convert_target: Some("v2ray".into()),
                ..SubItem::default()
            })
            .expect("save");
        let report = refresh_subscriptions_with_convert(
            &engine,
            SubUpdateRequest {
                sub_ids: vec![saved.id.clone()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            100,
        )
        .await;
        assert_eq!(report.success_count(), 1, "{:?}", report.entries);
        assert_eq!(engine.profiles_by_subid(&saved.id).unwrap().len(), 1);
        let requests = captured.lock().unwrap().clone();
        assert_eq!(requests.len(), 1, "{requests:?}");
        assert!(requests[0].contains("target=v2ray"), "{}", requests[0]);
    }

    #[tokio::test]
    async fn convert_failure_preserves_old_group() {
        let (base, _captured) =
            spawn_capture_server("HTTP/1.1 500 Internal Server Error", "boom").await;
        let engine = crate::engine::AppEngine::in_memory();
        set_convert_url(&engine, format!("{base}?url={{0}}"));
        let saved = engine
            .save_sub_item(SubItem {
                remarks: "conv".into(),
                url: "https://example.com/origin".into(),
                convert_target: Some("clash".into()),
                ..SubItem::default()
            })
            .expect("save");

        let mut old = subscriptions::resolve_uri(
            "vless://99999999-9999-9999-9999-999999999999@old.example:443?encryption=none#keep",
        )
        .unwrap();
        old.index_id = "old-conv".into();
        old.subid = saved.id.clone();
        engine
            .save_profile(old, domain::DesiredRevision::new(engine.desired_revision()))
            .unwrap();

        let report = refresh_subscriptions_with_convert(
            &engine,
            SubUpdateRequest {
                sub_ids: vec![saved.id.clone()],
                via_proxy: false,
                proxy_url: None,
            },
            &CancellationToken::new(),
            100,
        )
        .await;
        assert_eq!(report.success_count(), 0);
        assert!(
            matches!(
                report.entries[0].outcome,
                SubUpdateOutcome::PreservedError { .. } | SubUpdateOutcome::Failed { .. }
            ),
            "converter failure must be structured: {:?}",
            report.entries[0].outcome
        );
        let profiles = engine.profiles_by_subid(&saved.id).unwrap();
        assert_eq!(profiles.len(), 1);
        assert!(profiles.iter().any(|p| p.index_id == "old-conv"));
    }

    // -- R4-17: IsSub retention, stable identity & active remap ---------------

    #[test]
    fn replace_keeps_manual_is_sub_false_in_group() {
        use crate::synthetic::synthetic_full_profile;
        let engine = crate::engine::AppEngine::in_memory();
        let mut manual = synthetic_full_profile(1);
        manual.subid = "s-A".into();
        manual.is_sub = false;
        let manual_id = manual.index_id.clone();
        let (_saved, revision) = engine
            .save_imported_profile(manual.clone(), domain::DesiredRevision::ZERO)
            .unwrap();
        let mut sub = synthetic_full_profile(2);
        sub.subid = "s-A".into();
        sub.is_sub = true;
        engine.save_imported_profile(sub.clone(), revision).unwrap();

        let (added, removed) = engine.replace_sub_profiles("s-A", vec![], true).unwrap();
        assert_eq!(added, 0);
        assert_eq!(removed, 1);
        let remaining = engine.profiles_by_subid("s-A").unwrap();
        assert!(
            remaining.iter().any(|p| p.index_id == manual_id),
            "manual IsSub=false node must survive a subscription replace"
        );
        assert!(!remaining.iter().any(|p| p.index_id == sub.index_id));
    }

    #[test]
    fn subscription_candidates_are_marked_is_sub() {
        let mut profile = Profile {
            is_sub: false,
            ..Default::default()
        };
        mark_subscription_candidates(std::slice::from_mut(&mut profile));
        assert!(profile.is_sub);
    }

    #[test]
    fn profiles_match_uses_transport_identity() {
        use crate::synthetic::synthetic_full_profile;
        let a = synthetic_full_profile(1);
        let mut b = a.clone();
        b.index_id = "other".into();
        b.remarks = "renamed".into();
        assert!(profiles_match(&a, &b, false));
        assert!(!profiles_match(&a, &b, true), "remarks differ");
        let mut c = b.clone();
        c.address = "203.0.113.7".into();
        assert!(!profiles_match(&a, &c, false), "address differs");
    }

    #[test]
    fn find_matched_profile_falls_back_to_remarks() {
        use crate::synthetic::synthetic_full_profile;
        let target = synthetic_full_profile(1);
        let mut candidate = target.clone();
        candidate.index_id = "n-2".into();
        candidate.address = "203.0.113.9".into();
        candidate.port = 8443;
        candidate.password = "different".into();
        let found = find_matched_profile(std::slice::from_ref(&candidate), &target).unwrap();
        assert_eq!(found.index_id, "n-2");
    }

    #[test]
    fn remap_active_after_replace_points_to_matched_new_node() {
        use crate::synthetic::synthetic_full_profile;
        let engine = crate::engine::AppEngine::in_memory();
        let mut old = synthetic_full_profile(1);
        old.subid = "s-r".into();
        old.is_sub = true;
        engine
            .save_imported_profile(old.clone(), domain::DesiredRevision::ZERO)
            .unwrap();
        engine.set_active(Some(old.index_id.clone())).unwrap();

        let mut new = old.clone();
        new.index_id = "new-active".into();
        engine
            .replace_sub_profiles("s-r", vec![new.clone()], true)
            .unwrap();
        let new_list = engine.profiles_by_subid("s-r").unwrap();

        let mapped =
            remap_active_after_replace(&engine, std::slice::from_ref(&old), &new_list).unwrap();
        assert_eq!(mapped.as_deref(), Some("new-active"));
        assert_eq!(engine.active_profile().as_deref(), Some("new-active"));
    }
}
