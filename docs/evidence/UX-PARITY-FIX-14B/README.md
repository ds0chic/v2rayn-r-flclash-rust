# UX-PARITY-FIX-14B 证据

任务卡：`docs/tasks/FIX-14B.md`（来自 `docs/tasks/FIX-14.md` 登记、`repair-queue.md` 第 39 行「当前备份恢复另卡」）。
唯一用户流程：当前备份恢复 —— `file_selector` 选本地备份 zip → 恢复 → 重开使用。

## 改动文件（本卡）

- `apps/desktop/lib/features/backup/backup_picker.dart` — 新增 `BackupPicker` 注入缝、`FileSelectorBackupPicker`（`openFile` zip / `getDirectoryPath`）与 `backupPickerProvider`。
- `apps/desktop/lib/features/backup/backup_controller.dart` — 新增 `restoreFromArchive`（recognize → `t16_backup_import_upstream`）、`restoreFromDirectory`（`t16_backup_restore`）、`restoreListed`、`_refreshAfterRestore`；`restoreBundle` 成功后缀刷新。
- `apps/desktop/lib/features/backup/backup_and_restore_view.dart` — 移除手输「备份包目录」文本框，改「选择备份 ZIP 恢复」「选择备份包目录恢复」按钮；列表显示「时间 · N 项资源 · 路径」并支持一键恢复。
- `apps/desktop/test/fix14b_backup_restore_test.dart` — 新增 5 例。
- `apps/desktop/test/fix14_backup_activate_test.dart`、`apps/desktop/test/t16_backup_test.dart` — 恢复用例迁移到选择器注入。
- `compat/features.yaml` — F-BACKUP-002 追加 evidence/notes（未删行）。
- `docs/tasks/FIX-14B.md`、本目录。

未改：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`crates/**`、`features/{settings,profiles,subs,runtime,monitor,update,routing}/**`。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format lib/features/backup test/fix14b_backup_restore_test.dart test/t16_backup_test.dart test/fix14_backup_activate_test.dart` | 1 changed（`backup_controller.dart`），其余 0 |
| `flutter analyze lib/features/backup test/fix14b_backup_restore_test.dart test/t16_backup_test.dart test/fix14_backup_activate_test.dart` | No issues found |
| `flutter test test/fix14b_backup_restore_test.dart test/fix14_backup_activate_test.dart test/t16_backup_test.dart` | 14 passed / 0 failed |

## 新增 UI 用例（`apps/desktop/test/fix14b_backup_restore_test.dart`）

- `cancelled archive picker is a no-op`：选择器返回 null → 不调用 `backup_recognize`/`backup_import`，状态仍为「就绪」。
- `archive restore recognizes, imports and refreshes state`：选 `/tmp/good.zip` → `backup_recognize` → `backup_import`，并在恢复后重读 `get_core_versions`（刷新）。
- `directory restore routes through bundle restore and refreshes`：选 `/tmp/good-bundle` → `backup_restore`，恢复后刷新。
- `corrupted zip surfaces the error and keeps existing config`：`_FailingImportBridge` 注入失败 → 展示「本地恢复失败」+`E_FIELD_FORMAT`+「现有配置」。
- `listed backup shows time/size and restores on selection`：列出 `/tmp/backups` → 渲染「…项资源…」→ 点「恢复」→ `backup_restore:/tmp/backups`。

## 上游对照

冻结 `BackupAndRestoreViewModel.cs` `LocalRestore`（`:108`）：先校验 zip 含 `guiConfigs`，非法直接提示；`:129` 先备份当前配置；`:133/134` 关闭业务与 DB 句柄；`:137` 解压覆盖；`:150` 退出重启。窗口 `BackupAndRestoreView.xaml.cs:27/36` 用 `SaveFileDialog`/`OpenFileDialog`（`Zip|*.zip`）。
本卡以 `file_selector` 等价 `OpenFileDialog`（zip 类型组 + 取消返回 null），恢复动作复用 FIX-14 的 `quiesce`/`reopen`；`Shutdown(true)` 进程重启仍由 FIX-14 登记的 root-owned `RestoreLifecycle` 处理。

## 未完成 / 接口缺口

- 本地备份仍写 bundle 目录，未产出原版命名 `backup_*.zip`；缺 `zip_bundle` 本地导出 FRB。
- `t16_backup_list` DTO 无 bundle 路径/字节大小；列表只能显示时间与资源数，root 为空项无法一键恢复。
- 项目 bundle zip 若需本地恢复，缺本地解包入口（当前 zip 走 recognize+import_upstream）。
- 恢复后 profiles/settings 自动刷新属 root-owned `RestoreLifecycle`（另一卡）。
- WebDAV 远端为 FIX-14C，本卡未做。
- 未做原版真实窗口同夹具逐事件对照，保持 `implemented`。
