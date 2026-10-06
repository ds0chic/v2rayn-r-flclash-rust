# SP-22 高频状态和大IO背压 — 证据

基线 `393fafd`，不 commit。缺陷依据 CP-15（同步全量读取/overlay；`docs/evidence/complete-port-audit-2026-10-06/README.md` §CP-15），
生命周期隔离依据 CP-17（SP-07 已修：控制事件与高频日志隔离）。前置 SP-07/SP-20/SP-21 是否 verified 以各自证据为准，
本文不宣称。

本次唯一用户流程：用户测速同时浏览日志/连接并筛选节点，界面不被全量 overlay 与 IO 阻塞。

## 改动（仅用户指定写锁内）

- `crates/application/src/monitor.rs`（SHA256 `30C2FC8F…32F98CD`）：
  - `merge_delay_map`（测速延迟按 ID 增量覆盖）、`coalesce_log_batch`（单 burst 截尾，上限 `MAX_LOG_BATCH_LINES=2000`）、
    `LogService::ingest_lines`（burst 限流入口，挤掉头部计入 overflow，不丢尾部最终结果）；
  - `ClashApiService.client` 改为 `Arc<ClashApiClient>`（对外签名不变：`new`/`from_client`/`client()`/`supported`/
    各 async 方法签名均不变；`crates/bridge_api/src/api/monitor.rs` 与 `tests/t15_monitor.rs` 调用方无需改动）；
  - `group_delay` 改串行为 8 路有界并发（`MAX_GROUP_DELAY_CONCURRENCY=8`，`JoinSet`+`Semaphore`，无新增依赖），
    保持控制器顺序、失败记 `-1`（上游约定），最终向量完整（最终结果不丢）；
  - 新增 `group_delay_with_progress`（逐探针 `progress` 回调做增量覆盖；future 丢弃即安全点取消，在途探针 abort，
    不发布半份结果）。
- `apps/desktop/lib/features/monitor/monitor_incremental.dart`（新增，纯函数）：
  `logCoalesceWindow/filterDebounceWindow=150ms`、`mergeLogTail`（有界尾部合并，不 mutated 输入）、
  `mergeDelayMap`、`mergeConnectionsById`（存活原位更新/新行追加/消失丢弃）、
  `filterConnections`（有界 `maxFilteredConnections=300`，early-exit，返回命中总数+截断标记）、
  `sortProxiesForDisplay`（复制后排序，不 mutated 状态）。
- `monitor_controller.dart`：`_onLogs` 改为 150ms 合并刷新（计数器/暂停标记每批立即生效，行覆盖一次有界尾合并；
  traffic 生命周期旁路合并窗口）；`testProxy`/`testGroup` 走按 ID 覆盖合并；`clearLogs` 同步清空待刷新缓冲。
- `connections_view.dart`：筛选输入 150ms 防抖 + 有界筛选（超量截断明示“显示 X / 命中 Y”）；不再每键全量扫描建表。
- `proxies_view.dart`：`build` 内原地排序改为纯复制排序（修复每次重建 mutated 共享 read model 并触发全量重排）。
- `logs_view.dart`：关键词输入 150ms 防抖。
- `apps/desktop/test/repair/sp_22_backpressure_test.dart`（新增，7 测试，合成数据，无宿主网络）。
- Rust 新测试：`monitor.rs` 内 `sp22_*` 4 测试（合成 stub，端口≥11808 探测，10808 未触碰）。

## 先红后修

红：新测试引用尚不存在的 API，Rust `cargo test` 编译失败（`merge_delay_map`/`coalesce_log_batch`/
`ingest_lines`/`group_delay_with_progress` 未找到），Dart `flutter test` 加载失败（`monitor_incremental.dart`
缺失）。修后绿（见下）。另修正两处测试自身误设：Dart `mergeConnectionsById` 期望误留快照已消失行；
Rust 并发测试误把 `with_progress` 返回的名字向量当 `(name, delay)` 对。

## 定向检查

说明：主仓工作树在执行期间有其他 agent 并发改动（`engine.rs`/`tun_plan.rs`/`net_host/*` 等），且中途一次
`git stash` 卷入并清空了本卡已改文件（后按 stash 内容原样恢复，`monitor.rs` 以验证态 hash 为准；
`git stash@{0}` 仍保留他人文件，本卡未动它）。Rust 全量编译在主仓内被他人 `tun_plan.rs` 破坏阻塞，
故 Rust 验证在干净 worktree（`393fafd` + 仅本卡 `monitor.rs` patch）完成；Dart 验证在主仓完成。

| 命令 | 结果 exit |
|---|---|
| 干净 worktree `cargo test -p application --locked --lib`（334 passed，含 monitor 29/新 sp22 4） | 0 |
| 干净 worktree `cargo test -p application --locked --test t15_monitor`（2 passed，`group_delay` 回归） | 0 |
| 干净 worktree `cargo clippy -p application --all-targets --locked -- -D warnings` | 0 |
| 干净 worktree `cargo fmt -p application -- --check`（本卡文件无 diff） | 0 |
| 主仓 `dart format --output=none --set-exit-if-changed lib/features/monitor test/repair/sp_22_backpressure_test.dart` | 0 |
| 主仓 `flutter analyze` | 0（No issues） |
| 主仓 `flutter test test/repair/sp_22_backpressure_test.dart test/fix11_monitor_session_test.dart test/repair/sp_20_connections_full_test.dart test/repair/sp_20_connections_prep_test.dart`（29 passed：sp22 7 + 既有 monitor/log 22） | 0 |

`t10_core_matrix` 在干净 worktree 失败：缺 `tools/cores/xray/v26.3.27/xray.exe`（真实内核二进制环境缺失，
与本卡无关，未运行真实核）。端口：Rust 并发测试用 `≥11808` 探测绑定，`10808` 未触碰；宿主代理/路由/TUN/DNS/
Run-key 无写入；合成数据 only（`*.example.invalid` / `flood N` / `SPEEDTEST DONE id=syn-final`）。

## 未验证 / 阻塞（如实）

1. 主仓内 `cargo test -p application`/`clippy` 被他人 `tun_plan.rs` 未提交破坏阻塞（写锁外，未动）；本卡 Rust
   证据来自干净 worktree，待该文件 owner 修复后由 SP-34 整合门禁复验。
2. 10k 节点 + 10k 连接 + 测速日志并发真实核采样、队列/内存无持续增长、generation 切换/取消恢复的整机采样，
   需真实内核与 SP-31 性能门禁，verity 未做；本卡做到纯合同 + 合成并发 + 有界链路。
3. `group_delay_with_progress` 的逐项 UI 流式刷新需 bridge/FRB 新增接口（共享 DTO 写锁在整合者），本次 Dart 侧
   仍为整组一次合并（Rust 侧已并发加速 + 保序 + 全量），缺口已登记，不削减需求。
4. 任务卡 `tasks/SP-22.md` 与 `execution-manifest.json` 状态未改（写锁外：只允许 monitor/**、`monitor.rs`、
   本卡测试与证据）。本卡当前自评 `implemented`（正确合同绿；整机采样与发布门禁未做，不标 `verified`），
   请整合者按本证据更新卡与 manifest。
