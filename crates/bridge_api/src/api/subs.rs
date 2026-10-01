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
/// Registry of live update jobs, so `cancel_job` can reach their token.
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
/// local session port (falling back per upstream when none is running). The
/// returned `job_id` can be passed to `cancel_job`.
pub async fn update_subscriptions(sub_ids: Vec<String>, via_proxy: bool) -> SubUpdateResult {
    let (job_id, token) = register_sub_job();
    let proxy_url = via_proxy.then(|| engine().local_proxy_url()).flatten();
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

fn render_export(profiles: &[Profile], kind: &str) -> Result<String, DomainError> {
    match kind {
        "inner" => subscriptions::to_inner_uri(profiles).ok_or_else(|| {
            DomainError::new(domain::codes::INVALID_ARGUMENT, "error.export_nothing")
                .with_detail("no exportable inner uri")
        }),
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
    fn import_rejects_free_text() {
        let result = import_from_text("just words".to_string(), None, false);
        assert!(!result.ok);
        assert_eq!(result.imported, 0);
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
}
