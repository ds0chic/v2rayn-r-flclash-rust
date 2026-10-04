# RE-PROF-14 — 自动列宽 / 拖动排序开关 / 搜索时机

状态：`implemented`（Dart widget/纯函数覆盖已通过；未做原版实机双窗口逐事件对照，未跑真实 Windows 窗口，故不写 `verified`）。

任务 ID：RE-PROF-14

本次唯一用户流程：
1. 点顶部“自动列宽”按钮，按表头与当前单元格内容测量并设置列宽（等价上游每列 `Auto`），宽度持久化、重开保留。
2. `UiItem.EnableDragDropSort=false` 时节点表不注册任何行拖动/放置处理；`true` 时注册拖动重排。
3. 顶部过滤器：非空查询按 Enter 提交生效；清空输入立即刷新；搜索范围对齐上游 `remarks`/`address`。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核结论见 `docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-14。开始 HEAD `842520b`，工作树干净。本卡只读禁止清单内文件。

对应 feature / field / action / layout ID：`F-PROFILE-014`（节点表渲染/过滤）、`ACT-PROF-033`(拖动排序)、`FLD-CFG-067`(EnableAutoAdjustMainLvColWidth)、`FLD-CFG-076`(EnableDragDropSort)。

必读上游文件、符号和固定 commit：
- `v2rayN/Views/ProfilesView.xaml:27-34`：`lstProfiles` 拖拽重排仅在 `EnableDragDropSort` 为真时注册。
- `v2rayN/Views/ProfilesView.xaml:70-79`：`btnAutofitColumnWidth` → `BtnAutofitColumnWidth_Click`。
- `v2rayN/Views/ProfilesView.xaml.cs:296-314`：`AutofitColumnWidth()` 对每列 `Width = new DataGridLength(1, Auto)`。
- `v2rayN/Views/ProfilesView.xaml.cs:316-322`：`TxtServerFilter_PreviewKeyDown` 在 Enter/Return 调 `ViewModel.RefreshServers()`。
- `ServiceLib/ViewModels/ProfilesViewModel.cs:112-114/343-350`：`ServerFilterChanged` 仅在 `_serverFilter.IsNullOrEmpty()` 时刷新。
- `ServiceLib/Manager/AppManager.cs:202-235`：`ProfileModels(subid, filter)` 的 `filter` 只匹配 `(a.remarks like '%q%' or a.address like '%q%')`（第 226 行）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：自动列宽读 `state.visible` 与 `state.columns`；拖动开关读设置文档 `UiItem.EnableDragDropSort`；过滤读输入框文本。
- 输出：自动列宽写列宽并经 `_persistColumns` 落 `ui_state.json`；开关决定是否构建 `Draggable`/`DragTarget`；过滤在 Enter/清空时更新 `visible`。
- 错误：设置缺失/畸形 → 拖动关闭（默认 false）；自动列宽无内容时按表头测量并钳制 40..600。
- 取消：均为纯 UI 行为，无内核副作用。
- 权限：仅本机 UI；不启动内核、不监听端口、不写系统代理/TUN、不碰 10808。
- 持久化：列宽沿用既有 `column_layout` 段；拖动开关为引擎设置（`UiItem`，`RestartApp` 生效），本卡只读不写。
- 生效：列宽即时；拖动开关按设置文档解析（上游要求重启应用）。

兼容决策（RE-PROF-14c，明确记录）：
- UI 默认对齐上游：`_FilterField.onChanged` → `updateFilterInput`（非空仅暂存、清空立即刷新）；`onSubmitted`（Enter）→ `submitFilter` 提交非空查询。
- `ProfilesController.setFilter` 保留为“立即筛选”的程序化/测试入口（原 `recheck05`/`re_prof_06`/`fix10b` 用例依赖），不属 UI 路径；UI 不使用它。此为有意保留的内部 seam，已在证据登记。
- 搜索字段由原先的多字段（组名/协议/TLS/IP/端口）收窄为 `remarks`/`address`，与冻结 `AppManager.ProfileModels` 一致；`profiles_models_test` 既有断言本就写“remarks/address”。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_table.dart,profiles_page.dart,profiles_controller.dart,profiles_models.dart,table_actions.dart}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/RE-PROF-13-14/**`、`compat/*.yaml`（仅追加）。未改 `main_shell.dart`、`app.dart`、`frb_generated.*`、`lib/bridge/**`、`features/*` 其它域、`crates/**`。

禁止改变的已有行为：FIX-01/05/10B 已提交的选择/拖选/排序语义；`handleDrop` 直接调用的重排语义；列设置对话框行为。

测试夹具和原版预期：合成节点（不下载、不连接）。原版预期：自动列宽按内容；开关 false 无拖拽、true 有拖拽；非空过滤仅 Enter 生效、清空立即生效、字段 remarks/address。

本次必须通过的命令/真实场景（实际运行）：
- `dart format`（改动文件）：Passed。
- `flutter analyze`：No issues found。
- `flutter test test/re_prof_14_autofit_test.dart`：1 passed。
- `flutter test test/re_prof_14_drag_disabled_test.dart`：1 passed。
- `flutter test test/re_prof_14_drag_enabled_test.dart`：1 passed（首跑 flaky 未发现用例，重跑通过）。
- `flutter test test/profiles_filter_test.dart`：1 passed（Enter/清空语义）。
- `flutter test test/ux_space01_group_flow_test.dart`：1 passed（更新为 Enter 提交）。
- 回归：`profiles_pointer_test`（拖动启用）、`profiles_models_test`、`fix10b_profile_order_test`、`recheck05_hidden_selection_test`、`re_prof_06_scope_test`、`profiles_render_test`、`t05_profiles_ui_test`（重跑）、`profiles_keyboard_test`、`reprof04_sort_readback_test`、`recheck02_active_apply_test`、`profiles_share_shortcut_test`、`recheck01_group_inheritance_test` 均通过。

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-13-14/`（`README.md`、`observations.json`、`runs.txt`）。

完成条件：widget 覆盖自动列宽改变并持久化列宽、开关 false 不注册拖拽/true 注册、Enter/清空过滤语义；`flutter analyze` 通过。未跑真实 Windows 窗口与原版实机，保持 `implemented`。

本轮实际结果：见 `docs/evidence/recheck-fixes/RE-PROF-13-14/README.md`。
