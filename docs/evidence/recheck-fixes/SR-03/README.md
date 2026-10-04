# SR-03 修复证据（恢复/导入后的 RestoreLifecycle）

状态：`implemented`。Dart 合成 bridge/controller 用例通过；Rust 应用层用例已写入但本轮被非本卡范围的 `crates/updater` 编译错误阻塞，未运行，故不写 `verified`。

- 仓库 HEAD：`e9b8e74`。**注意：进入任务时工作树并非干净**，存在并发未提交改动（`crates/updater/**`、`crates/application/src/{speedtest,update_service}.rs`、`apps/desktop/lib/features/profiles/**` 等），其中 `crates/updater/src/install.rs:191` 有类型错误，属冻结范围，本卡不得修改。
- 冻结原版：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
- 缺口来源：`docs/evidence/parity-recheck-2026-10-04/settings.md` SR-03 全节。
- 未改宿主代理/自启/TUN，未占用 127.0.0.1:10808，未读/写用户凭据；夹具全部合成。

## 结论

- 恢复/导入现在走完整 RestoreLifecycle：`AppEngine::prepare_restore` 先停订阅 scheduler（有界 drain，防止换库期间写库）并停止活动 runtime 会话（idle 时跳过），再 quiesce；文件交换后 `reopen` 必被调用，换库后重开失败返回结构化错误，不再被忽略。
- `BackupService` 新增 `restore_with_lifecycle` / `import_upstream_with_lifecycle`，用 `finish_lifecycle` 合并「操作结果 + reopen 结果」：操作成功但 reopen 失败 → `error.restore_engine_reopen_failed`；两者都失败 → 保留操作错误并追加 reopen 说明。
- FRB `t16_backup_restore`、`t16_backup_import_upstream`、WebDAV 恢复（upstream 分支与项目 bundle 分支）全部改用生命周期函数，删除原先 `let _ = engine().quiesce()/reopen()` 的忽略写法。
- Dart `BackupController` 成功后重载 settings/profiles/routing/DNS/subs、按恢复前状态重启 scheduler（silent，不覆盖恢复文案）、`RuntimeController.resyncAfterRestore` 停旧会话并按活动节点 apply；失败分支保持旧 provider、如实报错，不出现「已重载」。
- 文案由「请重开窗口查看」改为「节点/分组/主题/运行会话已重载」，只报告已经发生的事实。

## 上游对照

- `work/.../ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:133` `AppExitAsync(false)`、`:134` `DisposeDbConnectionAsync()`、`:137` 解压覆盖、`:141` `RebootAsAdmin`、`:150` `Shutdown(true)`：原版以整进程重启让新进程自然重建 UI/配置/内核，旧运行状态不残留。
- 本仓库侧等价物：`prepare_restore`（停会话/调度器 + `quiesce`）→ 交换（`restore`/`import_upstream`，含 FIX-14/SR-02 资源与回滚）→ `reopen` → Dart 全量 provider reload + 按活动节点 apply。差异：不重启进程，改为进程内重开并显式重载长期状态；reopen 失败无法继续时返回结构化错误而非静默成功。
- 未做原版实机双窗口逐事件对照，故保持 `implemented`。

## 改动文件

- `crates/application/src/engine.rs`
  - 新增 `stop_sub_scheduler_blocking(timeout)`：`take` 调度器后 `stop()` 并等待 `is_finished()`（≤2s），确保无在途 tick。
  - 新增 `prepare_restore()`：停调度器 → 若 snapshot 非 `Stopped` 则 `stop_runtime()?` → `quiesce()?`。
- `crates/application/src/backup_service.rs`
  - 新增 `restore_with_lifecycle`、`import_upstream_with_lifecycle`。
  - 新增 `finish_lifecycle`、`reopen_failure`（结构化 reopen 失败，保留原操作错误 detail）。
- `crates/bridge_api/src/api/t16.rs`：`t16_backup_restore`、`t16_backup_import_upstream`、`t16_webdav_restore` 两个分支改用生命周期函数。
- `apps/desktop/lib/features/runtime/runtime_controller.dart`：新增 `resyncAfterRestore()`（stop → 有活动节点则 applyActive）。
- `apps/desktop/lib/features/subs/subs_controller.dart`：`startScheduler`/`stopScheduler` 增加 `silent` 参数。
- `apps/desktop/lib/features/backup/backup_controller.dart`：恢复方法改 async；新增 `_schedulerWasRunning`、`_reloadProviders`、`_restartScheduler`、`_refreshWindowState`、`_resyncRuntime`；成功/失败分支按合同处理；文案更新。
- `crates/application/tests/t16_backup.rs`：新增 3 用例（见下）。
- `apps/desktop/test/sr03_restore_lifecycle_test.dart`：新增 4 用例。
- `apps/desktop/test/fix14_backup_activate_test.dart`、`apps/desktop/test/fix14c_webdav_backup_test.dart`：成功文案断言改为「已重载」。
- `compat/features.yaml`：F-BACKUP-002/003 追加 SR-03 evidence 行与 notes（仅追加）。
- `docs/tasks/SR-03.md`、本目录。

## 实际命令与结果

```powershell
# Dart：格式、分析、测试
& 'C:\Users\Colby\toolchains\flutter\bin\dart.bat' format lib/features/backup/backup_controller.dart lib/features/runtime/runtime_controller.dart lib/features/subs/subs_controller.dart test/sr03_restore_lifecycle_test.dart
# Formatted

& 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat' analyze
# No issues found!

& 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat' test test/sr03_restore_lifecycle_test.dart
# 4 passed / 0 failed

& 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat' test test/sr02_04_backup_import_test.dart test/t16_backup_test.dart test/fix14b_backup_restore_test.dart test/fix14c_webdav_backup_test.dart test/fix14_backup_activate_test.dart test/sr03_restore_lifecycle_test.dart
# 23 passed / 0 failed

# Rust：本卡文件格式检查通过；用例执行被外部阻塞
& 'C:\Users\Colby\.cargo\bin\rustfmt.exe' --edition 2021 crates/application/src/engine.rs crates/application/src/backup_service.rs crates/application/tests/t16_backup.rs crates/bridge_api/src/api/t16.rs
# exit 0，已按 rustfmt 格式化
& 'C:\Users\Colby\.cargo\bin\cargo.exe' fmt -p application -p bridge_api -- --check
# 修改前仅本卡两处 diff，格式化后应无 diff（未再复跑完整 check）

& 'C:\Users\Colby\.cargo\bin\cargo.exe' test -p application --test t16_backup --locked
# FAILED（连续两次）：crates/updater/src/install.rs:191 E0308
#   expected `&Path`, found `&String` in normalize_lexically(&self.keep_name)
# updater 属冻结范围，未修改；本卡 Rust 用例未能编译/运行。
```

### 新增 Rust 用例（已写入，本轮未运行）

- `restore_lifecycle_reloads_settings_and_active`：合成 bundle（config 含 `active_index_id=restored-active`、`desired_revision=7`）→ `restore_with_lifecycle`（`NullRuntimeClient`）→ 断言 `engine.active_profile()==restored-active` 且 `desired_revision()==7`（成功路径 reopen 后 settings/active 一致）。
- `restore_lifecycle_propagates_stop_failure_before_exchange`：注入 `StopFailingRuntime`（snapshot=Running，stop=Err）→ `restore_with_lifecycle` 必须 Err 且 live DB 仍为 `existing`（换库前中止）。
- `engine_reopen_propagates_corrupt_config`：quiesce 后把 `guiNConfig.json` 写坏 → `reopen()` 必须 Err（损坏场景失败传播）。

### 新增 Dart 用例

- `sr03_restore_lifecycle_test.dart`：4 用例——成功恢复断言 settings/profiles/subs 重载计数增长、scheduler 重启、runtime 先 stop 后按活动节点 apply；失败恢复断言 error 文案且不含「已重载」、会话被重新 apply；原版 ZIP 导入走同一生命周期；无活动节点时保持 stopped、绝不 apply。

## 未完成 / 边界

- **blocker**：`crates/updater/src/install.rs:191` 的 `&String`/`&Path` 类型错误来自并发未提交改动，导致 `application` 无法编译，`cargo test -p application --test t16_backup` 连续两次失败；sealed 范围禁止本卡修改 updater。解除后需复跑 Rust 用例。
- 未在真实 Windows 窗口 + 真实 FRB 下肉眼对照原版恢复窗口；未启动真实内核。
- reopen 失败采用「如实报错、保留已落盘的恢复结果」，未对成功后的 reopen 失败做整库回滚（换库已成功且回滚后同样面临重开失败；下一个进程启动会加载恢复态）。这是有意的语义选择，登记在此。
- `quiesce` 自身失败仅在锁中毒时发生，未构造针对性注入；失败传播由 stop 阶段用例与损坏 reopen 用例覆盖。
