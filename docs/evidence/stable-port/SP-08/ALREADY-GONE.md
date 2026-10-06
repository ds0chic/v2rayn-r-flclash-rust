# SP-08 follow-up: `AlreadyGone` cleanup idempotency end-to-end

状态：helper + host 接线 implemented；本卡整体仍 identified（跨层 UI N-H1 待 SP-00，授权隔离机真实收敛未验）。

基线：工作树有并行他卡未提交改动（`git status` 见下），本次只动写锁内文件，不 commit。
`crates/ipc_contract/**` 未动（合同 `HelperResult::AlreadyGone { resource }` 复用 SP-00 v2 已有变体）。

## 1. 后端信号（最小设计）

`services/privileged_helper/src/backend.rs` 新增：

- `RouteRemovalOutcome::{Removed(u32), AlreadyGone}`
- `TunResetOutcome::{Reset, AlreadyGone}`

`HelperBackend::remove_routes` / `reset_tun_address` 返回上述枚举；
`Err(HelperError)` 仍只表示真实失败（权限/后端/参数错）， journal 保留。

- 真机 `services/privileged_helper/src/windows.rs`（编译未执行 OS 调用）：
  `DeleteIpForwardEntry2` / `DeleteUnicastIpAddressEntry` 仅 `ERROR_NOT_FOUND (1168)`
  映射 `AlreadyGone`；其余非零码仍 `HelperError::Backend`。批量删除全 gone 才
  `AlreadyGone`，部分成功按 `Removed/Reset` 计；TUN 无 registry 记录直接
  `AlreadyGone`（幂等），成功与 AlreadyGone 均删 registry，失败保留。
- `FakeBackend`：`already_gone: BTreeSet<FakeOp>` + `already_gone_on(op)` /
  `set_already_gone(op, bool)` / `clear_already_gone()`；`fail_on` 优先于
  already-gone；attempts 照计、calls 照录。

## 2. helper 侧接线

`services/privileged_helper/src/server.rs`：

- `RemoveRoutes` dispatch：`Removed(count)` → `RoutesRemoved`；`AlreadyGone` →
  `AlreadyGone { resource }`（单条用 `route_label`，多条 `routes count=N first=<label>`，
  有界脱敏）。两种都做 `lease.routes.retain` + `journal.mark_released`，
  dispatch `Ok` → audit `Ok`（`remove_routes already_gone ...`）。
- `release_routes` / `release_tun_addresses`：`Removed/Reset` 与 `AlreadyGone`
  均 `mark_released` 且不计 failure；`Err` 才 `mark_failed` + 保留 owned +
  failures。零 failure 才 `closed=true`，audit `Ok`；AlreadyGone 释放后重试无
  backend 工作（closed 短路）。

## 3. net_host 侧接线

`services/net_host/src/helper_client.rs`：

- 新增 `is_already_gone_result` / `is_cleanup_confirmed`
 （`RoutesRemoved | AlreadyGone`）/ `map_cleanup_result`
 （确认 → `Ok`；`E_NOT_FOUND` → `Ok` 收敛；其余 `Err` 保留）。
- `PipeHelperLink::cleanup`（Windows）改走 `map_cleanup_result`：
  `AlreadyGone` 与 `RoutesRemoved` 同为成功确认，`E_NOT_FOUND` 亦收敛；
  意外 `Ok` 变体报 `tun_apply_failed` 而非吞掉。
- `FakeHelperLink` 新增 `FakeHelperFault::CleanupAlreadyGone`：
  清空自有 routes/tun（模拟 OS 已无），记 `cleanup already_gone`，
  返回 `E_NOT_FOUND`，走 `is_already_absent` 收敛路径。

`services/net_host/src/tun_lease.rs`：`cleanup_tun_lease` /
`reconcile_stale_tun` / `retry_tun_cleanup` 无结构改动——`Ok`（含 AlreadyGone
经 pipe 归一）与 `E_NOT_FOUND` 两条收敛路径均清 journal + pending；
注释更新（helper 已有返回该语义的路径，不再是“不可能误收敛”的保守假设）。

## 4. 测试（FakeBackend + in-memory link，无 10808、无真实 OS 写）

helper（`cargo test -p privileged_helper`）新增 4：

- `dispatch.rs::sp08_remove_routes_reports_removed_vs_already_gone`
 （removed vs already-gone dispatch，含 journal 释放 + audit Ok 断言）。
- `sp08_cleanup_journal.rs::sp08_cleanup_already_gone_releases_journal_and_closes`
- `sp08_cleanup_journal.rs::sp08_tun_reset_already_gone_releases_journal_and_closes`
- `sp08_cleanup_journal.rs::sp08_retry_after_failure_converges_on_already_gone`
 （fail → AlreadyGone 重试收敛，故障注入重试链不断）。

net_host（`cargo test -p net_host`）新增 5：

- `helper_client.rs::sp08_already_gone_result_is_a_cleanup_confirmation`
 （`is_*` + `map_cleanup_result` 三路：AlreadyGone/Removed/NotFound→Ok，
  Backend→Err）。
- `helper_client.rs::sp08_fake_already_gone_cleanup_reports_not_found_and_clears_state`
- `tun_lease.rs::sp08_cleanup_already_gone_fault_converges_like_success`
- `tun_lease.rs::sp08_reconcile_already_gone_fault_counts_cleaned`
- `tun_lease.rs::sp08_retry_after_failure_converges_on_already_gone`
  既有 `CleanupFails/Unavailable` 重试测试保持全绿。

## 5. 定向检查（exit 均为 0）

- `cargo fmt -p privileged_helper -p net_host -- --check` → 0
- `cargo clippy -p privileged_helper -p net_host --all-targets --locked -- -D warnings` → 0
- `cargo test -p privileged_helper --locked` → 0
  （lib 16、dispatch 32、loopback 8、sp08 7；real_windows 1 ignored 需提权）
- `cargo test -p net_host --locked` → 0（134 passed / 0 failed）

未运行：workspace 全量门禁（发布候选才跑）；授权隔离机真实 TUN/路由
`ERROR_NOT_FOUND` 收敛（无环境，未验证）；跨层 UI 重试入口 N-H1（待 SP-00）。

## 6. 改动文件（本卡）

- `services/privileged_helper/src/{backend,server,windows,lib}.rs`
- `services/privileged_helper/tests/{dispatch,sp08_cleanup_journal}.rs`
- `services/net_host/src/{helper_client,tun_lease}.rs`（测试内新增用例）
- 本目录 `ALREADY-GONE.md`（新）

未动：`crates/ipc_contract/**`、`crates/bridge_api/**`、
`crates/application/**`、workspace Cargo 文件、127.0.0.1:10808。
