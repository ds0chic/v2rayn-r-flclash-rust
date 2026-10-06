# SP-09 证据（准备范围，A03 + §5 接线）

状态：implemented（§5 net_host 侧 helper v2 接线完成，fake 链路验证；
真实提权联调与长运行未验，不标 verified）。
本卡唯一流程——用户 TUN 长期运行不被 idle 误回收、管理者失联后按真实
归属恢复——的完整实现需要 `RenewLease` / `GetLeaseStatus` /
`ReleaseOwnedResources` / `GetOwnedCoreStatus`（A02 独占
`services/privileged_helper/**`，本次未动）；接口未就绪时 host 侧只做
结构/合同与准备，不伪造 helper 成功。

基线：`92d46dd`（不 commit）。审计依据 CP-12；idle 超时本身已修
（helper `request_timeout` 与 `idle_timeout=24h` 分离，有
`idle_connection_survives_request_timeout` 约束）。

约束遵守：虚拟时间（u64 ms）+ 纯函数断言，无 sleep、无端口、无 OS
写入；`tun_ownership_key` 为纯映射，无 helper/IO。

## 1. 本次准备内容（写锁内）

- `services/net_host/src/lifecycle.rs`：保活 watch 值与纯决策——
  `HELPER_RENEW_INTERVAL_MS=15s` / `HELPER_LEASE_TERM_MS=90s` /
  `HELPER_RENEW_FAILURES_BEFORE_RECONCILE=3`（RUNTIME_TUN_SOLUTION
  §6.1 拟值，系 A02 协商输入，非已协商事实）；`renew_due` /
  `lease_expired` / `renew_reconcile_due` /
  `active_lease_survives_ui_idle`（活跃租约不受 UI idle 影响的所有权规
  则：失联才回收，重开按 journal 归属恢复，永不按 desired 猜）。
  均 `allow(dead_code)` 暂存，待 A02 续租协议落地接线（N-H2）。
- `crates/application/src/tun_plan.rs`：`tun_ownership_key(adapter,
  if_index, route_digest)`——重开归属恢复键（大小写/空格不敏感，
  index 与 digest 区分），host journal 比对同字段。
- host 侧归属恢复链（SP-08 同批落地，本目录引用）：boot
  `reconcile_stale_tun` 按 journaled owned 记录恢复（helper 不可达则
  pending，不删）；`try_restore` 沿用冻结旧 plan/exe（不读新
  desired）；`retry_tun_cleanup` 只确认释放、永不重 apply 试探。

## 2. 虚拟时间验证（exit 均为 0，日志同目录）

- `cargo test -p net_host --locked lifecycle` → 0：
  8 passed（含 4 个新增：renew 周期、term 裕量、**虚拟 24h 稳续租永不
  过期**、死 owner 到期 + 3 连败进对账）（`keepalive-lifecycle.log`）
- `cargo test -p application --locked tun` → 0（`keepalive-application-tun.log`；
  含 `tun_ownership_key` 稳定/区分 2 用例）
- `cargo fmt --all -- --check` 与
  `cargo clippy -p net_host -p application --all-targets --locked -- -D warnings`
  → 0（日志见 `../SP-08/host-checks-*.log`）

## 3. 接口需求/阻塞（登记）

- N-H2（A02）：`RenewLease` + `GetLeaseStatus`（owner/session/
  generation 鉴权、独立 request/lease 超时、连续失败对账）。host 侧
  actor 与 pipe 序列化待协议就绪后接线。
- N-H3（A02）：逐资源释放报告与 `AlreadyGone` 语义（见 SP-08
  HOST_SIDE §4）；`GetOwnedCoreStatus`（提升核持续探活/退出）缺口已在
  session 侧注明（helper 分支观察仍为 SP-00 登记项）。
- 未验证：真实 24h/48h 长会话、睡眠恢复保活时间线、管理者失联后 live
  恢复、迟到 renew/异 owner 复活拒绝（合同已定，待协议与隔离环境）。
  实际长运行另验，不标 verified。

## 4. 改动与状态

本准备改动：`services/net_host/src/lifecycle.rs`、
`crates/application/src/tun_plan.rs`、本目录 `README.md` + 2 份命令日志。
`tasks/SP-09.md` 与 manifest SP-09 块更新为“准备完成、实现阻塞”
如实状态（见卡）。

## 5. 2026-10-07 net_host 侧 helper v2 接线（N-H2 落地，implemented）

前置接口已就绪：`2ea50bd` 落地 helper 协议 v2（`RenewLease` /
`GetLeaseStatus` / `PollCoreExits` + `AlreadyGone`，SP-00
`HELPER-OPS-V2.md`）。本次只动 `services/net_host`（未动
`crates/ipc_contract/**`、`services/privileged_helper/**`、
`crates/bridge_api/**`、`crates/application/**`、workspace Cargo 文件）：

- `services/net_host/src/helper_client.rs`：`HelperLink` 新增
  `renew_lease` / `get_lease_status` / `poll_core_exits`（既有
  request/response 管道 + 合同 per-request 超时类；错位结果转结构化
  `tun_apply_failed`）。`PipeHelperLink`（Windows）走真实 `call`；
  `FakeHelperLink` 复刻服务端语义（逐 handle 过期/pid/released、
  drain-once pending→observed、与合同一致的空/越界/0/duplicates 拒绝、
  `seed_lease`/`inject_core_exit` 故障注入）；`DryRun`/`Unavailable`
  按“无自有租约”返回结构化错误（poll 在 dry-run 下为空）。
- `services/net_host/src/session.rs`：`SidecarSession` 新增
  `lease_last_renew_ms` / `lease_last_confirmed_ms` /
  `lease_renew_failures`；reconcile 通路对每个有 helper 链路且有非零
  handle 的提权 sidecar——到期（15s 节奏）才调一次 `RenewLease`，成功
  确认（失败清零）、失败记一次重试；90s term 无确认、或 3 连败、或
  `GetLeaseStatus == Released` 一次性确认释放时，经 staged 记录
  （`stage_elevated_sidecar_exit`，与 `note_elevated_sidecar_exit` 同源）
  以 code-unknown 降级，由 `reconcile_exits` 既有侧车退出分支消费。
  无链路/无 handle（含 dry-run 零 handle）不调用；helper 上报 PID 只做
  归属标识，永不按 PID kill。
- `services/net_host/src/lifecycle.rs`：15s/90s/3 次 watch 值与
  `renew_due` / `lease_expired` / `renew_reconcile_due` 去 staged 标记
  转正（生产调用）；其余 staged 项（`session_readiness` 等）保持不变。

定向检查（exit 均为 0）：

- `cargo fmt -p net_host -- --check` → 0
- `cargo clippy -p net_host --all-targets --locked -- -D warnings` → 0
- `cargo test -p net_host --locked` → 0（129 passed；新增 13：
  fake 链路 6（续租延长/未知句柄/Released/poll drain-once/越界拒绝/
  链路中断）+ 接线 7（到期确认/3连败对账/过期即对账/Released 快捷对账/
  无 handle 不调用/session 级 poll 降级恰一次/未知 handle 不伪造））
- 未动 `ipc_contract` 公共 API，未跑 `cargo test -p ipc_contract`。

未验证（保持 implemented，不标 verified）：真实提权 helper 联调
（UAC 提权 + 命名管道真实往返）、24h/48h 长运行与睡眠恢复、迟到
renew/异 owner lease 的真机清理行为。
