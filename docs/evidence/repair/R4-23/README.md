# R4-23 监控连接日志统计 证据

状态：implemented（2026-10-05，HEAD a95897f）。Dart 控制器契约测试（10）与复现测试（2，修复前失败）、Rust Clash 本地 mock 测试通过；`flutter analyze` 无问题；Rust 全门禁通过；`flutter build windows --release` 通过。真实 mihomo/sing-box Clash 核心、真实慢盘 GUI 帧/命令耗时未实测，故未 verified。

HEAD/基线：应用 HEAD `a95897f`，应用基线 `77c74ed`，上游冻结 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；armed=false。测试夹具只用合成统计/本地 mock；未占用/修改 10808；未改宿主系统代理/注册表/路由/TUN/自启；未读取用户凭据；未终止任何非本项目进程。

## 本卡唯一用户流程

运行中查看监控并切会话或隐藏后再查看。

## 缺陷与方案

- D25（源码确认）：`MonitorController` 的异步 Clash 读取（`refreshConnections/refreshProxies/refreshClashMode`）无会话版本淘汰，且连接自动刷新定时器不合并请求。慢响应在切核/停核后到达会把旧会话的连接/代理覆盖到新会话上；两个定时器 tick 可发起重叠请求并按乱序覆盖。
- 修复：控制器新增 `_sessionGeneration` 与每类请求 token。`syncRuntimeSession` 检测到 applied 会话签名变化时推进代际、失效 token、清空旧会话的连接/代理/模式读模型（统计 per-node 行与日志环是会话无关的，保留）。异步读取在应用结果前校验「代际 + 最新 token」；命中同一代际的在途连接/代理请求合并为一个。关闭/全关/选择代理等变更操作后强制发起一次新读取（新 token），使先前的慢响应作废。
- Rust 侧：R4-10 已把滚动统计落盘移出 hub 共享读锁（`persist_rows` 在锁外执行），本轮不回退；R3-06 的 WS 调度、R3-07 的 Custom secret、RR-MON-REBIND 的可重试 store bind 均保留。仅新增一个 Clash 本地 mock 集成测试，未改任何生产 Rust 逻辑、未改 FRB 生成物。

## 改动文件

- `apps/desktop/lib/features/monitor/monitor_controller.dart`：会话代际淘汰 + 在途合并；切会话清空旧 Clash 读模型；变更操作后强制重读。
- `apps/desktop/test/support/fake_monitor_bridge.dart`：新增 `connectionsGate`/`proxiesGate` 测试闸门，用于在会话切换期间保持读取在途。
- `apps/desktop/test/r4_23_contract_test.dart`（新增，10 项契约）。
- `apps/desktop/test/repair/r4_23_repro_test.dart`（新增，2 项复现，修复前失败）。
- `crates/bridge_api/src/api/monitor.rs`：新增测试 `clash_connections_mock_lists_and_closes_over_floor_port` 与本地 Clash HTTP mock（仅测试代码）。
- `docs/repair/tasks/R4-23.md`：状态与执行记录。
- `docs/evidence/repair/R4-23/**`：本证据。

## 必过场景覆盖

| 场景 | 证据 |
|---|---|
| 采集暂停与滚动暂停分开 | 契约：`collection pause and scroll pause are independent`；Rust `log_filter_and_pause` |
| 连接/组操作是真 API | 契约：`closing a connection uses the real bridge...`；Rust mock `clash_connections_mock_lists_and_closes_over_floor_port`（GET /connections、DELETE /connections/{id}、DELETE /connections/） |
| 迟到响应按 session 淘汰 | 复现/契约：`a late connection read from a replaced session is dropped`、`late proxes/mode reads from a replaced session are dropped` |
| 锁外 flush | R4-10 保留：`persist_rows_writes_through_store_handle_without_the_service` + poll 锁外落盘；本轮未回退 |
| 归属 actual 目标 | 保留：engine `monitor_session()` 取 applied_session + apply_facts（`engine.rs` 测试 `applied_session_reports_apply_target_not_later_selection`） |
| 隐藏页不停止采集 | 契约：`hiding the page freezes the view but never the collection`；`fix11c_logs_test.dart` |
| 统计重开 | 契约：`applied session binds statistics...`（会话独立保留 per-node 行）；Rust `bound_sqlite_store_persists_and_reloads_server_stat_items` |
| 统计行非零（R3-06 不回退） | Rust 通过：`production_hub_drives_singbox_ws_statistics`；契约断言 `hasTodayNodes`/`todayUp` |
| 绑定失败可见可重试（RR-MON-REBIND 不回退） | 契约：`a store bind/load failure is visible and cleared on retry`；Rust `store_bind_failure_is_visible_and_retryable` |

## 实际命令与结果

- 复现（修复前，临时 `git stash` 还原控制器）：`flutter test test/repair/r4_23_repro_test.dart` → 2 failed（旧会话连接被套用；重叠请求 2 次）。随后 `git stash pop`。
- 修复后：`flutter test test/r4_23_contract_test.dart test/repair/r4_23_repro_test.dart` → All tests passed（12）。
- `flutter analyze`（apps/desktop）→ No issues found。
- `dart format --output=none --set-exit-if-changed <4 个文件>` → 0 changed。
- `cargo fmt --all -- --check` → clean。
- `cargo clippy --workspace --all-targets --locked -- -D warnings` → Finished，无警告。
- `cargo test --workspace --locked` → EXIT=0，所有 test result: ok。
- `cargo test -p bridge_api --locked clash_connections_mock_lists_and_closes_over_floor_port` → 1 passed（本地 mock 端口由 `bind_floor_listener` 取首个空闲 >=11808）。
- `flutter build windows --release` → Built `build\windows\x64\runner\Release\v2rayn_desktop.exe`。

## 未完成/未验证

- 真实 mihomo/sing-box Clash API、真实连接关闭副作用未在真机核心上验收（未下载/启动外部内核）；本地 mock 只证明请求路径与 DTO 映射。
- 慢盘下同步端点/UI 帧与命令耗时未做定量测量（R4-10 已登记）。
- 其他并行子代理编辑 `profiles_controller.dart`、`bridge_api/src/api/speedtest.rs` 时曾短暂使全仓不可编译（已由对方修复，非本卡改动）；`t18b_runtime_e2e` 的 socks auth 断言在并行满载下偶发失败、单独运行时通过（EXIT=0）。

## 下一步前置

- 接口整合者无需 FRB 重生成：本轮未改任何 `#[frb]` 函数签名（仅控制器内部与测试）。
- 真实核心 Clash 连接/关闭与慢盘 GUI trace 需在授权隔离环境补测后才可提升 verified。
