# R3-SET-04 — 恢复后重载并重注册 GlobalHotkeys（P1）

状态：`implemented`（Dart fake registrar 覆盖 K→恢复 J→立即 J→打开/取消仍 J；未注册真实 OS 热键）。

任务 ID：R3-SET-04

本次唯一用户流程：已注册显示窗口组合 K，恢复含显示窗口组合 J 的备份；恢复后立即使用 J 生效；打开/取消热键窗口后仍为 J。

前置任务及已验证证据：冻结版；开始 HEAD `6699c31`；复核见 `docs/evidence/parity-recheck-2026-10-04/round3-settings.md` R3-SET-04。

上游对照：`U/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:141/150`、`U/v2rayN/Manager/HotkeyManager.cs:25`。

对应 feature / field / action：ACT-HOTKEY-006..008；`GlobalHotkeys`、`KeyEventItem`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：恢复成功后 Dart 重载 settings 文档。
- 输出：`HotkeyController.reloadFromSettings()` 读取恢复后的 `GlobalHotkeys` 并 `registerAll()`。
- 错误：注册冲突如实上报（沿用现有 conflicts）。
- 权限：仅在本机进程内注册；测试用 fake registrar，不注册真实 OS 热键。

允许修改的模块：`apps/desktop/lib/features/backup/backup_controller.dart`、`apps/desktop/lib/features/settings/hotkeys.dart`、`apps/desktop/test/**`、本卡、证据目录、compat 台账（仅追加）。

禁止改变的已有行为：不改 `global_hotkey_window.dart`；不改 `bridge/api/**`；真实 OS 热键不注册。

测试夹具和原版预期：fake `HotkeyRegistrar` 记录注册组合；合成 `GlobalHotkeys`（K=54、J=53）。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed <changed files>`
- `flutter analyze`
- `flutter test test/r3_set_restore_test.dart`
- `flutter test test/sr05_sr06_hotkey_test.dart`

证据文件位置：`docs/evidence/recheck-fixes/R3-SET-01-04/`。

完成条件：恢复后 registrar 立即注册 J；open→cancel 后仍注册 J；未安装 shell dispatcher 的纯 widget/测试环境不触碰原生插件。门禁通过。

接口缺口（登记）：`_reloadProviders` 之外若存在其它恢复入口（如未来新增），需复用 `reloadFromSettings`；真实 OS 热键组合的网络/系统级验证仍待专项。
