# SP-06 主核和 sidecar 退出观察 — 证据

状态：implemented（定向检查全绿；真实 Xray 11977/11978 退出、正式包 GUI、
提权 sidecar 真实退出未跑，不写 verified）。
基线：`8da1452`。不要 commit，根整合者验收提交。
合成 stub 核心（`.cmd`，`ping` 占位）仅用于故障注入（经持有句柄 kill，
`port 0` 走 liveness 就绪，与既有 rr06 脚手架一致）；Dart 侧脚本化 bridge，
无原生库、无 socket；测试端口规则：net_host 测试用 port 0（无监听），
Dart 合成端口 11977（≥11808，无绑定）；绝不占/改 10808；不动宿主代理/
路由/TUN/DNS/Run-key；只停本项目自有进程；`work/`、`outputs/` 只读未动；
`compat/` 未动；并行他卡文件（engine.rs、subs.rs、bridge_port.dart、
sp14/import_batch、manifest 他卡块）未动。

唯一用户流程：用户启动后本项目核心退出，界面显示实际退出并允许恢复。

## 0. 缺陷复现（红）

- 审计红合同（基线行为）：CP-01 —— 真实自有 Xray 退出后端口已死，
  net_host 仍报告 Running + 旧 PID/端点、无错误，见
  `docs/evidence/complete-port-audit-2026-10-06/runtime/real-loopback.log`
  第 8–12 行（`OBSERVED_AFTER_CORE_EXIT snapshot_state=running`，
  `socks_port_alive=False`）。
- 本卡红（基线 `8da1452` 上先写正确预期，全部失败后再修）：
  - `cargo test -p net_host --locked`：8 failed（含
    `sp06_main_exit…: an exited core must not report Running, left: Running`，
    即 ghost 复现；`sp06_sidecar_exit…: left: Running`），73 passed
    （旧回归全过，红只属新预期）。
  - `cargo test -p ipc_contract --locked`：2 failed
   （`mark_main_exited` / `mark_sidecar_exited` 未实现），34 passed。
  - `flutter test test/repair/sp_06_exit_contract_test.dart`：
    编译失败（`isCoreExited` / `canRecover` / `isSidecarDegraded`
    缺失，即 UI 侧无退出呈现）+ 本文件 1 处 async 笔误（已修）。
  - `privileged_helper`：`inject_exit` / `poll_core_exits` 未实现，
    3 项新测试 panic。

## 1. 改动文件

- `services/net_host/src/managed_process.rs`（新增）：
  句柄权威观察事实。`ObservedExit{pid,exit_code,at_ms}` +
  `exit_detail`（数字摘要，无秘密）+ `main_exit_error`
 （`E_INTERNAL/error.core_exited`，fatal：调用方撤端点）+
  `sidecar_exit_error`（`E_INTERNAL/error.sidecar_exited`，带 sidecar id）。
- `services/net_host/src/lifecycle.rs`（新增）：
  纯决策，无句柄无 IO。`should_reconcile`（仅无在途命令且有会话时对账，
  在途 apply 拥有会话直到就绪）、`next_generation`（每次退出 +1，
  desired 不变也推 generation，§3.1）、`state_for_sidecar_exit`
 （主活着 Degraded，否则 Stopped）。
- `services/net_host/src/session.rs`：
  `Inner` 新增 `actual_generation` / `last_exit: Option<CoreExitFact>` /
  `last_exit_sidecar`；`SidecarSession` 新增 `id`（退出归因）；
  新增 `reconcile_exits` 并接在 `ipc_snapshot` / `detail_frame` /
  `operation_status` 读取前（任一读都先对账，ready 后退出不再残留
  Running 缓存）。主退出：收会话、逆序停 sidecar、journal Finalized、
  撤 pid/ports/session_id/config（保留 applied_revision 历史）、记
  lastExit + 结构化 error、generation +1、epoch bump +
  `runtime_state_changed` + `error_raised` 事件；sidecar 退出：主活着则
  Degraded 并保留主端点、移除已收 sidecar 条目、同等记事实/推 generation。
  不可读状态永不伪造迁移；提权 sidecar 无可 poll 句柄，保持不动
  （不误杀，见 §5 缺口 1）。新会话成功时清 lastExit/generation 归零。
- `crates/ipc_contract/src/stable.rs`：
  `CoreExitFact::observed` + `RuntimeActualDescriptor::mark_main_exited`
  （main_state `exited`、清 main_pid/endpoints、记 last_exit/last_error、
  generation +1、冻结历史保留）/ `mark_sidecar_exited`
  （sidecar 条目 `exited`、主 `running→degraded`、同样记事实/推 generation）。
  纯新增方法，wire 字段零改名，STABLE_CONTRACT_VERSION 不变。
- `services/privileged_helper/src/backend.rs` + `src/windows.rs`：
  `HelperBackend::poll_core_exits() -> Vec<CoreExit{handle,pid,exit_code}>`
  （只观察、只报一次、永不 kill；未知 handle 永不记录）；
  `FakeBackend::inject_exit` 故障注入；Windows 真实现用自有进程句柄
  `GetExitCodeProcess` 轮询（句柄权威，无 PID 扫描；
  查不到读作未知，不伪造）。stop 的 PID+creation 双绑保留，无复用误杀。
- `apps/desktop/lib/features/runtime/runtime_bridge.dart`：
  `isCoreExited`（Stopped + `error.core_exited`，用户 stop 不算退出）、
  `isSidecarDegraded`（`Degraded` 或 `error.sidecar_exited`）、
  `canRecover`（退出/降级即提供恢复入口：`applyActive` 重试 /
  `reload` / `stop` 后重起；干净 stop 无退出恢复）、`exitStatusLabel`
  （已退出/已降级 + 后端数字 detail 原样）、`statusLabel` 扩展
  （退出读已退出、降级读已降级，其余沿旧未运行；既有 r4_06/t05
  断言仍过）。状态栏挂件本身属 SP-17（见 §5 缺口 4）。
- 新增测试：`session.rs` 内 `sp06_*` 2 项（stub kill 注入，单 block_on，
  见 §3  harness 说明）；`managed_process` 3 项 + `lifecycle` 3 项 +
  `stable.rs` 2 项；`backend.rs` 3 项（注入/poll 一次性/stop 幂等/
  未知 handle）；`apps/desktop/test/repair/sp_06_exit_contract_test.dart`
  4 项（退出呈现+恢复入口/降级保端点/重试回 Running/干净 stop 非退出）。

## 2. 本次范围 / 实际结果 / 未运行范围

本次范围（定向检查，VALIDATION_POLICY §1）：

| 命令 | exit | 结果 |
|---|---|---|
| `cargo fmt --all -- --check` | 非 0（他卡） | 本卡 6 文件零 diff；剩余 diff 全在 SP-14 并行文件（engine.rs、sp14_import_batch.rs、subs.rs），未动 |
| `cargo clippy -p net_host -p ipc_contract -p privileged_helper --all-targets --locked -- -D warnings` | 0 | 通过（中途修 `unwrap_or_default`/`if let`/多余 mut/Windows `Result<()>` 处理 5 处） |
| `cargo test -p net_host --locked` | 0 | 81 passed（含 sp06 2/2、lifecycle 3/3、managed_process 3/3、sp04/sp05/rr10/rr06/r3 全过），0 failed |
| `cargo test -p ipc_contract --locked` | 0 | 36 passed（含 stable 退出迁移 2/2），0 failed |
| `cargo test -p privileged_helper --locked`（附带） | 0 | lib 13 + dispatch 28 + loopback 8 全过（含 poll 3/3） |
| `flutter analyze`（apps/desktop） | 0 | No issues found |
| `flutter test test/repair/sp_06_exit_contract_test.dart` | 0 | 4/4 |
| `flutter test sp_04 + sp_05`（受影响回归） | 0 | 11/11（sp04 7/7 + sp05 4/4） |
| `dart format`（本卡 2 文件） | 0 | 通过（sp_06 测试文件 1 处已修） |

未运行范围（不是本卡故障，不刷全量）：完整 workspace 门禁、
`flutter build windows --release`、正式包 GUI、真实 Xray 11977/11978
退出重演、提权 sidecar 真实退出、24h/500 切换——留 SP-30/SP-34 及
§5 整合项；覆盖 ID 不批量标 verified。

## 3. 正确合同（摘）与 harness 说明

- 退出即事实：主退出 → Stopped + 撤端点 + 保留 applied_revision 历史 +
  `lastExit{pid,exit_code,at_ms}` + `lastError` + generation +1 + epoch
  bump + 双事件；sidecar 退出（主活）→ Degraded + 保主端点 +
  同等记事实/推 generation；clean stop 不产生 lastExit。
- 句柄权威：观察只用持有句柄 `try_wait`；不可读 = 未知，不迁移；
  在途命令拥有会话，读路径不抢；kill 只对持有句柄，永不按名/按 PID 杀。
- UI：Running 永不伪造；退出读已退出（含后端数字 detail），降级读
  已降级；`canRecover` 为真即允许 `applyActive`/`reload` 重试与
  stop 后重起；重试成功回 Running 并清恢复入口。
- Harness 经验（供他卡参考）：① stub 路径走进程全局 env，持
  `rr10_lock` 必须覆盖整个用例体（只锁 set 区会在并发下把别卡的
  must-fail 变成 false-pass，本卡中途亲测一次 rr10 红）；② 单测内只用
  一个 `block_on`：每个 runtime drop 会等 stub 管道关闭，多个 block_on
  会把 stub 自然寿命烧掉再观察（本卡中途亲测 60s 空转），与生产代码无关。

## 4. 正式入口 / 重开 / 最终消费者

- net_host 侧：`GetSnapshot` / detail 帧 / `GetOperation` 任一读都先
  对账；审计 `t03_client` 的 DETAIL/snapshot 轮询路径同样受益（无需改
  server.rs/main.rs，文件锁外）。
- Dart 侧正式入口复用既有命令：`applyActive`（含 F5 `reload`）/
  `stop`；退出后 `restoreActiveOnLaunch` 按原逻辑重拉（Stopped 非 Running
  即会重试，不会把死会话当活）。
- 重开：lastExit/actual_generation 为进程内事实（随新 apply 归零/刷新），
  journal 退出记 Finalized；跨重启对账以 journal + 新一轮观察为准，
  不伪造前世退出。
- 最终消费者：monitor 会话签名（`hasAppliedEndpoint`）在退出后为假，
  自动停止绑定旧端点；状态栏 actual 摘要挂件接线属 SP-17。

## 5. 未完成 / 阻塞（含整合者共享接口）

1. （需 SP-00 整合者加 wire）提权 sidecar 退出观察未接通：
   helper 后端 `poll_core_exits` 已就绪（fake + Windows 真实现），但
   `HelperOp` 协议无 `PollCoreExits` 查询、`HelperLink`
   （`services/net_host/src/helper_client.rs`，本卡文件锁外）无对应方法，
   net_host 对提权 sidecar 保持“未知即不动”（不误杀、不伪造）。
   建议：`HelperOp::PollCoreExits{handle?}` + link 透传 + session 对账消费。
2. （需整合者 FRB regen）`SnapshotDto`/FRB 面尚未暴露
   `lastExit{pid,exit_code,at_ms}` / `actualGeneration` /
   `mainState/sidecarStates`：`stable::RuntimeActualDescriptor` 迁移已就绪，
   Dart 暂用既有 state/error 字段呈现（数字 detail 原样透出）。
   bridge_api 本卡文件锁外，未私改。
3. （需整合者确认）持续推送节拍：对账挂在 IPC 读路径；无读即无发现，
   发现后靠 `runtime_state_changed` + `error_raised` 事件推送 UI 重读。
   server.rs watchdog 常驻 tick（文件锁外）是否加 `reconcile_exits`
   由整合者定。
4. （SP-17 范围）状态栏/托盘挂件的已退出/已降级样式与重试按钮接线：
   `RuntimeView.statusLabel/canRecover/exitStatusLabel` 数据侧已就绪，
   `status_bar_view.dart` 本卡文件锁外，未动。
5. 真实 Xray 11977/11978 退出重演（审计 real-loopback 同款）与正式包
   GUI：未运行；本卡 stub 注入只证机制。

## 6. owned 回收

- Rust：`std::env::temp_dir` run_root（测后 `remove_dir_all`）；stub
  `.cmd` 经持有句柄 `start_kill` + `wait` 回收（单 block_on 内完成，
  无残留 ping）；端口：net_host 用 port 0（无监听），Dart 合成 11977
  无绑定；无 10808、无 OS 写入。
- Dart：脚本化 bridge + `ProviderContainer`（dispose）/ stream 关闭，
  无平台调用。
