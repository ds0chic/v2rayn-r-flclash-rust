//! T16 FRB functions: backup/restore, upstream ZIP import, WebDAV and the
//! built-in core/application update pipeline.
//!
//! Network calls only ever hit the endpoint the caller configured; the update
//! path refuses to run when "via proxy" is requested but no local session port
//! is known (it never silently falls back to a direct connection). No function
//! here replaces a live installation outside the managed cores directory.

use std::path::{Path, PathBuf};

use application::{BackupManifest, BackupService, ImportStatus, WebDavClient, WebDavConfig};
use domain::{codes, CancellationToken, DomainError};
use flutter_rust_bridge::frb;

use crate::api::contract::{
    AppliedCoreDto, ApplyCoreResultDto, BackupListDto, BackupManifestDto, BackupResultDto,
    CleanupResultDto, CoreUpdateDto, CoreVersionsDto, EntityCountDto, ErrorDto, ExternalSpecDto,
    ImportSummaryDto, InstalledCoreDto, RecognitionDto, RestoreResultDto, SimpleResult,
    UpdateReportDto, UpdateTargetDto, VerificationDto, WebDavCheckDto, WebDavConfigDto,
    WebDavConfigResultDto, WebDavEntryDto, WebDavListDto, WebDavOpDto,
};
use crate::api::engine::{engine, error_dto};

/// Last update flags the user selected (RR-04).
///
/// The generated FRB signature of `t16_apply_app_update_spec` takes no
/// arguments, so it cannot carry flags without regenerating bindings. The
/// check / apply-core calls record the most recent selection here and the app
/// self-update action reuses it; when nothing was selected yet the safe
/// defaults (`prerelease=false`, direct connection) apply.
#[frb(ignore)]
#[derive(Clone, Default)]
struct UpdateFlags {
    prerelease: bool,
    proxy: Option<String>,
}

fn update_flags() -> &'static std::sync::Mutex<UpdateFlags> {
    static FLAGS: std::sync::OnceLock<std::sync::Mutex<UpdateFlags>> = std::sync::OnceLock::new();
    FLAGS.get_or_init(|| std::sync::Mutex::new(UpdateFlags::default()))
}

fn remember_update_flags(prerelease: bool, proxy: Option<String>) {
    if let Ok(mut flags) = update_flags().lock() {
        flags.prerelease = prerelease;
        flags.proxy = proxy;
    }
}

fn last_update_flags() -> UpdateFlags {
    update_flags()
        .lock()
        .map(|flags| flags.clone())
        .unwrap_or_default()
}

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
    let service = match backup_service() {
        Ok(service) => service,
        Err(error) => {
            return BackupResultDto {
                ok: false,
                root: None,
                manifest: None,
                error: Some(error_dto(error)),
            }
        }
    };
    match service.create_local(Path::new(&dest_root), now_epoch()) {
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
            items: items.iter().map(|m| manifest_dto(m, None)).collect(),
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
    let service = match backup_service() {
        Ok(service) => service,
        Err(error) => {
            return RestoreResultDto {
                ok: false,
                restored: false,
                target_backup: None,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let work = match work_dir("restore") {
        Ok(path) => path,
        Err(error) => {
            return RestoreResultDto {
                ok: false,
                restored: false,
                target_backup: None,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    // SR-03: stop the managed session/timer, quiesce, exchange and reopen in
    // one lifecycle. A failed quiesce/reopen is a structured error; it is never
    // dropped, so the UI cannot be told a swap succeeded when the engine is
    // still bound to the old (or no) storage.
    let outcome = service.restore_with_lifecycle(engine(), Path::new(&bundle_dir), &work);
    match outcome {
        Ok(report) => RestoreResultDto {
            ok: report.restored,
            restored: report.restored,
            target_backup: report.target_backup,
            message: report.message,
            error: None,
        },
        Err(error) => RestoreResultDto {
            ok: false,
            restored: false,
            target_backup: None,
            message: String::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `backup_recognize` — recognise an upstream or project archive. Async: reads
/// the archive directory/central directory.
pub fn t16_backup_recognize(path: String) -> RecognitionDto {
    let service = match backup_service() {
        Ok(service) => service,
        Err(error) => {
            return RecognitionDto {
                is_upstream: false,
                has_config: false,
                has_db: false,
                layout: String::new(),
                entries: Vec::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    match service.recognize(Path::new(&path)) {
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
    let service = match backup_service() {
        Ok(service) => service,
        Err(error) => {
            return ImportSummaryDto {
                ok: false,
                status: "failed".to_string(),
                source_version: 0,
                imported_rows: 0,
                warnings: 0,
                errors: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let work = match work_dir("import") {
        Ok(path) => path,
        Err(error) => {
            return ImportSummaryDto {
                ok: false,
                status: "failed".to_string(),
                source_version: 0,
                imported_rows: 0,
                warnings: 0,
                errors: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    // SR-03: same lifecycle as the local restore; quiesce/reopen failures are
    // propagated as structured errors instead of being ignored.
    let outcome =
        service.import_upstream_with_lifecycle(engine(), Path::new(&path), &work, now_epoch());
    match outcome {
        Ok(report) => {
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
        Err(error) => ImportSummaryDto {
            ok: false,
            status: "failed".to_string(),
            source_version: 0,
            imported_rows: 0,
            warnings: 0,
            errors: 1,
            message: String::new(),
            error: Some(error_dto(error)),
        },
    }
}

/// `backup_import_upstream_merge` — the explicitly named migration/merge
/// import that appends the source rows to the live database (R3-SET-02).
///
/// Registered as an independent entry; the checked-in `frb_generated` bindings
/// are not regenerated, so the UI restore path stays on
/// [`t16_backup_import_upstream`] (replace).
#[frb(sync)]
pub fn t16_backup_import_upstream_merge(path: String) -> ImportSummaryDto {
    let service = match backup_service() {
        Ok(service) => service,
        Err(error) => {
            return ImportSummaryDto {
                ok: false,
                status: "failed".to_string(),
                source_version: 0,
                imported_rows: 0,
                warnings: 0,
                errors: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let work = match work_dir("import-merge") {
        Ok(path) => path,
        Err(error) => {
            return ImportSummaryDto {
                ok: false,
                status: "failed".to_string(),
                source_version: 0,
                imported_rows: 0,
                warnings: 0,
                errors: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let outcome = service.import_upstream_merge_with_lifecycle(
        engine(),
        Path::new(&path),
        &work,
        now_epoch(),
    );
    match outcome {
        Ok(report) => {
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
        Err(error) => ImportSummaryDto {
            ok: false,
            status: "failed".to_string(),
            source_version: 0,
            imported_rows: 0,
            warnings: 0,
            errors: 1,
            message: String::new(),
            error: Some(error_dto(error)),
        },
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
    let patch = serde_json::json!({
        "Url": cfg.url,
        "UserName": cfg.user_name,
        "Password": cfg.password,
        "DirName": cfg.dir_name,
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
    let bytes = match application::zip_upstream_layout(&backup.root) {
        Ok(bytes) => bytes,
        Err(error) => {
            return WebDavOpDto {
                ok: false,
                bytes: 0,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let length = bytes.len() as u64;
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
    match client.upload(bytes).await {
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
    let service = match backup_service() {
        Ok(service) => service,
        Err(error) => {
            return RestoreResultDto {
                ok: false,
                restored: false,
                target_backup: None,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let work = match work_dir("webdav-restore") {
        Ok(path) => path,
        Err(error) => {
            return RestoreResultDto {
                ok: false,
                restored: false,
                target_backup: None,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let client = match webdav_client(cfg) {
        Ok(client) => client,
        Err(error) => {
            return RestoreResultDto {
                ok: false,
                restored: false,
                target_backup: None,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let bytes = match client.download().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return RestoreResultDto {
                ok: false,
                restored: false,
                target_backup: None,
                message: String::new(),
                error: Some(error_dto(error)),
            }
        }
    };
    let zip_path = work.join("remote-backup.zip");
    if let Err(error) = std::fs::write(&zip_path, &bytes) {
        return RestoreResultDto {
            ok: false,
            restored: false,
            target_backup: None,
            message: String::new(),
            error: Some(ErrorDto {
                code: codes::INTERNAL.to_string(),
                message_key: "error.webdav_write".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: Some(error.to_string()),
            }),
        };
    }
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
        let outcome =
            service.import_upstream_with_lifecycle(engine(), &zip_path, &work, now_epoch());
        return match outcome {
            Ok(report) => {
                let changed = report.status.changed_target()
                    || report.status == ImportStatus::AlreadyImported;
                RestoreResultDto {
                    ok: changed,
                    restored: changed,
                    target_backup: None,
                    message: report.user_summary,
                    error: None,
                }
            }
            Err(error) => RestoreResultDto {
                ok: false,
                restored: false,
                target_backup: None,
                message: String::new(),
                error: Some(error_dto(error)),
            },
        };
    }
    let unpacked = work.join("unpacked");
    if let Err(error) = application::extract_bundle_zip(&zip_path, &unpacked) {
        return RestoreResultDto {
            ok: false,
            restored: false,
            target_backup: None,
            message: String::new(),
            error: Some(error_dto(error)),
        };
    }
    // SR-03: lifecycle-aware restore; quiesce/reopen failures surface.
    let outcome = service.restore_with_lifecycle(engine(), &unpacked, &work);
    match outcome {
        Ok(report) => RestoreResultDto {
            ok: report.restored,
            restored: report.restored,
            target_backup: report.target_backup,
            message: report.message,
            error: None,
        },
        Err(error) => RestoreResultDto {
            ok: false,
            restored: false,
            target_backup: None,
            message: String::new(),
            error: Some(error_dto(error)),
        },
    }
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

/// `check_updates` — check the selected cores (empty = all built-ins).
pub async fn t16_check_updates(
    cores: Vec<String>,
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
    remember_update_flags(prerelease, proxy.clone());
    let service = update_service();
    let selected = if cores.is_empty() {
        application::BUILTIN_TARGETS
            .iter()
            .map(|c| (*c).to_string())
            .collect::<Vec<_>>()
    } else {
        cores
    };
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
            match service.check_app_update(prerelease, proxy.as_deref()).await {
                Ok(check) => checks.push(core_update_dto(check)),
                Err(error) => {
                    return UpdateReportDto {
                        ok: false,
                        checks,
                        error: Some(error_dto(error)),
                    }
                }
            }
            continue;
        }
        match service
            .check_core(&core, prerelease, proxy.as_deref())
            .await
        {
            Ok(check) => checks.push(core_update_dto(check)),
            Err(error) => {
                return UpdateReportDto {
                    ok: false,
                    checks,
                    error: Some(error_dto(error)),
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
    remember_update_flags(prerelease, proxy.clone());
    let service = update_service();
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
        let check = match service
            .check_core(&core, prerelease, proxy.as_deref())
            .await
        {
            Ok(check) => check,
            Err(error) => {
                return ApplyCoreResultDto {
                    ok: false,
                    applied,
                    skipped,
                    error: Some(error_dto(error)),
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
        match service.apply_core(&request, &token).await {
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
                    error: Some(error_dto(error)),
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
pub async fn t16_apply_app_update_spec() -> ExternalSpecDto {
    let service = update_service();
    let flags = last_update_flags();
    let check = match service
        .check_app_update(flags.prerelease, flags.proxy.as_deref())
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
                error: Some(error_dto(error)),
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
        proxy: flags.proxy.clone(),
    };
    let helper = service.app_layout().runner_exe();
    let token = CancellationToken::new();
    match service
        .app_update_spec_verified(
            &request,
            signature_url.as_deref(),
            verifier.as_ref(),
            helper,
            std::process::id(),
            &token,
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
            error: Some(error_dto(error)),
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

#[cfg(test)]
mod tests {
    use super::*;

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
            .block_on(t16_check_updates(vec!["xray".to_string()], false, true));
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
            .block_on(t16_check_updates(vec!["v2rayN".to_string()], false, false));
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
    fn dgst_parse_reads_sha256_line() {
        let text = "MD5= a\nSHA1= b\nSHA2-256= D004C392AABB\nSHA2-512= c\n";
        assert_eq!(
            application::parse_dgst_sha256(text).as_deref(),
            Some("d004c392aabb")
        );
    }
}
