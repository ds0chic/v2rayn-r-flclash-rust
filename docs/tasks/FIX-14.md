# FIX-14 — 导入原版 ZIP → 重开并使用：配置激活与资源完整恢复

状态：`implemented`（合成原版 ZIP/目录的导入、激活、递归资源回拷、哈希/回滚与重开路径已由本机 Rust/Flutter 合成测试覆盖；未做原版真实窗口同夹具逐事件对照，故不写 `verified`）。

任务 ID：FIX-14

本次唯一用户流程：在「备份与还原」窗口导入原版 v2rayN ZIP（含 `guiConfigs/`），使原版 `guiNConfig.json` 实际成为活动 settings/active/group，而不只是写进 DB 的 `meta.upstream_config`；恢复前 quiesce 业务/关闭 DB 句柄，文件交换后重载 Engine 状态；子目录资源完整收集与回拷，hash/错误/回滚含资源，后续保存不覆写恢复配置。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `b638821`。审查结论见 `docs/evidence/parity-review-2026-10-03/repair-queue.md` FIX-14 行、`settings-report.md` SET-12/13/14、`all-items.json` 的 `F-BACKUP-001..004`/`ACT-BACKUP-001/002/004/005`/`ACT-WIN-007`/`LAY-BACKUP-001`。

对应 feature / action / layout ID：`F-BACKUP-001`、`F-BACKUP-002`、`F-BACKUP-004`、`ACT-BACKUP-001`、`ACT-BACKUP-002`、`ACT-WIN-007`、`LAY-BACKUP-001`；字段 `FLD-CFG-001`（IndexId）、`FLD-CFG-116`（RoutingIndexId）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `v2rayN/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:92`（`LocalBackup`：`FileUtils.CreateFromDirectory` 整目录 ZIP）、`:108`（`LocalRestore`：校验含 `guiConfigs`）、`:133/134`（`AppManager.Instance.AppExitAsync(false)` + `SQLiteHelper.Instance.DisposeDbConnectionAsync()`）、`:150`（`AppManager.Instance.Shutdown(true)`）、`ServiceLib/Handler/ConfigHandler.cs`（Restore/备份段）、`ServiceLib/Manager/AppManager.cs`（`MigrateProfileExtra`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：原版导出的 `backup_*.zip` 或解包目录（含 `guiConfigs/guiNConfig.json`、`guiNDB.db` 及子目录资源）；本项目 `BackupService` 的 bundle 根目录。
- 输出：候选库提交后的 `ImportReport`；`guiNConfig.json` 被上游配置覆盖并注入引擎 meta（`IndexId`/`SubIndexId` 重映射、`active_index_id`）；`RestoreReport`。
- 错误：非法 ZIP/缺 `guiConfigs`/hash 不匹配在写库前拒绝；资源回拷失败时回滚已提交的 DB/config/资源，现有数据不变。
- 取消：窗口关闭即取消；导入/恢复为同步桥接调用，无半提交。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN、不监听端口、不触碰 10808。
- 持久化：SQLite `guiNDB.db` + `guiNConfig.json` + 递归资源文件；`meta.upstream_config` 保留原文。
- 生效：`AppEngine::quiesce`（丢弃 SQLite 句柄）→ 文件交换 → `AppEngine::reopen`（从盘上 config 重载 settings/active/revisions/templates/rule_mode）。

允许修改并实际修改的模块：`crates/application/src/backup_service.rs`、`crates/application/src/engine.rs`（仅新增 `quiesce`/`reopen`/`lock_error`，未动 FIX-07/10/11 语义）、`crates/bridge_api/src/api/t16.rs`（仅调整 `t16_backup_restore`/`t16_backup_import_upstream` 调用序，无签名变化）、`crates/application/tests/t16_backup.rs`、`apps/desktop/lib/features/backup/backup_controller.dart`、`apps/desktop/test/fix14_backup_activate_test.dart`、`docs/evidence/UX-PARITY-FIX-14/**`、本卡、`compat/features.yaml`（仅 F-BACKUP-002 追加 evidence/notes）。未新增 FRB 函数，未改 `frb_generated`。

禁止改变的已有行为：`main_shell.dart`、两处 `frb_generated`、`features/profiles/**`、`features/subs/**`、`features/runtime/**`、`features/monitor/**`、`features/update/**`、`crates/updater/**`、`crates/subscriptions/**`；未删除任何入口或降低分母。

测试夹具和原版预期：合成原版 ZIP/目录（`fixtures/synthetic/upstream-v2`）+ 合成嵌套资源目录；全部在 tempdir，无真实订阅 URL/凭据。原版预期：整目录打包（含子目录）、恢复前关闭业务与 DB 句柄、恢复后重启/重载、恢复配置成为活动配置。

本次必须通过的命令/真实场景：
- `cargo fmt -p application -p bridge_api`
- `cargo check -p application -p bridge_api --locked`
- `cargo test -p application --test t16_backup --locked`
- `cargo test -p application --lib --locked`
- `cargo test -p bridge_api --locked`
- `cargo clippy -p application -p bridge_api --locked --lib`；`cargo clippy -p application --locked --test t16_backup`
- `dart format --output=none --set-exit-if-changed lib/features/backup test/fix14_backup_activate_test.dart`；`flutter analyze`；`flutter test test/fix14_backup_activate_test.dart test/t16_backup_test.dart`
- 真实窗口若环境抖动记录待根代理统一跑；证据存 `docs/evidence/UX-PARITY-FIX-14/`。

证据文件位置：`docs/evidence/UX-PARITY-FIX-14/README.md`、`observations.json`。

完成条件：导入原版 ZIP 后活动 settings/active 生效且未知键保留；递归资源收集/回拷完整；hash/错误/回滚含资源；恢复前 quiesce、交换后 reopen，后续保存不覆写；wire DTO 衔接复用既有 `ConfigDocument`（未重复造）。合成测试全绿。未做原版实机逐事件对照，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记，FIX-14B）：当前项目自有 bundle 的「本地恢复」UI 仍为手输目录；原版 `SaveFileDialog`/`OpenFileDialog` 与 `backup_*.zip` 命名未接。属另卡。
- 接口缺口（登记，FIX-14C）：WebDAV 远端 `backup.zip` 布局与项目 manifest bundle 不互通；真实远端 TLS 未测。属另卡。
- 接口缺口（登记，root-owned RestoreLifecycle）：原版以 `Shutdown(true)` 退出重启；本卡以进程内 `quiesce`+`reopen` 等价实现 DB/config/资源闭环，但 Flutter 侧（`main_shell`/app bootstrap、profiles/settings 列表刷新）未接入「恢复后重载」通知，`ProfilesState.groupSubId` 当前组选择未持久化，重开后需人工选组。建议由根代理冻结 `RestoreLifecycle` 接口并统一刷新入口。
- 接口缺口（登记）：原版 `IndexId`/`SubIndexId` 与当前 `active_index_id` 两套状态已在导入时映射；若后续需要「当前组」稳定持久化，缺共享 getter/命令通道（关联 FIX-06）。

本轮实际结果：`BackupService::collect_resources` 改为递归（含子目录、排除 `.work`/tmp/DB/config/manifest/wal/shm）；`restore` 在候选库提交后回拷 config + 全部资源，任一失败回滚 DB（`.bak`）、config 与资源；`import_upstream` 成功后 `activate_upstream_config` 把上游配置写入活动 `guiNConfig.json` 并重映射 `IndexId`/`SubIndexId`/`active_index_id`；`AppEngine::quiesce/reopen` 支撑交换前后句柄关闭与状态重载；bridge 两个入口包裹 quiesce→交换→reopen。`t16_backup` 9/9、`application --lib` 157/157、`bridge_api` 43/43、`fix14_backup_activate_test` 3/3、`t16_backup_test` 6/6 全绿。`application --all-targets` clippy 仅 `tests/fix13_tun_presocks_plan.rs` 既有 `result_large_err` 报错（非本卡文件）；`flutter analyze` 仅 `lib/app/shell/status_bar_view.dart` 既有 unused import（非本卡文件）。
