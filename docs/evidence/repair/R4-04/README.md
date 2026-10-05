# R4-04 命令队列与迟到响应 — 证据

状态：implemented（Dart 契约测试与 Rust 单测通过；`flutter analyze`、Rust 门禁、`flutter build windows --release` 通过；真实 GUI 连续启停/重试未实测）。

HEAD/baseline：`cbce523`（应用基线 `77c74ed`，上游冻结 `7d6a967`）。armed=false。无端口使用（本卡测试不监听任何端口，未碰 10808）。合成夹具；未改宿主系统代理/注册表/路由/TUN/自启；未读取用户凭据；未改 `main_shell.dart`/`app.dart`/frb_generated/bridge/api/profiles/subs/settings/routing/monitor。

## 本次唯一用户流程

连续启动/停止/重试仍反馈及时，并落到可信最终状态（不假造 Running、不无限排队、迟到响应不覆盖新状态）。

## 对照的缺陷与方案条目

- D06：pending 不立即暴露、apply 无统一在途合并 → 本地 pending 同步可见 + 有界合并队列。
- D07：`crates/application/src/net_host_client.rs` 先等 `req_mutex` 再 `recv_timeout`，排队不计入总截止时间；超时 worker 不由代码取消 → 请求槽由 worker 持有、以统一 deadline 等待、worker 计数可观测。
- D37 / §3 第 4-6 条：`apply` 终态/排队/取消合同含糊 → 队列 deadline 含等待、未知结果先 reconcile、不假造成功。

## 实现概要

### Rust（`crates/application/src/net_host_client.rs`）

- 以 `RequestGate` 取代 `req_mutex`：单一请求槽 + `Condvar`。`acquire(deadline)` 在**同一 deadline**内等待前一个 worker，排队时间计入总时限。
- worker 完成时才 `record_finish()` 释放槽；若调用方超时，槽仍被占用，后续请求不会在其背后再起线程 → worker 数**恒 ≤ 1**、不累积。
- 新增可观测计数 `NetHostClient::request_worker_counts() -> (active, spawned)`。

### Dart（`apps/desktop/lib/features/runtime/runtime_controller.dart` / `runtime_bridge.dart`）

- 有界合并命令队列：同时最多 1 个活动命令 + 1 个合并 apply + 1 个合并 stop；apply 取最后目标，waiters 随替代命令一起完成，不挂起调用方。
- `commandDeadline`（含排队时间）到期即拒绝排队命令并给出 `E_TIMEOUT`，不迟到执行。
- `refresh()` 在途合并：并发刷新共用同一 future，事件风暴不叠加到同一 IPC 锁。
- 迟到响应淘汰：`_stateGeneration` 在命令启动或空闲期 epoch 变化时推进；被超越的 snapshot/命令结果丢弃并计入 `staleResponsesDropped`。
- 未知结果先 reconcile：stop 返回 `E_TIMEOUT/E_UNAVAILABLE/E_BRIDGE` 时先刷新真实快照并置 `reconcileNeeded`，不直接报成功/失败。
- `RuntimeView` 新增只读字段 `pendingCommands`、`staleResponsesDropped`、`reconcileNeeded`（保持既有字段与 R4-01/R4-02 的 `commandPending`/`isBusy`/显式目标接口不变）。

## 断言复现

`test/r4_04_contract_test.dart`（6 条，全部通过）：

1. 本地 pending 在任何 await 前同步可见，且为 Stopped、非 Running。
2. 队列有界且合并到最新目标（3 次 apply → 2 次调用，lastTarget=node-c，pending=2→0）。
3. 排队超过 deadline 的命令被拒绝（0 次额外调用，`E_TIMEOUT`）。
4. 并发 refresh 合并（在途 1 次，合并后最多再 1 次，无风暴）。
5. 被命令 generation 超越的 snapshot 被丢弃（`staleResponsesDropped>0`）。
6. stop 未知结果先 reconcile（保留错误 + `reconcileNeeded=true` + 发生刷新）。

Rust 单测（`request_worker_counts` 相关 4 条新增）：
`request_slot_deadline_includes_queue_wait`、`request_slot_does_not_accumulate_workers`、`net_host_client_exposes_worker_counts` 等全部通过。

## 实际命令与结果

- `flutter analyze` → No issues found!
- `flutter test test/r4_04_contract_test.dart` → All tests passed（6）。
- 回归 `test/r4_01_contract_test.dart`、`r4_02_contract_test.dart`、`r4_08_contract_test.dart`、`r4_27_contract_test.dart`、`t18b_runtime_ui_test.dart` → 全部通过（并行 5 个套件时 `r4_08` 曾偶发 “did not complete”，单独重跑通过；t18b 同因并行偶发，单独重跑通过）。
- `cargo fmt --all -- --check` → 通过（无输出）。
- `cargo clippy --workspace --all-targets --locked -- -D warnings` → Finished，无警告。
- `cargo test --workspace --locked` → 全部 test result: ok，EXIT=0；新增 4 条 `net_host_client` 单测通过。
- `flutter build windows --release` → Built `build\windows\x64\runner\Release\v2rayn_desktop.exe`。

## 构建前实例检查

`Get-Process v2rayn_desktop,net_host,v2rayN`：发现 `net_host` (PID 32340, `dist\v2rayN-R-1.0.0+1-windows-x…`) 与本进程无关，未持有 `build\` 输出锁，未由本卡启动，故未停止；`v2rayN` (PID 11728) 为上游 C# 程序，未触碰。未按进程名批量终止。

## 未完成 / 未验证

- 真实 GUI 连续应用/重载/停止与 60s 真超时不累积 worker 的端到端未运行（无 armed 包，未启动受管内核）。
- 未做 Windows 真实命名管道压力与 release 帧耗时量化。
- 接口缺口：Dart 侧没有结构化 operation-status 查询（IPC `GetOperation` 未经 bridge_api/FRB 暴露），reconcile 只能靠 snapshot；登记见 `compat`（仅追加）。修复需整合者固定接口与唯一写入者。
