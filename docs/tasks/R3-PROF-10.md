# R3-PROF-10 — 默认选中、Esc 保留选择与主行键盘导航

状态：`implemented`（切组/新增默认选中、Esc 保留、Shift/方向键从主行推导均以合成 controller 断言通过；未运行真实 Windows 窗口，故不写 `verified`）。

任务 ID：R3-PROF-10

本次唯一用户流程：在节点表切换到非空订阅分组、或新增保存节点后，表格自动把当前行设为 pending→活动→第一行；按 Esc 只停止测速并保留当前选择；Shift 范围选择与方向键移动都从主行（`primaryId`）推导，而不是从选择集合的任意首/尾项。

前置任务及已验证证据：复核结论见 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` 的 R3-PROF-10 行与 RE-PROF-05 剩余段（`:552-559` 只求交、`:965-972` Esc 清选择、`extendSelection`/方向键从 Set/首个可见项推导）。上游冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。开始应用 HEAD `cd141f5`（工作树干净）。旧拖选 auto-scroll 修复（`f328d0f`）与本卡不同，不被本卡覆盖。

对应 feature / action / layout ID：`RE-PROF-05`、`R3-PROF-10`、`ACT-PROF-038`（Esc 停测速）、`ACT-PROF-001..`（方向键导航）。

必读上游文件、符号和固定 commit：
- `UP/ServiceLib/ViewModels/ProfilesViewModel.cs:361-379`（`RefreshServersBiz`：`_pendingSelectIndexId` → 活动 `_config.IndexId` → `lstModel.First()`）。
- `UP/v2rayN/Views/ProfilesView.xaml.cs:289-291`（`Escape -> ServerSpeedtestStop()`，不清选择）、`:268-287`（Delete/T/U/D/B）。
- 冻结 `MainWindowViewModel.cs:690-695`（无默认对象时 Reload 提前返回，行为不冲突）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`setGroupSubId`（切组）、`saveDraft`（新增保存，`draft.indexId.isEmpty`）、`selectRow(shift/ctrl)`、`emitAction(escape/navigateUp/navigateDown)`。
- 输出：`ProfilesState.selected` / `primaryId` / `activeId` 更新为上游默认选择语义；Esc 不再改 `selected`。
- 错误：空组不选；新增节点不在当前可见组时回退到活动/第一行（与上游一致）。
- 取消：Esc 停测速（`cancelSpeedTest`）后保留选择并记录 `escape` 事件。
- 权限：纯 UI/控制器状态，无网络、无内核、无系统代理/TUN。
- 持久化：无新增（`activeId` 由既有 `setActiveProfile` 持久化）。
- 生效：切组/新增后表格立即可编辑/Enter；Shift 二次扩展锚点稳定；方向键从当前行推进。

允许修改的模块：`apps/desktop/lib/features/profiles/profiles_controller.dart`、`apps/desktop/lib/features/profiles/table_actions.dart`（`extendSelection` 增显式 anchor）、`apps/desktop/test/**`（新增 controller 测试 + 更新受影响断言）、本卡、`docs/evidence/recheck-fixes/R3-PROF-08-10/**`、compat 台账（仅追加）。

禁止改变的已有行为：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/subs|runtime|settings|monitor|update|backup|routing/**`、`crates/**`；不改 RE-PROF-13/14；不改 FIX-10B 排序写链；不删入口或降分母。

测试夹具和原版预期：`StoredBridge`/`SyntheticBridgePort` 合成节点（RFC 5737 地址）；原版预期：非空组默认选 pending→active→first；Esc 只 `ServerSpeedtestStop`；Shift/方向键以主行为基准。

本次必须通过的命令/真实场景：
- `dart format`（改动文件）；`flutter analyze`；
- `flutter test`（逐文件）`test/recheck_r3_prof10_default_selection_test.dart`；
- 回归：`recheck05_hidden_selection_test.dart`、`table_actions_test.dart`、`profiles_select_basic_test.dart`、`t17_table_keyboard_test.dart`、`t15b_speedtest_test.dart`、`recheck_r3_prof_controller_test.dart`。

证据文件位置：`docs/evidence/recheck-fixes/R3-PROF-08-10/`（`README.md`、`observations.json`）。

完成条件：切到非空组/新增保存后默认选中 pending→活动→第一行；Esc 只停测速并保留选择；Shift 范围与方向键从 `primaryId` 推导、多选时移动自当前行；回归绿；门禁通过。未跑真实窗口，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`ProfilesState` 只有 `primaryId`，没有独立于主行的持久 `anchor`；Shift 二次扩展把主行当作锚点（WPF 亦以 `SelectedItem` 为锚）。若将来要区分「按下列锚点」和「当前行」，需新增显式 anchor 字段。
- 接口缺口（登记）：新增保存的 pending 选中依赖 `saveDraft` 的 `draft.indexId.isEmpty` 判定；`ProfileDto` 无显式 isNew 标志。

本轮实际结果：`table_actions.dart` 的 `extendSelection` 增显式 `anchorId`；`profiles_controller.dart` 的 `selectRow`(shift) 用 `primaryId` 为锚并保持主行、`navigateSelection` 从 `primaryId` 推进、`emitAction(escape)` 只停测速保留选择、新增 `_selectDefaultRow` 并在 `setGroupSubId`/新增 `saveDraft` 调用。测试：`recheck_r3_prof10_default_selection_test.dart` 6/6 通过；`table_actions_test` 全绿；`profiles_select_basic_test`（Esc 现保留 4 项）全绿；`recheck05` 更新为默认选中语义后全绿；`t17_table_keyboard_test`、`t15b_speedtest_test`、`recheck_r3_prof_controller_test` 全绿。`flutter analyze` No issues。
