# UX-PARITY-FIX-14 证据

任务卡：`docs/tasks/FIX-14.md`。唯一流程：导入原版 ZIP → 配置实际生效 → 重开使用；恢复前 quiesce、交换后重载；子目录资源完整回拷；hash/错误/回滚含资源。

## 改动文件（本卡）

- `crates/application/src/backup_service.rs` — 递归资源收集；restore 回拷 config+资源并在失败时回滚 DB/config/资源；`import_upstream` 成功后 `activate_upstream_config` 写活动 `guiNConfig.json` 并重映射 id。
- `crates/application/src/engine.rs` — 新增 `AppEngine::quiesce()`（丢弃 SQLite 句柄）与 `reopen()`（重开存储并从盘上 config 重载 settings/active/revisions/templates/rule_mode）。
- `crates/bridge_api/src/api/t16.rs` — `t16_backup_restore` / `t16_backup_import_upstream` 包裹 quiesce → 交换 → reopen；无签名变化。
- `crates/application/tests/t16_backup.rs` — 新增 4 个用例。
- `apps/desktop/lib/features/backup/backup_controller.dart` — 成功提示报告「配置与资源已重载」「原版设置/活动节点已激活」。
- `apps/desktop/test/fix14_backup_activate_test.dart` — 新增导入/恢复成功提示与错误路径 widget 测试。
- `compat/features.yaml` — F-BACKUP-002 追加 evidence/notes（未删行）。
- `docs/tasks/FIX-14.md`、本目录。

未改：`main_shell.dart`、两处 `frb_generated`、`features/profiles|subs|runtime|monitor|update/**`、`crates/updater/**`、`crates/subscriptions/**`。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p application -p bridge_api` | ok |
| `cargo check -p application -p bridge_api --locked` | Finished（0 error） |
| `cargo test -p application --test t16_backup --locked` | 9 passed / 0 failed |
| `cargo test -p application --lib --locked` | 157 passed / 0 failed |
| `cargo test -p bridge_api --locked` | 43 passed / 0 failed |
| `cargo clippy -p application -p bridge_api --locked --lib` | Finished（本卡 0 warning） |
| `cargo clippy -p application --locked --test t16_backup` | Finished |
| `dart format --output=none --set-exit-if-changed lib/features/backup test/fix14_backup_activate_test.dart` | 0 changed |
| `flutter analyze` | 仅既有 `status_bar_view.dart` unused import（非本卡） |
| `flutter test test/fix14_backup_activate_test.dart` | 3 passed |
| `flutter test test/t16_backup_test.dart` | 6 passed |

`cargo clippy -p application --all-targets` 命中既有 `tests/fix13_tun_presocks_plan.rs` 的 `result_large_err`（FIX-13 文件，非本卡），故未作为本卡门禁。

## 新增 Rust 用例（`crates/application/tests/t16_backup.rs`）

- `nested_resources_roundtrip_and_restore_complete`：`custom/sub/node.json` + `pac/local.pac` 进入 manifest，恢复后逐字节一致。
- `tampered_resource_rejects_restore_without_damage`：资源 hash 不匹配 → 恢复拒绝，目标 DB/资源不变。
- `resource_copy_failure_rolls_back_database_and_config`：资源回拷被阻塞 → DB 从 `.bak` 回滚、config 回滚，Remark 仍为 `existing`。
- `upstream_import_activates_config_and_active_id`：导入 fixture 后活动 `guiNConfig.json` 的 `active_index_id`/`IndexId` = `derived_id("profile", "<fingerprint>:node-hk")`，`BrandNewRootItem`/`UIItem.CurrentTheme` 保留，`AppEngine::open` 读出同一 active。
- `live_engine_quiesce_restore_reopen_roundtrip`：目标 dir 已由 `AppEngine::open` 打开 DB；`quiesce` → `BackupService::restore`（交换）→ `reopen`，恢复后 DB 顶层行 Remark=`source`（非恢复前 `existing`）、config 未知键 `BrandNewRootItem` 存在，证明「当前打开 DB 的恢复路径」不损坏。

## 上游对照

冻结 `BackupAndRestoreViewModel.cs`：`LocalBackup` 先 `CopyDirectory` 再 `CreateFromDirectory`（整目录 ZIP，含子目录）；`LocalRestore` 先 `AppManager.Instance.AppExitAsync(false)` + `SQLiteHelper.Instance.DisposeDbConnectionAsync()` 关闭业务/DB 句柄，解包后 `AppManager.Instance.Shutdown(true)` 退出重启。本卡以 `AppEngine::quiesce`（丢 SQLite 句柄）→ 文件交换（含回滚）→ `AppEngine::reopen`（重载 settings/active/revisions）等价完成，未改内核/平台副作用。

## Wire DTO 衔接（FIX-04）

激活复用 `persistence::ConfigDocument`（`to_json_value` 保留 PascalCase 原文与未知键）与既有的 `upstream_config` meta，未重复造 wire DTO。FIX-04 的 `subscriptions` wire 与经 `import_from_path` 的候选流程保持一致。

## 未完成 / 接口缺口

- FIX-14B：项目自有 bundle 的本地恢复仍为手输目录，缺原版 `OpenFileDialog`/`backup_*.zip`。
- FIX-14C：WebDAV 远端 `backup.zip` 布局与项目 manifest 不互通；真实远端 TLS 未测。
- root-owned `RestoreLifecycle`：Flutter 侧恢复后未自动刷新 profiles/settings 列表；`ProfilesState.groupSubId` 当前组未持久化，重开需人工选组。
- 未做原版真实窗口同夹具逐事件对照，保持 `implemented`。
