# SP-10 证据（准备范围：授权隔离机启用 TUN 的本机可独立部分）

状态：**implemented**（§7 net_host 侧 PollCoreExits 接线完成，fake 链路
验证；真实 OS 路由接管未验，不得 verified）。

本次唯一流程：用户在授权隔离机启用 TUN，主核/sidecar 就绪、IPv4/IPv6
正常且标签真实。本机只做可独立部分——就绪判定/归属/IPv6 探测输入/
标签数据链；真实 auto-route/全局接管留授权隔离机，本机不触发 OS 写入。

基线：`393fafd`（不 commit）。审计依据 CP-12；缺口定位见
`docs/evidence/complete-port-audit-2026-10-06/runtime/README.md`
（TUN-A04/A05/A06）与 `RUNTIME_TUN_SOLUTION.md` §5–§8。前置 SP-05/SP-06/
SP-08/SP-09 的 verified 状态以其当前证据为准，本文不代称。

约束遵守：合成数据；测试端口全部 ≥11808 且先探测（sidecar 坏端口经
临时 `TcpListener(127.0.0.1:0)` 取空闲端口，主核 stub 用 port 0）；
10808/宿主代理/路由/TUN/Run-key 零触碰（Fake/DryRun link + `.cmd` stub，
无 OS 写入）；不改写锁外文件（engine.rs/lib.rs/privileged_helper/
IPC/FRB/Cargo 锁/runtime_bridge/runtime_controller 均未动）。

## 1. 改动文件（仅写锁内）

- `services/net_host/src/session.rs`：就绪判定
  - `SidecarSession` 新增 `elevated_pid`（helper 上报的归属身份，只做
    退出归因，永不做本地 kill 目标）与 `elevated_exit_code`
   （helper 观测到的退出报告）。
  - `start_elevated_sidecar`：`RunElevatedCore` 成功只算派发；真实路径
    要求 handle != 0（`elevated_launch_verdict`），dry-run `(0,0)` 保持
    模拟成功；记录 helper 上报 PID。
  - 超时先查实际结果：`wait_ready` 到 deadline 后先最终 `try_wait`
    （边缘退出报 Exited），再最后一次即时建连（实际已就绪则接受
    Ready）；`start_prepared` Timeout 臂与普通 sidecar 超时路径同样
    先查 `try_wait`——已退出的报 `error.core_exited`（带 code），只有
    活着但沉默的才报 `error.readiness_timeout`。
  - `note_elevated_sidecar_exit(id, code)`（staged：A02 `PollCoreExits`
    落地前生产无调用方，返回 false 永不伪造退出）与 `reconcile_exits`
    提权分支：有报告的提权 sidecar 走与普通 sidecar 同一 Degraded 路径
    （撤 sidecar、保主核端点、generation+1、`last_exit_sidecar`、
    `error.sidecar_exited`）；无报告的提权 sidecar 保持不动（不误杀）。
  - 纯合同 `confirm_readiness_timeout` / `TimeoutConfirm`（staged）。
- `services/net_host/src/lifecycle.rs`：纯就绪聚合 `session_readiness`
  / `SessionReadiness`（主核死→NotReady；sidecar 失败或提权观测缺席→
  Degraded；staged 待 A02 接线）。
- `services/net_host/src/tun_lease.rs`：`lease_ownership_key`
 （host 侧 `adapter|index|digest` 归属 key，与
  `application::tun_ownership_key` 同格式同归一化；net_host 不得依赖
  application crate，统一由 SP-00 整合；staged）。
- `crates/application/src/tun_plan.rs`：可注入 IPv6 探测输入
  `Ipv6Probe{Unknown,NoGlobal,HasGlobal}` + `TunProbeContext`（默认全
  Unknown）+ `resolve_ipv6_for_plan`：显式请求在 Unknown 下保留并附
  `tun.ipv6_unverified` 诊断（永不静默 false），在 NoGlobal 下拒绝并附
  `tun.ipv6_no_global`（永不虚假宣称），HasGlobal 静默通过；未启用/
  无地址保持排除无 note。生产 codegen 调用方接线（当前恒 false/空）
  由 SP-00 整合，本次不改 `codegen.rs`（写锁外）——缺口如实登记（§4）。
- `apps/desktop/lib/features/runtime/tun_toggle.dart`（仅就绪/标签数据
  链）：`tunActualLabel` 只读 `runtime_tun` 事实——dry-run 租约读
  `模拟 (adapter if=N)`；desired=false 但有活租约读
  `仍启用(关闭待确认)`；其余顺序不变（helper 拒绝→失败已回滚；停止→
  未启用；运行无租约→已请求(未验证)）。pending-cleanup 租约在快照无
  pending 事实时仍读 desired（缺 N-H1 pending 面，见 §4）。
- 本卡测试：`session.rs`（4）、`lifecycle.rs`（1）、`tun_lease.rs`
  （2）、`tun_plan.rs`（1）单测；`apps/desktop/test/repair/
  sp_10_tun_label_test.dart`（2）。

## 2. 先红后修

- 红：新增 Rust 合同先行时 `session_readiness`/`SessionReadiness` 等
  符号缺失，`cargo test -p net_host --locked sp10` 编译失败（E0425/
  E0433，见 `red-net_host-sp10.log`）。
- Dart：旧 `tunActualLabel` 首行 `if (!desired) return '未启用'` 且无
  dry-run 分支——dry-run 租约旧读“已启用”、desired=false 活租约旧读
  “未启用”，与新合同（`模拟…`/`仍启用(关闭待确认)`）逐项冲突（见
  diff 记录 `tun-toggle-label.diff` 的旧分支）。
- 修后：`cargo test -p net_host --locked` 116 passed / 0 failed；
  `cargo test -p application --locked tun`（含
  `sp10_ipv6_probe_decision_matrix`，lib tun 32 passed）exit 0；
  `flutter test test/repair/sp_10_tun_label_test.dart
  test/fix13_tun_toggle_test.dart` 11/11 passed。

## 3. 定向检查（exit 均为 0，日志同目录）

- `cargo fmt -- --check <4 个 Rust 文件>` → 0（`fmt-check.log`；只查
  自己文件，未跑 `--all` 以免触碰并发 agent 的在途文件）。
- `cargo clippy -p net_host -p application --all-targets --locked -- -D warnings` → 0（`clippy.log`）。
- `cargo test -p net_host --locked` → 0（116 passed，`test-net_host.log`）。
- `cargo test -p application --locked tun` → 0（`test-application-tun.log`）。
- `flutter analyze` → 0（No issues found，`flutter-analyze.log`）。
- `flutter test`（本卡相关：sp_10 + fix13）→ 0，11 passed
 （`flutter-test-sp10.log`）；`dart format` 自己两文件 → 0 changed。
- 未运行：workspace 全量门禁（VALIDATION_POLICY：发布候选才跑）；
  真实 TUN auto-route/默认路由/地址/MTU/DNS（授权隔离机，无环境，
  未验证）；24h/48h 长会话、睡眠恢复（未验证）。

## 4. 接口需求/阻塞（登记，不私改共享 DTO）

1. N-H2/N-H3（A02，privileged_helper 写锁外）：`PollCoreExits`/
   `GetOwnedCoreStatus`（提权 sidecar 持续退出观测）、`RenewLease`/
   `GetLeaseStatus`（SP-09 已登记）。`note_elevated_sidecar_exit` 与
   `session_readiness(elevated_pending)` 是 staged 消费端，协议就绪后
   由 SP-00 整合接线；此前提权 sidecar 退出在生产仍不可见。
2. IPv6 平台只读探测（RT-12）：`TunProbeContext` 输入已就绪，但生产
   `codegen.rs` 装配仍恒 false/空（`codegen.rs` 在写锁外，本次未动）；
   探测实现（platform 只读边界）与装配接线待 SP-00 整合。
3. 归属统一：`lease_ownership_key`（host）与 `tun_ownership_key`
   （application）格式已 pin 一致，跨 crate 复用/SSOT 由 SP-00 定。
4. 标签 pending 面：desired=false 待清理租约的精确标签需 N-H1
   pending-cleanup 快照过 IPC/FRB（SP-08 已登记）；快照无 pending
   事实前按 §1 口径显示。
5. 真实接管（授权隔离机前置）：Windows 普通/管理员、Xray Legacy 前置
   sing-box、sing-box 自身 TUN、适用非 Legacy 主核 TUN；IPv4-only/
   双栈/无 global IPv6/指定前缀与 route excludes；stack/MTU/ICMP/
   strict-route；UAC 取消/拒绝/丢失、主/侧核退出、cleanup 部分失败、
   睡眠恢复、连续开关、24h/48h 会话；每场景 before/after 资源快照、
   真实流量/DNS/IPv6 出口、owned 回收与重开。全部未验证——本卡保持
   identified，不标 verified。

## 5. 工作树说明（并发执行）

- 本卡改动只覆盖 §1 文件 + 本目录证据 + `sp_10_tun_label_test.dart`。
- 执行中曾因与并发 agent 共享工作树导致 `git stash -u` 后
  `stash pop` 被另一 agent 的 `t16.rs` 在途改动阻塞；已仅取回本卡
  5 个文件（`git checkout 'stash@{0}' -- <5 files>`），stash 条目保留
  未删，他人文件（`t16.rs`/`monitor*.dart`/`engine.rs`/`monitor.rs`/
  `update_service.rs` 等在途改动）一概未碰。中间一度出现他 agent 在
  途 `monitor.rs` 测试编译失败，曾短暂阻塞 application lib-test；
  对方修好后复绿（`test-application-tun.log` 为最终绿结果）。
- 卡与 manifest 状态：`tasks/SP-10.md` 与 execution-manifest 不在本卡
  写锁内，本次未改；其 identified 状态与本证据一致（真实接管未验，
  不得 verified）。

## 6. 对应 feature/field/action ID

沿 `docs/repair/coverage.csv` owner_task=R4-25 集合选取本次唯一流程
（授权隔离机启用 TUN→主核/sidecar 就绪→IPv4/IPv6 正常→标签真实），
在隔离机实测前不将 owner 全集标 verified。本卡整体状态自 §7 起为
implemented（fake 链路接线完成，真实接管未验）。

## 7. 2026-10-07 net_host 侧 PollCoreExits 接线（N-H2 落地，implemented）

前置接口已就绪：`2ea50bd` 落地 helper 协议 v2（含 `PollCoreExits`
drain-once + `GetLeaseStatus` 可见已 Drain 退出，SP-00
`HELPER-OPS-V2.md`）。本次只动 `services/net_host`（范围同 SP-09
§5，未动共享 DTO/Cargo）：

- `reconcile_exits` 读取通路新增 helper 观测段：对每个“有 helper 链
  路 + 非零 handle + 无本地 child + 尚未 staged”的提权 sidecar，向其
  自有链路发一次 `PollCoreExits { handles: [handle] }`（有界：单链路单
  handle，远小于 256 上限；失败则忽略、本次读不 fabricate，下次读重
  试）。命中自有 handle 的观测经 `stage_elevated_sidecar_exit`（与
  `note_elevated_sidecar_exit` 同源）记入 `elevated_exit_code` 并补齐
  helper 上报 PID（仅归属标识），随即由既有侧车退出分支消费：
  Degraded + `error.sidecar_exited` + fact generation +1，与普通
  sidecar 退出同一通路。未列出 handle 的观测直接丢弃。
- 同通路随后按 SP-09 节奏续租（见 SP-09 §5）；两段共享同一门控。
- 定向检查与 SP-09 §5 同批：`cargo fmt -p net_host -- --check` →
  0；`cargo clippy -p net_host --all-targets --locked -- -D warnings`
  → 0；`cargo test -p net_host --locked` → 0（129 passed，含 session
  级 `sp10_polled_exit_degrades_the_session_exactly_once`——首读
  Degraded 且 generation 1、次读 generation 不变——与
  `sp10_unknown_polled_handle_fabricates_no_exit`——Running、
  generation 0、无 last_exit）。

未验证（保持 implemented，不标 verified）：真实提权 sidecar 的退出
观测全链路（UAC 提权 + 真实 helper 进程退出 + 命名管道 poll）、§4.5
所列 OS 路由/地址/TUN 接管实测、24h/48h 与睡眠恢复。
