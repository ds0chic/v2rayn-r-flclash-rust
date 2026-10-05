//! T09 FRB functions: subscription CRUD, the update pipeline/job, the
//! scheduler and node import/export (share URIs, base64 lists, inner URIs).
//!
//! The dialog/screenshot evidence lives in `docs/evidence/T09-wiring.md`; the
//! pure Fmt/parse/download logic is the `subscriptions` crate (T09) and is not
//! re-implemented here.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use application::{SubItem, SubUpdateReport, SubUpdateRequest};
use domain::job::JobId;
use domain::{CancellationToken, ConfigType, DomainError, Profile};
use flutter_rust_bridge::frb;

use crate::api::contract::{
    DeleteSubsResult, ErrorDto, ImportResult, JobDto, ParseIssueDto, ShareExportResult,
    SimpleResult, SubItemDto, SubItemDtoResult, SubUpdateResult, SubsPageDto, UriParseResult,
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

/// Per-process registry of live subscription update jobs (diagnostics only).
/// Cancellation always goes through `JobManager::cancel` via the job id
/// returned to the caller; this registry is never read by `cancel_job`.
static SUB_JOBS: OnceLock<Mutex<Vec<JobId>>> = OnceLock::new();

fn sub_jobs() -> &'static Mutex<Vec<JobId>> {
    SUB_JOBS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Process-global terminal reports keyed by job id (SR-01).
///
/// Kept after the job finishes so the UI can resolve the same job id into its
/// true per-group outcome instead of inferring "all succeeded" from the job
/// state. `sub_update_report` exposes it once FRB bindings are regenerated.
static SUB_REPORTS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn sub_reports() -> &'static Mutex<HashMap<String, String>> {
    SUB_REPORTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Marker prefix for the report parked on the terminal job's stage key.
///
/// `job_view` already round-trips `stage_key`, so this carries the backend
/// per-group report to the UI for the matching job id without changing the
/// generated bridge bindings. The dedicated [`sub_update_report`] getter takes
/// over once FRB is regenerated.
pub const SUB_REPORT_STAGE_PREFIX: &str = "subs.report:";

pub(crate) fn remember_sub_report(job_id: &str, report_json: String) {
    if let Ok(mut reports) = sub_reports().lock() {
        reports.insert(job_id.to_string(), report_json);
    }
}

/// `sub_update_report` — the terminal per-group report JSON for `job_id`.
///
/// Returns `None` for an unknown/never-run id. Implemented here but absent
/// from the checked-in `frb_generated` bindings; the bridge currently reaches
/// the report through the job stage key. Regenerating FRB wires this getter
/// directly, replacing the stage-key carrier.
#[frb(sync)]
pub fn sub_update_report(job_id: String) -> Option<String> {
    sub_reports()
        .lock()
        .ok()
        .and_then(|reports| reports.get(&job_id).cloned())
}

/// A cancellation token plus the job id for a subscription update.
fn register_sub_job() -> (JobId, CancellationToken) {
    let job = engine().jobs().start("update_subscription");
    let token = engine().jobs().token(&job.job_id).unwrap_or_default();
    if let Ok(mut jobs) = sub_jobs().lock() {
        jobs.push(job.job_id.clone());
    }
    (job.job_id, token)
}

/// Run the refresh on a dedicated worker thread and report its completion.
///
/// The caller (`update_subscriptions`) has already handed the real job id back
/// to the UI, so the progress/cancel channel is bound to that id before any
/// network work begins.
fn spawn_sub_update(
    engine: application::AppEngine,
    request: SubUpdateRequest,
    token: CancellationToken,
    job_id: JobId,
) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let report = match runtime {
            // FIX-09B: resolve `ConstItem.SubConvertUrl` from settings and route
            // `ConvertTarget` subscriptions through the converter service.
            Ok(rt) => rt.block_on(application::subs::refresh_subscriptions_with_convert(
                &engine,
                request,
                &token,
                MAX_IMPORT_ITEMS,
            )),
            Err(err) => {
                let mut report = SubUpdateReport::default();
                report.entries.push(application::SubUpdateEntry {
                    sub_id: String::new(),
                    remarks: String::new(),
                    outcome: application::SubUpdateOutcome::Failed {
                        code: domain::codes::INTERNAL.to_string(),
                        message: err.to_string(),
                    },
                });
                report
            }
        };
        let mut payload = application::report_to_json(&report);
        if let Some(map) = payload.as_object_mut() {
            map.insert(
                "job_id".to_string(),
                serde_json::Value::String(job_id.0.clone()),
            );
        }
        // SR-01: cache the full report and park it on the job's stage key so
        // the UI can resolve the same job id into the real per-group outcome.
        let report_json = payload.to_string();
        remember_sub_report(&job_id.0, report_json.clone());
        let _ = engine.jobs().progress(
            &job_id,
            None,
            Some(format!("{SUB_REPORT_STAGE_PREFIX}{report_json}")),
        );
        finish_sub_job(&job_id, &report);
        emit_control("subscriptions_updated", payload);
    });
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

/// `update_subscriptions` — the F-SUB-003 pipeline.
///
/// `sub_ids` empty means every subscription; `via_proxy` uses the recorded
/// local session port. When `via_proxy` is requested but no local proxy
/// endpoint is known, a structured `E_PROXY_UNAVAILABLE` error is returned
/// and no direct download is attempted.
///
/// On acceptance the real `job_id` is returned *before* any network work
/// starts (SET-03): the refresh runs on a worker thread, the UI binds its
/// progress (`job_view`) and cancellation (`cancel_job`) to that id, and the
/// final report is pushed as a `subscriptions_updated` control event. A
/// cancellation never replaces the old group, reports a fake success or
/// leaves a half-written candidate set (the pipeline is candidate-first).
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
    let _ = engine()
        .jobs()
        .progress(&job_id, None, Some("subs.downloading".to_string()));
    spawn_sub_update(engine().clone(), request, token, job_id.clone());
    // `ok` means "accepted"; the terminal state arrives through the job and
    // the `subscriptions_updated` event. `success = 0`/empty entries must
    // never be read as a completed run.
    SubUpdateResult {
        ok: true,
        success: 0,
        cancelled: false,
        entries: Vec::new(),
        job_id: Some(job_id.0),
        error: None,
    }
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

/// FIX-04B: materialize complete-config imports (v2ray/sing-box JSON, Clash
/// YAML) into the RT-08 file-type form.
///
/// The parser parks the verbatim payload under `extra["RawConfig"]`; here it is
/// written under `<data>/config/<index_id><ext>` and `Address` is pointed at
/// the stored file name, exactly like upstream `WriteAllText` +
/// `Address = fileName`. Inline payloads (`customConfigText`, FIX-04 inner
/// `CustomOutboundObj`) are left untouched, so the file-type and inline-type
/// forms never impersonate each other.
///
/// With no data directory (in-memory test engine) the payload is written to the
/// system temp dir and the absolute path is stored instead of the bare name.
fn materialize_custom_configs(profiles: &mut [Profile]) {
    let data_dir = engine().data_dir().map(|dir| dir.to_path_buf());
    for profile in profiles.iter_mut() {
        if !matches!(
            profile.config_type,
            ConfigType::Custom | ConfigType::Outbound
        ) {
            continue;
        }
        let Some(raw) = subscriptions::take_raw_config(profile) else {
            continue;
        };
        let ext = subscriptions::detect_config_extension(&raw);
        // Name by content, not by the freshly minted `IndexId`: the
        // preview+commit pipeline parses the same payload twice (once to
        // preview, once to commit), and a content-addressed file is reused
        // instead of leaving an orphan behind. Identical configs share a file
        // (their content is byte-for-byte the same).
        let name = format!("import-{}{}", content_stem(&raw), ext);
        let (dir, stored) = match &data_dir {
            Some(base) => (base.join("config"), name.clone()),
            None => {
                let dir = std::env::temp_dir();
                let absolute = dir.join(&name).to_string_lossy().into_owned();
                (dir, absolute)
            }
        };
        let written =
            std::fs::create_dir_all(&dir).is_ok() && std::fs::write(dir.join(&name), &raw).is_ok();
        if written {
            profile.address = stored;
        } else {
            // Never drop the payload silently; keep it for a later retry.
            profile.extra.insert(
                subscriptions::fmt::batch::RAW_CONFIG_KEY.to_string(),
                serde_json::Value::String(raw),
            );
        }
    }
}

/// Stable FNV-1a content digest used to name materialized config files.
fn content_stem(raw: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in raw.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Result of the shared import decode: profiles plus located per-line issues.
struct ParsedImport {
    profiles: Vec<Profile>,
    errors: Vec<subscriptions::ParseIssue>,
    deduped: usize,
}

fn parse_issue_dto(e: &subscriptions::ParseIssue) -> ParseIssueDto {
    ParseIssueDto {
        code: e.code.clone(),
        message: e.message.clone(),
        item_index: e.item_index.map(|i| i as u32),
        byte_offset: e.byte_offset.map(|o| o as u64),
    }
}

fn parse_error_result(errors: &[subscriptions::ParseIssue]) -> ImportResult {
    ImportResult {
        ok: false,
        imported: 0,
        profiles: Vec::new(),
        errors: errors.iter().map(parse_issue_dto).collect(),
        error: Some(ErrorDto {
            code: domain::codes::FIELD_FORMAT.to_string(),
            message_key: "error.import_nothing".to_string(),
            field_path: None,
            retryable: false,
            operation_id: None,
            detail: None,
        }),
    }
}

/// Shared import decode used by both the preview and the commit entry points.
///
/// `materialize` writes complete-config payloads to disk; the preview phase
/// passes `false` so a proactive preview never leaves files behind.
fn parse_import(
    text: &str,
    subid: Option<&str>,
    deduplicate: bool,
    materialize: bool,
) -> ParsedImport {
    let parsed = subscriptions::parse_content(
        text,
        subscriptions::ContentHint::Auto,
        &subscriptions::ParseOptions {
            subid: subid.unwrap_or_default().to_string(),
            max_items: MAX_IMPORT_ITEMS,
            ..subscriptions::ParseOptions::default()
        },
    );
    let mut profiles = parsed.profiles;
    let mut deduped = 0usize;
    if deduplicate {
        let (kept, removed) = subscriptions::deduplicate(&profiles, true);
        profiles = kept;
        deduped = removed;
    }
    for profile in &mut profiles {
        if profile.subid.is_empty() {
            profile.subid = subid.unwrap_or_default().to_string();
        }
        if profile.index_id.trim().is_empty() {
            profile.index_id = application::new_index_id();
        }
        // Manual batch import (node page paste/scan, upstream
        // `AddBatchServers(..., isSub: false)` at `MainWindowViewModel.cs:484-502`)
        // is not subscription-sourced: the node may live in a subscription
        // group but a subscription update only replaces `IsSub = 1` rows, so
        // this flag is what keeps the imported node alive (R4-17 / D04).
        // Real subscription content goes through the refresh pipeline
        // (`refresh_subscriptions_with_convert`), which marks candidates true.
        profile.is_sub = false;
    }
    if materialize {
        materialize_custom_configs(&mut profiles);
    }
    ParsedImport {
        profiles,
        errors: parsed.errors,
        deduped,
    }
}

/// `preview_import_text` — parse/preview without persisting (R4-16).
///
/// Decodes and normalises the payload but never touches SQLite and never writes
/// config files; the caller previews the result and then commits through
/// [`import_from_text`] (or a future single-transaction `commit_import_text`).
/// The binding is pending the next FRB regeneration.
#[frb(sync)]
pub fn preview_import_text(text: String, subid: Option<String>) -> ImportResult {
    let parsed = parse_import(&text, subid.as_deref(), false, false);
    if parsed.profiles.is_empty() {
        return parse_error_result(&parsed.errors);
    }
    ImportResult {
        ok: true,
        imported: parsed.profiles.len() as u32,
        profiles: parsed.profiles.into_iter().map(profile_dto).collect(),
        errors: parsed.errors.iter().map(parse_issue_dto).collect(),
        error: None,
    }
}

/// `import_from_text` — parse share URIs / base64 / inner URIs, then commit.
///
/// `subid` non-empty attaches the imported nodes to that subscription and
/// inserts them in one `replace_sub_profiles(..., remove_existing: false)`
/// transaction; `deduplicate` applies the upstream `KeepOlderDedupl` collapse.
/// Returns the imported profiles plus located per-line errors
/// (F-IMPORT-001/002/005).
pub fn import_from_text(text: String, subid: Option<String>, deduplicate: bool) -> ImportResult {
    let parsed = parse_import(&text, subid.as_deref(), deduplicate, true);
    if parsed.profiles.is_empty() {
        return parse_error_result(&parsed.errors);
    }
    let profiles = parsed.profiles;
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
            serde_json::json!({ "sub_id": sub, "imported": count, "deduplicated": parsed.deduped }),
        );
    }
    ImportResult {
        ok: true,
        imported: count,
        profiles: profiles.into_iter().map(profile_dto).collect(),
        errors: parsed.errors.iter().map(parse_issue_dto).collect(),
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
    use domain::CoreType;

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
        // R4-17 / D04: a manual batch import is `IsSub = false`, so an update
        // of the group it was pasted into does not delete it.
        assert!(!profile.is_sub);
    }

    #[test]
    fn preview_import_does_not_persist() {
        // R4-16: the preview phase must never touch SQLite, even when a group
        // is supplied, so it can safely run before the user confirms.
        let _guard = crate::api::engine::engine_test_lock();
        let sub = "sub-r416-preview";
        let before = engine().profiles_by_subid(sub).unwrap_or_default().len();
        let text = "vless://11111111-1111-1111-1111-111111111111@preview.example:443?encryption=none#preview";
        let result = preview_import_text(text.to_string(), Some(sub.into()));
        assert!(result.ok, "{:?}", result.error.map(|e| e.code));
        assert_eq!(result.imported, 1);
        let after = engine().profiles_by_subid(sub).unwrap_or_default().len();
        assert_eq!(after, before, "preview must not persist profiles");
        let _ = engine().delete_sub_items(&[sub.to_string()]);
    }

    #[test]
    fn manual_import_keeps_duplicates_without_dedup() {
        // Upstream `AddBatchServersCommon` applies `Distinct()` only when
        // `isSub`; a manual paste/scan (`isSub: false`) keeps duplicates.
        let text = "vless://11111111-1111-1111-1111-111111111111@dup.example:443?encryption=none#dup\n\
                    vless://11111111-1111-1111-1111-111111111111@dup.example:443?encryption=none#dup";
        let manual = import_from_text(text.to_string(), None, false);
        assert!(manual.ok);
        assert_eq!(manual.imported, 2, "manual batch import keeps duplicates");
        let collapsed = import_from_text(text.to_string(), None, true);
        assert_eq!(collapsed.imported, 1, "explicit dedup collapses duplicates");
    }

    #[test]
    fn commit_import_persists_once_and_is_not_sub() {
        // R4-16: with a group, `import_from_text` is the single batch commit;
        // every row is inserted once, bound to the group and flagged
        // `IsSub = false` so a later subscription refresh does not delete it.
        let _guard = crate::api::engine::engine_test_lock();
        let sub = "sub-r416-commit";
        let _ = engine().delete_sub_items(&[sub.to_string()]);
        let text = "vless://11111111-2222-3333-4444-555555555555@commit.example:443?encryption=none#commit";
        let result = import_from_text(text.to_string(), Some(sub.into()), false);
        assert!(result.ok, "{:?}", result.error.map(|e| e.code));
        assert_eq!(result.imported, 1);
        assert!(!result.profiles[0].is_sub);
        let stored = engine().profiles_by_subid(sub).unwrap_or_default();
        let matching: Vec<_> = stored.iter().filter(|p| p.remarks == "commit").collect();
        assert_eq!(matching.len(), 1, "exactly one row inserted");
        assert!(!matching[0].is_sub);
        let _ = engine().delete_sub_items(&[sub.to_string()]);
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

    fn synthetic_full_v2ray() -> String {
        r#"{"inbounds":[{"port":11888,"protocol":"socks","settings":{"udp":true}}],"outbounds":[{"protocol":"vmess","tag":"fix04b-marker","settings":{"vnext":[{"address":"node.example.invalid","port":11980,"users":[{"id":"11111111-2222-3333-4444-555555555555"}]}]},"streamSettings":{"network":"tcp"}}],"x-future":[1,2,3]}"#.to_string()
    }

    fn synthetic_full_singbox() -> String {
        r#"{"inbounds":[],"outbounds":[{"type":"vless","tag":"fix04b-sbox","server":"node.example.invalid","server_port":11981,"uuid":"11111111-2222-3333-4444-555555555555"}],"x-future":"kept"}"#.to_string()
    }

    fn materialized_path(dto: &crate::api::contract::ProfileDto) -> std::path::PathBuf {
        if std::path::Path::new(&dto.address).is_absolute() {
            std::path::PathBuf::from(&dto.address)
        } else {
            engine()
                .data_dir()
                .expect("data dir")
                .join("config")
                .join(&dto.address)
        }
    }

    #[test]
    fn full_v2ray_config_imports_as_file_type_and_generates() {
        let _guard = crate::api::engine::engine_test_lock();
        let result = import_from_text(synthetic_full_v2ray(), None, false);
        assert!(
            result.ok,
            "{:?}",
            result.error.as_ref().map(|e| e.code.clone())
        );
        assert_eq!(result.imported, 1);
        let dto = result.profiles.into_iter().next().expect("one profile");
        assert_eq!(dto.config_type, ConfigType::Custom);
        assert_eq!(dto.core_type, Some(CoreType::Xray));
        assert!(
            !dto.address.trim().is_empty(),
            "file-type Address must be set"
        );
        assert!(!dto.proto_extra.extra_json.contains("customConfigText"));
        assert!(!dto.extra_json.contains("RawConfig"));

        let text = std::fs::read_to_string(materialized_path(&dto)).expect("materialized file");
        assert!(text.contains("fix04b-marker"));
        assert!(text.contains("x-future"), "unknown keys must survive");

        let saved = crate::api::engine::save_imported_profile(
            dto.clone(),
            crate::api::engine::profile_revision(),
        );
        assert!(
            saved.ok,
            "{:?}",
            saved.error.as_ref().map(|e| e.code.clone())
        );
        let saved = saved.profile.expect("saved profile");
        let read_back = crate::api::engine::get_profile(saved.index_id.clone()).expect("read back");
        assert_eq!(
            read_back.address, dto.address,
            "reopen keeps the file Address"
        );

        let input = engine()
            .build_codegen_input(
                &saved.index_id,
                CoreType::Xray,
                &application::codegen::CodegenOptions::default(),
            )
            .expect("codegen input");
        let generated = application::codegen::generate(CoreType::Xray, &input).expect("generate");
        let out = serde_json::to_string(&generated.main).unwrap();
        assert!(out.contains("fix04b-marker"), "{out}");
        assert!(out.contains("x-future"), "{out}");
    }

    #[test]
    fn full_singbox_config_imports_as_file_type() {
        let _guard = crate::api::engine::engine_test_lock();
        let result = import_from_text(synthetic_full_singbox(), None, false);
        assert!(
            result.ok,
            "{:?}",
            result.error.as_ref().map(|e| e.code.clone())
        );
        let dto = result.profiles.into_iter().next().expect("one profile");
        assert_eq!(dto.config_type, ConfigType::Custom);
        assert_eq!(dto.core_type, Some(CoreType::SingBox));
        let text = std::fs::read_to_string(materialized_path(&dto)).expect("file");
        assert!(text.contains("fix04b-sbox"));
        assert!(text.contains("x-future"));
    }

    #[test]
    fn clash_yaml_imports_as_file_type_yaml() {
        let _guard = crate::api::engine::engine_test_lock();
        let yaml = "proxies:\n  - name: fix04b-clash\n    type: ss\n    server: node.example.invalid\n    port: 11982\nrules:\n  - MATCH,DIRECT\nmixed-port: 7890\nx-future: kept\n";
        let result = import_from_text(yaml.to_string(), None, false);
        assert!(
            result.ok,
            "{:?}",
            result.error.as_ref().map(|e| e.code.clone())
        );
        let dto = result.profiles.into_iter().next().expect("one profile");
        assert_eq!(dto.config_type, ConfigType::Custom);
        assert_eq!(dto.core_type, Some(CoreType::Mihomo));
        assert!(dto.address.ends_with(".yaml"), "{}", dto.address);
        let text = std::fs::read_to_string(materialized_path(&dto)).expect("file");
        assert!(text.contains("x-future: kept"));
    }

    #[test]
    fn file_type_and_inline_type_do_not_impersonate() {
        let file_result = import_from_text(synthetic_full_v2ray(), None, false);
        let file_dto = file_result.profiles.into_iter().next().expect("file node");
        assert!(!file_dto.address.is_empty());
        assert!(!file_dto.proto_extra.extra_json.contains("customConfigText"));

        let payload = serde_json::json!({
            "IndexId": "fix04b-inline",
            "ConfigType": ConfigType::Outbound.value(),
            "CoreType": CoreType::Xray.value(),
            "ConfigVersion": 4,
            "Remarks": "内联出站",
            "CustomOutboundObj": {"protocol": "freedom", "tag": "inline"}
        });
        let uri = format!(
            "v2rayn://outbound/{}",
            subscriptions::util::base64_urlsafe_nopad(payload.to_string().as_bytes())
        );
        let inline_result = import_from_text(uri, None, false);
        assert!(
            inline_result.ok,
            "{:?}",
            inline_result.error.as_ref().map(|e| e.code.clone())
        );
        let inline_dto = inline_result
            .profiles
            .into_iter()
            .next()
            .expect("inline node");
        assert_eq!(inline_dto.config_type, ConfigType::Outbound);
        assert!(
            inline_dto.address.is_empty(),
            "inline form keeps Address empty"
        );
        assert!(inline_dto
            .proto_extra
            .extra_json
            .contains("customConfigText"));
    }

    #[test]
    fn sub_update_report_round_trips_and_unknown_is_none() {
        // SR-01: the terminal report is retrievable by job id and survives
        // after the job is done (the getter is not yet in frb_generated).
        assert!(sub_update_report("job-never-ran".into()).is_none());
        remember_sub_report("job-sr01", r#"{"success":1,"entries":[]}"#.into());
        assert_eq!(
            sub_update_report("job-sr01".into()).as_deref(),
            Some(r#"{"success":1,"entries":[]}"#)
        );
    }
}
