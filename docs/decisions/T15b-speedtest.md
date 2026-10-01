# T15b 架构决策 — 测速任务与结果闭环

## ADR-1：每节点一个临时内核，而非上游的“单内核多 inbound”

上游 `CoreManager.LoadCoreConfigSpeedtest(List)` 在一个临时内核里为整批节点各建一个
mixed inbound，再对每个端口做 RealPing，省进程数。重构的 `RuntimePlan` / net-host
测试会话以**单内核单配置**为单位，多 inbound 生成尚未实现。

决策：每个节点的 RealPing / 下载 / 混合测试各自开一个隔离的临时内核
（`SpeedTestSession::open`），并发上限用 `MixedConcurrencyCount`（默认 10）而非
page size，避免为 1000 个节点同时拉起 1000 个进程。TCPing 无需内核，仍按 page size
并发。

代价：并发度低于上游；收益：进程隔离清晰、临时内核与主会话完全分离、失败/取消
路径可逐个清理，且不需要为多 inbound 扩展配置生成器。

## ADR-2：UI 用轮询而非 FRB 事件流刷新结果

Rust 侧仍按 50–100ms 批次聚合结果（`SpeedTestRunner::run` 的 `on_batch`），并暴露
`speedtest_subscribe` 批量事件流。但 FRB 流在测试中订阅后取消存在生命周期挂起风险，
且应用侧只需“结果列刷新”这一行为。

决策：`ProfilesController` 在任务运行期间以 150ms 间隔轮询
`speedtest_active_jobs()` 与 `speedtest_results()`，作业结束即停；Rust 侧批次聚合
仍保留为可观测/可接线能力。

## ADR-3：受限测试会话走 net-host，端口地板 11808

`ipc_contract::TestSessionOperation::Open` 携带一个完整 `RuntimePlan`（已由应用层
生成好的配置）+ `max_duration_ms`。net-host 在 `Inner.test_sessions` 中独立持有
临时子进程（JobGuard 保证随 net-host 退出被杀），绝不触碰主会话
`Inner.session`。所有端口强制 ≥ 11808（`check_test_ports` / `find_free_test_port`），
`max_duration_ms` 夹在 1–60s 由 watchdog 兜底关闭。

## ADR-4：SOCKS5 + 明文 HTTP 探针；TLS 未决

新增 `application::speedtest::http_get_via_socks`：手工 SOCKS5 无认证握手 + HTTP/1.0
GET，250ms 轮询读取以便即时取消。本回合真实延迟/下载只在本地明文 HTTP 目标上验证；
`https://` 的 TLS 测量未实现，登记为未决，绝不伪造结果。

## ADR-5：UDP 明确不支持并禁用

`SpeedTestSession::udp_ping` 默认返回 `None`，runner 输出显式
`test_session.udp_unsupported` 失败；`speedtest_supported().udp = false`；右键菜单
UDP 项 `enabled:false`。对齐任务要求“无法真实实现则登记 unresolved 并禁用，不假装”。

## ADR-6：结果闭环与排序稳定

`ProfileExStore`（Rust，全局单例）持有 `ProfileExItem`；bridge 的
`applySpeedTestOverlay` 只在结果字段有效时覆盖 `ProfileSummary` 的
delay/speed/ipInfo。表的排序/滚动锚点仍由既有 `applySort` 的稳定排序保障，结果刷新
不改行序。`ClearServerStatistics`（流量统计）与测试结果分属不同 store，互不误伤。
