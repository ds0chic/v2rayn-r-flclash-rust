//! T09 FRB functions: subscription CRUD, the update pipeline/job, the
//! scheduler and node import/export (share URIs, base64 lists, inner URIs).
//!
//! The dialog/screenshot evidence lives in `docs/evidence/T09-wiring.md`; the
//! pure Fmt/parse/download logic is the `subscriptions` crate (T09) and is not
//! re-implemented here.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use application::{SubItem, SubUpdateReport, SubUpdateRequest};
use domain::job::JobId;
use domain::{CancellationToken, DomainError, Profile};
use flutter_rust_bridge::frb;

use crate::api::contract::{
    DeleteSubsResult, ErrorDto, ImportResult, JobDto, ParseIssueDto, ShareExportResult,
    SimpleResult, SubItemDto, SubItemDtoResult, SubUpdateEntryDto, SubUpdateResult, SubsPageDto,
    UriParseResult,
};
use crate::api::engine::{emit_control, engine, error_dto, job_view_dto, profile_dto};

/// Maximum profiles an import/refresh will produce. Bounds memory and keeps the
/// synthetic test payloads small.
pub const MAX_IMPORT_ITEMS: usize = 10_000;

/// Poll interval for the background scheduler.
pub const SCHEDULER_TICK: Duration = Duration::from_secs(60);

// -- subscription CRUD -----------------------------------------------------

fn sub_to_dto(item: SubItem) -> SubItemDto {
    SubItemDto {
        id: item.id,
        remarks: item.remarks,
        url: item.url,
        more_url: item.more_url,
        enabled: item.enabled,
        user_agent: item.user_agent,
        request_headers: item.request_headers,
        sort: item.sort,
        filter: item.filter,
        auto_update_interval: item.auto_update_interval,
        update_time: item.update_time,
        convert_target: item.convert_target,
        prev_profile: item.prev_profile,
        next_profile: item.next_profile,
        pre_socks_port: item.pre_socks_port,
        memo: item.memo,
        custom_core_type: item.custom_core_type,
    }
}

fn dto_to_sub(d: SubItemDto) -> SubItem {
    SubItem {
        id: d.id,
        remarks: d.remarks,
        url: d.url,
        more_url: d.more_url,
        enabled: d.enabled,
        user_agent: d.user_agent,
        request_headers: d.request_headers,
        sort: d.sort,
        filter: d.filter,
        auto_update_interval: d.auto_update_interval,
        update_time: d.update_time,
        convert_target: d.convert_target,
        prev_profile: d.prev_profile,
        next_profile: d.next_profile,
        pre_socks_port: d.pre_socks_port,
        memo: d.memo,
        custom_core_type: d.custom_core_type,
    }
}

/// `list_sub_items` — every subscription ordered by `Sort`.
#[frb(sync)]
pub fn list_sub_items() -> SubsPageDto {
    match engine().list_sub_items() {
        Ok(items) => SubsPageDto {
            items: items.into_iter().map(sub_to_dto).collect(),
            error: None,
        },
        Err(error) => SubsPageDto {
            items: Vec::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `get_sub_item` — one subscription by id.
#[frb(sync)]
pub fn get_sub_item(id: String) -> Option<SubItemDto> {
    engine().get_sub_item(&id).ok().flatten().map(sub_to_dto)
}

/// `save_sub_item` — validate + persist; assigns id/sort when new.
#[frb(sync)]
pub fn save_sub_item(item: SubItemDto) -> SubItemDtoResult {
    match engine().save_sub_item(dto_to_sub(item)) {
        Ok(saved) => SubItemDtoResult {
            ok: true,
            item: Some(sub_to_dto(saved)),
            error: None,
        },
        Err(error) => SubItemDtoResult {
            ok: false,
            item: None,
            error: Some(error_dto(error)),
        },
    }
}

/// `delete_sub_items` — delete a selection.
#[frb(sync)]
pub fn delete_sub_items(ids: Vec<String>) -> DeleteSubsResult {
    match engine().delete_sub_items(&ids) {
        Ok(removed) => DeleteSubsResult {
            ok: true,
            removed,
            error: None,
        },
        Err(error) => DeleteSubsResult {
            ok: false,
            removed: 0,
            error: Some(error_dto(error)),
        },
    }
}

/// `set_sub_enabled` — enable/disable one subscription.
#[frb(sync)]
pub fn set_sub_enabled(id: String, enabled: bool) -> SubItemDtoResult {
    match engine().set_sub_enabled(&id, enabled) {
        Ok(item) => SubItemDtoResult {
            ok: true,
            item: Some(sub_to_dto(item)),
            error: None,
        },
        Err(error) => SubItemDtoResult {
            ok: false,
            item: None,
            error: Some(error_dto(error)),
        },
    }
}

/// `reorder_sub_items` — persist a new display order.
#[frb(sync)]
pub fn reorder_sub_items(ids: Vec<String>) -> SimpleResult {
    match engine().reorder_sub_items(&ids) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(error) => SimpleResult {
            ok: false,
            error: Some(error_dto(error)),
        },
    }
}

/// `validate_sub_item` — UI-side pre-check without persisting.
#[frb(sync)]
pub fn validate_sub_item(item: SubItemDto) -> SimpleResult {
    match dto_to_sub(item).validate() {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(error) => SimpleResult {
            ok: false,
            error: Some(error_dto(error)),
        },
    }
}

/// `set_local_proxy_port` — record the running session's mixed/socks port.
#[frb(sync)]
pub fn set_local_proxy_port(port: Option<u16>) {
    engine().set_local_proxy_port(port);
}

// -- update pipeline / job -------------------------------------------------

/// Per-process counter minting subscription update job ids.
static SUB_JOB_SEQ: OnceLock<AtomicU64> = OnceLock::new();
/// Debug-only count of live update jobs (not a cancellation path).
/// Cancellation always goes through `JobManager::cancel` via the job id
/// returned to the caller; this registry is never read by `cancel_job`.
static SUB_JOBS: OnceLock<Mutex<Vec<JobId>>> = OnceLock::new();

fn sub_job_seq() -> &'static AtomicU64 {
    SUB_JOB_SEQ.get_or_init(|| AtomicU64::new(0))
}

fn sub_jobs() -> &'static Mutex<Vec<JobId>> {
    SUB_JOBS.get_or_init(|| Mutex::new(Vec::new()))
}

/// A cancellation token plus the job id for a subscription update.
fn register_sub_job() -> (JobId, CancellationToken) {
    let seq = sub_job_seq().fetch_add(1, Ordering::AcqRel) + 1;
    let _seq_id = JobId::new(format!("sub-job-{seq:08}"));
    let job = engine().jobs().start("update_subscription");
    let token = engine().jobs().token(&job.job_id).unwrap_or_default();
    if let Ok(mut jobs) = sub_jobs().lock() {
        jobs.push(job.job_id.clone());
    }
    (job.job_id, token)
}

fn finish_sub_job(job_id: &JobId, report: &SubUpdateReport) {
    use domain::JobState;
    let state = if report.cancelled() {
        JobState::Cancelled
    } else if report.success_count() > 0 {
        JobState::Done
    } else {
        JobState::Failed
    };
    let error = (state == JobState::Failed)
        .then(|| DomainError::new(domain::codes::UNAVAILABLE, "error.sub_update_failed"));
    engine().jobs().finish(job_id, state, error);
    if let Ok(mut jobs) = sub_jobs().lock() {
        jobs.retain(|id| id != job_id);
    }
}

fn entry_to_dto(entry: application::SubUpdateEntry) -> SubUpdateEntryDto {
    use application::SubUpdateOutcome as O;
    let (status, added, existing, code, message) = match entry.outcome {
        O::Updated { added, removed } => (
            "updated".to_string(),
            Some(added as u32),
            Some(removed as u32),
            None,
            None,
        ),
        O::PreservedEmpty { existing } => (
            "preserved_empty".to_string(),
            None,
            Some(existing as u32),
            None,
            None,
        ),
        O::PreservedError { code, message } => (
            "preserved_error".to_string(),
            None,
            None,
            Some(code),
            Some(message),
        ),
        O::Skipped { reason } => ("skipped".to_string(), None, None, None, Some(reason)),
        O::Cancelled => ("cancelled".to_string(), None, None, None, None),
        O::Failed { code, message } => {
            ("failed".to_string(), None, None, Some(code), Some(message))
        }
    };
    SubUpdateEntryDto {
        sub_id: entry.sub_id,
        remarks: entry.remarks,
        status,
        added,
        existing,
        code,
        message,
    }
}

fn report_to_result(report: SubUpdateReport) -> SubUpdateResult {
    SubUpdateResult {
        ok: report.success_count() > 0,
        success: report.success_count() as u32,
        cancelled: report.cancelled(),
        entries: report.entries.into_iter().map(entry_to_dto).collect(),
        job_id: None,
        error: None,
    }
}

/// `update_subscriptions` — the F-SUB-003 pipeline.
///
/// `sub_ids` empty means every subscription; `via_proxy` uses the recorded
/// local session port. When `via_proxy` is requested but no local proxy
/// endpoint is known, a structured `E_PROXY_UNAVAILABLE` error is returned
/// and no direct download is attempted. The returned `job_id` can be passed
/// to `cancel_job`.
pub async fn update_subscriptions(sub_ids: Vec<String>, via_proxy: bool) -> SubUpdateResult {
    let proxy_url = via_proxy.then(|| engine().local_proxy_url()).flatten();
    if via_proxy
        && proxy_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return SubUpdateResult {
            ok: false,
            success: 0,
            cancelled: false,
            entries: Vec::new(),
            job_id: None,
            error: Some(ErrorDto {
                code: domain::codes::PROXY_UNAVAILABLE.to_string(),
                message_key: "error.proxy_unavailable".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: Some("no local proxy endpoint available".to_string()),
            }),
        };
    }
    let (job_id, token) = register_sub_job();
    let request = SubUpdateRequest {
        sub_ids,
        via_proxy,
        proxy_url,
    };
    let report = engine()
        .refresh_subscriptions(request, &token, MAX_IMPORT_ITEMS)
        .await;
    finish_sub_job(&job_id, &report);
    emit_control(
        "subscriptions_updated",
        application::report_to_json(&report),
    );
    let mut result = report_to_result(report);
    result.job_id = Some(job_id.0);
    result
}

/// `update_subscription` — refresh a single subscription.
pub async fn update_subscription(sub_id: String, via_proxy: bool) -> SubUpdateResult {
    update_subscriptions(vec![sub_id], via_proxy).await
}

/// `start_sub_scheduler` — start the periodic update loop (idempotent).
#[frb(sync)]
pub fn start_sub_scheduler() -> SimpleResult {
    let started = engine().start_sub_scheduler(SCHEDULER_TICK, MAX_IMPORT_ITEMS);
    SimpleResult {
        ok: started,
        error: (!started).then(|| ErrorDto {
            code: domain::codes::INTERNAL.to_string(),
            message_key: "error.scheduler_start_failed".to_string(),
            field_path: None,
            retryable: false,
            operation_id: None,
            detail: None,
        }),
    }
}

/// `stop_sub_scheduler` — graceful stop; never holds up process exit.
#[frb(sync)]
pub fn stop_sub_scheduler() -> SimpleResult {
    engine().stop_sub_scheduler();
    SimpleResult {
        ok: true,
        error: None,
    }
}

/// `sub_scheduler_running` — diagnostics.
#[frb(sync)]
pub fn sub_scheduler_running() -> bool {
    engine().sub_scheduler_running()
}

/// `job_view` — current state of any job (including subscription updates).
#[frb(sync)]
pub fn job_view(job_id: String) -> Option<JobDto> {
    engine().jobs().get(&JobId::new(job_id)).map(job_view_dto)
}

// -- node import -----------------------------------------------------------

/// `import_from_text` — parse share URIs / base64 / inner URIs from text.
///
/// `subid` non-empty attaches the imported nodes to that subscription;
/// `deduplicate` applies the upstream `KeepOlderDedupl` collapse. Returns the
/// imported profiles plus located per-line errors (F-IMPORT-001/002/005).
pub fn import_from_text(text: String, subid: Option<String>, deduplicate: bool) -> ImportResult {
    let parsed = subscriptions::parse_content(
        &text,
        subscriptions::ContentHint::Auto,
        &subscriptions::ParseOptions {
            subid: subid.clone().unwrap_or_default(),
            max_items: MAX_IMPORT_ITEMS,
            ..subscriptions::ParseOptions::default()
        },
    );
    if parsed.profiles.is_empty() {
        return ImportResult {
            ok: false,
            imported: 0,
            profiles: Vec::new(),
            errors: parsed
                .errors
                .iter()
                .map(|e| ParseIssueDto {
                    code: e.code.clone(),
                    message: e.message.clone(),
                    item_index: e.item_index.map(|i| i as u32),
                    byte_offset: e.byte_offset.map(|o| o as u64),
                })
                .collect(),
            error: Some(ErrorDto {
                code: domain::codes::FIELD_FORMAT.to_string(),
                message_key: "error.import_nothing".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: None,
            }),
        };
    }
    let mut profiles = parsed.profiles;
    let mut deduped = 0usize;
    if deduplicate {
        let (kept, removed) = subscriptions::deduplicate(&profiles, true);
        profiles = kept;
        deduped = removed;
    }
    for profile in &mut profiles {
        if profile.subid.is_empty() {
            profile.subid = subid.clone().unwrap_or_default();
        }
        if profile.index_id.trim().is_empty() {
            profile.index_id = application::new_index_id();
        }
        profile.is_sub = true;
    }
    let count = profiles.len() as u32;
    if let Some(sub) = subid.filter(|s| !s.is_empty()) {
        if let Err(error) = engine().replace_sub_profiles(&sub, profiles.clone(), false) {
            return ImportResult {
                ok: false,
                imported: 0,
                profiles: Vec::new(),
                errors: Vec::new(),
                error: Some(error_dto(error)),
            };
        }
        emit_control(
            "profiles_imported",
            serde_json::json!({ "sub_id": sub, "imported": count, "deduplicated": deduped }),
        );
    }
    ImportResult {
        ok: true,
        imported: count,
        profiles: profiles.into_iter().map(profile_dto).collect(),
        errors: parsed
            .errors
            .iter()
            .map(|e| ParseIssueDto {
                code: e.code.clone(),
                message: e.message.clone(),
                item_index: e.item_index.map(|i| i as u32),
                byte_offset: e.byte_offset.map(|o| o as u64),
            })
            .collect(),
        error: None,
    }
}

/// `parse_share_uri` — resolve a single line without persisting it.
#[frb(sync)]
pub fn parse_share_uri(line: String) -> UriParseResult {
    match subscriptions::resolve_uri(&line) {
        Ok(profile) => UriParseResult {
            ok: true,
            profile: Some(profile_dto(profile)),
            error: None,
        },
        Err(err) => UriParseResult {
            ok: false,
            profile: None,
            error: Some(error_dto(err.to_domain())),
        },
    }
}

// -- node export -----------------------------------------------------------

/// Render the selected profiles as share URIs / base64 / inner URIs.
///
/// Returns the text to place on the clipboard or in a file. `kind` is one of
/// `share`, `base64`, `inner`.
pub fn export_profiles(ids: Vec<String>, kind: String) -> ShareExportResult {
    let mut profiles = Vec::new();
    for id in &ids {
        match engine().profile_by_id(id) {
            Ok(Some(profile)) => profiles.push(profile),
            Ok(None) => {}
            Err(error) => {
                return ShareExportResult {
                    ok: false,
                    text: String::new(),
                    count: 0,
                    error: Some(error_dto(error)),
                };
            }
        }
    }
    if profiles.is_empty() {
        return ShareExportResult {
            ok: false,
            text: String::new(),
            count: 0,
            error: Some(ErrorDto {
                code: domain::codes::NOT_FOUND.to_string(),
                message_key: "error.no_profiles_selected".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: None,
            }),
        };
    }

    let rendered = match render_export(&profiles, kind.as_str()) {
        Ok(text) => text,
        Err(error) => {
            return ShareExportResult {
                ok: false,
                text: String::new(),
                count: 0,
                error: Some(error_dto(error)),
            };
        }
    };
    ShareExportResult {
        ok: true,
        text: rendered,
        count: profiles.len() as u32,
        error: None,
    }
}

/// Resolve an `Outbound` file-path `Address` to parsed JSON for inner export.
///
/// Tries the path as-is, then joined with the engine data dir (the local
/// equivalent of upstream `Utils.GetConfigPath`). Only local files are read;
/// a missing/unparseable file yields `None` and the node is skipped.
fn load_outbound_json(path: &str, data_dir: Option<&std::path::Path>) -> Option<serde_json::Value> {
    let mut candidates = vec![std::path::PathBuf::from(path)];
    if let Some(dir) = data_dir {
        candidates.push(dir.join(path));
    }
    candidates.into_iter().find_map(|candidate| {
        std::fs::read_to_string(candidate)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
    })
}

fn render_export(profiles: &[Profile], kind: &str) -> Result<String, DomainError> {
    match kind {
        "inner" => {
            // File-backed `Outbound` nodes resolve `Address` like upstream
            // `ToUriSingle` (direct path, then the data dir as the config-dir
            // equivalent). Inline-type nodes never reach the loader: the
            // subscriptions crate serves them from preserved inline content.
            let data_dir = engine().data_dir().map(|dir| dir.to_path_buf());
            let loader = |path: &str| load_outbound_json(path, data_dir.as_deref());
            subscriptions::to_inner_uri_with_outbound_loader(profiles, &loader).ok_or_else(|| {
                DomainError::new(domain::codes::INVALID_ARGUMENT, "error.export_nothing")
                    .with_detail("no exportable inner uri")
            })
        }
        "base64" => {
            let mut lines = Vec::new();
            for profile in profiles {
                match subscriptions::to_uri(profile) {
                    Ok(uri) => lines.push(uri),
                    Err(err) => return Err(err.to_domain()),
                }
            }
            let joined = lines.join("\n");
            Ok(subscriptions::util::base64_encode(&joined))
        }
        _ => {
            let mut lines = Vec::new();
            for profile in profiles {
                match subscriptions::to_uri(profile) {
                    Ok(uri) => lines.push(uri),
                    Err(err) => return Err(err.to_domain()),
                }
            }
            Ok(lines.join("\n"))
        }
    }
}

/// `write_export_file` — write export text to a file (URI text only; kernel
/// config export is deferred to T10+).
#[frb(sync)]
pub fn write_export_file(path: String, text: String) -> SimpleResult {
    match std::fs::write(&path, text) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(err) => SimpleResult {
            ok: false,
            error: Some(ErrorDto {
                code: domain::codes::INTERNAL.to_string(),
                message_key: "error.file_write_failed".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: Some(err.to_string()),
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_share_uri_assigns_ids_and_subid() {
        let _guard = crate::api::engine::engine_test_lock();
        let text =
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?encryption=none#one";
        let result = import_from_text(text.to_string(), Some("sub-x".into()), true);
        assert!(result.ok, "{:?}", result.error.map(|e| e.code));
        assert_eq!(result.imported, 1);
        let profile = &result.profiles[0];
        assert_eq!(profile.subid, "sub-x");
        assert!(!profile.index_id.is_empty());
    }

    #[test]
    fn save_empty_url_plain_group_round_trips() {
        // FIX-06 / SET-01: a real-bridge save of a remarks-only plain group
        // (empty URL) succeeds and reads back from persistence; the URL is
        // only validated when non-empty.
        let _guard = crate::api::engine::engine_test_lock();
        let result = save_sub_item(SubItemDto {
            id: String::new(),
            remarks: "普通分组".into(),
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
        });
        assert!(result.ok, "{:?}", result.error.map(|e| e.code));
        let saved = result.item.expect("saved item");
        assert!(saved.url.is_empty());
        let read_back = get_sub_item(saved.id.clone()).expect("read back");
        assert_eq!(read_back.remarks, "普通分组");
        assert!(read_back.url.is_empty());
        let _ = delete_sub_items(vec![saved.id]);
    }

    #[test]
    fn save_nonempty_invalid_url_is_rejected() {
        let _guard = crate::api::engine::engine_test_lock();
        let result = save_sub_item(SubItemDto {
            id: String::new(),
            remarks: "bad".into(),
            url: "ftp://example.com".into(),
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
        });
        assert!(!result.ok);
        let error = result.error.expect("field error");
        assert_eq!(error.code, domain::codes::FIELD_FORMAT);
        assert_eq!(error.field_path.as_deref(), Some("url"));
    }

    #[test]
    fn import_rejects_free_text() {
        let result = import_from_text("just words".to_string(), None, false);
        assert!(!result.ok);
        assert_eq!(result.imported, 0);
    }

    #[test]
    fn update_via_proxy_without_endpoint_returns_proxy_unavailable() {
        // Drive the future on a dedicated current-thread runtime while
        // holding the engine lock. `block_on` keeps the guard from crossing
        // an `.await` in this test body (clippy::await_holding_lock) while
        // still serializing against the other process-global engine tests.
        let _guard = crate::api::engine::engine_test_lock();
        engine().set_local_proxy_port(None);
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(update_subscriptions(Vec::new(), true));
        assert!(!result.ok);
        assert_eq!(result.success, 0);
        let error = result.error.expect("structured proxy error");
        assert_eq!(error.code, domain::codes::PROXY_UNAVAILABLE);
    }

    #[test]
    fn export_share_and_base64_agree() {
        // Exercise the pure renderer directly; the process-global engine is
        // shared with the engine tests and must not be reseeded here.
        let text =
            "vless://11111111-1111-1111-1111-111111111111@example.com:443?encryption=none#one";
        let parsed = subscriptions::resolve_uri(text).unwrap();
        let profiles = vec![parsed];
        let share = render_export(&profiles, "share").unwrap();
        assert!(share.starts_with("vless://"));
        let base64 = render_export(&profiles, "base64").unwrap();
        let decoded = subscriptions::util::base64_decode(&base64).unwrap();
        assert!(decoded.contains("vless://"));
    }

    #[test]
    fn import_inner_uri_parses_frozen_pascal_case_shape() {
        // No engine lock: subid-less imports never touch persistence.
        let payload = serde_json::json!({
            "IndexId": "bridge-vless",
            "ConfigType": 5,
            "ConfigVersion": 4,
            "Remarks": "桥接合成",
            "Address": "node.example.invalid",
            "Port": 11984,
            "Password": "11111111-2222-3333-4444-555555555555",
            "Network": "tcp",
            "ProtoExtraObj": {"VlessEncryption": "none"}
        });
        let uri = format!(
            "v2rayn://vless/{}",
            subscriptions::util::base64_urlsafe_nopad(payload.to_string().as_bytes())
        );
        let result = import_from_text(uri, None, false);
        assert!(result.ok, "{:?}", result.error.map(|e| e.code));
        assert_eq!(result.imported, 1);
        let profile = &result.profiles[0];
        assert_eq!(profile.config_type, domain::ConfigType::Vless);
        assert_eq!(profile.remarks, "桥接合成");
    }

    #[test]
    fn inner_export_serves_inline_and_file_outbound() {
        use domain::ConfigType;

        let mut inline = Profile {
            index_id: "inline-out".into(),
            config_type: ConfigType::Outbound,
            remarks: "内联出站".into(),
            ..Default::default()
        };
        inline.proto_extra.extra.insert(
            "customConfigText".into(),
            serde_json::Value::String(r#"{"tag":"inline-out"}"#.into()),
        );
        let text = render_export(&[inline], "inner").unwrap();
        assert!(text.starts_with("v2rayn://outbound/"));

        let dir = std::env::temp_dir();
        let file_name = format!("fix04-outbound-{}.json", std::process::id());
        let file_path = dir.join(&file_name);
        std::fs::write(&file_path, r#"{"tag":"file-out"}"#).unwrap();
        let file_node = Profile {
            index_id: "file-out".into(),
            config_type: ConfigType::Outbound,
            remarks: "文件出站".into(),
            address: file_path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let text = render_export(&[file_node], "inner").unwrap();
        assert!(text.starts_with("v2rayn://outbound/"));
        std::fs::remove_file(&file_path).ok();

        let missing = Profile {
            index_id: "missing-out".into(),
            config_type: ConfigType::Outbound,
            remarks: "缺文件出站".into(),
            address: "fix04-definitely-missing-outbound.json".into(),
            ..Default::default()
        };
        assert!(render_export(&[missing], "inner").is_err());
    }
}
