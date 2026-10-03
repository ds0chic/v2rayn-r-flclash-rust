# FIX-11 — 内核运行→看速度/统计→切核：applied 会话注入统计管线

状态：`implemented`（统计采集/持久化链路已从 FIX-07 `AppliedSession` 消费并在正常用户入口接线；Rust 单测 + 真实内核 `/debug/vars` 探针通过；未做原版实机双窗口逐事件对照，且未跑「net-host + 真实内核 + Flutter UI 同屏非零」整链，故不写 `verified`）。

任务 ID：FIX-11

本次唯一用户流程：受管内核运行后，状态栏/统计展示出现真实速度与当日/累计流量；切换或停止内核后旧会话采集停止；隐藏页面只暂停页面级 UI 刷新，活动会话的统计采集与持久化继续。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `59dfa97`（注意：本仓工作树含其他并行子代理在 `subs.rs`/`update_service.rs`/`bridge_api/subs.rs`/`runtime/**` 的在途改动，未回退）。审查结论见 `docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 36 行 FIX-11、`runtime-report.md` RT-09/10/17/18/19、`profiles-report.md` PR-22/29、`settings-report.md` SET-18、`root-report.md` ROOT-08。FIX-07 已实现 `AppliedSession`（`proxy_port/session_id/active_index_id/applied_revision`），本卡消费它。

对应 feature / field / action / layout ID：`F-MONITOR-002`（活动统计）、`F-MONITOR-003`（历史统计持久化/清空）、`F-MONITOR-006`（实时速度）、`F-PROFILE-015`（节点统计展示，只读投影）、`FLD-ENT-110..115`（ServerStatItem 字段）、`ENUM-011`、`ACT-MAIN-030`（清除统计）、`LAY-MAIN-006`（状态栏宿主）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/Services/Statistics/StatisticsXrayService.cs`（1 Hz `debug/vars`、`ParseOutput`、`linkBase`）、`ServiceLib/Services/CoreConfig/V2ray/V2rayStatisticService.cs`（`stats`/`metrics.listen=StatePort`/`policy.system.statsOutbound*`）、`ServiceLib/Models/ServerStatItem.cs`（IndexId/Total/Today/DateNow）、`ServiceLib/Manager/StatisticsManager.cs`、`MainWindowViewModel` 的 `UpdateStatistics`/Timer 段与 `ClearServerStatisticsCmd`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：FIX-07 已发布的 applied 会话事实（core、绑定端口、active node、applied revision）；`monitor_start_polling` 每次从 `AppEngine::monitor_session()` 重新读取。
- 输出：`StatsSnapshotDto`/`TrafficBatchDto`（含 `NodeTrafficDto` 行）；状态栏速率与当日累计；`ServerStatItem` 落库。
- 错误：无 applied 会话时不发布端点、端口清零、poller 空转；core/解析失败仅停止本次节奏，不伪造运行事实。
- 取消：停止/换核经正常停止动作；无独立取消入口。
- 权限：无 TUN/系统代理/注册表写入；只读 loopback `debug/vars`；仅停止本项目 PID。
- 持久化：`ServerStatItem`（SQLite `guiNDB.db`），`flush_store` 于每次成功 apply 后；`clear_all` 落库删除。
- 生效：正常启动/`applyActive` → runtime 状态 Running → `MonitorController` 监听触发 `syncSession` → Rust 绑定端点/端口/节点。

允许修改的模块：`apps/desktop/lib/features/monitor/**`、`apps/desktop/lib/features/runtime/**`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`crates/application/src/{engine.rs,monitor.rs,store_repo.rs}`（仅统计/会话最小改动，不动 FIX-07/10 已改区域语义）、`crates/bridge_api/src/api/monitor.rs`、`docs/evidence/UX-PARITY-FIX-11/**`、本卡、`compat` 台账（仅追加）。

禁止改变的已有行为：`main_shell.dart`、两处 `frb_generated`、`features/subs/**`、`features/profiles/**`、`features/update/**`、`crates/updater/**`、`crates/subscriptions/**`；不删入口/不降分母；不伪造运行事实；不占用/修改 10808。

测试夹具和原版预期：合成节点/订阅；真实内核 `tools/cores/xray/v26.3.27`，端口 ≥11808 且先探测。原版预期：`EnableStatistics || DisplayRealTimeSpeed` 时才生成 `stats`+`metrics`；`/debug/vars` 的 `stats.outbound[key].{uplink,downlink}` 按 `proxy`/`direct` 分类；`ServerStatItem` 按活动 `IndexId` 计当日/累计，跨日重置当日、重开恢复、清空删除。

本次必须通过的命令/真实场景（实际结果见证据 README）：
- `cargo check -p application --locked` / `cargo check -p bridge_api --locked` / `rustfmt --check`（改动文件）
- `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings`
- `cargo test -p application --lib --locked monitor`
- `flutter analyze`、`flutter test test/fix11_monitor_session_test.dart`、`flutter test test/t15a_*.dart test/runtime_controller_test.dart test/t18b_runtime_ui_test.dart`
- 真实内核探针：Xray v26.3.27，socks 11850 / metrics 11855（先探测；仅停本任务 PID），经 socks 造流量后 `/debug/vars` 出现非零 `proxy` 统计。

证据文件位置：`docs/evidence/UX-PARITY-FIX-11/`（`observations.json`、`real-core-debug-vars.txt`、`README.md`）。

完成条件：applied 会话注入 core/统计端口/active node，正常入口启用轮询；换核/停核清端口停采；隐藏只暂停页面级刷新；`ServerStatItem` 真实落库、当日/跨日/重开/清空准确；Rust/Flutter 针对性测试与门禁通过。整链实机与节点表投影未做，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：节点表统计列投影在 `features/profiles`（PR-22），本卡不改；`MonitorState.nodes`（`NodeTrafficDto`）已就绪，需 profiles 卡消费。
- 接口缺口（登记）：`AppliedSession` 仍只有单一 `proxy_port`；本卡用 `AppEngine::monitor_session()` 附带的 `(core, state_port, state_port2)` 派生，未扩 `RuntimeDetail`/`SnapshotDto`、未重生成 FRB。
- 拆卡登记：**FIX-11B** = Clash 选择/关闭连接/模式/延迟 URL/重试（PR-29、RT-18）；**FIX-11C** = 日志采集/自动刷新/滚动/过滤/复制（RT-19、SET-18/ACT-MSG-*）。本卡不做这两项全流程。

本轮实际结果：`AppEngine` 新增 `MonitorSession` 与 `monitor_session()/monitor_settings()/traffic_store()`；`StatsService::set_store`；`bridge_api` monitor 的 `sync_from_engine_session` 绑定真实 `SqliteTrafficStore` 并在无会话时清端口；`monitor_start_polling` 先同步再起 poller；Flutter `MonitorController` 监听 runtime 会话、`MonitorState.nodes`、`MonitorBridge.syncSession`。门禁与测试全部通过（见 README 表）；真实内核 `/debug/vars` 非零（`real-core-debug-vars.txt`）。整链实机未跑，保持 `implemented`。
