//! T16 FRB functions: backup/restore, upstream ZIP import, WebDAV and the
//! built-in core/application update pipeline.
//!
//! Network calls only ever hit the endpoint the caller configured; the update
//! path refuses to run when "via proxy" is requested but no local session port
//! is known (it never silently falls back to a direct connection). No function
//! here replaces a live installation outside the managed cores directory.

use std::path::{Path, PathBuf};
use std::time::Duration;

use application::{
    AppEngine, BackupManifest, BackupService, ImportStatus, WebDavClient, WebDavConfig,
};
use domain::{codes, CancellationToken, DomainError};
use flutter_rust_bridge::frb;

use crate::api::contract::{
    AppRestartCommandDto, AppRollbackResultDto, AppliedCoreDto, ApplyCoreResultDto, BackupListDto,
    BackupManifestDto, BackupResultDto, CleanupResultDto, CoreUpdateDto, CoreVersionsDto,
    EntityCountDto, ErrorDto, ExternalSpecDto, ImportSummaryDto, InstalledCoreDto, RecognitionDto,
    RestoreResultDto, SimpleResult, UpdateReportDto, UpdateTargetDto, VerificationDto,
    WebDavCheckDto, WebDavConfigDto, WebDavConfigResultDto, WebDavEntryDto, WebDavListDto,
    WebDavOpDto,
};
use crate::api::engine::{engine, error_dto};

// Per-operation update flags (SP-27).
//
// Every T16 check/apply entry takes its `prerelease` / `via_proxy` selection
// as explicit arguments, so one operation never inherits another operation's
// flags. The FRB bindings for `t16_apply_app_update_spec` take no arguments,
// so the no-arg entry below always stages with the safe defaults
// (`prerelease=false`, direct connection); the explicit per-operation entry
// point is `t16_apply_app_update_spec_with_flags`. Verification of the
// application package is never skippable on either path.

fn work_dir(kind: &str) -> Result<PathBuf, DomainError> {
    let base = engine()
        .data_dir()
        .ok_or_else(|| DomainError::new(codes::UNAVAILABLE, "error.engine_not_persistent"))?
        .join(".work")
        .join(kind);
    if base.exists() {
        std::fs::remove_dir_all(&base).map_err(|e| {
            DomainError::new(codes::INTERNAL, "error.backup_workdir").with_detail(e.to_string())
        })?;
    }
    std::fs::create_dir_all(&base).map_err(|e| {
        DomainError::new(codes::INTERNAL, "error.backup_workdir").with_detail(e.to_string())
    })?;
    Ok(base)
}

fn backup_service() -> Result<BackupService, DomainError> {
    let dir = engine()
        .data_dir()
        .ok_or_else(|| DomainError::new(codes::UNAVAILABLE, "error.engine_not_persistent"))?;
    Ok(BackupService::new(dir))
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn manifest_dto(manifest: &BackupManifest, root: Option<&Path>) -> BackupManifestDto {
    BackupManifestDto {
        format_version: manifest.format_version,
        created_at: manifest.created_at,
        app_source_commit: manifest.app_source_commit.clone(),
        db_sha256: manifest.db_sha256.clone(),
        config_sha256: manifest.config_sha256.clone(),
        root: root.map(|p| p.to_string_lossy().into_owned()),
        resource_count: manifest.referenced_resources.len() as u32,
        entity_counts: manifest
            .entity_counts
            .iter()
            .map(|count| EntityCountDto {
                table: count.table.clone(),
                source_rows: count.source_rows,
                imported_rows: count.imported_rows,
                migrated_rows: count.migrated_rows,
                skipped_rows: count.skipped_rows,
            })
            .collect(),
    }
}

/// `backup_local` — write a versioned bundle under `dest_root`. Async: copies
/// files and hashes the database, so it must not run on the UI isolate.
pub fn t16_backup_local(dest_root: String) -> BackupResultDto {
    match backup_service()
        .and_then(|service| service.create_local(Path::new(&dest_root), now_epoch()))
    {
        Ok(backup) => BackupResultDto {
            ok: true,
            root: Some(backup.root.to_string_lossy().into_owned()),
            manifest: Some(manifest_dto(&backup.manifest, Some(&backup.root))),
            error: None,
        },
        Err(error) => BackupResultDto {
            ok: false,
            root: None,
            manifest: None,
            error: Some(error_dto(error)),
        },
    }
}

/// `backup_list` — bundles directly under `parent`. Async: directory scan.
pub fn t16_backup_list(parent: String) -> BackupListDto {
    match BackupService::list(Path::new(&parent)) {
        Ok(items) => BackupListDto {
            items: items
                .iter()
                .map(|(root, m)| manifest_dto(m, Some(root)))
                .collect(),
            error: None,
        },
        Err(error) => BackupListDto {
            items: Vec::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `backup_verify` — verify a bundle without writing anything. Async: reads the
/// whole bundle and re-hashes it.
pub fn t16_backup_verify(bundle_dir: String) -> VerificationDto {
    let service = match backup_service() {
        Ok(service) => service,
        Err(error) => {
            return VerificationDto {
                ok: false,
                missing: Vec::new(),
                mismatched: vec![error.message_key],
            }
        }
    };
    match service.verify(Path::new(&bundle_dir)) {
        Ok(result) => VerificationDto {
            ok: result.ok,
            missing: result.missing,
            mismatched: result.mismatched,
        },
        Err(error) => VerificationDto {
            ok: false,
            missing: Vec::new(),
            mismatched: vec![error.detail.unwrap_or(error.message_key)],
        },
    }
}

/// `backup_restore` — verify then restore a bundle. Async: DB lifecycle swap +
/// file I/O.
pub fn t16_backup_restore(bundle_dir: String) -> RestoreResultDto {
    // SR-03: stop the managed session/timer, quiesce, exchange and reopen in
    // one lifecycle. A failed quiesce/reopen is a structured error; it is never
    // dropped, so the UI cannot be told a swap succeeded when the engine is
    // still bound to the old (or no) storage.
    let outcome = backup_service().and_then(|service| {
        let work = work_dir("restore")?;
        service.restore_with_lifecycle(engine(), Path::new(&bundle_dir), &work)
    });
    match outcome {
        Ok(report) => {
            if report.restored {
                super::speedtest::reload_profile_ex_after_restore();
            }
            RestoreResultDto {
                ok: report.restored,
                restored: report.restored,
                target_backup: report.target_backup,
                message: report.message,
                error: None,
            }
        }
        Err(error) => restore_failed(error),
    }
}

fn restore_failed(error: DomainError) -> RestoreResultDto {
    RestoreResultDto {
        ok: false,
        restored: false,
        target_backup: None,
        message: String::new(),
        error: Some(error_dto(error)),
    }
}

/// `backup_recognize` — recognise an upstream or project archive. Async: reads
/// the archive directory/central directory.
pub fn t16_backup_recognize(path: String) -> RecognitionDto {
    match backup_service().and_then(|service| service.recognize(Path::new(&path))) {
        Ok(result) => RecognitionDto {
            is_upstream: result.is_upstream,
            has_config: result.has_config,
            has_db: result.has_db,
            layout: result.layout,
            entries: result.entries,
            error: None,
        },
        Err(error) => RecognitionDto {
            is_upstream: false,
            has_config: false,
            has_db: false,
            layout: String::new(),
            entries: Vec::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `backup_import_upstream` — restore a directory/ZIP via the T04 candidate
/// flow with replace semantics (the upstream `guiNDB.db` becomes the database).
/// Async: DB lifecycle swap + file I/O.
pub fn t16_backup_import_upstream(path: String) -> ImportSummaryDto {
    import_upstream(&path, false)
}

/// `backup_import_upstream_merge` — the explicitly named migration/merge
/// import that appends the source rows to the live database (R3-SET-02).
///
/// Registered as an independent entry; the checked-in `frb_generated` bindings
/// are not regenerated, so the UI restore path stays on
/// [`t16_backup_import_upstream`] (replace).
#[frb(sync)]
pub fn t16_backup_import_upstream_merge(path: String) -> ImportSummaryDto {
    import_upstream(&path, true)
}

fn import_upstream(path: &str, merge: bool) -> ImportSummaryDto {
    let kind = if merge { "import-merge" } else { "import" };
    let prepared = backup_service().and_then(|service| work_dir(kind).map(|work| (service, work)));
    let (service, work) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return import_failed(error, 0),
    };
    import_upstream_inner(
        &service,
        engine(),
        Path::new(path),
        &work,
        merge,
        now_epoch(),
    )
}

// S8-02 seam: the import body takes its service/engine/work explicitly so
// tests can drive it against an isolated engine without touching the FRB
// entry points above (whose signatures are frozen).
fn import_upstream_inner(
    service: &BackupService,
    engine: &AppEngine,
    path: &Path,
    work: &Path,
    merge: bool,
    now: i64,
) -> ImportSummaryDto {
    // SR-03: same lifecycle as the local restore; quiesce/reopen failures are
    // propagated as structured errors instead of being ignored.
    let outcome = if merge {
        service.import_upstream_merge_with_lifecycle(engine, path, work, now)
    } else {
        service.import_upstream_with_lifecycle(engine, path, work, now)
    };
    match outcome {
        Ok(report) => {
            // S8-02: same overlay reset as the local/WebDAV restore paths. A
            // changed target invalidates every cached delay/speed row; without
            // this a stale entry survives and a later flush writes it back
            // over the freshly imported ProfileEx.
            if report.status.changed_target() {
                super::speedtest::reload_profile_ex_after_restore();
            }
            let imported_rows = report.counts.iter().map(|c| c.imported_rows).sum::<u64>();
            let status = match report.status {
                ImportStatus::Imported => "imported",
                ImportStatus::AlreadyImported => "already_imported",
                ImportStatus::Rejected => "rejected",
                ImportStatus::Failed => "failed",
            };
            ImportSummaryDto {
                ok: report.status.changed_target()
                    || report.status == ImportStatus::AlreadyImported,
                status: status.to_string(),
                source_version: report.source_version,
                imported_rows,
                warnings: report.warnings.len() as u32,
                errors: report.errors.len() as u32,
                message: report.user_summary,
                error: None,
            }
        }
        Err(error) => import_failed(error, 1),
    }
}

/// `errors` is 0 when the import never started and 1 when it ran and failed.
fn import_failed(error: DomainError, errors: u32) -> ImportSummaryDto {
    ImportSummaryDto {
        ok: false,
        status: "failed".to_string(),
        source_version: 0,
        imported_rows: 0,
        warnings: 0,
        errors,
        message: String::new(),
        error: Some(error_dto(error)),
    }
}

// -- WebDAV -----------------------------------------------------------------

fn dto_to_webdav(cfg: WebDavConfigDto) -> WebDavConfig {
    WebDavConfig::new(cfg.url, cfg.user_name, cfg.password, cfg.dir_name)
}

fn webdav_dto(cfg: &domain::WebDavItem) -> WebDavConfigDto {
    WebDavConfigDto {
        url: cfg.url.clone().unwrap_or_default(),
        user_name: cfg.user_name.clone().unwrap_or_default(),
        password: cfg.password.clone().unwrap_or_default(),
        dir_name: cfg.dir_name.clone().unwrap_or_default(),
    }
}

/// `webdav_config_get` — the persisted `WebDavItem` (secret not logged).
#[frb(sync)]
pub fn t16_webdav_config_get() -> WebDavConfigResultDto {
    match engine().load_settings() {
        Ok(loaded) => WebDavConfigResultDto {
            ok: true,
            config: Some(webdav_dto(&loaded.settings.web_dav_item)),
            revision: loaded
                .group_revisions
                .get("WebDavItem")
                .copied()
                .unwrap_or(0),
            error: None,
        },
        Err(error) => WebDavConfigResultDto {
            ok: false,
            config: None,
            revision: 0,
            error: Some(error_dto(error)),
        },
    }
}

/// `webdav_config_save` — persist the `WebDavItem` group.
#[frb(sync)]
pub fn t16_webdav_config_save(
    cfg: WebDavConfigDto,
    expected_revision: u64,
) -> WebDavConfigResultDto {
    // Bridge save-time gate: reuse the application-level `validated_dir()`
    // check so an illegal remote directory is rejected with the existing
    // FIELD_FORMAT / error.webdav_url_invalid contract before anything is
    // persisted. No new keys.
    let validated = dto_to_webdav(cfg);
    if let Err(error) = validated.validated_dir() {
        return WebDavConfigResultDto {
            ok: false,
            config: None,
            revision: expected_revision,
            error: Some(error_dto(error)),
        };
    }
    let patch = serde_json::json!({
        "Url": validated.url,
        "UserName": validated.user_name,
        "Password": validated.password,
        "DirName": validated.dir_name,
    });
    match engine().save_settings_group("WebDavItem", patch, expected_revision) {
        Ok(_) => t16_webdav_config_get(),
        Err(error) => WebDavConfigResultDto {
            ok: false,
            config: None,
            revision: expected_revision,
            error: Some(error_dto(error)),
        },
    }
}

fn webdav_client(cfg: WebDavConfigDto) -> Result<WebDavClient, DomainError> {
    application::WebDavClient::new(dto_to_webdav(cfg), std::time::Duration::from_secs(30), None)
}

/// `webdav_check` — connect, creating the default directory when missing.
pub async fn t16_webdav_check(cfg: WebDavConfigDto) -> WebDavCheckDto {
    let client = match webdav_client(cfg) {
        Ok(client) => client,
        Err(error) => {
            return WebDavCheckDto {
                ok: false,
                created_dir: false,
                status: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    match client.check().await {
        Ok(result) => WebDavCheckDto {
            ok: true,
            created_dir: result.created_dir,
            status: result.status,
            message: if result.created_dir {
                "error.webdav_dir_created".to_string()
            } else {
                "error.webdav_ok".to_string()
            },
            error: None,
        },
        Err(error) => WebDavCheckDto {
            ok: false,
            created_dir: false,
            status: 0,
            message: String::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `webdav_list` — list the remote backup directory.
pub async fn t16_webdav_list(cfg: WebDavConfigDto) -> WebDavListDto {
    let client = match webdav_client(cfg) {
        Ok(client) => client,
        Err(error) => {
            return WebDavListDto {
                ok: false,
                items: Vec::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    match client.list().await {
        Ok(entries) => WebDavListDto {
            ok: true,
            items: entries
                .into_iter()
                .map(|entry| WebDavEntryDto {
                    href: entry.href,
                    is_dir: entry.is_dir,
                    size: entry.size,
                    modified: entry.modified,
                })
                .collect(),
            error: None,
        },
        Err(error) => WebDavListDto {
            ok: false,
            items: Vec::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `webdav_backup` — pack a local bundle and PUT it as `backup.zip`.
pub async fn t16_webdav_backup(cfg: WebDavConfigDto) -> WebDavOpDto {
    let service = match backup_service() {
        Ok(service) => service,
        Err(error) => {
            return WebDavOpDto {
                ok: false,
                bytes: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let work = match work_dir("webdav-backup") {
        Ok(path) => path,
        Err(error) => {
            return WebDavOpDto {
                ok: false,
                bytes: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let bundle_dir = work.join("bundle");
    let backup = match service.create_local(&bundle_dir, now_epoch()) {
        Ok(backup) => backup,
        Err(error) => {
            return WebDavOpDto {
                ok: false,
                bytes: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    // The remote file must be an upstream-interoperable `guiConfigs/` ZIP, not
    // this project's manifest bundle, so a v2rayN client can restore it.
    let zip_path = work.join("backup.zip");
    let length = match application::zip_upstream_layout_to_file(&backup.root, &zip_path) {
        Ok(length) => length,
        Err(error) => {
            return WebDavOpDto {
                ok: false,
                bytes: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let client = match webdav_client(cfg) {
        Ok(client) => client,
        Err(error) => {
            return WebDavOpDto {
                ok: false,
                bytes: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    // Upstream ensures the collection exists (MKCOL when missing) before PUT.
    if let Err(error) = client.check().await {
        return WebDavOpDto {
            ok: false,
            bytes: 0,
            message: String::new(),
            error: Some(error_dto(error)),
        };
    }
    match client.upload_file(&zip_path, length).await {
        Ok(bytes) => WebDavOpDto {
            ok: true,
            bytes,
            message: format!("uploaded {length} bytes"),
            error: None,
        },
        Err(error) => WebDavOpDto {
            ok: false,
            bytes: 0,
            message: String::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `webdav_restore` — GET `backup.zip`, extract and restore it.
pub async fn t16_webdav_restore(cfg: WebDavConfigDto) -> RestoreResultDto {
    webdav_restore(cfg).await.unwrap_or_else(restore_failed)
}

async fn webdav_restore(cfg: WebDavConfigDto) -> Result<RestoreResultDto, DomainError> {
    let service = backup_service()?;
    let work = work_dir("webdav-restore")?;
    let client = webdav_client(cfg)?;
    let zip_path = work.join("remote-backup.zip");
    client.download_to_file(&zip_path).await?;
    // An upstream `guiConfigs/` archive (the layout this project now uploads)
    // goes through the same candidate + activation flow as a local FIX-14
    // import: quiesce -> swap/activate -> reopen. A project manifest bundle
    // keeps the project restore path. Both reclose the live handles.
    let upstream = service
        .recognize(&zip_path)
        .map(|recognition| recognition.is_upstream)
        .unwrap_or(false);
    if upstream {
        // SR-03: lifecycle-aware import; quiesce/reopen failures surface.
        let report =
            service.import_upstream_with_lifecycle(engine(), &zip_path, &work, now_epoch())?;
        let changed =
            report.status.changed_target() || report.status == ImportStatus::AlreadyImported;
        if changed {
            super::speedtest::reload_profile_ex_after_restore();
        }
        return Ok(RestoreResultDto {
            ok: changed,
            restored: changed,
            target_backup: None,
            message: report.user_summary,
            error: None,
        });
    }
    let unpacked = work.join("unpacked");
    application::extract_bundle_zip(&zip_path, &unpacked)?;
    // SR-03: lifecycle-aware restore; quiesce/reopen failures surface.
    let report = service.restore_with_lifecycle(engine(), &unpacked, &work)?;
    if report.restored {
        super::speedtest::reload_profile_ex_after_restore();
    }
    Ok(RestoreResultDto {
        ok: report.restored,
        restored: report.restored,
        target_backup: report.target_backup,
        message: report.message,
        error: None,
    })
}

// -- update -----------------------------------------------------------------

fn cores_root() -> PathBuf {
    // Single source of truth shared with AppEngine/NetHostClient.
    engine().cores_root()
}

/// Explicit application install root: the directory holding the running
/// executable (override with `V2RAYN_R_APP_ROOT` for tests). This is what the
/// application-upgrade coordinator is allowed to touch; it is never derived
/// from `cores_root.parent()`.
fn app_install_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("V2RAYN_R_APP_ROOT") {
        if !dir.trim().is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn update_service() -> application::UpdateService {
    let mut service = application::UpdateService::new(cores_root());
    if let Some(root) = app_install_root() {
        service = service.with_app_install_root(root);
    }
    // SP-27 / SP28-L1-001: the Geo `.dat` template follows the persisted
    // `GeoSourceUrl` (falling back to the built-in) exactly like upstream
    // `UpdateService.GetGeoFilesRequest`.
    if let Ok(loaded) = engine().load_settings() {
        service = service.with_geo_source(application::dns::effective_geo_source(
            &loaded.settings.const_item,
        ));
    }
    service
}

fn update_target_dto(target: application::UpdateTargetInfo) -> UpdateTargetDto {
    UpdateTargetDto {
        core: target.core,
        repo: target.repo,
        supported: target.supported,
        prerelease_capable: target.prerelease_capable,
        max_version: target.max_version,
        note: target.note,
    }
}

fn core_update_dto(check: application::CoreUpdateCheck) -> CoreUpdateDto {
    CoreUpdateDto {
        core: check.core,
        supported: check.supported,
        note: check.note,
        installed_version: check.installed_version,
        remote_version: check.remote_version,
        has_update: check.has_update,
        asset_name: check.asset_name,
        download_url: check.download_url,
        expected_sha256: check.expected_sha256,
        dgst_url: check.dgst_url,
    }
}

fn proxy_unavailable() -> ErrorDto {
    ErrorDto {
        code: codes::PROXY_UNAVAILABLE.to_string(),
        message_key: "error.proxy_unavailable".to_string(),
        field_path: None,
        retryable: false,
        operation_id: None,
        detail: Some("no local proxy endpoint available".to_string()),
    }
}

/// `update_targets` — the built-in target list for the UI (disabled targets
/// carry a note and must be rendered disabled).
#[frb(sync)]
pub fn t16_update_targets() -> Vec<UpdateTargetDto> {
    application::builtin_targets(false)
        .into_iter()
        .map(update_target_dto)
        .collect()
}

/// S8-05: bridge-layer backstop timeouts for the T16 update operations.
///
/// The inner clients already bound each hop (`UPDATE_TIMEOUT` = 30s metadata,
/// `DOWNLOAD_TIMEOUT` = 180s artifact stream, 30s WebDAV client), but a stall
/// those do not bound (half-open proxy connection, TLS stall, unresponsive
/// mirror) must surface as a structured error instead of wedging the UI in a
/// busy state. Values derive from the inner bounds: check steps double the
/// 30s metadata bound; apply steps cover a 30s check plus a 180s download
/// plus install/hash margin; the application stage is the most generous
/// because its package dwarfs any core archive. No new error keys: timeouts
/// reuse the established `E_TIMEOUT` / `error.timeout` contract. (A
/// user-cancellable channel needs a new FRB function and stays with B07b.)
const T16_CHECK_STEP_TIMEOUT: Duration = Duration::from_secs(60);
const T16_APPLY_STEP_TIMEOUT: Duration = Duration::from_secs(300);
const T16_APP_STAGE_TIMEOUT: Duration = Duration::from_secs(600);

fn update_timeout(op: &str) -> ErrorDto {
    error_dto(
        DomainError::new(codes::TIMEOUT, "error.timeout")
            .with_detail(format!("{op} exceeded the bridge time budget")),
    )
}

/// S8-05: bound one network step; a stall surfaces as
/// `E_TIMEOUT` / `error.timeout` so the existing `Err` branches return a
/// structured DTO (with the partial results so far) instead of hanging.
async fn update_step<T>(
    op: &str,
    timeout: Duration,
    step: impl std::future::Future<Output = Result<T, DomainError>>,
) -> Result<T, ErrorDto> {
    match tokio::time::timeout(timeout, step).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error_dto(error)),
        Err(_) => Err(update_timeout(op)),
    }
}

/// `check_updates` — check the selected cores. Upstream contract: `null`
/// means every built-in target, an empty list means none (the user unchecked
/// every row) and is never expanded back to all.
pub async fn t16_check_updates(
    cores: Option<Vec<String>>,
    prerelease: bool,
    via_proxy: bool,
) -> UpdateReportDto {
    let proxy = via_proxy.then(|| engine().local_proxy_url()).flatten();
    if via_proxy
        && proxy
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return UpdateReportDto {
            ok: false,
            checks: Vec::new(),
            error: Some(proxy_unavailable()),
        };
    }
    let service = update_service();
    let selected = match cores {
        None => application::BUILTIN_TARGETS
            .iter()
            .map(|c| (*c).to_string())
            .collect::<Vec<_>>(),
        Some(list) => list,
    };
    check_updates_inner(
        &service,
        selected,
        prerelease,
        proxy,
        T16_CHECK_STEP_TIMEOUT,
    )
    .await
}

// S8-05 seam: the check body takes its service/selection/timeout explicitly
// so tests can drive it against a loopback mock without touching the FRB
// entry point above (whose signature is frozen).
async fn check_updates_inner(
    service: &application::UpdateService,
    selected: Vec<String>,
    prerelease: bool,
    proxy: Option<String>,
    step_timeout: Duration,
) -> UpdateReportDto {
    let mut checks = Vec::new();
    for core in selected {
        // R3-08: the application target must be checked against its own release
        // source, never the upstream `2dust/v2rayN` repo. When this build has no
        // source configured the row is reported explicitly blocked instead of
        // presenting the upstream release as an available self-update.
        if core == "v2rayN" {
            if service.app_repo.is_none() {
                checks.push(core_update_dto(service.app_source_unconfigured_check()));
                continue;
            }
            match update_step(
                "check_updates",
                step_timeout,
                service.check_app_update(prerelease, proxy.as_deref()),
            )
            .await
            {
                Ok(check) => checks.push(core_update_dto(check)),
                Err(error) => {
                    return UpdateReportDto {
                        ok: false,
                        checks,
                        error: Some(error),
                    }
                }
            }
            continue;
        }
        match update_step(
            "check_updates",
            step_timeout,
            service.check_core(&core, prerelease, proxy.as_deref()),
        )
        .await
        {
            Ok(check) => checks.push(core_update_dto(check)),
            Err(error) => {
                return UpdateReportDto {
                    ok: false,
                    checks,
                    error: Some(error),
                }
            }
        }
    }
    UpdateReportDto {
        ok: true,
        checks,
        error: None,
    }
}

/// `apply_core_update` — download+verify+install every selected target that has
/// a newer version. Runs only inside the managed cores directory.
pub async fn t16_apply_core_update(
    cores: Vec<String>,
    prerelease: bool,
    via_proxy: bool,
) -> ApplyCoreResultDto {
    let proxy = via_proxy.then(|| engine().local_proxy_url()).flatten();
    if via_proxy
        && proxy
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return ApplyCoreResultDto {
            ok: false,
            applied: Vec::new(),
            skipped: Vec::new(),
            error: Some(proxy_unavailable()),
        };
    }
    let service = update_service();
    apply_core_update_inner(
        &service,
        engine(),
        cores,
        prerelease,
        proxy,
        T16_CHECK_STEP_TIMEOUT,
        T16_APPLY_STEP_TIMEOUT,
    )
    .await
}

// S8-05 seam: same rationale as `check_updates_inner`. The engine travels
// explicitly so tests can run the managed stop/replace/restart cycle against
// an isolated engine; production passes the global one. The old post-loop
// restart is gone: `apply_core_managed` restarts only a previously running
// target, so keeping the tail would restart twice.
async fn apply_core_update_inner(
    service: &application::UpdateService,
    engine: &AppEngine,
    cores: Vec<String>,
    prerelease: bool,
    proxy: Option<String>,
    check_timeout: Duration,
    stage_timeout: Duration,
) -> ApplyCoreResultDto {
    // S8-05: every network step below runs under the bridge time budget.
    let token = CancellationToken::new();
    let mut applied = Vec::new();
    let mut skipped = Vec::new();
    for core in cores {
        // The application is not a runnable core and must never be unpacked
        // into the cores directory; its update goes through the external
        // upgrade path (`t16_apply_app_update_spec`).
        if core == "v2rayN" {
            skipped.push(core);
            continue;
        }
        // SP-27 / SP28-L1-001: Geo `.dat` files have no release/version row;
        // apply refreshes them from the pinned template (stage-all then
        // copy-over, old files intact on failure). The reported "version" is
        // the content hash, never a fabricated remote version.
        if core == application::GEO_FILES_TARGET {
            match update_step(
                "apply_core_update",
                stage_timeout,
                service.apply_geo_files(proxy.as_deref(), &token),
            )
            .await
            {
                Ok(outcome) => {
                    let version = outcome
                        .files
                        .iter()
                        .map(|file| {
                            format!("{}:{}", file.name, &file.sha256[..8.min(file.sha256.len())])
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    applied.push(AppliedCoreDto {
                        core: outcome.target,
                        version,
                        installed_dir: Some(outcome.bin_dir.to_string_lossy().into_owned()),
                        kept_previous: None,
                    });
                }
                Err(error) => {
                    return ApplyCoreResultDto {
                        ok: false,
                        applied,
                        skipped,
                        error: Some(error),
                    }
                }
            }
            continue;
        }
        let check = match update_step(
            "apply_core_update",
            check_timeout,
            service.check_core(&core, prerelease, proxy.as_deref()),
        )
        .await
        {
            Ok(check) => check,
            Err(error) => {
                return ApplyCoreResultDto {
                    ok: false,
                    applied,
                    skipped,
                    error: Some(error),
                }
            }
        };
        let (Some(asset_name), Some(download_url), Some(version)) = (
            check.asset_name.clone(),
            check.download_url.clone(),
            check.remote_version.clone(),
        ) else {
            skipped.push(core);
            continue;
        };
        if !check.supported || !check.has_update {
            skipped.push(core);
            continue;
        }
        let request = application::CoreApplyRequest {
            core: check.core.clone(),
            version,
            asset_name,
            download_url,
            expected_sha256: check.expected_sha256,
            dgst_url: check.dgst_url,
            proxy: proxy.clone(),
        };
        // Managed wiring: a running target stops its project-held session,
        // replaces, then restarts from its run plan; an idle target installs
        // exactly like the original path. Failure rollback lives inside.
        match update_step(
            "apply_core_update",
            stage_timeout,
            service.apply_core_managed(engine, &request, &token),
        )
        .await
        {
            Ok(outcome) => applied.push(AppliedCoreDto {
                core: outcome.core,
                version: outcome.version,
                installed_dir: Some(outcome.installed_dir.to_string_lossy().into_owned()),
                kept_previous: outcome
                    .kept_previous
                    .map(|p| p.to_string_lossy().into_owned()),
            }),
            Err(error) => {
                return ApplyCoreResultDto {
                    ok: false,
                    applied,
                    skipped,
                    error: Some(error),
                }
            }
        }
    }
    ApplyCoreResultDto {
        ok: true,
        applied,
        skipped,
        error: None,
    }
}

/// `apply_app_update_spec` — stage the application update and return the
/// external-upgrade spec. No process is started.
async fn app_update_spec_inner(prerelease: bool, via_proxy: bool) -> ExternalSpecDto {
    let service = update_service();
    let proxy = via_proxy.then(|| engine().local_proxy_url()).flatten();
    if via_proxy
        && proxy
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return ExternalSpecDto {
            ok: false,
            helper_exe: None,
            source: None,
            install_root: None,
            wait_for_pid: 0,
            args: Vec::new(),
            error: Some(proxy_unavailable()),
        };
    }
    app_update_spec_stage(
        &service,
        proxy,
        prerelease,
        T16_CHECK_STEP_TIMEOUT,
        T16_APP_STAGE_TIMEOUT,
    )
    .await
}

// S8-05 seam: same rationale as `check_updates_inner`.
async fn app_update_spec_stage(
    service: &application::UpdateService,
    proxy: Option<String>,
    prerelease: bool,
    check_timeout: Duration,
    stage_timeout: Duration,
) -> ExternalSpecDto {
    // S8-05: every network step below runs under the bridge time budget.
    let check = match update_step(
        "apply_app_update_spec",
        check_timeout,
        service.check_app_update(prerelease, proxy.as_deref()),
    )
    .await
    {
        Ok(check) => check,
        Err(error) => {
            return ExternalSpecDto {
                ok: false,
                helper_exe: None,
                source: None,
                install_root: None,
                wait_for_pid: 0,
                args: Vec::new(),
                error: Some(error),
            }
        }
    };
    let (Some(asset_name), Some(download_url), Some(version)) =
        (check.asset_name, check.download_url, check.remote_version)
    else {
        return ExternalSpecDto {
            ok: false,
            helper_exe: None,
            source: None,
            install_root: None,
            wait_for_pid: 0,
            args: Vec::new(),
            error: Some(ErrorDto {
                code: codes::NOT_FOUND.to_string(),
                message_key: "error.update_unsupported".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: Some("application update is not supported here".to_string()),
            }),
        };
    };
    // RT-15: the app's own update must be verified against the bundled
    // upstream OpenPGP trust root before staging. Fail closed when no backend
    // can enforce it; a missing/wrong signature is rejected downstream.
    let verifier = match application::app_signature_verifier() {
        Ok(verifier) => verifier,
        Err(error) => {
            return ExternalSpecDto {
                ok: false,
                helper_exe: None,
                source: None,
                install_root: None,
                wait_for_pid: 0,
                args: Vec::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let signature_url = check.sig_url.clone();
    let request = application::CoreApplyRequest {
        core: "v2rayN".to_string(),
        version,
        asset_name,
        download_url,
        expected_sha256: check.expected_sha256,
        dgst_url: check.dgst_url,
        proxy: proxy.clone(),
    };
    let helper = service.app_layout().runner_exe();
    let token = CancellationToken::new();
    match update_step(
        "apply_app_update_spec",
        stage_timeout,
        service.app_update_spec_verified(
            &request,
            signature_url.as_deref(),
            verifier.as_ref(),
            helper,
            std::process::id(),
            &token,
        ),
    )
    .await
    {
        Ok(spec) => ExternalSpecDto {
            ok: true,
            helper_exe: Some(spec.helper_exe.to_string_lossy().into_owned()),
            source: Some(spec.source.to_string_lossy().into_owned()),
            install_root: Some(spec.install_root.to_string_lossy().into_owned()),
            wait_for_pid: spec.wait_for_pid,
            args: spec.args,
            error: None,
        },
        Err(error) => ExternalSpecDto {
            ok: false,
            helper_exe: None,
            source: None,
            install_root: None,
            wait_for_pid: 0,
            args: Vec::new(),
            error: Some(error),
        },
    }
}

/// `cleanup_logs_tmp` — manual trigger of the 1h Test / 7d log+temp cleanup.
#[frb(sync)]
pub fn t16_cleanup_logs_tmp() -> CleanupResultDto {
    let Some(dir) = engine().data_dir() else {
        return CleanupResultDto {
            ok: true,
            deleted: 0,
            bytes: 0,
            skipped: 0,
            error: None,
        };
    };
    match application::cleanup_logs_tmp(dir, now_epoch(), 7, 1) {
        Ok(report) => CleanupResultDto {
            ok: true,
            deleted: report.deleted,
            bytes: report.bytes,
            skipped: report.skipped,
            error: None,
        },
        Err(error) => CleanupResultDto {
            ok: false,
            deleted: 0,
            bytes: 0,
            skipped: 0,
            error: Some(error_dto(error)),
        },
    }
}

/// `open_config_dir` — reveal the data directory in the file manager.
#[frb(sync)]
pub fn t16_open_config_dir() -> SimpleResult {
    let Some(dir) = engine().data_dir() else {
        return SimpleResult {
            ok: false,
            error: Some(ErrorDto {
                code: codes::UNAVAILABLE.to_string(),
                message_key: "error.engine_not_persistent".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: None,
            }),
        };
    };
    match open_path(dir) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(error) => SimpleResult {
            ok: false,
            error: Some(ErrorDto {
                code: codes::INTERNAL.to_string(),
                message_key: "error.open_dir_failed".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: Some(error),
            }),
        },
    }
}

fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `get_core_versions` — read-only probe of installed core directories.
#[frb(sync)]
pub fn t16_get_core_versions() -> CoreVersionsDto {
    let service = update_service();
    CoreVersionsDto {
        items: service
            .installed_cores()
            .into_iter()
            .map(|core| InstalledCoreDto {
                core: core.core,
                dir: core.dir,
                version: core.version,
                executable: core.executable,
            })
            .collect(),
        error: None,
    }
}

/// `apply_app_update_spec_with_flags` — parameterized self-update staging
/// (R4-29): the `prerelease` / `via_proxy` selection is passed explicitly
/// instead of being read from a process-global "last check" slot.
pub async fn t16_apply_app_update_spec_with_flags(
    prerelease: bool,
    via_proxy: bool,
) -> ExternalSpecDto {
    app_update_spec_inner(prerelease, via_proxy).await
}

/// Backward-compatible no-arg entry point (SP-27).
///
/// The generated FRB signature takes no arguments, so it cannot carry the
/// current check toggles. It stages with the safe defaults (stable channel,
/// direct connection) and never reuses another operation's flags; the
/// per-operation selection travels through
/// [`t16_apply_app_update_spec_with_flags`] once the Bridge/UI wiring lands
/// (SP-00 follow-up).
pub async fn t16_apply_app_update_spec() -> ExternalSpecDto {
    app_update_spec_inner(false, false).await
}

/// `rollback_app_upgrade` — restore the `app.previous` payload kept by the last
/// external replacement (R4-29). No network; file effects stay inside the
/// install root.
#[frb(sync)]
pub fn t16_rollback_app_upgrade() -> AppRollbackResultDto {
    match update_service().rollback_app_upgrade() {
        Ok(path) => AppRollbackResultDto {
            ok: true,
            restored: Some(path.to_string_lossy().into_owned()),
            error: None,
        },
        Err(error) => AppRollbackResultDto {
            ok: false,
            restored: None,
            error: Some(error_dto(error)),
        },
    }
}

/// `app_restart_command` — the relaunch command the external upgrade runner
/// executes after a replacement (R4-29). Constructed only; never spawned here.
#[frb(sync)]
pub fn t16_app_restart_command() -> AppRestartCommandDto {
    let command = update_service().app_restart_command();
    AppRestartCommandDto {
        program: command.program.to_string_lossy().into_owned(),
        working_dir: command.working_dir.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use application::runtime_client::{
        ApplyOutcome, OperationStatusView, RuntimeClient, RuntimeSnapshot,
    };
    use domain::{CancelOutcome, ConfigType, DesiredRevision, JobId, Profile, RuntimeState};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    #[test]
    fn update_targets_list_builtins_with_caps() {
        let targets = t16_update_targets();
        let singbox = targets.iter().find(|t| t.core == "sing_box").unwrap();
        assert!(singbox.supported);
        assert_eq!(singbox.max_version.as_deref(), Some("1.14.4294967295"));
        // R4-21: every frozen proxy core is listed. Cores with no in-app
        // download asset are shown disabled as manual, never omitted.
        for core in [
            "v2fly",
            "v2fly_v5",
            "xray",
            "sing_box",
            "mihomo",
            "hysteria",
            "naiveproxy",
            "tuic",
            "juicity",
            "hysteria2",
            "brook",
            "overtls",
            "shadowquic",
            "mieru",
        ] {
            assert!(
                targets.iter().any(|t| t.core == core),
                "target list omitted {core}"
            );
        }
        let tuic = targets.iter().find(|t| t.core == "tuic").unwrap();
        assert!(!tuic.supported);
        assert_eq!(tuic.note.as_deref(), Some("error.update_manual"));
    }

    #[test]
    fn update_via_proxy_without_port_is_structured() {
        let _guard = crate::api::engine::engine_test_lock();
        engine().set_local_proxy_port(None);
        let report = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(t16_check_updates(
                Some(vec!["xray".to_string()]),
                false,
                true,
            ));
        assert!(!report.ok);
        assert_eq!(report.error.unwrap().code, codes::PROXY_UNAVAILABLE);
    }

    #[test]
    fn check_updates_blocks_app_target_without_own_source() {
        // No network: with no self-release source configured the application
        // target is returned as an explicit blocked row (R3-08), never the
        // upstream v2rayN release info.
        if update_service().app_repo.is_some() {
            return;
        }
        let report = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(t16_check_updates(
                Some(vec!["v2rayN".to_string()]),
                false,
                false,
            ));
        assert!(report.ok);
        let app = report
            .checks
            .iter()
            .find(|c| c.core == "v2rayN")
            .expect("application row present");
        assert!(!app.supported);
        assert_eq!(
            app.note.as_deref(),
            Some("error.update_app_source_unconfigured")
        );
        assert!(app.remote_version.is_none());
        assert!(app.download_url.is_none());
    }

    #[test]
    fn check_updates_empty_selection_checks_nothing() {
        // Upstream contract (AUD-DESK-12): an empty selection means the user
        // unchecked every row and must be checked as nothing; only `None`
        // (not provided) means all built-ins. No network is touched here.
        let report = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(t16_check_updates(Some(Vec::new()), false, false));
        assert!(report.ok);
        assert!(report.checks.is_empty());
    }

    #[test]
    fn app_update_spec_with_flags_via_proxy_without_port_is_structured() {
        // R4-29: the parameterized entry point must reject "via proxy" with no
        // known local endpoint before any network/verify step.
        let _guard = crate::api::engine::engine_test_lock();
        engine().set_local_proxy_port(None);
        let spec = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(t16_apply_app_update_spec_with_flags(false, true));
        assert!(!spec.ok);
        assert_eq!(spec.error.unwrap().code, codes::PROXY_UNAVAILABLE);
    }

    #[test]
    fn app_update_spec_reports_unconfigured_source_without_network() {
        // SP-27: with no own release source configured (the shipped default),
        // both staging entries fail closed with
        // `error.update_app_source_unconfigured` before any download/verify
        // step, and never fabricate a spec. No network is touched.
        if update_service().app_repo.is_some() {
            return;
        }
        // S8-05: the update pipeline now bounds every network step with a
        // bridge timeout, so tests driving it need a timer driver (the
        // production FRB runtime always has one). No network is touched.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");
        for spec in [
            rt.block_on(t16_apply_app_update_spec_with_flags(false, false)),
            rt.block_on(t16_apply_app_update_spec()),
        ] {
            assert!(!spec.ok);
            assert!(spec.helper_exe.is_none());
            assert!(spec.source.is_none());
            let error = spec.error.expect("structured error");
            assert_eq!(error.message_key, "error.update_app_source_unconfigured");
        }
    }

    #[test]
    fn app_restart_command_is_exposed() {
        let command = t16_app_restart_command();
        assert!(!command.program.is_empty());
        assert!(!command.working_dir.is_empty());
    }

    #[test]
    fn rollback_without_previous_is_structured_not_found() {
        // No real install root is touched: a fresh temp layout has no
        // `app.previous`, so the rollback path reports a structured error.
        let root = std::env::temp_dir().join(format!("r4-29-rollback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&root);
        let service =
            application::UpdateService::new(root.join("cores")).with_app_install_root(root.clone());
        let error = service
            .rollback_app_upgrade()
            .expect_err("no previous payload");
        assert_eq!(error.code, codes::CONFLICT);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dgst_parse_reads_sha256_line() {
        let text = "MD5= a\nSHA1= b\nSHA2-256= D004C392AABB\nSHA2-512= c\n";
        assert_eq!(
            application::parse_dgst_sha256(text).as_deref(),
            Some("d004c392aabb")
        );
    }

    // S8-02: the upstream import must reset the speedtest in-memory overlay
    // exactly like the local/WebDAV restore paths do, or a stale overlay
    // entry (old delay for a node id) survives the import and a later flush
    // writes it back over the freshly imported ProfileEx.
    fn s8_02_live_root(tag: &str, seed_db: bool) -> PathBuf {
        let root = std::env::temp_dir().join(format!("b07a-s8-02-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("live")).expect("live dir");
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic/upstream-v2");
        // The checked-in fixture databases predate the current schema (no
        // `EchForceQuery` column). Replace swaps the whole file so a raw copy
        // is fine; merge inserts rows into the live file, so the live
        // database is created fresh (current schema, like a migrated app
        // database) by the engine open below.
        if seed_db {
            std::fs::copy(
                fixture.join("guiNDB.db"),
                root.join("live").join("guiNDB.db"),
            )
            .expect("fixture db copy");
        }
        std::fs::copy(
            fixture.join("guiNConfig.json"),
            root.join("live").join("guiNConfig.json"),
        )
        .expect("fixture config copy");
        root
    }

    fn s8_02_import_resets_stale_overlay(merge: bool, tag: &str) {
        let _guard = crate::api::engine::engine_test_lock();
        crate::api::speedtest::reset_speedtest_for_test();
        let stale_id = format!("s8-02-stale-{tag}");
        crate::api::speedtest::apply_speedtest_result_for_test(
            stale_id.clone(),
            88,
            1.0,
            "stale".to_string(),
        );
        assert!(
            crate::api::speedtest::speedtest_results()
                .iter()
                .any(|r| r.index_id == stale_id),
            "stale overlay entry seeded"
        );

        let root = s8_02_live_root(tag, !merge);
        let live = root.join("live");
        let engine = application::AppEngine::open_with_runtime(
            &live,
            std::sync::Arc::new(application::runtime_client::NullRuntimeClient::new()),
        )
        .expect("open test engine");
        let service = BackupService::new(&live);
        let work = root.join("work");
        std::fs::create_dir_all(&work).expect("work dir");
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic/upstream-v3");
        let summary = import_upstream_inner(&service, &engine, &source, &work, merge, now_epoch());
        let err_text = summary.error.as_ref().map(|e| {
            format!(
                "{} {} {}",
                e.code,
                e.message_key,
                e.detail.as_deref().unwrap_or("")
            )
        });
        assert!(
            summary.ok,
            "import ok: {} err={err_text:?}",
            summary.message
        );
        assert_eq!(summary.status, "imported");
        let stale_left = crate::api::speedtest::speedtest_results()
            .iter()
            .any(|r| r.index_id == stale_id);
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            !stale_left,
            "stale overlay entry survived the upstream import"
        );
    }

    #[test]
    fn s8_02_import_replace_resets_stale_speedtest_overlay() {
        s8_02_import_resets_stale_overlay(false, "replace");
    }

    #[test]
    fn s8_02_import_merge_resets_stale_speedtest_overlay() {
        s8_02_import_resets_stale_overlay(true, "merge");
    }

    // S8-05: a stall the inner clients do not bound must surface as the
    // structured E_TIMEOUT / error.timeout contract, never a wedged UI.
    // The loopback mock accepts the connection and then never answers.
    fn s8_05_hang_api_base() -> String {
        // Probe first-free like the other plan tests; 10808 is never touched.
        let listener = (11808..=11888)
            .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
            .expect("hang mock port");
        let addr = listener.local_addr().expect("mock addr");
        std::thread::spawn(move || {
            for mut s in listener.incoming().flatten() {
                let mut buf = [0u8; 1024];
                let _ = std::io::Read::read(&mut s, &mut buf);
                std::thread::sleep(Duration::from_secs(120));
            }
        });
        format!("http://{addr}/repos")
    }

    fn s8_05_mock_service(tag: &str, with_app_repo: bool) -> application::UpdateService {
        let cores = std::env::temp_dir().join(format!("b07a-s8-05-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cores);
        std::fs::create_dir_all(&cores).expect("cores dir");
        let mut service =
            application::UpdateService::new(&cores).with_api_base(s8_05_hang_api_base());
        if with_app_repo {
            service.app_repo = Some("owner/name".to_string());
        }
        service
    }

    fn s8_05_block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
            .block_on(future)
    }

    #[test]
    fn s8_05_check_updates_hang_returns_timeout() {
        let service = s8_05_mock_service("check", false);
        let report = s8_05_block_on(check_updates_inner(
            &service,
            vec!["xray".to_string()],
            false,
            None,
            Duration::from_millis(200),
        ));
        assert!(!report.ok);
        let error = report.error.expect("structured timeout error");
        assert_eq!(error.code, codes::TIMEOUT);
        assert_eq!(error.message_key, "error.timeout");
    }

    #[test]
    fn s8_05_apply_core_update_hang_returns_timeout() {
        let service = s8_05_mock_service("apply", false);
        let result = s8_05_block_on(apply_core_update_inner(
            &service,
            engine(),
            vec!["xray".to_string()],
            false,
            None,
            Duration::from_millis(200),
            Duration::from_millis(200),
        ));
        assert!(!result.ok);
        let error = result.error.expect("structured timeout error");
        assert_eq!(error.code, codes::TIMEOUT);
        assert_eq!(error.message_key, "error.timeout");
    }

    #[test]
    fn s8_05_app_update_spec_hang_returns_timeout() {
        let service = s8_05_mock_service("spec", true);
        let spec = s8_05_block_on(app_update_spec_stage(
            &service,
            None,
            false,
            Duration::from_millis(200),
            Duration::from_millis(200),
        ));
        assert!(!spec.ok);
        let error = spec.error.expect("structured timeout error");
        assert_eq!(error.code, codes::TIMEOUT);
        assert_eq!(error.message_key, "error.timeout");
    }

    #[test]
    fn s8_05_update_step_bounds_a_hung_future_without_network() {
        // Fast guard on the shared helper every update path uses: a future
        // that never resolves becomes E_TIMEOUT, and ready values (ok and
        // domain errors) pass through untouched.
        let timeout_err = match s8_05_block_on(update_step::<u8>(
            "probe",
            Duration::from_millis(50),
            std::future::pending(),
        )) {
            Ok(_) => panic!("hung step times out"),
            Err(error) => error,
        };
        assert_eq!(timeout_err.code, codes::TIMEOUT);
        assert_eq!(timeout_err.message_key, "error.timeout");
        let ok_value = match s8_05_block_on(update_step(
            "probe",
            Duration::from_secs(60),
            std::future::ready(Ok(7u8)),
        )) {
            Ok(value) => value,
            Err(_) => panic!("ready ok passes through"),
        };
        assert_eq!(ok_value, 7);
        let inner_err = match s8_05_block_on(update_step(
            "probe",
            Duration::from_secs(60),
            std::future::ready(Result::<u8, DomainError>::Err(DomainError::new(
                codes::UNAVAILABLE,
                "error.engine_not_persistent",
            ))),
        )) {
            Ok(_) => panic!("domain errors pass through"),
            Err(error) => error,
        };
        assert_eq!(inner_err.code, codes::UNAVAILABLE);
    }

    // Managed-update wiring: `t16_apply_core_update` must route a running
    // target through `apply_core_managed` (stop -> replace -> restart) while
    // an idle target keeps the original plain-install path. Loopback mock
    // serves the release metadata and a synthetic zip (std only, no exe
    // inside so the arch gate passes, every file dotted); ports >= 11808.
    struct S8wFake {
        calls: Mutex<Vec<String>>,
        next_op: AtomicUsize,
        running: Mutex<bool>,
        port: Mutex<u16>,
    }

    impl S8wFake {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                next_op: AtomicUsize::new(0),
                running: Mutex::new(false),
                port: Mutex::new(0),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl RuntimeClient for S8wFake {
        fn snapshot(&self) -> Result<RuntimeSnapshot, DomainError> {
            let running = *self.running.lock().unwrap();
            Ok(RuntimeSnapshot {
                state: if running {
                    RuntimeState::Running
                } else {
                    RuntimeState::Stopped
                },
                ports: vec![*self.port.lock().unwrap()],
                ..RuntimeSnapshot::default()
            })
        }

        fn operation_status(&self, operation_id: &str) -> Result<OperationStatusView, DomainError> {
            Ok(OperationStatusView {
                operation_id: operation_id.to_string(),
                job_id: None,
                state: domain::JobState::Running,
                cancel: None,
                error: None,
            })
        }

        fn apply(&self, _plan: &domain::RuntimePlan) -> Result<ApplyOutcome, DomainError> {
            self.calls.lock().unwrap().push("apply".to_string());
            *self.running.lock().unwrap() = true;
            Ok(ApplyOutcome::Accepted {
                operation_id: format!("op-{}", self.next_op.fetch_add(1, Ordering::SeqCst) + 1),
            })
        }

        fn stop(&self) -> Result<(), DomainError> {
            self.calls.lock().unwrap().push("stop".to_string());
            *self.running.lock().unwrap() = false;
            Ok(())
        }

        fn cancel(&self, _job_id: &JobId) -> Result<CancelOutcome, DomainError> {
            Ok(CancelOutcome::AlreadyFinished)
        }
    }

    fn s8w_crc32(bytes: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, slot) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *slot = c;
        }
        let mut crc = 0xFFFF_FFFFu32;
        for b in bytes {
            crc = table[((crc ^ u32::from(*b)) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc ^ 0xFFFF_FFFF
    }

    fn s8w_push_u16(buf: &mut Vec<u8>, v: u16) {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    fn s8w_push_u32(buf: &mut Vec<u8>, v: u32) {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Minimal stored (uncompressed) zip: `payload/note.txt`. Every name is
    /// dotted and none is an `.exe`, so the arch gate finds no candidate and
    /// passes; the single top-level dir survives `flatten_single_dir`.
    fn s8w_zip() -> Vec<u8> {
        let entries: &[(&str, &[u8])] = &[
            ("payload/", b""),
            ("payload/note.txt", b"synthetic core payload"),
        ];
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in entries {
            let is_dir = name.ends_with('/');
            let crc = s8w_crc32(data);
            let offset = out.len() as u32;
            s8w_push_u32(&mut out, 0x0403_4B50);
            s8w_push_u16(&mut out, 20);
            s8w_push_u16(&mut out, 0);
            s8w_push_u16(&mut out, 0);
            s8w_push_u16(&mut out, 0);
            s8w_push_u16(&mut out, 0);
            s8w_push_u32(&mut out, crc);
            s8w_push_u32(&mut out, data.len() as u32);
            s8w_push_u32(&mut out, data.len() as u32);
            s8w_push_u16(&mut out, name.len() as u16);
            s8w_push_u16(&mut out, 0);
            out.extend_from_slice(name.as_bytes());
            if !is_dir {
                out.extend_from_slice(data);
            }
            s8w_push_u32(&mut central, 0x0201_4B50);
            s8w_push_u16(&mut central, 20);
            s8w_push_u16(&mut central, 20);
            s8w_push_u16(&mut central, 0);
            s8w_push_u16(&mut central, 0);
            s8w_push_u16(&mut central, 0);
            s8w_push_u16(&mut central, 0);
            s8w_push_u32(&mut central, crc);
            s8w_push_u32(&mut central, data.len() as u32);
            s8w_push_u32(&mut central, data.len() as u32);
            s8w_push_u16(&mut central, name.len() as u16);
            s8w_push_u16(&mut central, 0);
            s8w_push_u16(&mut central, 0);
            s8w_push_u16(&mut central, 0);
            s8w_push_u16(&mut central, 0);
            s8w_push_u32(&mut central, if is_dir { 0x10 } else { 0 });
            s8w_push_u32(&mut central, offset);
            central.extend_from_slice(name.as_bytes());
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);
        s8w_push_u32(&mut out, 0x0605_4B50);
        s8w_push_u16(&mut out, 0);
        s8w_push_u16(&mut out, 0);
        s8w_push_u16(&mut out, entries.len() as u16);
        s8w_push_u16(&mut out, entries.len() as u16);
        s8w_push_u32(&mut out, cd_size);
        s8w_push_u32(&mut out, cd_offset);
        s8w_push_u16(&mut out, 0);
        out
    }

    /// Loopback release mock: metadata JSON (no digest, so hash verification
    /// is skipped exactly like a digest-less upstream asset) plus the zip.
    fn s8w_mock_api_base() -> String {
        let zip = s8w_zip();
        let listener = (11808..=11888)
            .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
            .expect("wiring mock port");
        let addr = listener.local_addr().expect("mock addr");
        let base = format!("http://{addr}");
        let json = format!(
            "[{{\"tag_name\":\"v26.4.0\",\"name\":\"Xray\",\"prerelease\":false,\"assets\":[{{\"name\":\"Xray-windows-64.zip\",\"size\":{},\"browser_download_url\":\"{base}/assets/xray.zip\"}}]}}]",
            zip.len()
        );
        std::thread::spawn(move || {
            for mut s in listener.incoming().flatten() {
                let mut buf = [0u8; 4096];
                let n = std::io::Read::read(&mut s, &mut buf).unwrap_or(0);
                let path = String::from_utf8_lossy(&buf[..n])
                    .split("\r\n")
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_string();
                let (status, content_type, body) = if path == "/repos/XTLS/Xray-core/releases" {
                    ("200 OK", "application/json", json.clone().into_bytes())
                } else if path == "/assets/xray.zip" {
                    ("200 OK", "application/zip", zip.clone())
                } else {
                    ("404 Not Found", "text/plain", b"nope".to_vec())
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut s, head.as_bytes());
                let _ = std::io::Write::write_all(&mut s, &body);
            }
        });
        format!("{base}/repos")
    }

    fn s8w_seed(engine: &application::AppEngine) {
        engine.seed(vec![Profile {
            index_id: "node-a".to_string(),
            config_type: ConfigType::Vless,
            remarks: "synthetic-node-a".to_string(),
            address: "192.0.2.10".into(),
            port: 443,
            password: "11111111-2222-3333-4444-555555555555".into(),
            ..Default::default()
        }]);
        engine.set_active(Some("node-a".to_string())).unwrap();
    }

    fn s8w_free_port(mut base: u16) -> u16 {
        loop {
            assert!(base < 60000, "port scan exhausted");
            if base == 10808 {
                base += 1;
                continue;
            }
            if std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
                return base;
            }
            base += 1;
        }
    }

    fn s8w_start_session(engine: &application::AppEngine, fake: &S8wFake, base: u16) {
        let port = s8w_free_port(base);
        *fake.port.lock().unwrap() = port;
        let loaded = engine.load_settings().expect("load settings");
        let mut settings = loaded.settings;
        if let Some(first) = settings.inbound.first_mut() {
            first.local_port = i32::from(port);
        }
        engine
            .save_settings(settings, loaded.revision)
            .expect("save settings");
        let revision = engine.desired_revision();
        let plan = engine
            .build_runtime_plan("node-a", revision)
            .expect("synthetic plan must build");
        engine
            .apply_runtime(plan, DesiredRevision::new(revision))
            .expect("session must start");
        engine.snapshot().expect("snapshot must reconcile");
        assert!(
            engine.applied_session().is_some(),
            "test setup: a session must be running"
        );
    }

    fn s8w_service(cores: &Path, api_base: String) -> application::UpdateService {
        application::UpdateService::new(cores).with_api_base(api_base)
    }

    fn s8w_result_error(result: &ApplyCoreResultDto) -> String {
        result
            .error
            .as_ref()
            .map(|e| {
                format!(
                    "{} {} {}",
                    e.code,
                    e.message_key,
                    e.detail.as_deref().unwrap_or("")
                )
            })
            .unwrap_or_default()
    }

    #[test]
    fn s8w_apply_running_core_stops_replaces_and_restarts() {
        let api_base = s8w_mock_api_base();
        let root = std::env::temp_dir().join(format!("b07a-s8w-{}-running", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cores = root.join("cores");
        std::fs::create_dir_all(&cores).expect("cores dir");
        let service = s8w_service(&cores, api_base);
        let fake = Arc::new(S8wFake::new());
        let engine = application::AppEngine::with_runtime(fake.clone());
        s8w_seed(&engine);
        s8w_start_session(&engine, &fake, 11961);
        // An older installed version, so the replacement keeps `.previous`.
        let old = cores.join("xray").join("26.3.27");
        std::fs::create_dir_all(&old).expect("old dir");
        std::fs::write(old.join("xray.exe"), b"old-synthetic").expect("old exe");

        let result = s8_05_block_on(apply_core_update_inner(
            &service,
            &engine,
            vec!["xray".to_string()],
            false,
            None,
            T16_CHECK_STEP_TIMEOUT,
            T16_APPLY_STEP_TIMEOUT,
        ));
        assert!(result.ok, "apply ok: {}", s8w_result_error(&result));
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.applied[0].core, "xray");
        assert_eq!(result.applied[0].version, "26.4.0");
        // Setup apply, managed stop, managed restart: no other order is safe.
        assert_eq!(fake.calls(), vec!["apply", "stop", "apply"]);
        engine.snapshot().expect("snapshot must reconcile");
        assert!(
            engine.applied_session().is_some(),
            "the previous version must be running again after replace"
        );
        assert!(cores.join("xray").join("26.4.0").is_dir());
        assert!(cores.join("xray.previous").is_dir());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn s8w_apply_idle_core_takes_original_path() {
        let api_base = s8w_mock_api_base();
        let root = std::env::temp_dir().join(format!("b07a-s8w-{}-idle", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cores = root.join("cores");
        std::fs::create_dir_all(&cores).expect("cores dir");
        let service = s8w_service(&cores, api_base);
        let fake = Arc::new(S8wFake::new());
        let engine = application::AppEngine::with_runtime(fake.clone());
        s8w_seed(&engine);

        let result = s8_05_block_on(apply_core_update_inner(
            &service,
            &engine,
            vec!["xray".to_string()],
            false,
            None,
            T16_CHECK_STEP_TIMEOUT,
            T16_APPLY_STEP_TIMEOUT,
        ));
        assert!(result.ok, "apply ok: {}", s8w_result_error(&result));
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.applied[0].version, "26.4.0");
        // Nothing was running: no stop, no restart, plain install.
        assert!(fake.calls().is_empty());
        assert!(cores.join("xray").join("26.4.0").is_dir());
        assert!(engine.applied_session().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn s8w_webdav_config_save_rejects_traversing_dir() {
        // Bridge save-time gate: an illegal dir_name must be rejected with
        // the existing error contract before anything is persisted.
        let _guard = crate::api::engine::engine_test_lock();
        let before = t16_webdav_config_get();
        let original = before.config.clone().expect("webdav config present");
        let bad = WebDavConfigDto {
            url: original.url.clone(),
            user_name: original.user_name.clone(),
            password: original.password.clone(),
            dir_name: "../other".to_string(),
        };
        let saved = t16_webdav_config_save(bad, before.revision);
        // Restore before asserting so even the pre-fix failure leaves the
        // shared test engine exactly as found.
        let cur = t16_webdav_config_get();
        let restored = t16_webdav_config_save(original.clone(), cur.revision);
        assert!(restored.ok, "restore must succeed");
        assert!(!saved.ok, "traversing dir_name must be rejected");
        let error = saved.error.expect("structured error");
        assert_eq!(error.code, codes::FIELD_FORMAT);
        assert_eq!(error.message_key, "error.webdav_url_invalid");
        let after = t16_webdav_config_get();
        assert_eq!(after.config.expect("config").dir_name, original.dir_name);
    }
}
