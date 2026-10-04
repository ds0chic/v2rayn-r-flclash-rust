# R3-WPF-ROUTING-WINDOW — 路由设置改为独立顶层窗口

状态：`implemented`（已迁移为原生第二顶层窗口；发布包隔离数据目录 HWND 探针通过；未做逐事件/DPI 对照，故未达 `verified`。证据：`docs/evidence/recheck-fixes/R3-WPF-ROUTING-WINDOW/`）。

任务 ID：R3-WPF-ROUTING-WINDOW。

本次唯一用户流程：主窗口选择“设置→路由设置”后打开独立、隶属主窗口的“路由设置”窗口；编辑规则/默认路由后关闭，按冻结原版的 IsModified 返回值刷新路由菜单并重启服务，取消/关闭未修改时不触发重载。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；[三窗口真机对照](../evidence/recheck-fixes/R3-WPF-COMPARE/README.md)；Wave K 工具栏/列头/标题已对齐。依赖 `R3-WPF-OPTION-WINDOW` 的最小多窗口宿主 spike/窗口生命周期接口，但路由业务单独验收。

对应 feature / field / action / layout ID：`ACT-MAIN-025`、`ACT-ROUTE-001`、`LAY-ROUTINGSET-001`、`F-ROUTING-001`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/ViewModels/MainWindowViewModel.cs:598-610`；`v2rayN/v2rayN/Views/RoutingSettingWindow.xaml` 与 `.xaml.cs:43-49`（Closing/IsModified）；当前 `apps/desktop/lib/features/routing/routing_windows.dart:35-49`、`main_shell.dart`、`routing_controller.dart`。

输入、输出、错误、取消、权限、持久化及生效语义：输入为现有规则集与策略快照。添加/删除/设默认/导入沿用当前后端修订与错误反馈；窗口变更后关闭才按原版返回修改事实，未修改或打开失败不报已重载。Esc、标题栏关闭、主窗口隐藏和重复打开要保持窗口/选择/草稿状态可解释。此卡仅窗口形态，不借机改系统代理/TUN 或真实核心配置。

允许修改的模块：`apps/desktop/lib/features/routing/routing_windows.dart`、`routing_actions.dart`、共享桌面窗口宿主/通信模块、必要 `apps/desktop/windows/**`、目标测试、此卡与证据。

禁止改变的已有行为：规则排序/默认/导入的持久化语义、Wave K 五列与按钮文字、现有 FRB/Rust 合同；不降原版菜单/布局台账分母、不动 10808 或用户配置。

测试夹具和原版预期：隔离 data dir 内合成两规则集；原版是单独 Windows 顶层窗口，`ShowDialogAsync` 返回 true 后主窗口刷新菜单并 Reload。对照两个 HWND、owner/focus、Ctrl+A/Enter/Delete、修改后关闭与未修改关闭、100%/150% DPI。

本次必须通过的命令/真实场景：`dart format --output=none --set-exit-if-changed lib test integration_test`、`flutter analyze`、目标路由 widget/integration tests、`flutter build windows --release`；隔离发布包的真实窗口探针和冻结 WPF 同场景对照。真实运行服务重载可用受管合成配置和≥11808已探测端口，不写宿主系统代理。

证据文件位置：`docs/evidence/recheck-fixes/R3-WPF-ROUTING-WINDOW/`（窗口探针、原版/RC截图、操作/重载事件、测试日志）。

完成条件：独立窗口和修改/关闭/主界面刷新流程实测通过；只把 dialog 改名或只验证组件树不算完成。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。
