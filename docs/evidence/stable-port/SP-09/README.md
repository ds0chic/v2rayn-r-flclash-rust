# SP-09 证据（准备范围，A03）

状态：identified（准备完成，行为实现被 A02 helper 接口阻塞）。
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
