# RR-MON-REBIND — 监控重绑：只按 applied 会话变化重建 source，store 失败可见可重试

状态：`implemented`（Rust monitor tests 8/8、Dart 3 用例通过；未做真实内核 + Flutter 同屏非零流量整链，故不写 `verified`）。

任务 ID：RR-MON-REBIND

本次唯一用户流程：受管内核运行后，仅当日志/心跳等无关 `RuntimeView` 变化时不再重建统计 source（保持 generation 与 WS collector）；当 applied 会话（core/端点端口/统计开关/活动节点）真实变化或停止时正确重建；`ServerStatItem` store 打开/加载失败可见并可重试，切换 Engine/dataDir 后重新绑定。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；当前复核 HEAD `cbf4fa7`。复核结论见 `docs/evidence/parity-recheck-2026-10-04/runtime.md`「新接监控仍需重点实测的调度风险」第 1、3 条（`:118`、`:120`）。FIX-11 已实现 applied 会话注入统计管线；本卡只修其调度与 store 绑定边界，不改统计/Clash/日志语义。

对应 feature / field / action / layout ID：`F-MONITOR-002`（流量统计）、`F-MONITOR-003`（历史统计与清零）、`F-PROFILE-015`（节点统计展示）。

必读上游文件、符号和固定 commit：`v2rayN/ServiceLib/Manager/StatisticsManager.cs`、`ServiceLib/Services/Statistics/StatisticsXrayService.cs`（source 生命周期）、`StatisticsSingboxService.cs`（WS collector）；当前实现 `crates/bridge_api/src/api/monitor.rs`（`sync_from_engine_session`/`poll_loop`/`build_source`/`MonitorHub`）、`apps/desktop/lib/features/monitor/{monitor_controller.dart,monitor_bridge.dart}`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`AppEngine::monitor_session()`（core/state_port/state_port2/active_index）、`monitor_settings()`（EnableStatistics/DisplayRealTimeSpeed）、`data_dir()`/`traffic_store()`；Flutter 侧 `RuntimeView`。
- 输出：仅真实变化时 `source_sig=None` → poller 重建 `StatsSource`；`StatsSnapshotDto.error` 携带 store 绑定错误；`MonitorState.error` 展示该错误。
- 错误：store 打开/加载失败 → `store_error` 置位、`store_bound=false`（可重试），不再静默 `bound=true`。
- 取消：无独立取消；停止会话时端口清零、poller 空转。
- 权限：只读 loopback；无系统代理/注册表/TUN；只停本项目 PID/端口；禁占 10808。
- 持久化：`ServerStatItem`（SQLite `guiNDB.db`），仅成功绑定后 `load()`；dataDir 变化重建绑定。
- 生效：正常启动/`applyActive` → `MonitorController` 监听 → 仅签名变化调用 `syncSession` → Rust 仅在 applied 变化时重建。

允许修改的模块：`apps/desktop/lib/features/monitor/{monitor_controller.dart,monitor_bridge.dart}`、`crates/bridge_api/src/api/monitor.rs`、`crates/application/src/monitor.rs`（本卡未改）、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/RE-PROF-12-MON/**`、`compat` 台账（仅追加）。

禁止改变的已有行为：`crates/application/src/engine.rs` 等禁改文件、`main_shell.dart`、两处 `frb_generated`、`lib/bridge/api/**`；不改 FIX-11/11B/11C 统计轮询/Clash/日志语义，不改 RR-07/10 端点/预检语义；不删入口/降分母。

测试夹具和原版预期：合成 store 目录 + 打开失败/成功闭包；合成 RuntimeView（同 applied、仅 heartbeat seq 变化）vs 真实端点变化。原版预期：source 不因无关日志/心跳重建，generation 与 WS collector 保持；store 失败可见并恢复后重绑。

本次必须通过的命令/真实场景（实际结果见证据）：
- `cargo fmt -p bridge_api -- --check`
- `cargo test -p bridge_api --lib monitor::tests --locked`
- `flutter analyze`
- `flutter test test/rr_mon_rebind_test.dart`（隔离运行）

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-12-MON/README.md`。

完成条件：无关 RuntimeView 变化不触发 source 重建、真实 applied 变化触发；store 打开/加载失败在快照错误可见且可重试、dataDir 变化重绑；上述命令通过。真实内核整链未跑，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`stats_snapshot()` 已有 `error` 字段可承载 store 绑定错误，未新增 IPC；若后续需要专门的 store 状态/重绑命令，需新 FRB 契约并重生成桥接，本卡不造。
- 接口缺口（登记）：`poll_loop` 仍为 `current-thread` Tokio + 阻塞 sleep（runtime.md 风险第 2 条），本卡未改；需独立卡在同一 production hub + 真实 sing-box 场景验证 WS pump 调度。

本轮实际结果：`bridge_api/src/api/monitor.rs` 新增 `store_dir/store_error/session_sig`，`sync_from_engine_session` 仅在 applied 签名变化时清 `source_sig`，`rebind_store` 在打开/加载失败时置 `store_bound=false` 并写 `store_error`（经 `stats_snapshot().error` 暴露）、dataDir 变化时强制重绑；`MonitorController.syncRuntimeSession` 以 `monitorSessionSignature` 去重，`refreshStats` 将快照错误映射到 `MonitorState.error`。Rust `monitor::tests` 8/8（含新增 `store_bind_failure_is_visible_and_retryable`），Dart RE-PROF-12/RR-MON-REBIND/FIX-11 共 12/12 通过；`flutter analyze` 仅报与本卡无关的既有 warning。
