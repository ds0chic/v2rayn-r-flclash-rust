# RE-PROF-01/05/02 + R-01 修复证据

日期：2026-10-04。开始 HEAD `d27eff0`（工作树干净）。冻结对照 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

状态：`implemented`。控制台针对性 widget/controller 测试通过；未跑真实 Windows 窗口端到端与原版实机双窗口，不写 `verified`。

本代理未启动内核、未监听端口（除既有合成桥）、未改宿主系统代理/TUN/路由/注册表、未读用户凭据、未 git add/commit、未改 `work/`、`outputs/`。

## 改动文件

- `apps/desktop/lib/features/profiles/profiles_controller.dart`
  - `newDraft` 写入 `state.groupSubId`（对齐上游 `MainWindowViewModel.AddServerAsync` 的 `ProfileItem{Subid=_config.SubIndexId}`）。
  - `_recompute` 将 `selected` 与新的 `visible` 求交（对齐 `ProfilesViewModel.RefreshServersBiz:361-379`），覆盖 `setGroupSubId`/`setFilter`/`reload`/排序/移动等所有调用点。
- `apps/desktop/lib/features/subs/subs_actions.dart`
  - `importFromClipboard` 与共享 `importShareText`（扫码/粘贴复用）在命令开始时快照 `groupSubId`，作为 `subid` 传给 `bridge.importFromText`，并把快照传给 `persistImportedProfiles`；异步解析期间换组不漂移。
- `apps/desktop/lib/features/subs/import_persistence.dart`
  - `persistImportedProfiles(..., {String? subid})`：返回 DTO `subid` 为空且快照非空时补快照组再 `saveImportedProfile`；空/无快照保持无分组。
- `apps/desktop/lib/features/profiles/profile_actions.dart`
  - `editSelectedProfile`：保存成功且对象是活动节点时 `applyAfterEditIfActive`。
  - `deleteSelectedProfiles`：删除集合含活动 ID 时走 `reconcileActiveAfterRemoval`。
  - 新增 `nextActiveAfterRemoval`（对齐 `ConfigHandler.SetDefaultServer:427-445`：当前可见 `Port>0` → 全库 `Port>0` → 无候选清空不 apply）。
  - `activateProfileDetailed` + `activateProfileById`（保留 bool 兼容 main_shell/托盘），`setActiveSelected` 分开反馈持久化与 apply 失败。
- `apps/desktop/lib/features/runtime/runtime_controller.dart`
  - 新增 `reload()`：忙碌保护 + refresh 最新 desired revision + 有活动节点时 `applyActive`。
- 测试：`test/recheck01_group_inheritance_test.dart`、`test/recheck01_import_snapshot_widget_test.dart`、`test/recheck02_active_apply_test.dart`、`test/recheck05_hidden_selection_test.dart`、`test/recheck_r01_runtime_reload_test.dart`、`test/support/counting_runtime_bridge.dart`。
- `docs/tasks/RE-PROF-01-05-02.md`、本目录。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format <11 改动文件>` | 5 changed，正常 |
| `flutter analyze` | No issues found |
| `flutter test test/recheck01_group_inheritance_test.dart test/recheck05_hidden_selection_test.dart test/recheck_r01_runtime_reload_test.dart` | +9 通过 |
| `flutter test test/recheck01_import_snapshot_widget_test.dart` | +1 通过 |
| `flutter test test/recheck02_active_apply_test.dart` | +4 通过 |
| `flutter test test/t06a_controller_test.dart test/context_menu_move_test.dart test/fix10b_profile_order_test.dart` | +10 通过（回归） |
| `flutter test test/ux_parity_fix05_scan_test.dart test/t09_import_export_test.dart` | +6 通过（回归） |

未运行：真实 Windows 窗口、`flutter build windows`、全量 `flutter test`、原版实机双窗口。

## 上游对照结论

- RE-PROF-01：`AddServerAsync:449-455` 建对象即带 `_config.SubIndexId`；`:497`/`:547` 剪贴板与扫码把同一个当前组传给 `AddBatchServers`。修复后 `newDraft` 与两个导入入口都继承命令开始时的当前组；`import_persistence` 防御补空。
- RE-PROF-05：`RefreshServersBiz:361-379` 用新可见列表重建选择（`_pendingSelectIndexId` → `_config.IndexId` → 首项）。本实现不复制默认选中项逻辑，但求交清除隐藏选择，满足“批量入口只作用可见选中”。命令上下文快照（FIX-01）仍独立生效。
- RE-PROF-02：`EditServerAsync:494-501` 与 `RemoveServerAsync:515-527` 在 `ret==true`/删除含活动后 `Reload()`；`ConfigHandler.SetDefaultServer:427-445` 回退当前列表 `Port>0` 再全库 `Port>0`。已实现等价的 apply + 回退与清空分支，并把持久化/应用失败分开反馈。
- R-01：`MainWindow.xaml.cs:233` F5 与 `MainWindowViewModel` 菜单共用 `Reload()`。已提供共享 `RuntimeController.reload()`，但未改 `main_shell.dart`，故 ACT-MAIN-035 仍是 `preserved_only`；接线补丁见下。

## main_shell.dart 接线补丁（禁止本卡修改，交界面负责人）

1. F5（`apps/desktop/lib/app/shell/main_shell.dart:164-168`）把占位提示替换为共享用例：
```dart
const SingleActivator(LogicalKeyboardKey.f5): () =>
    _guarded(
      ref,
      () => ref.read(runtimeControllerProvider.notifier).reload(),
    ),
```
2. 菜单 `_onMenuActionId`（同文件 `297-428` 的 `switch (actionId)`，`default:` 之前）新增：
```dart
      case 'ACT-MAIN-035':
        ref.read(runtimeControllerProvider.notifier).reload();
```
3. 解除保留态（`apps/desktop/lib/app/menu/main_menu.dart:156-163`）：删除该条目的 `preservedOnly: true`（并把注释改为已接线）。`runtimeControllerProvider` 已在 main_shell 导入。

补丁后 ACT-MAIN-035 可从菜单与 F5 共用 `reload()`；本卡不含该改动，故台账状态保持 `preserved_only`。

## 未完成 / 接口缺口 / 下一步前置

- 真实 Windows 窗口验证（隔离 data dir、合成节点、不启动内核）未跑；建议对 RE-PROF-01（普通/TUIC/粘贴/扫码四种入口 + 重开）和 RE-PROF-05（切组/过滤后批量目标）各复跑一次。
- R-01 需上述 `main_shell.dart`/`main_menu.dart` 接线，且需要真实运行验证 apply 效果。
- RE-PROF-02 的运行效果需在批准的隔离环境用 ≥11808 空闲端口实测，未在本回合执行。
- `editSelectedProfile` 的端到端 apply 通过 `applyAfterEditIfActive` 单测覆盖；未驱动真实编辑器对话框。
