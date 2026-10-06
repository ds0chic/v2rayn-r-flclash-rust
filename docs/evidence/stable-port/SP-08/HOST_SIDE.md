# SP-08 证据（host 侧范围，A03）

状态：host 侧 implemented（定向门禁全绿，见 §2）；本卡整体仍 identified：
跨层 UI 接线（SP-00 整合）与授权隔离机真实收敛未验。helper 侧见同目录
`README.md`，双方均不写 verified。

基线：`92d46dd`（工作树有并行他卡未提交改动，本次只动写锁内文件，不
commit）。审计依据 CP-04；idle 误回收（CP-12）当时已修
（`idle_timeout` 默认 24h，`request_timeout` 独立），本卡只验证并补 host
侧归属恢复准备（SP-09 范围另见 `../SP-09/README.md`）。

唯一用户流程：停止 TUN 时某资源清理失败，用户能看到待清理并重试成功。

约束遵守：合成数据 + in-memory `FakeHelperLink`/故障注入；无端口监听
（文件/journal 断言，无需端口）；不触发宿主真实 OS 写入（无 pipe、无
helper 进程、无路由/DNS/适配器调用）；只清理本项目 owner 记录资源；
`services/privileged_helper/**`、engine.rs、IPC/stable DTO、FRB、Cargo
锁均未动。

## 0. 缺陷复现（红）

基线行为即 CP-04：`cleanup_tun_lease` 吞掉 helper 错误仍删 journal 报
`Ok`；`reconcile_stale_tun` 非 unavailable 失败直接按 cleaned 丢记录。

本卡红探针（只用基线已有公开 API，修后转为常驻回归，期望未动）：

- `cargo test -p net_host --locked tun_lease::tests::sp08` → exit 101，
  1 passed / 2 failed：
  - `sp08_cleanup_failure_is_not_success_and_keeps_journal`：
    `an unconfirmed cleanup cannot report success: ()`
  - `sp08_reconcile_backend_failure_keeps_pending_journal`：
    `left: 0 / right: 1`（pending 被计成 cleaned，journal 已删）

## 1. 改动文件（仅写锁内）

- `services/net_host/src/tun_lease.rs`：`cleanup_tun_lease` 失败返回
  `Err` 并保留 journal；新增 durable `PendingCleanup`
 （`tun_pending.json`：session/adapter/if/route_digest/dry_run/last
  error/attempts，可重开恢复）；`retry_tun_cleanup` 按 journaled owned
  记录重试；`reconcile_stale_tun` 任何未确认失败都记 pending 保记录，
  仅 `E_NOT_FOUND`（AlreadyAbsent 幂等）收敛；失配时清理**记录侧**
  资源；`is_already_absent`、`list_pending_cleanup` 供 host 侧消费。
- `services/net_host/src/helper_client.rs`：`PipeHelperLink::cleanup`
  传播 `RemoveRoutes` 错误（此前 `let _` 吞掉）；`FakeHelperLink` 新增
  `CleanupFails` / `CleanupUnavailable` 故障 + `set_fault` /
  `cleanup_attempts`（测试钩子，无生产语义变化）。
- `services/net_host/src/session.rs`：`Inner::pending_tun_cleanup` 内存
  态；`release_tun_lease` 改返回 `Result`，失败恢复内存租约（以 journal
  为准）并刷新 pending；`stop_managed_inner` 清理未确认时记
  Degraded + stop 操作 Failed（不再报干净关闭）；新增
  `retry_tun_cleanup` / `pending_cleanup_snapshot`（N-H1 数据源，待 SP-00
  接 IPC/FRB）；boot `reconcile_tun_leases` 把 pending 装回内存；
  `try_restore` 保持冻结旧 plan/exe（未动语义，归属恢复走 journal）。
- `services/net_host/src/lifecycle.rs`：SP-09 保活准备（见 SP-09 证据）。
- `crates/application/src/tun_plan.rs`：`tun_ownership_key`（见 SP-09 证据）。

## 2. 定向检查（exit 均为 0，日志同目录）

- `cargo fmt --all -- --check` → 0（`host-checks-fmt.log` 空）
- `cargo clippy -p net_host -p application --all-targets --locked -- -D warnings` → 0
- `cargo test -p net_host --locked` → 0：`109 passed / 0 failed`
  （`host-net_host-test.log`）
- `cargo test -p application --locked tun` → 0（`../SP-09/keepalive-application-tun.log`；
  命中 target 29+3+1 通过，`tun_plan` 20/0 含 2 个新增归属 key 用例）

未运行：workspace 全量门禁（VALIDATION_POLICY：发布候选才跑）；授权
隔离机失败→重开→retry 真实收敛；跨层 UI 重试入口（待 SP-00）。

## 3. 新增测试与通过数

21 个新增（全部通过，计入上面 109/20 内）：

- `tun_lease.rs` 11：清理失败非成功+保 journal；reconcile 后端失败记
  pending（红转绿）；unreachable 记 pending；AlreadyAbsent 收敛；仅
  NOT_FOUND 判 absent；pending 上榜+脱敏；attempts 累加；失败后重试收
  敛；重试仍失败刷新；无 journal 重试清陈旧 pending；pending JSON 回环。
- `helper_client.rs` 2：清理失败保资源可重试；unavailable 可重试保资源。
- `session.rs` 2：stop 清理失败 → Degraded + 操作 Failed + 待清理可见，
  helper 恢复后显式重试收敛回 Stopped；重开时未确认租约保持 pending，
  来 helper 后重试收敛（dry-run 记录 + Fake 链，无 OS）。
- `lifecycle.rs` 4 + `tun_plan.rs` 2：见 SP-09 证据。

## 4. 接口需求/阻塞（登记，不私改共享面）

- N-H1（SP-00 整合者）：`pending_cleanup_snapshot` /
  `retry_tun_cleanup` 需经 IPC/FRB 暴露为“待清理查询 + 重试清理”入口；
  稳定快照 DTO 由整合者扩展，本次只提供数据源。
- N-H2（A02 helper 接口，阻塞真续租）：`RenewLease` /
  `GetLeaseStatus`（鉴权 + generation/lease 校验 + 独立超时）未就绪，
  host 侧只备了 watch 值与纯决策，未伪造任何 helper 成功。
- N-H3（A02 helper 接口，阻塞逐资源确认）：`ReleaseOwnedResources`
  的逐资源 `Released/AlreadyGone/Pending/Conflict` 报告未就绪；host
  侧 `is_already_absent` 仅 `E_NOT_FOUND` 收敛——helper 现无任何路径返
  回它（helper 侧对“已不存在”保守记失败保留），故今日不可能误收敛；
  跨 helper→host 的已确认缺席语义由 SP-00 定。
- 真实收敛（授权隔离机失败→重开→retry，错误跨 helper→net_host→UI
  可见）无环境，保持 blocked，不标 verified。

## 5. 对应 ID 与 git 状态

沿 `docs/repair/coverage.csv` owner_task=R4-05、R4-25 集合选取本次唯一
流程（停止 TUN 某资源清理失败→可见待清理→重试成功）；不将 owner 全集
标 verified。

本卡改动（`git status --short` 全树含并行他卡，此处仅列本卡）：
`services/net_host/src/{helper_client,session,lifecycle,tun_lease}.rs`、
`crates/application/src/tun_plan.rs`、本目录 `HOST_SIDE.md` +
`host-*.log`、`../SP-09/`（准备范围）。
