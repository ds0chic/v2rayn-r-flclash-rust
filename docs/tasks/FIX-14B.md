# FIX-14B — 当前备份恢复：file_selector 选择本地备份 → 恢复 → 重开使用

状态：`implemented`（UI 层本地恢复的 `file_selector` 选择/取消、识别/激活分发、bundle 目录恢复、列表时间/资源展示与一键恢复、失败结构化展示与恢复后重读 settings 已由本机 Flutter widget 测试覆盖；未做原版真实窗口同夹具逐事件对照，且本地 zip 产出/列表路径字段存在已登记接口缺口，故不写 `verified`）。

任务 ID：FIX-14B

本次唯一用户流程：在「备份与还原」窗口点「选择备份 ZIP 恢复」→ 系统文件选择器选本地备份 zip（取消不动作）→ 识别为可恢复备份后走 FIX-14 的 quiesce→激活→reopen 路径恢复；或点「选择备份包目录恢复」→ 选项目 bundle 目录 → 走 `t16_backup_restore`（quiesce→交换→reopen）；「列出备份」列出本地 bundle 的时间/资源数并可一键恢复；失败展示结构化错误且现有配置不变。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；FIX-14 卡（`docs/tasks/FIX-14.md`）、`docs/evidence/UX-PARITY-FIX-14/README.md`；审查结论见 `docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 39 行 FIX-14 行「当前备份恢复/WebDAV另卡」、`settings-report.md` SET-12/13/14。

对应 feature / action / layout ID：`F-BACKUP-001`、`F-BACKUP-002`、`ACT-WIN-007`、`LAY-BACKUP-001`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `v2rayN/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:92`（`LocalBackup`：整目录 ZIP，命名 `backup_yyyyMMddHHmmss.zip`）、`:108`（`LocalRestore`：先校验含 `guiConfigs`，非法直接提示）、`:129/133/134`（先备份当前配置，再 `AppExitAsync(false)`+`DisposeDbConnectionAsync()`）、`:137`（`ZipExtractToFile` 覆盖配置目录）、`:150`（`Shutdown(true)` 退出重启）；`v2rayN/v2rayN/Views/BackupAndRestoreView.xaml.cs:27`（`SaveFileDialog("Zip|*.zip")`）、`:36`（`OpenFileDialog("Zip|*.zip|All|*.*")`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：系统文件选择器返回的本地 `backup_*.zip` 路径，或项目 bundle 目录路径。`OpenFileDialog` 类型组 `zip`；目录选择用 `getDirectoryPath`。
- 输出：ZIP 恢复经 `t16_backup_recognize` 识别为可恢复后调用 `t16_backup_import_upstream`（FIX-14 的 quiesce→候选提交→激活 `guiNConfig.json`→reopen）；bundle 目录恢复调用 `t16_backup_restore`（FIX-14 的 quiesce→交换→reopen）；`BackupStatus` 展示结果。
- 错误：识别失败/非可恢复 ZIP/恢复失败展示结构化 `code / messageKey[: detail]`；提示「现有配置未修改」。
- 取消：选择器返回 `null` 即无动作，不改变任何状态、不调用任何备份桥接函数。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN、不监听端口、不触碰 10808。
- 持久化：沿用 FIX-14 的 SQLite/`guiNConfig.json`/资源回拷与回滚，本卡不新增存储路径。
- 生效：恢复成功后窗口重读 engine 拥有的 settings（`t16_webdav_config_get`/`t16_get_core_versions`），提示重开窗口查看；不会用恢复前的陈旧内存状态覆写恢复配置。

允许修改并实际修改的模块：`apps/desktop/lib/features/backup/backup_picker.dart`（新增 `BackupPicker` 注入缝 + `file_selector` 默认实现 + `backupPickerProvider`）、`apps/desktop/lib/features/backup/backup_controller.dart`（新增 `restoreFromArchive`/`restoreFromDirectory`/`restoreListed`/`_refreshAfterRestore`；`restoreBundle` 成功后缀刷新）、`apps/desktop/lib/features/backup/backup_and_restore_view.dart`（用选择器按钮替换手输「备份包目录」文本框；列表显示时间/资源数并支持一键恢复）、`apps/desktop/test/fix14b_backup_restore_test.dart`（新增）、`apps/desktop/test/fix14_backup_activate_test.dart`、`apps/desktop/test/t16_backup_test.dart`、`docs/evidence/UX-PARITY-FIX-14B/**`、本卡、`compat/features.yaml`（仅 F-BACKUP-002 追加 evidence/notes）。未新增 FRB 函数，未改 `frb_generated`、`lib/bridge/api/**`、`crates/**`。

禁止改变的已有行为：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/settings|profiles|subs|runtime|monitor|update|routing/**`、`crates/**`；未删除任何入口或降低分母；未改 WebDAV 远端语义（另卡 FIX-14C）。

测试夹具和原版预期：`SyntheticBridgePort`（含 `t16Calls`）+ 注入式 `BackupPicker` 假实现；损坏路径 `/tmp/broken` 与子类 `_FailingImportBridge` 做故障注入。原版预期：本地备份为 `backup_*.zip`（SaveFileDialog），本地恢复为 OpenFileDialog 选 zip，取消不动作，非法 zip 直接提示，恢复后配置与备份一致并重启。本卡在无 Rust 变更约束下以「ZIP 识别+上游激活」与「bundle 目录 restore」两条既有路径等价覆盖恢复动作，差异登记为接口缺口。

本次必须通过的命令/真实场景（实际执行结果）：
- `dart format lib/features/backup test/fix14b_backup_restore_test.dart test/t16_backup_test.dart test/fix14_backup_activate_test.dart`：1 changed（`backup_controller.dart`），其余 0。
- `flutter analyze lib/features/backup test/fix14b_backup_restore_test.dart test/t16_backup_test.dart test/fix14_backup_activate_test.dart`：No issues found。
- `flutter test test/fix14b_backup_restore_test.dart test/fix14_backup_activate_test.dart test/t16_backup_test.dart`：14 passed / 0 failed。
- 真实窗口与 Rust 侧未改，故未跑；`cargo` 门禁不在本卡范围。

证据文件位置：`docs/evidence/UX-PARITY-FIX-14B/README.md`、`docs/evidence/UX-PARITY-FIX-14B/observations.json`。

完成条件：选择本地 zip 用 `file_selector`（取消不动作），识别后经既有 quiesce→激活/reopen 路径恢复；bundle 目录经 `t16_backup_restore` 恢复；列表显示时间/资源数并可恢复；失败展示结构化错误且不改现有数据；恢复后窗口重读 settings；合成测试全绿。未做原版实机对照，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：F-BACKUP-001 本地备份当前写 bundle 目录（`create_local`），未生成原版命名 `backup_*.zip`；缺「创建 bundle 后 `zip_bundle` 落地为 `backup_<ts>.zip`」的本地导出 FRB 函数。需根代理重生成。
- 接口缺口（登记）：`t16_backup_list` 的 `BackupManifestDto` 不返回 bundle 路径与字节大小；列表只能显示 `created_at` 与 `resourceCount`，真实数据（root 为空）无法一键恢复，需 Rust 在 manifest/list DTO 补 `root`/`bytes`。当前对 root 为空项报「该备份未提供路径，无法直接恢复」。
- 接口缺口（登记）：项目自带 bundle 若以 zip 保存，本地恢复需要先解包再 `t16_backup_restore`（`extract_bundle_zip` 无本地 FRB 入口）；当前 zip 一律走 `recognize`+`import_upstream`（对含 `guiConfigs` 的原版 zip 正确）。若需支持项目 bundle zip，需新增本地解包入口或让 `restore` 接受 zip。
- 接口缺口（登记，root-owned RestoreLifecycle）：恢复后 profiles/settings 列表自动刷新仍由根代理统一（见 FIX-14 登记），本卡仅在备份窗口内重读 settings 并提示重开。

本轮实际结果：新增 `BackupPicker` 抽象与 `file_selector` 实现；`backup_controller.dart` 新增本地 ZIP 恢复（recognize→import_upstream）、bundle 目录恢复（restore）、列表项恢复、恢复后 `_refreshAfterRestore`；视图移除手输「备份包目录」文本框，改为「选择备份 ZIP 恢复」「选择备份包目录恢复」按钮，列表显示「时间 · N 项资源 · 路径」并可一键恢复。新增 `fix14b_backup_restore_test.dart` 5 例（取消无动作、ZIP 识别/导入/刷新、目录 restore/刷新、损坏 zip 结构化错误且保留现有配置、列表展示与一键恢复），并迁移 FIX-14/`t16_backup_test` 的恢复用例到选择器。合计 14/14 通过；`flutter analyze`（本卡范围）无问题。
