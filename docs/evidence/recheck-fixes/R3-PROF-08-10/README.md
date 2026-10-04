# R3-PROF-08 / R3-PROF-10 证据

日期：2026-10-04；开始应用 HEAD `cd141f5`（工作树干净）。冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

证据边界：本轮只运行合成 widget/controller 测试；未启动真实内核/FRB，未联网，未读用户凭据，未碰 10808，未改系统代理/注册表/TUN，未杀外部进程。因此状态写 `implemented` 而非 `verified`。

## R3-PROF-08（选择器列 / 默认组 / 键鼠 / 搜索 / 可见确认）

改动：
- `lib/bridge/bridge_port.dart`：新增 `joinPickerRows(candidates, results, subItems)`，把候选 `ProfileDto` 与 `ProfileExItem` 测速结果、订阅备注 join 成 `ProfileSummary`（订阅列 = `SubRemarks`，无则 `subid`），复用 `applySpeedTestOverlay`。
- `lib/features/profiles/group_editor_dialog.dart`：
  - 选择器行改为 `ProfileSummary`；延迟/速度列复用主表 `defaultProfileColumns()` 的 `DelayVal/SpeedVal` 显示（失败/未测 `-`/`失败`），默认序用 `orderByPersistedSort`（`ProfileExItem.Sort`），表头排序复用 `applySort`（失败/未测两向沉底）。
  - 默认组 = 主窗口当前组：`showNodePicker` 增 `currentGroupSubId`；生产由 `_GroupEditorDialogState` 经 `ProviderScope` 读 `profilesControllerProvider.groupSubId` 与 `bridgePortProvider.speedTestResults()`，无 scope 回退。
  - 搜索只匹配 remarks/address（移除端口）。
  - 键鼠合同：`CallbackShortcuts` Ctrl+A 全选可见、Enter/OK 确认、Esc 取消；手写双击检测（首次点击即时选中，300ms 内同行二次点击确认），避免 `onDoubleTap` 延迟单击。
  - 换组/过滤剪枝隐藏选择；确认结果与「已选 X/Y」只含可见项。

对应上游：`ProfilesSelectViewModel.cs:182-208/158-180`、`ProfilesSelectWindow.xaml.cs:79/93-122/144-151`。

## R3-PROF-10（默认选中 / Esc / 主行导航）

改动：
- `lib/features/profiles/table_actions.dart`：`extendSelection(rows, current, anchorId, id)` 使用显式锚点。
- `lib/features/profiles/profiles_controller.dart`：
  - 新增 `_selectDefaultRow`（pending → active → first，上游 `ProfilesViewModel.cs:361-379`），在 `setGroupSubId`（非空组）与新增 `saveDraft`（`draft.indexId` 为空）调用。
  - `selectRow(shift)` 以 `primaryId` 为锚并保持主行；`navigateSelection` 从 `primaryId` 推进。
  - `emitAction(escape)` 只 `cancelSpeedTest()` 并保留选择，记录 `escape`（上游 `ProfilesView.xaml.cs:289-291`）。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter analyze` | exit 0，No issues |
| `flutter test` 新 picker 5 文件 + `recheck_r3_prof10_default_selection_test.dart` | 全绿（prof10 6/6） |
| `flutter test` `recheck_reprof11_picker_*`（6）+ `reprof10_group_preview_test` | 16 断言全绿 |
| `flutter test` `table_actions_test` / `profiles_select_basic_test` / `t17_table_keyboard_test` / `t15b_speedtest_test` / `recheck_r3_prof_controller_test` | 全绿（`profiles_select_basic` Esc 断言改为保留 4 项） |
| `flutter test` `recheck05_hidden_selection_test`（更新为默认选中语义） | 全绿 |
| `flutter test` 拖选/键盘/过滤/组编辑器回归 | 全绿；个别文件偶发 `did not complete`（仓库已知 leaked-resource flake），单独重跑即绿 |

未运行：真实 Windows 窗口/FRB 集成、真实内核、全量 `flutter test`/`flutter build windows`。
