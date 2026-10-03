# UX-PARITY-FIX-07 — 设为活动 → 正常生效 → 重开仍生效

应用基线 HEAD：`c1c9c77`（干净基线）。冻结上游：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
本回合未改 `work/`、`outputs/`；未占用/修改 10808；未改宿主系统代理/注册表/路由/TUN；测试内核端口 11841（≥11808，先探测）。

## 变更概览

- `crates/application/src/runtime_client.rs`：新增最小接口 `AppliedSession{session_id, active_index_id, proxy_port, applied_revision}`；`NullRuntimeClient` 增加 `mark_running_with/set_state`，`stop()` 清端口/会话。
- `crates/application/src/engine.rs`：`AppEngine` 增 `applied_session`/`apply_target`；`apply_runtime` 记录 apply 目标；`snapshot()` 时 `reconcile_applied_session`（仅 `Running` 且有端口才发布；`Stopped/Degraded/RollingBack` 撤回；busy 保留）；`local_proxy_url()` 优先实际会话端口。
- `crates/application/src/lib.rs`：导出 `AppliedSession`。
- `services/net_host/src/session.rs`：stop/rollback 清空 `detail.ports/session_id/config_sha256`，失败候选不发布端点、停核撤回。
- `apps/desktop/lib/features/runtime/runtime_bridge.dart`：`RuntimeBridge.activeProfileId()`；`RuntimeView.proxyPort/hasAppliedEndpoint`。
- `apps/desktop/lib/features/runtime/runtime_controller.dart`：`applyActive` 先 refresh 取最新 desired revision；新增 `restoreActiveOnLaunch`/`restoreAttempted`。
- `apps/desktop/lib/app/app.dart`：普通启动调用 `restoreActiveOnLaunch()`（不再只靠测试 env 武装）。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p application -- --check` | exit 0 |
| `cargo clippy -p application --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test -p application --lib --locked` | exit 0；150 passed |
| `cargo fmt -p net_host -- --check` | exit 0 |
| `cargo clippy -p net_host --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test -p net_host --locked` | exit 0；44 passed |
| `dart format`（本次改动文件） | exit 0 |
| `flutter analyze` | No issues found |
| `flutter test test/runtime_controller_test.dart` | exit 0；8 passed |
| `flutter test test/t18b_runtime_ui_test.dart` | exit 0；3 passed |
| `flutter test integration_test/ux_parity_fix07_active_test.dart -d windows`（activate） | exit 0；1 passed |
| 同 data dir 跑 `reopen` | exit 0；1 passed |

## 真实窗口观察

`observations.json`（activate，`recordingComplete=true`）：
- `safe-inbound-port-saved`：`safePort=11841`、`persistedPort=11841`。
- `node-imported`：`imported=1`，合成订阅 `s-…`，错误 null。
- `set-active-persisted`：期望/持久化/控制器活动 ID 三处一致 `p-…`。

`reopen-observations.json`（reopen，`recordingComplete=true`）：
- `reopen-active-persisted`：`getActiveProfile()` 与控制器 `activeId` 均等于 activate 记录 ID。
- `reopen-restore-attempted`：`restoreAttempted=true`、`state=Running`、`error=null`、`ports=[11841]`、`sessionId=s-…`、`desiredRevision=3`、`appliedRevision=3`。

即：普通重开（未设任何 `V2RAYN_R_*` 武装变量）后，活动节点仍生效并真实启动了受管 Xray；applied 端点来自 net-host 实际报告，而非配置期望端口。

## 截图

- `activate-window.png`：activate 模式主窗口。
- `reopen-window.png`：reopen 模式主窗口（恢复运行）。

## 过程卫生

- 仅探测/使用 11841；未监听/连接 10808。
- 未按名杀进程。测试结束后 11841 无监听、无本任务 Xray 残留（`Get-NetTCPConnection -LocalPort 11841` 为空；留存进程仅为用户官方 v2rayN 的 `xray.exe` 与既有 `dist/…/net_host.exe`，非本任务启动）。
- 一次 activate 运行遇到 Flutter Windows 引擎 `exit 79`（已知环境问题），清空独立 data dir 后重试通过。
