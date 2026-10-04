# RE-PROF-09 — 节点表四流量列消费真实统计

状态：`implemented`（Dart 映射用受控统计夹具、Rust `StatsService`/`stats_snapshot` 单测通过；未做原版实机双窗口逐事件对照，也未启动真实内核流量，故不写 `verified`）。

任务 ID：RE-PROF-09

本次唯一用户流程：节点表「今日上传 / 今日下载 / 总上传 / 总下载」四列按节点 `IndexId` 连接监控侧 `ServerStatItem`，展示真实累计与今日流量，而不再恒为 `0 B`。

前置证据：`docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-09 全节（原版 `ProfilesViewModel.cs:311-331/400-437`，当前 `bridge_port.dart:342-345` 只叠测速、`dtoToSummary:815-833` 四值 `BigInt.zero`）。冻结上游 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始 HEAD `472ca54`（执行期存在其它并行子代理的未提交改动）。

对应 feature / field / action / layout ID：`F-PROFILE-015`、`F-MONITOR-003`（流量统计/历史统计与清零）、`FLD-ENT-107/108/109/110`（ServerStatItem `TotalUp/TotalDown/TodayUp/TodayDown`）、`LAY-PROFILES-002`（TodayUp/TodayDown/TotalUp/TotalDown 列）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/ViewModels/ProfilesViewModel.cs:398-437`（`GetProfileItemsEx` 把 `StatisticsManager.Instance.ServerStat` 按 `IndexId` 连接 `ProfileItemModel`，`EnableStatistics` 为假时整组为空，`t22 == null` 时空串）、`:304-328`（`UpdateStatistics` 同源累加）、`ServiceLib/Common/Utils.cs:161-180`（`HumanFy`，1024 进制，缺失不展示）、`crates/application/src/monitor.rs` 的 `StatsService`（`ServerStatItem` 汇总/跨日归零/`clear_all`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`FrbBridgePort.fetchSummaries` 在读取 `queryAllProfiles` 节点行后，额外读取监控 `stats_snapshot()` 的 `nodes`（`NodeTrafficDto`：`index_id/total_up/total_down/today_up/today_down/date_now`）。
- 输出：`applyNodeStatsOverlay` 按 `IndexId` 覆盖节点行四流量列；无匹配统计行的节点保留“未知”零默认（不伪造）。
- 错误：统计未启用（`StatsSnapshotDto.enabled == false`，对应上游 `EnableStatistics`）时跳过叠加，不显示零伪数据；`stats_snapshot()` 自身同步只读，无网络。
- 取消：无（只读快照）。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不监听端口、不改系统代理/TUN。
- 持久化：`ServerStatItem` 由 FIX-11 既有 `StatsService` 写入 SQLite（`store_repo.rs`），重开经 `monitor_start_polling → sync_from_engine_session` 绑定并 `load()`。
- 生效：节点表刷新/测速轮询/重开时 `fetchSummaries` 读取最新快照，四列随之更新；节点删除后行集合不含该 id，映射不会指向旧数据。
- 后台统计语义：读取 `stats_snapshot()` 只读，不暂停 1 Hz 采集；隐藏/不可见不进入该路径，保持 FIX-11 语义。

允许修改的模块（实际）：`apps/desktop/lib/bridge/bridge_port.dart`（手写 seam）、`apps/desktop/test/reprof09_node_stats_test.dart`、本卡、`docs/evidence/recheck-fixes/RE-PROF-09/**`、`compat/features.yaml`（仅追加 notes）。未改任何 Rust 源、未改生成桥接、`main_shell.dart`、`app.dart`、`lib/bridge/api/**`、`features/subs|runtime|settings|update|backup|routing/**`。

禁止改变的已有行为：FIX-11/11B/11C 的统计轮询、Clash、日志语义；测速 overlay 与持久化排序读回；`dtoToSummary` 仍以零为“未知”基值（由 overlay 填充）。

测试夹具和原版预期：受控 `NodeStat` 夹具（`SyntheticBridgePort.seedNodeStat`）模拟监控快照，断言按 id 命中与不命中行为；Rust 侧用真实 `t15_monitor`/`StatsService` 单测与临时 SQLite 验证汇总/跨日/清空/重开。原版预期：`EnableStatistics` 为真时四列按 `IndexId` 连接显示，统计禁用时无统计展示。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib/bridge/bridge_port.dart test/reprof09_node_stats_test.dart`
- `flutter analyze`
- `flutter test test/reprof09_node_stats_test.dart`
- Rust 单 crate：`cargo test -p application --locked --lib monitor::`、`cargo test -p bridge_api --locked --lib monitor`

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-09/`（`README.md`、`observations.json`）。

完成条件：四列按 `IndexId` 消费 `ServerStatItem` 的今日/累计流量不再恒 0；受控夹具下四列非零、刷新/重开一致、统计禁用不显示、删除后不串数据；`stats_snapshot` 与 `StatsService` 汇总/单位/清空/重开单测通过；门禁 format/analyze/test 通过。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记，FRB 重生成需求：无）：本卡直接复用已生成的 `monitor.statsSnapshot()` / `NodeTrafficDto`，未新增桥接函数或字段，故不需要根代理重跑 FRB。
- 边界（登记）：真实“重开”依赖 `monitor_start_polling → sync_from_engine_session` 在存在已应用会话时绑定并加载 SQLite `ServerStatItem`；无已应用会话时该路径不加载存储。若要求“未应用内核时就显示历史统计”，需另开小任务调整 `sync_from_engine_session`（不在本卡，避免动 FIX-11 语义）。
- 边界（登记）：`Store` 仅绑定一次（`store_bound`），换库场景的重新绑定未在本卡处理。
- 显示差异（登记）：上游 `HumanFy` 对 `<=0` 显示 `0.0 B`、单元到 PB；当前 `profiles_models.formatBytes` 到 TB 且 0 显示 `0 B`。数值口径一致（1024 进制），文案差异未在本卡改动。

本轮实际结果：`bridge_port.dart` 新增 `NodeStat`、`nodeStatFromDto`、`applyNodeStatsOverlay` 与 `_statBytes`；`FrbBridgePort.fetchSummaries` 在 `stats.enabled` 时把 `statsSnapshot().nodes` 按 `IndexId` 叠加；`SyntheticBridgePort` 增受控 `seedNodeStat/statsEnabled` 读链夹具。`flutter analyze` 无问题；`reprof09_node_stats_test.dart` 5/5 通过；Rust `application` monitor 17/17、`bridge_api` monitor 7/7 通过。未运行真实内核流量与真实 Windows 窗口。
