# FIX-11C — 内核运行→日志页：采集→刷新/滚动→过滤→复制

状态：`implemented`（Flutter 针对性测试覆盖采集/暂停/自动刷新/滚动/清空/过滤/复制/有界缓冲；Rust bridge 单测覆盖暂停丢批；未做「net-host + 真实内核 + Flutter 同屏」整链与上游窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-11C

本次唯一用户流程：内核运行后进入日志（消息）页：内核 stdout/net-host 行按 `LogEnabled`/级别语义进入有界采集；页内可自动刷新、自动滚动到底、暂停/继续采集、清空；按关键字/级别过滤只影响呈现；复制全部/选中到剪贴板，格式与上游 `MsgView` 一致；隐藏页面只暂停页面级 UI 刷新，Rust 环形缓冲继续采集。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `a93949a`（工作树含其他并行子代理在 `backup/**`、`subs.rs`、`subscriptions/**`、`t16.rs`、`codegen.rs` 的在途改动，未回退）。审查结论见 `docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 36 行 FIX-11、`runtime-report.md` RT-19、`settings-report.md` SET-18、`all-items.csv` 的 ACT-MSG-001/002/003、FLD-CFG-065/066。FIX-11（统计/会话注入）与 FIX-11B（Clash 操作）已提交，本卡不改其语义。

对应 feature / field / action / layout ID：`F-MONITOR-001`（消息/日志页）、`ACT-MSG-001`（自动刷新）、`ACT-MSG-002`（过滤）、`ACT-MSG-003`（复制/全选/清空）、`LAY-MSG-001`、`FLG` 级消息与日志级别、`SET-18`、`RT-19`。

必读上游文件、符号和固定 commit（`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967`）：
- `ServiceLib/ViewModels/MsgViewModel.cs`：`NumMaxMsg=500` 有界队列、`MsgFilter`（正则 `Utils.IsRegexMatch`）、`AutoRefresh`（默认 true，持久化 `MsgUIItem.AutoRefresh`）、`EnqueueQueueMsg`/`EnqueueWithLimit`、`FlushQueueToView`（`AutoRefresh && ShowInTaskbar` 才刷视图）、`ClearMsg` 在 VM 已注释。
- `v2rayN/Views/MsgView.xaml(.cs)`：两个 Toggle（`togAutoRefresh`、`togScrollToEnd`）、过滤 ComboBox、复制按钮、清空按钮、右键菜单（全选/复制选中/复制全部/清空）；`ShowMsg` 超 `NumMaxMsg` 先 `ClearMsg`；`ClearMsg` 清文本框并插入 `----- Message cleared -----`；`menuMsgViewCopy_Click` 复制 `SelectedText.TrimEx()`，`menuMsgViewCopyAll_Click` 复制 `txtMsg.Text`。
- `ServiceLib/Services/CoreConfig/V2ray/V2rayLogService.cs`、`SingboxLogService.cs`：`CoreBasicItem.LogEnabled` 决定内核日志配置；`Handler/LoggingHandler` 负责落盘/转发。
- 说明（登记）：任务卡原文写 `LogsViewModel.cs/LogsView.xaml`，冻结提交实际为 `MsgViewModel.cs`/`MsgView.xaml(.cs)`，本卡以实际文件为准。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：引擎单一事件 sink 的 `log_batch`/`log_line` 事件（`bridge_api::engine` 调用 `monitor::ingest_runtime_event`）→ Rust `LogService` 有界环形缓冲（10000 行 / 10MB）。
- 输出：`LogBatchDto`/`LogPageDto`（`LogLineDto{text,level,truncated}` + 溢出/暂停计数）。
- 错误：内核不运行时不产生行、页面显示空态；无伪造行。
- 取消：无独立取消；暂停采集只停止入环，恢复后继续。
- 权限：无 TUN/系统代理/注册表；不占用 10808。
- 持久化：日志为内存环形缓冲，不落库（与上游消息窗口一致；`AutoRefresh`/`MainMsgFilter` 的上游配置回写属 `features/settings`，本卡不改，登记缺口）。
- 生效：内核 apply → runtime Running → `MonitorController` 订阅日志流；LogsView 挂载置 `setPageVisible('logs',true)` 并回读尾部，卸载置 false。

允许修改的模块：`apps/desktop/lib/features/monitor/{logs_view.dart,monitor_controller.dart,monitor_bridge.dart,monitor_format.dart}`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`crates/application/src/monitor.rs`、`crates/bridge_api/src/api/monitor.rs`、`docs/evidence/UX-PARITY-FIX-11C/**`、本卡、`compat` 台账（仅追加）。

禁止改变的已有行为：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/settings|profiles|subs|runtime|update|backup|routing/**`、`crates/application/src/engine.rs`、`crates/updater/**`；不删入口/不降分母；不伪造运行事实；不改 FIX-11/11B 统计与 Clash 语义。

测试夹具和原版预期：`FakeMonitorBridge` 合成日志批；原版预期：三态分离（采集入环、呈现刷新、滚动到底）；过滤不改采集；复制全部=各行原文并补换行；清空插入上游标记；有界丢弃最旧。

本次必须通过的命令/真实场景（实际结果见证据 README）：
- `flutter analyze`
- `flutter test test/fix11c_logs_test.dart test/t15a_logs_test.dart test/t15a_models_test.dart`（18 passed）
- 回归：`flutter test --concurrency=1 test/fix11_monitor_session_test.dart test/fix11b_clash_panel_test.dart test/t15a_statusbar_test.dart test/t15a_proxies_test.dart test/t15a_connections_test.dart test/runtime_controller_test.dart test/t18b_runtime_ui_test.dart`（26 passed）
- `cargo check -p bridge_api --locked`、`cargo clippy -p bridge_api --lib --locked -- -D warnings`（exit 0）
- `cargo test -p bridge_api --lib --locked monitor`（7 passed）、`cargo test -p application --lib --locked monitor`（18 passed）
- `cargo fmt -p bridge_api -- --check`（改动文件无 diff）

证据文件位置：`docs/evidence/UX-PARITY-FIX-11C/`（`README.md`、`observations.json`）。

完成条件：采集有界且暂停语义正确；隐藏页/暂停/自动刷新只影响呈现；过滤为呈现层；复制格式与上游一致；清空插入标记；FIX-11/11B 语义未动；门禁通过。整链实机未跑，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`MsgUIItem.AutoRefresh`/`MainMsgFilter`（FLD-CFG-065/066）的上游配置回写与重开恢复需 `features/settings`/`settings_controller`，本卡禁改，故当前为会话内状态，不回写配置。
- 接口缺口（登记）：上游 `MsgFilter` 用正则并提示非法正则；当前 Dart 用正则优先、非法则退化为不区分大小写子串匹配，未做非法正则提示。
- 接口缺口（登记）：复制选中按上游文本框选择语义实现；当前用 `SelectionArea` 原生选择（Ctrl+C），未提供独立「复制选中」按钮，仅「复制全部」按钮。
- 接口缺口（登记）：上游消息队列上限 500，当前 Dart 呈现上限 2000、Rust 环 10000/10MB（有界，语义一致）。

本轮实际结果：见 `docs/evidence/UX-PARITY-FIX-11C/README.md`。
