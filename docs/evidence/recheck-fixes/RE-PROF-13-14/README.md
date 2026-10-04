# RE-PROF-13 / RE-PROF-14 证据 — 活动节点标记、自动列宽、拖动开关、搜索时机

状态：`implemented`。日期 2026-10-04；开始 HEAD `842520b`（工作树干净，执行期存在其它并行子代理对 `bridge_port.dart` 的改动，本卡未触碰该文件）。冻结上游 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

本回合只改 `apps/desktop/lib/features/profiles/{profiles_table.dart,profiles_page.dart,profiles_controller.dart,profiles_models.dart,table_actions.dart}`、`apps/desktop/test/**`、`docs/tasks/RE-PROF-13.md`、`docs/tasks/RE-PROF-14.md`、本目录、`compat/features.yaml`、`compat/actions.yaml`（仅追加注释）。未改 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/**`、`features/{subs,runtime,settings,monitor,update,backup,routing}/**`、`crates/**`。未启动内核、未监听任何端口、未改系统代理/注册表/路由/TUN、未读用户凭据。

## 上游对照

- RE-PROF-13：`v2rayN/Views/ProfilesView.xaml:271-279` 的 `DataGridCell` 样式在 `IsActive=True` 时把整行单元格背景设为 `MaterialDesign.Brush.Primary.Light`，独立于 DataGrid 选择高亮。当前实现用 `activeRowFill(context)` 作活动行数据单元格填充、`activeRowMarkerColor(context)` 作 handle 列实心前导标记；选择行继续用 `Semantics.selectedRow`（`colorScheme.primaryContainer`）。
- RE-PROF-14a：`ProfilesView.xaml.cs:296-314` `AutofitColumnWidth()` 对每列设 `DataGridLength(1, Auto)`。当前 `ProfilesController.autofitColumns()` 用 `TextPainter` 按 12px 表头/单元格样式测量表头与可见行内容，取较大值 + 12px 内边距，钳制 40..600，经 `_persistColumns` 落 `ui_state.json` 的 `column_layout`。
- RE-PROF-14b：`ProfilesView.xaml:27-34` 仅 `EnableDragDropSort=true` 时注册拖拽重排。当前 `profilesEnableDragDropSortProvider` 读设置文档 `UiItem.EnableDragDropSort`（缺省 false）；`_handleCell` 仅在 true 时构建 `Draggable`/`DragTarget`，false 时只渲染序号与活动标记。
- RE-PROF-14c：`ProfilesViewModel.cs:343-350` `ServerFilterChanged` 仅清空时刷新；`ProfilesView.xaml.cs:316-322` Enter 调 `RefreshServers()`；`AppManager.cs:226` 只匹配 `remarks`/`address`。当前 `_FilterField.onChanged → updateFilterInput`（非空暂存、清空立即刷新），`onSubmitted → submitFilter`；`rowMatchesQuery` 收窄为 remarks/address。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format`（改动文件） | exit 0，无多余改动 |
| `dart format --output=none --set-exit-if-changed lib test` | exit 0，`Formatted 234 files (0 changed)` |
| `flutter analyze` | exit 0，`No issues found!` |
| `flutter test test/re_prof_13_active_marker_test.dart` | 1 passed |
| `flutter test test/re_prof_13_theme_colors_test.dart` | 1 passed |
| `flutter test test/re_prof_14_autofit_test.dart` | 1 passed |
| `flutter test test/re_prof_14_drag_disabled_test.dart` | 1 passed |
| `flutter test test/re_prof_14_drag_enabled_test.dart` | 1 passed（首跑 `No tests were found`，重跑通过） |
| `flutter test test/profiles_filter_test.dart` | 1 passed（Enter/清空语义，已按新时机更新） |
| `flutter test test/ux_space01_group_flow_test.dart` | 1 passed（已更新为 Enter 提交） |
| `flutter test test/profiles_pointer_test.dart` | 1 passed（改为 `dragDropSort: true`） |
| `flutter test test/{profiles_models,fix10b_profile_order,recheck05_hidden_selection,re_prof_06_scope,profiles_render,profiles_keyboard,reprof04_sort_readback,recheck02_active_apply,profiles_share_shortcut,recheck01_group_inheritance,ux_space01_entries,context_menu_model,context_menu_move}_test.dart` | 全部 passed |
| `flutter test test/t05_profiles_ui_test.dart` | 首跑 `did not complete`（flutter_tester 偶发），重跑 1 passed |

未运行（任务约束）：`flutter build windows --release`、全仓 `flutter test`、`cargo test --workspace`、真实 Windows 窗口、原版实机双窗口。

## 改动要点

- `profiles_table.dart`：`build` 读取 `profilesEnableDragDropSortProvider` 并传入单元格；`_dataCell` 活动行用 `activeRowFill`（选择优先）；`_handleCell` 增加带 key 的实心前导标记，false 时不构建 `Draggable`/`DragTarget`；新增 `@visibleForTesting activeRowFill/activeRowMarkerColor`。
- `profiles_controller.dart`：新增 `profilesEnableDragDropSortProvider` + `readEnableDragDropSort`；`ProfilesState` 增加 `filterInput`（未提交文本）；新增 `updateFilterInput`，`submitFilter` 改提交 `filterInput`；`setFilter` 保留立即筛选语义并同步 `filterInput`；`emitAction('autofit-columns')` 真实分派；新增 `autofitColumns`/`_measureText`（`TextPainter`）。
- `profiles_page.dart`：`onChanged → updateFilterInput`；自动列宽按钮用 `ProfileAction.autofitColumns`。
- `profiles_models.dart`：`rowMatchesQuery` 收窄为 `remarks`/`address`（对齐 `AppManager.ProfileModels`）。
- `table_actions.dart`：新增 `ProfileAction.autofitColumns`。

## 测试断言

- 活动行数据单元格填充 == `activeRowFill`，选中行 == `primaryContainer`，两者不相等；活动行 `active-marker-<id>` 存在且色 == `activeRowMarkerColor`（选 A 设活动→选 B，再选回 A 仍保留标记）。
- 浅/深主题下 `activeRowFill` 与 `primaryContainer` 均不相等，且两主题活动填充不同。
- 自动列宽后至少一列宽度变化、所有列宽 > 0、`lastEvent.action == 'autofit-columns'`、`column_layout.widths['ConfigType']` 已持久化。
- `EnableDragDropSort=false` → 无 `Draggable<ProfileSummary>`；`true` → 有。
- 过滤器：非空输入不即时改变 `visible`；Enter 后 `visible` 缩小且 lastEvent `refresh`；清空立即恢复；Ctrl+A 仍留在过滤器作用域。
- 回归：`setFilter` 立即筛选入口保持（`recheck05`/`re_prof_06`/`fix10b`），拖动启用后 `profiles_pointer_test` 通过。

## 边界与缺口

- 未跑真实 Windows 窗口与原版实机双窗口；活动标记的真实像素渲染与拖动重排的真机效果未验证。
- `setFilter` 作为“立即筛选”的程序化入口保留（非 UI 路径）；UI 走 Enter/清空，属有意保留的 seam，已在任务卡 RE-PROF-14c 登记。
- 搜索字段由多字段收窄为 remarks/address，属对齐上游的行为变更；若后续要求保留多字段增强，需新增设置项或明确产品决策。
- 拖动开关为 `RestartApp` 时机；运行中修改设置不会即时改变表格，符合上游。
- 自动列宽只测量当前可见行（上限 500 行采样），与 DataGrid 只测量已实现行一致；极端大列表下与“全量内容 Auto”可能有细微差异。
