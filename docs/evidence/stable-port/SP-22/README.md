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

## Continuation 2026-10-06：real-scale（10k 节点/连接 + 日志洪水）

基线 `467607e`，不 commit。本轮只新增两个测试文件 + 本证据（`monitor.rs` 与 Dart 业务代码零改动，
沿用上一轮已验证的增量 overlay/背压链路）。

### 新增

- `crates/application/tests/sp22_scale.rs`（6 测试，合成数据；stub 用 `tiny_http` 绑定探测到的
  `>= 11808` 回环端口，`10808` 未触碰）：
  - 10k 节点延迟 overlay：`merge_delay_map` 10k 项全量进入 map，复测按 ID 原位覆盖不膨胀；
  - 50k 行洪水（10×5000）：默认 ring（10k 行/10MiB）只保留 10k，`rejected=30000`（单 burst 截尾头）、
    `accepted=20000`、`dropped_lines=10000`，`accepted == total + dropped_lines` 闭环；
    控制队列存活，最终 `syn-final` 标记存活；
  - 字节预算：64 KiB 下 10k×~200B 只保留 350 行，`dropped_bytes=1804550` 计数；
  - 单 10k burst：`coalesce_log_batch` 只收尾部 2000，shed 头上报；
  - generation 切换：gen7 冻结的 close 在 gen8 判 stale，空 id 拒绝；
  - 取消恢复：300ms×4 慢探针中途 abort 发布零行；换新 stub 重跑全量有序恢复。
- `apps/desktop/test/repair/sp_22_scale_test.dart`（9 测试，`FakeMonitorBridge`，无 socket）：
  - 10k 覆盖/排序/连接合并/有界筛选/日志尾合并；
  - dropped/truncated 计数器先行于 150ms 行合并生效且 flush 后保持；
  - 会话切换即时清空旧行 + 在途 `refreshConnections` 被 gate 卡住时切换，late 响应丢弃；
    切换后新会话重读恢复；空 close id 永不到 bridge。

### 实测数字（命令原文见 `logs/sp22-scale-2026-10-06.log`）

Rust（debug，`--nocapture`）：10k overlay 6.7ms；50k 洪水 7.3ms
（total=10000，dropped_lines=10000，dropped_bytes=422000，rejected=30000，accepted=20000）；
字节预算 total=350/dropped=9650；恢复 `group_delay` 302.7ms（4×300ms 并发，串行需 ~1200ms）。
Dart（flutter_tester）：mergeDelayMap 3ms；sort 10k 10ms；mergeConnections 3ms；
filter 10k 17ms（命中 10000→返回 300 + 截断标记）；mergeLogTail 0ms；testGroup 10k 1ms。

### 定向检查（本轮，exit 均为 0）

`cargo fmt -p application -p subscriptions -- --check`；
`cargo clippy -p application -p subscriptions --all-targets --locked -- -D warnings`；
`cargo test -p application --locked`（lib 342 + 全部集成 suite，0 失败）；
`cargo test -p subscriptions --locked`（回归，0 失败）；
`dart format`（相关文件）、`flutter analyze`（No issues）；
`flutter test` 相关 5 文件共 38 passed
（sp22-scale 9 + sp22-backpressure 7 + fix11 3 + sp20-full 10 + sp20-prep 9）。
一次 compact-reporter 合跑曾报 4 个 SP-20 widget 用例 "did not complete"
（flutter_tester flake，无断言失败）；单文件重跑通过，`--reporter expanded` 合跑 38/38。

### 仍未验证（诚实口径：`implemented`，非 `verified`）

1. 真实内核 10k 节点 + 10k 连接 + 测速日志并发的整机采样、队列/内存无持续增长的长稳曲线，
   需真实内核与 SP-31 性能门禁；本轮是合成规模 + 真实本地 HTTP 控制器桩。
2. `group_delay_with_progress` 逐项 UI 流式刷新仍缺 bridge/FRB 接口（上一轮已登记，不削减需求）。
3. Workspace 全量门禁与 release 构建未跑（VALIDATION_POLICY：发布候选才跑，由 SP-34 整合）。
4. 本轮 `monitor.rs` 与 Dart 业务代码零改动；`crates/subscriptions` 未动（仅回归测试）。

## Continuation 2026-10-07 — real locked-core loopback sample

The prior `sp22-realcore-2k-2026-10-07.log` is an incomplete pre-fix attempt and is not a pass. The corrected fixture was rerun with the locked Mihomo 1.19.32 binary on probed loopback ports 21808–21810, an explicit temporary data directory, and a `MATCH,DIRECT` route. It now resets accepted upstream sockets to blocking mode and keeps transient idle timeouts from prematurely closing them. The exact commands, core/source hashes, output metrics, cleanup result, and known limits are in [`logs/sp22-realcore-rerun-2026-10-07.log`](logs/sp22-realcore-rerun-2026-10-07.log).

The 50-connection run passed in 6.56s: 50/50 echoed, table completeness 100%, and session totals matched 51,200 bytes in each direction. The default 2,000-connection run passed in 7.70s: 2,000/2,000 echoed, table completeness 100%, five controller polls at 37–54ms, session totals matched 2,048,000 bytes in each direction, the single-close action appeared in the live table, and the core exited with no residual test listener. `cargo fmt -p application -- --check` and targeted Clippy both exited 0.

This closes the earlier 2,000-connection smoke-test failure, but SP-22 remains `implemented`, not `verified`: the 10k-node/10k-connection long-stability run, UI/FRB acceptance, and 24-hour memory/queue-growth checks remain outstanding.

## Continuation 2026-10-07 — 10k real-connection run (integrator rerun)

After the fixture fix above, the integrator reran the same locked-core test at
the full scale with a strict wall-clock budget:
`SP22_REALCORE_CONNS=10000 cargo test -p application --locked --test sp22_realcore -- --nocapture`
(raw transcript: [`logs/sp22-realcore-10k-2026-10-07.log`](logs/sp22-realcore-10k-2026-10-07.log)).

Result (passed in 12.96s, exit 0):

- established 10000/10000 in 2.93s (dial fail 0); echo round trip io_fail=0,
  upstream bytes 10,240,023 each way;
- connection-table completeness 1.000 (10000/10000 seen), five controller
  polls at 217–244ms at 10k live connections, session totals 10,240,000 bytes
  each direction;
- single-close of a live connection reflected in the table; core log flood
  through the real pipeline: 20,008 lines ingested, ring kept 10,000,
  dropped_lines=10,008 / dropped_bytes=1,055,794, rejected=0;
- mihomo RSS 26.9MB → 432.8MB; core exited on teardown, no residual process.

Remaining gaps for SP-22 stay as registered: UI/FRB acceptance of the overlay
at 10k, and the long-stability (24h) memory/queue curve with SP-31/SP-35.
