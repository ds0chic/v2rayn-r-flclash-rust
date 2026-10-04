# R3-PROF-01..04 修复证据

日期：2026-10-04。起始 HEAD `6699c31`（工作树干净）。冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`（UP）。本卡只读生产源码/复核报告，未改 `work/`、`outputs/`；未 commit。所有测试为合成 `SyntheticBridgePort`（RFC 5737 文档地址、无网络），未占用 127.0.0.1:10808，未改系统代理/注册表/路由/TUN，未杀外部进程。

## 改动范围（仅允许模块）

- `apps/desktop/lib/features/profiles/profiles_models.dart`：`applySort` 对 `DelayVal`/`SpeedVal` 的 `<=0`（失败/未测）在两向沉底（对齐 `ConfigHandler.cs:1087-1105`）。
- `apps/desktop/lib/features/profiles/profiles_controller.dart`：
  - `ProfilesState.primaryId` 独立主行 + `copyWith`；选择方法（`selectRow`/`selectRange`/`selectAll`/`navigateSelection`/`handleRightTap`）维护；`_recompute` 隐藏时清理。
  - `isVisibleTarget` / `restoreContextTargets`（改为要求 `state.visible`，隐藏对象拒绝）。
  - `sortBy` 整组写序（`_groupScope`，忽略文字过滤，含隐藏行）；`_persistOrder({ids})`；`sortByResult` 同步 `state.sort`。
  - `readKeepOlderDedupl` / `DedupOutcome` / `removeDuplicateProfilesDetailed`（设置读取、`activeRemoved`、失败区分），保留 int 包装。
- `apps/desktop/lib/features/profiles/profile_actions.dart`：`resolveSingleTarget`；编辑/分享/设活动/重命名/完整配置导出接入 `targetId`；新增 `shareProfileQr`（主行分享，替代 subs 层单选入口）。
- `apps/desktop/lib/features/profiles/profiles_table.dart`：菜单命令消费 `command.primaryId`；`_usesPrimaryTarget` 精确限定可见性校验；`_removeDuplicate` 用 detailed 结果并触发活动回退/真实错误。
- 测试：`test/recheck_r3_prof_controller_test.dart`、`test/recheck_r3_prof_menu_target_test.dart`、`test/recheck_r3_prof04_dedup_widget_test.dart`；`test/support/profiles_harness.dart` 增加可选 `bridge`/`container` 注入。

未改：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`bridge/api/**`、`features/subs|runtime|settings|monitor|update|backup|routing/**`、`crates/**`。FIX-10/10B 排序写链与选中集合语义未改；RE-PROF-13/14 未回归。

## 实际命令与结果

工作目录 `apps/desktop`，Flutter `C:\Users\Colby\toolchains\flutter\bin\flutter.bat`。

| 命令 | 结果 |
|---|---|
| `dart format`（改动文件） | 0/已格式化，无失败 |
| `flutter analyze` | `No issues found!` |
| `flutter test test/recheck_r3_prof_controller_test.dart` | exit 0，8/8（主行、隐藏拒绝、两向沉底、结果排序 reload 稳定、整组写序、keep-older 设置、失败码、activeRemoved） |
| `flutter test test/recheck_r3_prof_menu_target_test.dart` | exit 0，1/1（多选主行=指针行；导出读捕获目标；隐藏拒绝）。首跑偶发 `did not complete`（flutter_tester 已知 flake），重试通过 |
| `flutter test test/recheck_r3_prof04_dedup_widget_test.dart` | exit 0，1/1（活动回退到 live 节点；删除失败显示 `E_DELETE_FAILED`） |
| `flutter test test/reprof04_sort_readback_test.dart` | exit 0，5/5（FIX-10B 读回/方向/失败提示不回归） |
| `flutter test test/re_prof_06_scope_test.dart` | exit 0，3/3（去重/移除无效范围不回归） |
| `flutter test test/fix10_dedup_test.dart` | exit 0，4/4（纯函数不回归） |
| `flutter test test/t15b_speedtest_test.dart` | exit 0，7/7 |
| `flutter test test/context_menu_move_test.dart` | exit 0，3/3 |
| `flutter test test/ux_parity_fix01_command_context_test.dart` | exit 0，3/3 |
| `flutter test test/re_prof_08_export_udp_test.dart` | exit 0，5/5（导出完整配置路径适配 `targetId`） |
| `flutter test test/t05_profiles_ui_test.dart` | exit 0，1/1 |
| `flutter test test/ux_parity_fix01_gen_group_guard_test.dart` | exit 0，1/1（修正 `_usesPrimaryTarget` 后组上下文命令不被误拒） |
| `flutter test test/context_menu_model_test.dart` | exit 0，2/2 |
| `flutter test test/re_prof_14_autofit_test.dart`、`re_prof_14_drag_enabled_test.dart`、`re_prof_14_drag_disabled_test.dart` | exit 0，3/3（RE-PROF-14 不回归） |
| `flutter test test/re_prof_13_active_marker_test.dart` | exit 0，1/1（首跑偶发 did not complete，重试通过） |
| `flutter test test/recheck05_hidden_selection_test.dart` | exit 0，2/2 |
| `flutter test test/profiles_share_shortcut_test.dart` | exit 0，1/1（Ctrl+C 批量 / Ctrl+F QR） |
| `flutter test test/profiles_pointer_test.dart` | exit 0，1/1 |
| `flutter test test/profiles_filter_test.dart` | exit 0，1/1（首跑偶发 did not complete，单独重跑通过） |
| `flutter test test/profiles_share_shortcut_empty_test.dart` | exit 0，1/1（单独重跑通过） |

已知环境 flake：`test/profiles_keyboard_test.dart` 在本机 Flutter 3.47.5 `flutter_tester` 上稳定 `did not complete`/`0xC0000005`，与 T12a/T15a 记录的已知 flutter_tester 崩溃一致（非本卡回归；同进程多文件后更易触发）。

## 上游对照结论

- R3-PROF-01：UP 单对象命令取 `SelectedProfile`（分享 `ProfilesViewModel.cs:593-607`）；现实现以独立 `primaryId`/捕获 `CommandContext.primaryId` 对齐，批量集合不折叠。
- R3-PROF-02：UP 命令只作用于打开时对象；现实现 `restoreContextTargets` 仅接受仍可见目标，隐藏/换组→拒绝提示；完整配置导出改用捕获目标（不读 live 选择）。
- R3-PROF-03：UP `ConfigHandler.SortServers:1004-1006` 整组（`ProfileModels(subId, "")`）且 `:1087-1105` 对 Delay/Speed `<=0` 补 `maxSort` 沉底；现实现一致，并修复 `sortByResult` → reload 回退。
- R3-PROF-04：UP `ConfigHandler.DedupServerList:1166-1168` 在 `!KeepOlderDedupl` 时 reverse；`ProfilesViewModel.cs:537-541` 删除后 Refresh+Reload；现实现读取设置、活动回退复用 RE-PROF-02、失败显示真实错误。

## 未完成 / 接口缺口

- 未做真实 Windows 窗口 + FRB 及真 SQLite 重开的端到端对照；因此状态保持 `implemented`，不写 `verified`。
- `features/subs/**` 的 `shareProfilesQr` 仍单选；profiles 层 `shareProfileQr` 承接主行分享，未在共享层收敛。
- `engine.delete_profiles` 不清理 active，DB 级一致性依赖 UI 回退；如需后端原子清理需新用例契约。
- 去重确认对话框弹出后到确认之间的换组/过滤未做组快照；仅保证菜单命令上下文不漂移。

## 下一步前置

在隔离 Windows 窗口用合成节点复跑同一批 UI 入口（多选右键命令目标、隐藏对象拒绝、整组排序写序、去重活动回退），并补真 SQLite 重开；上述缺口需新增用例契约后再动 `crates/` 或 `features/subs/`。
