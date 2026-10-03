# FIX-11C 证据 — 内核运行→日志页：采集/刷新/滚动/过滤/复制

状态：`implemented`。Flutter 针对性测试与 Rust bridge/application 单测通过；未跑「net-host + 真实内核 + Flutter 同屏非零」整链，也未与上游窗口逐事件对照，故不写 `verified`。

唯一用户流程：内核运行后进入日志（消息）页，行从内核 stdout/net-host 进入有界采集；自动刷新、自动滚动、暂停/继续采集、清空、关键字/级别过滤、复制全部/选中均可用；隐藏页面只冻结页面级呈现，Rust 环继续采集。

## 上游对照（冻结 `7d6a967`）

- `ServiceLib/ViewModels/MsgViewModel.cs`：有界队列 `NumMaxMsg=500`；`MsgFilter` 正则过滤在入队时；`AutoRefresh`（默认 true）+ `ShowInTaskbar` 才把队列刷进视图，隐藏时队列继续累积；`EnqueueWithLimit` 丢弃最旧。
- `v2rayN/Views/MsgView.xaml(.cs)`：两个独立 Toggle（自动刷新 `togAutoRefresh`、滚动到底 `togScrollToEnd`）；`btnCopy`/`menuMsgViewCopyAll` 复制 `txtMsg.Text`；`menuMsgViewCopy` 复制选中；`ClearMsg` 清文本框并插入 `----- Message cleared -----`；`ShowMsg` 超限先清。
- `V2rayLogService.cs`/`SingboxLogService.cs`：`CoreBasicItem.LogEnabled` 决定内核日志开关；行由 `LoggingHandler` 转发到消息队列。
- 差异登记：冻结提交中该页是 `MsgView`/`MsgViewModel`（非任务卡原文的 `LogsView`/`LogsViewModel`）。

## 本轮改动

- `apps/desktop/lib/features/monitor/monitor_controller.dart`
  - `MonitorState.autoRefresh`（呈现刷新开关，默认 true）；`visibleLogs` 关键字改为正则优先、非法正则退化不区分大小写子串。
  - `setMinLevel` 改为纯呈现层（不再下推 Rust 过滤，环内保留全部行）。
  - `_onLogs` 在 `batch.collectingPaused` 或页面隐藏或自动刷新关闭时冻结列表追加；新增 `_reloadLogs()` 在恢复可见/自动刷新时回读 Rust 尾部（按 `total-maxDisplayedLogs` 取最新页）。
  - `setPageVisible('logs',true)` 触发 `_ensureStreams()+_reloadLogs()`；隐藏只置标志，采集不动。
  - `setAutoRefresh()`；`clearLogs()` 清环并插入上游 `----- Message cleared -----` 标记。
- `apps/desktop/lib/features/monitor/logs_view.dart`
  - 挂载读 provider notifier 存字段，卸载时 `setPageVisible('logs',false)`（避免 `ref` 在 dispose 报错）。
  - 控件：关键字、级别、自动刷新、自动滚动、暂停/继续采集、复制全部、清空；列表包 `SelectionArea`（支持选中+Ctrl+C）。
- `apps/desktop/lib/features/monitor/monitor_format.dart`
  - `formatLogsForCopy()`：各行原文拼接，缺尾换行则补，等价上游 `txtMsg.Text`。
- `crates/bridge_api/src/api/monitor.rs`
  - `ingest_runtime_event`：采集暂停时不再把原始行 fan-out 给订阅者（RT-19：采集 vs 呈现）。
  - 新增单测 `paused_collection_drops_incoming_batch`。
- 测试：`apps/desktop/test/fix11c_logs_test.dart`（8 例）；更新 `t15a_logs_test.dart`（清空启用+标记、级别过滤仅呈现）；`support/fake_monitor_bridge.dart` 增 `setLogFilterCalls` 与 `emitLogs` 暂停参数。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `flutter analyze` | No issues found |
| `flutter test test/fix11c_logs_test.dart test/t15a_logs_test.dart test/t15a_models_test.dart` | 18 passed / 0 failed |
| `flutter test --concurrency=1 test/fix11_monitor_session_test.dart test/fix11b_clash_panel_test.dart test/t15a_statusbar_test.dart test/t15a_proxies_test.dart test/t15a_connections_test.dart test/runtime_controller_test.dart test/t18b_runtime_ui_test.dart` | 26 passed / 0 failed |
| `dart format --output=none --set-exit-if-changed`（改动文件） | 0 changed（格式化后） |
| `cargo check -p bridge_api --locked` | exit 0 |
| `cargo clippy -p bridge_api --lib --locked -- -D warnings` | exit 0 |
| `cargo test -p bridge_api --lib --locked monitor` | 7 passed / 0 failed |
| `cargo test -p application --lib --locked monitor` | 18 passed / 0 failed |
| `cargo fmt -p bridge_api -- --check` | 改动文件无 diff |

注：一次多文件并行 `flutter test` 出现 `t15a_statusbar_test` “did not complete” 的偶发并发假象，单独运行与 `--concurrency=1` 重跑均通过。

## 未完成 / 接口缺口

- 未运行整链实机（net-host + 真实内核 + Flutter 同屏）非零验收；逻辑已接线。
- `MsgUIItem.AutoRefresh`/`MainMsgFilter` 配置回写与重开恢复需 `features/settings`（本卡禁改），当前为会话内状态。
- 非法正则提示、独立「复制选中」按钮未做；当前正则退化子串 + `SelectionArea` 原生复制。
- 队列上限：上游 500，当前呈现 2000 / Rust 环 10000 行（均属有界）。

## 与门禁台账

`compat/actions.yaml` 中 `ACT-MSG-001/002/003` 仍为 `identified` 且 `implementation_location: null`。因台账硬约束为“仅追加不删行”，本轮未改写既有行；建议根代理据本卡把三项状态/实现位置/测试 id 更新为 `implemented` 与 `apps/desktop/lib/features/monitor/logs_view.dart` 等。
