# FIX-11 证据 — 内核运行→看速度/统计→切核

状态：`implemented`。统计采集/持久化链路已从 FIX-07 `AppliedSession` 消费并在正常用户入口接线；未做原版实机双窗口逐事件对照，也**未**跑通「net-host + 真实内核 + Flutter UI 同屏非零」的整链，故不写 `verified`。

唯一用户流程：内核运行后，在 StatusBar / 节点统计看到真实速度与当日/累计流量，切换或停止内核后旧会话采集停止，隐藏页面只暂停页面级 UI 刷新、不停止采集与落库。

## 上游对照（冻结 `7d6a967`）

- `ServiceLib/Services/Statistics/StatisticsXrayService.cs`：1 Hz 轮询 `http://127.0.0.1:{StatePort}/debug/vars`，`ParseOutput` 读 `stats.outbound[key].{uplink,downlink}`，`key.StartsWith("proxy")` 计 Proxy、`key == "direct"` 计 Direct，差值后交给 `MainWindowViewModel.UpdateStatistics`。
- `ServiceLib/Services/CoreConfig/V2ray/V2rayStatisticService.cs`：`GuiItem.EnableStatistics || DisplayRealTimeSpeed` 时写 `stats`/`metrics.listen=StatePort`/`policy.system.statsOutboundUplink|Downlink`。
- `ServiceLib/Models/ServerStatItem.cs`：`IndexId/TotalUp/TotalDown/TodayUp/TodayDown/DateNow`。
- 本仓库 `crates/core_adapters/src/stats/xray.rs`（嵌套 JSON 解析）与 `crates/application/src/monitor.rs::StatsService`（当日/跨日/累计、按 active node 归属）与上述一致。

## 本轮改动

- `crates/application/src/engine.rs`
  - 新增 `MonitorSession` 与 `AppEngine::{monitor_settings, monitor_session, traffic_store}`；`apply_runtime` 记录申请通过时的 `(core, state_port, state_port2)`；`monitor_session` 仅在 FIX-07 `AppliedSession` 存在时发布，未 apply 的 desired 节点不发布。
- `crates/application/src/monitor.rs`
  - `StatsService::set_store`，用于把真实 `ServerStatItem` 存储绑定进服务。
- `crates/bridge_api/src/api/monitor.rs`
  - `sync_from_engine_session`：从 engine 拉取 applied 会话的 core/统计端口/active node，绑定 `SqliteTrafficStore` 并 `load()`；无会话时清空端口使 poller 空转（停采旧会话）。
  - `monitor_start_polling` 每次调用先同步，再惰性启动 1 Hz poller（正常入口，非测试钩子）。
- `apps/desktop/lib/features/monitor/monitor_controller.dart`
  - `MonitorState.nodes`（ServerStatItem 只读投影）；`build()` `ref.listen` runtime 会话；`syncRuntimeSession` 在 `hasAppliedEndpoint` 时同步 + 订阅流 + 拉快照。
- `apps/desktop/lib/features/monitor/monitor_bridge.dart`：新增 `syncSession()`。
- 测试：`apps/desktop/test/fix11_monitor_session_test.dart`、`apps/desktop/test/support/fake_monitor_bridge.dart`。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo check -p application --locked` | exit 0 |
| `cargo check -p bridge_api --locked` | exit 0 |
| `rustfmt --check`（改动文件） | exit 0 |
| `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test -p application --lib --locked monitor` | 14 passed / 0 failed（含新增 `bound_sqlite_store_persists_and_reloads_server_stat_items`、`monitor_session_consumes_applied_session_facts`） |
| `flutter analyze` | No issues found |
| `flutter test test/fix11_monitor_session_test.dart` | 3 passed / 0 failed |
| `flutter test test/t15a_*.dart test/runtime_controller_test.dart test/t18b_runtime_ui_test.dart` | All tests passed |

真实内核探针（pinned Xray v26.3.27，socks 11850 / metrics 11855，端口先探测；仅停本任务 PID；不打系统代理/TUN/10808）：

```
"stats": {"inbound":{"api":{...},"socks-in":{"downlink":1311635,"uplink":465}},
          "outbound":{"proxy":{"downlink":1311575,"uplink":395}},"user":{}}
```

即 app 所用 `parse_xray_vars` 的嵌套结构在真实内核上确实非零。原始记录见 `real-core-debug-vars.txt`。注意：探针早期全 0 是 curl 尊重宿主 `no_proxy=127.0.0.1` 而绕过 socks 造成的测试假象，非内核或解析缺陷。

## 未完成 / 接口缺口

- 未运行「net-host + 真实内核 + Flutter 同屏」整链非零验收；只有 Rust 管线单测 + 真实内核 `/debug/vars` 探针。整链验收的前置：在隔离 data dir 启核后由 `MonitorController` 正常入口触发（已接线，未实机跑）。
- 节点表统计列投影（`F-PROFILE-015` / PR-22）在 `features/profiles`，本卡不改；`MonitorState.nodes` 已就绪，需 profiles 卡消费。
- Clash 选择/关闭/模式/延迟 URL/重试（PR-29）登记为 **FIX-11B**；日志采集/刷新/滚动/过滤/复制（RT-19/SET-18）登记为 **FIX-11C**。本卡不做其全流程。
- `ROOT-08` 的 `ShowClashUI` 随内核可见性条件在 `main_shell`/`side_tabs`，本卡不改。
