# R3-PROF-08 — 选择器节点延迟/速度/订阅/默认组与键鼠合同

状态：`implemented`（选择器列 join、默认组、搜索字段、Ctrl+A/Enter/双击/Esc、隐藏确认均以合成 widget 断言通过；未运行真实 Windows 窗口 / FRB 集成，故不写 `verified`）。

任务 ID：R3-PROF-08

本次唯一用户流程：从策略组编辑器打开「选择节点...」选择器（`ProfilesSelectWindow`）时，候选行显示与节点主表一致的延迟/速度/订阅分组，默认落在主窗口当前订阅分组，搜索只匹配备注/地址，Ctrl+A 全选可见、Enter/双击确认、Esc 取消，确认结果只含当前可见项。

前置任务及已验证证据：复核结论见 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` 的 R3-PROF-08 行与 RE-PROF-11 剩余段（`:693-697` 恒 `-`、`:691` 订阅列 subid、`:809` 空 sort 原样返回、`:817-820` 额外搜端口、`:838` 无默认组、缺键鼠合同）。上游冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。既有 picker 主体证据见 `recheck-fixes/RR-08-REPROF11/`。开始应用 HEAD `cd141f5`（工作树干净）。

对应 feature / action / layout ID：`RE-PROF-11`、`R3-PROF-08`、`LAY-PROFILES-002`（九列）、`ACT-PROF-013`（组子项选择）。

必读上游文件、符号和固定 commit：
- `UP/ServiceLib/ViewModels/ProfilesSelectViewModel.cs:182-208`（`GetProfileItemsEx`：`ProfileItem` join `ProfileExItem`，`OrderBy(Sort)`，`DelayVal/SpeedVal/SubRemarks`）、`:158-168`（`RefreshServersBiz` 默认选中）、`:170-180`（`RefreshSubscriptions` 默认主窗口当前组，`AllGroupServers` 置首）。
- `UP/v2rayN/Views/ProfilesSelectWindow.xaml.cs:79`（`MouseDoubleClick -> SelectFinish`）、`:93-122`（`Ctrl+A -> SelectAll`、`Enter -> SelectFinish`）、`:144-151`（查询框 Enter -> `RefreshServers`）、`:129-142`（Autofit）。
- `UP/ServiceLib/Handler/ConfigHandler.cs:1087-1105`（Delay/Speed 失败/未测沉底，主表已用）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`showNodePicker(..., currentGroupSubId, speedResults)`；生产入口由 `group_editor_dialog.dart` 在打开选择器时经 `ProviderScope` 读取 `profilesControllerProvider.groupSubId` 与 `bridgePortProvider.speedTestResults()`；无 Riverpod scope 的纯 widget 测试回退为空组/无 overlay。
- 输出：`joinPickerRows`（`lib/bridge/bridge_port.dart`）把候选 `ProfileDto` 与 `ProfileExItem` 结果、订阅备注 join 成 `ProfileSummary`，列显示复用节点主表 `defaultProfileColumns()` 的 `DelayVal/SpeedVal/SubRemarks` 显示函数，排序复用 `applySort`（失败/未测 `<=0` 两向沉底）、默认序复用 `orderByPersistedSort`（`ProfileExItem.Sort` 结果序）。
- 错误：无候选/类型过滤后为空显示「没有可选节点」；单选取消 `null`。
- 取消：Esc / 「取消」返回 `null`，不写库。
- 权限：纯 UI + 只读 bridge 查询（`speedTestResults()`），不启动内核、不监听端口、不写系统代理/TUN。
- 持久化：无（选择器只读；结果返回调用方）。
- 生效：确认返回可见项 `indexId` 列表，调用方（组编辑器）按现有路径追加子节点。

允许修改的模块：`apps/desktop/lib/features/profiles/group_editor_dialog.dart`、`apps/desktop/lib/bridge/bridge_port.dart`（新增手写 seam `joinPickerRows`）、`apps/desktop/test/**`（新增五个 picker 测试 + harness 扩展 + 受影响断言）、本卡、`docs/evidence/recheck-fixes/R3-PROF-08-10/**`、compat 台账（仅追加）。

禁止改变的已有行为：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/subs|runtime|settings|monitor|update|backup|routing/**`、`crates/**`；不改 RE-PROF-13/14 活动标记/自动列宽/搜索时机；不改 FIX-10B 排序写链；不删入口或降分母。

测试夹具和原版预期：合成 `ProfileDto`（RFC 5737 地址、文档域名、合成 UUID）+ 合成 `SpeedTestResultDto`；不下载、不连接、不读用户凭据。原版预期：默认组=主窗口当前组；订阅列=`SubRemarks`；`DelayVal=t33.Delay!=0?"{Delay}":""`、`SpeedVal=t33.Speed>0?"{Speed}":t33.Message`；Ctrl+A 全选可见、Enter/双击 `SelectFinish`、Esc 取消；搜索 remarks/address。

本次必须通过的命令/真实场景：
- `dart format`（改动文件）；`flutter analyze`；
- `flutter test`（逐文件）`test/recheck_r3_prof08_picker_columns_test.dart`、`_keyboard_`、`_double_click_`、`_esc_`、`_hidden_confirm_`；
- 回归：`recheck_reprof11_picker_*`、`reprof10_group_preview_test.dart`、`table_actions_test.dart`、`profiles_select_basic_test.dart`。

证据文件位置：`docs/evidence/recheck-fixes/R3-PROF-08-10/`（`README.md`、`observations.json`）。

完成条件：选择器延迟/速度列显示真实值、失败/未测按主表规则沉底/显示、订阅列显示 SubRemarks（无则 subid）、默认组=主窗口当前组、Ctrl+A/Enter/双击/Esc 合同、搜索字段只匹配 remarks/address、确认数量只含可见项（换组/过滤清隐藏选择）；回归绿；门禁通过。未跑真实窗口/FRB 集成，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`ProfileDto` 不携带 `SubRemarks`；当前用 `subItems` 的 `remarks` 映射 `subid`，无 `subItems` 时回退 `subid`。若后端在 `ProfileDto`/`ProfileSummary` 暴露真实 `SubRemarks`，应改由后端 join，避免前端二次映射。
- 接口缺口（登记）：生产入口只能经 `ProviderScope` 读取控制器/桥（`profile_actions.dart` 本轮冻结，未改 `showGroupEditor` 调用点）。若后续允许改调用方，建议把当前组与测速 overlay 作为显式参数下传。

本轮实际结果：`joinPickerRows` 新增于 `bridge_port.dart`；`group_editor_dialog.dart` 选择器改为 join 行、默认组、remarks/address 搜索、`CallbackShortcuts`（Ctrl+A/Enter/Esc）+ 手写双击确认、可见集确认与换组/过滤剪枝；`showNodePicker` 增可选 `currentGroupSubId/speedResults`；`_GroupEditorDialogState` 经 `ProviderScope` 读取活数据并在无 scope 时回退。测试：5 个新 picker 文件全绿；`reprof10_group_preview` 全绿；`table_actions_test` 全绿；`profiles_select_basic_test` 全绿；既有 `recheck_reprof11_picker_*` 全绿。`flutter analyze` No issues。
