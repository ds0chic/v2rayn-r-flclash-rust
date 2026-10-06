# SP-04 统一启动/停止/切换命令序列 — 证据

状态：implemented（Rust + Dart 门禁全绿；真实原生/正式包验收未跑，不写 verified）。
基线：`566faac`（SP-00/01/02/03/11 已完成）。不要 commit，根整合者验收提交。
合成数据与 fake 桥仅用于故障注入与并发编排；真实入口走真实 FRB 路径
（`apply_runtime` / `stop_runtime` / `get_operation` / `get_snapshot`，
本次零新增 FRB 面）。测试端口先探测且 ≥11808（Rust 侧 11921/11931/11941 起探），
Dart 侧无 socket；绝不占/改 10808；不改宿主系统代理/路由/TUN/DNS/Run-key；
只停本项目自有进程（net_host 侧 stub `cmd.exe`/`ping` 经持有句柄回收）；
`work/`、`outputs/` 只读未动；`compat/` 未动。

## 0. 缺陷复现（红）

- Dart `apps/desktop/test/repair/sp_04_runtime_sequence_test.dart` 初建 7 项，
  修前 2 pass / 4 fail / 1 skip：A 在途→stop→B 实际派发
  `[apply:a, apply:b, stop]` 最终 Stopped（最新 B 意图被旧 stop 覆盖）；
  apply `E_TIMEOUT` 后端 Running 但 UI Stopped 且 `reconcileNeeded=false`；
  传输 ACK 丢失（throw）同样 Stopped 无对账；cancel 尚无 API（编译期红）。
  与审计 CP-02 / `runtime/runtime-contracts.log`（1 pass / 5 fail）一致。
- Rust 红探针（机制级，临时去锁复现后恢复，见 §2）：
  `sp04_stop_waits_for_the_command_gate` 去 gate 后 0.2s 内 stop 直接完成
  （exit 101）；`sp04_concurrent_apply_stop_apply_serializes_in_admission_order`
  去锁后得到并发的 `["apply","stop","apply"]` 而非等待（exit 101）。

## 1. 改动文件

- `apps/desktop/lib/features/runtime/runtime_controller.dart`：
  删除 `_pendingApply/_pendingStop` 双分支优先级队列
  （`_pendingApply ?? _pendingStop` 即“apply 总优先”是 RUN-02 根因），改为
  单一权威 FIFO `_commandQueue` + 单调 `intentSeq`：提交顺序即执行顺序；
  stop 为清理 barrier（永不合并、永不被超车，含尚未开始项）；
  仅尾部连续未开始 apply 可合并（last target wins，合并 waiter 随幸存意图
  结算，永不悬挂）；`_enqueueStop` 独立成槽；新增 `cancelPending()`
  （仅丢弃未开始项，waiter 以 false 结算；在途命令跑到安全点，
  与 net_host `NotCancellable` 一致）；`_executeCommand` 返回 per-intent bool；
  apply/stop/异常三路未知结果（`E_TIMEOUT`/`E_UNAVAILABLE`/`E_BRIDGE`）
  统一走 `_reconcileUnknown`：先查 operation（best-effort，经新能力接口），
  再读权威 snapshot，原错误保留 + `reconcileNeeded=true`，永不重放提交；
  generation 旧回包丢弃保留。
- `apps/desktop/lib/features/runtime/runtime_bridge.dart`：
  新增 `RuntimeOperationView` + `OperationQueryBridge` 能力接口
  （仿 `ExplicitTargetRuntimeBridge` 花样，保持 `implements RuntimeBridge`
  的既有 fake 零改动编译）；`FrbRuntimeBridge` 用**既有**
  `getOperation` 端点实现（零新增 FRB 面，零生成物变更）。
  冻结审计物 `docs/evidence/.../runtime-contracts_test.dart` 的
  `AuditRuntime` 不受影响（未改 `RuntimeBridge` 既有签名）。
- `services/net_host/src/session.rs`：
  `HostState` 新增 `command_gate: tokio::sync::Mutex<()>`——会话级唯一
  命令序列：`apply_plan`（校验之后全程持 gate，失败路径自动释放）、
  `stop_managed`（持 gate → `stop_managed_inner`；`start_prepared` 内改调
  inner 版，杜绝自死锁）按接纳顺序串行；watchdog/Shutdown 走同一公有入口；
  查询（`ipc_snapshot`/`operation_status`/`detail_frame`）永不持 gate；
  `stop_managed_inner` 把 `terminate_and_wait` 移出 `inner` 锁
  （查询不排生命周期长锁，gate 仍持有故无交错）；`#[cfg(test)]`
  `command_gate()` 机制缝。
- `crates/application/src/engine.rs`：
  新增 `runtime_cmd_lock`（两构造点初始化）：`apply_runtime`
  （冻结 revision 校验之后持锁，stale 提交到不了 runtime）、`stop_runtime`
  持同一锁——同进程多窗口/isolate 提交串行化；`snapshot`/`operation_status`
  只读不持锁。
- 新增测试（先红后绿）：
  `apps/desktop/test/repair/sp_04_runtime_sequence_test.dart`（7 项）；
  `crates/application/tests/sp04_command_sequence.rs`（4 项，复用
  `ipc_contract::stable::OperationState` 做终态词汇断言，`free_port` 起探）；
  `session.rs` 内 `sp04_*` 3 项（stub core + port 0，无监听）。
- FRB/共享 DTO：零改动。`stable.rs` 复用为词汇与概念
  （`RuntimeIntentAction` Start/Stop、 receipt Accepted/Executing 语义）；
  `OperationState`↔`JobState` 整映射与 stop-operation-id 回传属 SP-05，
  已登记为下一前置，未私改 wire。

## 2. 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | 通过（中途本卡 1 处格式 diff 已修） |
| `cargo clippy -p application -p net_host --all-targets --locked -- -D warnings` | 0 | 通过（中途本卡测试闭包 `result_large_err` 1 处，已改闭包内 expect） |
| `cargo test -p application --locked` | 0 | 全绿：lib 288 + 全部集成文件，含 sp04 4/4 |
| `cargo test -p net_host --locked` | 0 | 71 passed（含 sp04 3/3），0 failed |
| `dart format --output=none --set-exit-if-changed lib test`（apps/desktop） | 0 | 通过（本卡 2 文件已格式化） |
| `flutter analyze`（apps/desktop） | 0 | No issues found |
| `flutter test test/repair/sp_04_runtime_sequence_test.dart` | 0 | 7/7 |
| `flutter test` 受影响 runtime 集（sp04 + runtime_controller + r4_01/02/04 + r3_01_07_10 + fix11_monitor + fix13_tun） | 0 | 57/57 |
| fix08 系列 + r4_06/08/11 + fix15c（逐文件单独补跑） | 0 | 全部单独通过（9+1+1+1+3+2+3+3+1+1+5） |
| 多文件批跑 | 基础设施 flaky | 基线 566faac 同批同样 `did not complete`（不同文件轮换），单文件全绿；与审计记录的批跑异常同类，非本卡回归。r4_07 等 3 基线异常文件未动 |
| `flutter build windows --release` / 正式包 GUI | 未运行 | 留 SP-30/SP-34；未跑不得写 verified |

## 3. 正确合同（摘）

- 唯一序列：提交顺序 = 执行顺序；stop barrier 永不被合并/超车
  （A 在途→stop→B 派发 `[apply:a, stop, apply:b]`，终态 Running B；
  A→B→stop 派发 `[apply:a, apply:b, stop]`，终态 Stopped）。
- 合并：仅尾部连续未开始 apply（last wins，不跨 barrier，不改变原版语义）。
- 冻结：目标/计划/revision 在提交前冻结；stale revision 拒绝且零 runtime 触碰；
  运输 ACK ≠ 业务提交。
- 未知结果：超时/断连/丢回包 → 先查 operation（best-effort）再读 snapshot，
  原错保留 + reconcile 标记，绝不生成新 ID 重放；旧 A/stop 回包只记历史
  （`staleResponsesDropped`），不覆盖最新 B 视图（generation/epoch 守卫）。
- 取消：在途不可取消（`NotCancellable`，跑到安全点）；仅排队项可取消，
  waiter 以 false 结算（其意图从未执行，不得报成功）。
- 后端：net_host 跨连接 apply/stop/shutdown 经 `command_gate` 串行；
  application 同进程 apply/stop 经 `runtime_cmd_lock` 串行；
  两类查询永不持命令锁/长 `inner` 锁。

## 4. 正式入口 / 重开 / 最终消费者

- 真实入口复用既有 FRB：`applyRuntime`（冻结 plan 经
  `build_runtime_plan` + `DesiredRevision` 校验）/`stopRuntime`/
  `getOperation`/`getSnapshot`；本次新增 Dart `operationStatus` 即
  `getOperation` 的只读消费，无新端点、无二次生成、无 no-diff 事项。
- 重开/恢复：本卡不改变持久化与 journal 语义（stop/finalize/journal 路径
  行为逐行保留，仅拆锁与加 gate）；`stop` 清 `last_plan/last_exe`、
  空 stop 不碰 error 等分支与改前逐字一致。
- 最终消费者：Rust 侧为 fake 背端 + 真实 `AppEngine::build_runtime_plan`
  冻结 plan（含探活端口 ≥11808）；Dart 侧为 ProviderContainer + 脚本化
  `SequenceBridge`（故障注入）；真实 FRB/SQLite/独立重开/实际消费者按
  卡面属 SP-30 E2E，未在本卡冒充。

## 5. owned 回收

- Rust 测试只建内存引擎 / temp run_root（`test_state` 唯一目录，测后删除）；
  stub core（`cmd.exe` + `ping`）经持有 `Child`/`JobGuard` 回收；
  探测用 `TcpListener` 即绑即放；无 10808、无宿主写入；
  `git status` 本卡为 4 改 + 2 新 + 本证据目录 + 任务卡/manifest 状态行。

## 6. 未验证与下一前置（阻塞/整合者事项）

- 未验证：真实 net_host 管道并发（本卡为 stub/内存级串行证明）、真实原生
  FRB 端到端、正式未武装包（SP-30/SP-34）、其它 OS/架构、24h/500 切换（SP-35）。
- 下一前置：SP-05（`OperationState`↔`JobState` 整映射、apply_target 冻结归属、
  apply job 终态与 `op:job` 复合 ID 往返——本卡已保留现状未动）；
  SP-06（退出观察与本卡序列的竞态仍按旧语义由 gate 串行，未做活跃监控）；
  SP-11 式多窗 `clientInstanceId` 全局接纳序号（本卡 UI 单序列 + host 单 gate，
  跨进程到达序即执行序，未做全局序号分配）。
- 阻塞（需根整合者，无）：零新增共享接口/DTO/FRB 面。如后续需要 stop 的
  operation-id 回传（wire 变更），由整合者排期，本卡已留 `operationId`
  透传位（`RuntimeActionResult.operationId` → `_reconcileUnknown`）。
