# R3-WPF-ROUTING — 路由设置窗口可见结构/文案对齐冻结 WPF

状态：`implemented`（widget 结构与文案断言通过；未在本轮做原版实机双窗口几何/逐事件对照，且窗口形态仍为内嵌 dialog，故不写 `verified`）。

任务 ID：R3-WPF-ROUTING

本次唯一用户流程：打开「设置 → 路由设置」窗口，可见结构应为：顶部工具栏 `添加规则集` / `一键导入规则集`；两行 `域名解析策略` / `sing-box 域名解析策略` 超链标签 + 下拉；区块标题 `预定义规则集列表`；表格列头 `别名 / 数量 / 排序 / 可选地址 (Url) / 自定义图标`；底部不再有非上游动作栏（仅保留 `关闭` 以关闭内嵌对话框）。行内右击弹出上游 DataGrid 上下文菜单项。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；对照证据 `docs/evidence/recheck-fixes/R3-WPF-COMPARE/README.md`（§4.3 R1–R4、`original-routing.png` vs `rc-routing.png`）。上游文件 `v2rayN/v2rayN/Views/RoutingSettingWindow.xaml`（ToolBarTray:27、策略 Grid:55-102、TabItem header:105、ContextMenu:118-144、Columns:157-177）、`RoutingSettingWindow.xaml.cs:15-31/98-106`、`ServiceLib/ViewModels/RoutingSettingViewModel.cs:22-68/84-105`、`ServiceLib/Resx/ResUI.zh-Hans.resx`（LvRemarks=别名:310、LvCount=数量:316、LvSort=排序:562、LvUrl=可选地址 (Url):313、LvCustomIcon=自定义图标:331、menuRoutingAdvancedAdd=添加规则集:811、menuRoutingAdvancedImportRules=一键导入规则集:814、TbdomainStrategy=域名解析策略:823、TbRoutingTabRuleList=预定义规则集列表:826、TbdomainStrategy4Singbox=sing-box 域名解析策略:1018、menuRoutingAdvancedRemove=移除所选规则:817、menuRoutingAdvancedSetDefault=设为活动规则:820、menuSelectAll=全选:532）。

对应 feature / field / action / layout ID：`F-ROUTING-002`、`LAY-ROUTINGSET-001`、`ACT-MAIN-025`。

必读上游文件、符号和固定 commit：见上（`RoutingSettingWindow.xaml/.xaml.cs`、`RoutingSettingViewModel.cs`、`ResUI.zh-Hans.resx`），固定 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：既有 bridge 用例（`listRoutings` / `listRoutingRules` / `saveRouting` / `deleteRouting` / `setDefaultRouting`）与 `settingsController` 的 `RoutingBasicItem` 策略组；未新增 IPC。
- 输出：`添加规则集` 复用既有新增方案窗口（`_openRuleset(null)`）；`移除所选规则` 复用既有删除确认（`_confirmDelete`）；`设为活动规则` 复用 `setDefault`；`一键导入规则集` 上游为 `ConfigHandler.InitRouting(config, true)`，Rust 侧暂无对应用例（登记缺口），本轮落到 `reload()` 并提示。
- 错误/取消：沿用既有行为；删除确认取消不改动；上下文菜单外点/Esc 关闭。
- 权限：仅本机 UI + 既有 bridge；不启动内核、不写系统代理/TUN、不监听端口、不读凭据。
- 持久化/生效：策略组走既有 `saveGroup('RoutingBasicItem', ...)`；方案增删改走既有用例；未新增落库路径。

允许修改的模块：`apps/desktop/lib/features/routing/{routing_windows.dart,routing_controller.dart,routing_actions.dart}`（本轮只改 `routing_windows.dart`，删除其不再使用的 `runtime_controller` import）、`apps/desktop/test/**`（新增 `r3_wpf_routing_structure_test.dart`）、`docs/tasks/R3-WPF-ROUTING.md`、`docs/evidence/recheck-fixes/R3-WPF-ROUTING/**`、`compat/features.yaml`（仅追加一行 evidence）。

禁止改变的已有行为：方案编辑器（`RoutingRulesetWindow`）与规则详情窗口、`_saveStrategy` 写全局 `RoutingBasicItem` 的语义、FIX-08C 的导入/导出 camelCase 与 Custom 排除语义、内置规则集数据（8/11/4）与 schema；不改 `main_shell.dart`、`app.dart`、`frb_generated`、`lib/bridge/api/**`、其它 features、`crates/**`。

测试夹具和原版预期：`test/support/profiles_harness.dart` 的合成 bridge（公开模板/合成数据，无真实节点/订阅/凭据）。原版预期（冻结 XAML + zh-Hans resx）：标题 `路由设置`；工具栏两项 `添加规则集` / `一键导入规则集`；策略标签 `域名解析策略` / `sing-box 域名解析策略`；TabItem 头 `预定义规则集列表`；列头 `别名/数量/排序/可选地址 (Url)/自定义图标`（无 `状态` 列）；底部无动作按钮（命令在工具栏 + 行右击上下文菜单 `添加规则集 / 移除所选规则 / 全选 / 设为活动规则 / 一键导入规则集`）。

本次必须通过的命令/真实场景：
- `dart format lib/features/routing/routing_windows.dart test/r3_wpf_routing_structure_test.dart`
- `flutter analyze lib/features/routing test/r3_wpf_routing_structure_test.dart`
- `flutter test test/r3_wpf_routing_structure_test.dart test/t11_routing_test.dart test/fix08_routing_draft_test.dart test/fix08c_routing_draft_test.dart`
- 回归：`flutter test test/t21e_dialogs_responsive_test.dart`（1000×700 无溢出）。

证据文件位置：`docs/evidence/recheck-fixes/R3-WPF-ROUTING/`（`README.md`、`test-run.log`）。

完成条件：widget 断言标题/工具栏/区块标题/策略标签/列头/底部按钮集/上下文菜单，且删除非上游 `备注/规则数/状态` 列与底部 `添加/删除/设为默认/应用`；既有路由测试（t11/fix08/fix08c）与 1000×700 响应式回归通过；未做原版实机逐事件对照且窗口仍为 dialog，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：「一键导入规则集」上游为 `ConfigHandler.InitRouting(config, true)`（重新导入/刷新内置路由模板）。当前 `lib/bridge/api/routing.dart` 无对应用例，且 bridge port 抽象位于 `features/profiles/**`（本卡禁改）。本轮按钮存在并落到 `reload()` + 提示「导入后端用例待接入」；建议后续在 `bridge_api`/`application` 增加 re-import builtin routing 用例后再接通。
- 接口缺口（登记）：上游上下文菜单含 `全选`（Ctrl+A 多选）。RC 列表当前为单选；本轮未实现多选，菜单未含 `全选`。建议后续卡在单选/多选模型统一时补。
- 接口缺口（登记）：窗口形态（原版独立窗口 vs RC 内嵌 dialog、`WindowBase`/标题栏关闭、`DialogResult=IsModified`）不在本轮；为避免内嵌 dialog 无关闭入口，底部保留单个 `关闭`。建议后续独立窗口卡移除该占位按钮。

本轮实际结果：`routing_windows.dart` 顶部新增工具栏（`routing-add` = 添加规则集、`routing-import-builtin` = 一键导入规则集）；策略行改为上游两行超链标签 + 300 宽下拉并修 `(空)` 显示为空；新增区块标题 `预定义规则集列表`（`routing-block-title`）；列头改为 `别名/数量/排序/可选地址 (Url)/自定义图标` 并去掉 `状态` 列、行展示 `customIcon`；底部动作栏缩为 `关闭`（`routing-close`），`添加/删除/设为默认/应用` 移入行右击上下文菜单（`添加规则集/移除所选规则/设为活动规则/一键导入规则集`）。新增 `test/r3_wpf_routing_structure_test.dart`（2 用例）断言上述结构/文案/按钮集与上下文菜单。运行 `flutter analyze lib/features/routing test/r3_wpf_routing_structure_test.dart` → No issues；`flutter test test/r3_wpf_routing_structure_test.dart test/t11_routing_test.dart test/fix08_routing_draft_test.dart test/fix08c_routing_draft_test.dart` → 13/13 通过；`t21e_dialogs_responsive_test.dart` 首次 exit（偶发），重试 1/1 通过。未跑 `flutter build windows`（本卡范围外）。
