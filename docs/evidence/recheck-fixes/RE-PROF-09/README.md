# RE-PROF-09 证据 — 节点表四流量列消费真实统计

状态：`implemented`。日期 2026-10-04；开始 HEAD `472ca54`（开场工作树干净，执行期存在其它并行子代理的未提交改动）。冻结上游 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

本回合只改 `apps/desktop/lib/bridge/bridge_port.dart`（手写 seam）、新增 `apps/desktop/test/reprof09_node_stats_test.dart`、本目录、`docs/tasks/RE-PROF-09.md`、`compat/features.yaml`（仅追加 notes）。未改任何 Rust 源、未改生成桥接 `frb_generated*`、`lib/bridge/api/**`、`main_shell.dart`、`app.dart`、`features/subs|runtime|settings|update|backup|routing/**`、`crates/application/src/{engine,backup_service,subs,dns,routing,speedtest}.rs`、`crates/updater/**`、`crates/subscriptions/**`。未启动内核、未监听任何端口、未改系统代理/注册表/路由/TUN、未读用户凭据。

## 上游对照

- `ServiceLib/ViewModels/ProfilesViewModel.cs:398-437`：`GetProfileItemsEx` 取 `StatisticsManager.Instance.ServerStat`，按 `IndexId` left join 节点，四列 = `t22 == null ? "" : Utils.HumanFy(t22.TodayDown/TodayUp/TotalDown/TotalUp)`；`EnableStatistics` 为假时 `lstServerStat = null`，不展示统计。
- `ProfilesViewModel.cs:304-328`：`UpdateStatistics` 同源把今日/累计写回 `ProfileItems`。
- `Utils.cs:161-180`：`HumanFy` 对 `<=0` 显示 `x.x B`，其后 1024 进制到 PB。
- Rust 现有 `crates/application/src/monitor.rs` 的 `StatsService` 已实现今日/累计 `ServerStatItem` 汇总、跨日 today 归零、`clear_all` 内存+存储清空、SQLite 重开加载；`crates/bridge_api/src/api/monitor.rs` 的 `stats_snapshot()` 已按 `IndexId` 输出四值。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format lib/bridge/bridge_port.dart test/reprof09_node_stats_test.dart` | exit 0；`Formatted 2 files (1 changed)` |
| `flutter analyze` | exit 0，`No issues found!` |
| `flutter test test/reprof09_node_stats_test.dart` | exit 0，5/5 通过（首跑因并行子代理未落盘的 `profiles_controller.dart` 半成品编译错、次跑 `Connection closed before test suite loaded`，重试后全绿） |
| `cargo test -p application --locked --lib monitor::` | exit 0，17 passed / 0 failed（含 `bound_sqlite_store_persists_and_reloads_server_stat_items`、`today_rollover_resets_today_but_keeps_total`、`clear_all_empties_memory_and_store`、`generation_change_resets_session_but_keeps_nodes`） |
| `cargo test -p bridge_api --locked --lib monitor` | exit 0，7 passed / 0 failed（含 `test_injection_updates_snapshot`：`stats_snapshot().nodes[0].index_id == "n1"`、`total_up == 200`，`clear_stats` 后 `nodes` 为空） |

未运行（任务约束）：`flutter build windows --release`、全仓 `flutter test`、全量 `cargo test --workspace`、真实 Windows 窗口、真实内核流量探针。

## 改动要点

- `bridge_port.dart`：新增 `NodeStat`（四值统一 `BigInt`，原生/web 同形）与 `nodeStatFromDto`（把 `NodeTrafficDto` 的 `PlatformInt64` 归一，负值钳零）；新增 `applyNodeStatsOverlay(rows, nodes)` 按 `IndexId` 覆盖四列，未命中保留零默认（对应上游 `t22 == null` 空）。
- `FrbBridgePort.fetchSummaries`：读取 `monitor.statsSnapshot()`，`enabled` 为真时叠加 `nodes`，再叠测速 overlay；读取只读快照，不暂停 1 Hz 采集（保持 FIX-11 语义）。
- `SyntheticBridgePort`：新增 `seedNodeStat/clearNodeStat/statsEnabled` 受控夹具，模型化“监控快照 → 节点表”读链；生产读链仍是 `FrbBridgePort → statsSnapshot()`。
- 删除/换库安全：join 以当前行 id 为准，已删除 id 的统计不会附着到其它行（有专门断言）。
- 未改 Rust、未改生成桥接，复用现有 monitor DTO。

## 测试断言

- `applyNodeStatsOverlay` 按 `IndexId` 命中，未命中保零，非流量列保留。
- 已删除 id 的统计不附着到剩余行。
- `formatBytes(1024) == '1.0 KB'`（1024 进制口径）。
- 控制器层：受控统计夹具下四列非零，重复 `reload()`（等价刷新/轮询/重开）一致。
- `statsEnabled == false` 时受控值不进入节点表（对应上游 `EnableStatistics` 门控）。

## 边界与缺口

- FRB 重生成需求：**无**。未新增桥接函数/字段，直接复用 `monitor.statsSnapshot()` / `NodeTrafficDto`。
- 真实重开依赖已应用会话触发 `monitor_start_polling → sync_from_engine_session` 绑定并加载 SQLite；无会话时不加载存储（未在本卡改动，避免动 FIX-11 语义）。
- `store_bound` 只绑定一次，换库重绑未处理（登记）。
- `HumanFy` 与 `profiles_models.formatBytes` 文案差异（`0.0 B`/PB vs `0 B`/TB）未改；数值口径一致。
- 未做原版实机双窗口逐事件对照；真实内核流量探针未跑。
