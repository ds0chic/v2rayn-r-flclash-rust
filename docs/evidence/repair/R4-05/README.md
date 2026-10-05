# R4-05 统一退出与快速重开 — 证据

状态：implemented（Dart 契约测试 7 条与新增 Rust 单测通过；`cargo fmt --all -- --check`、`cargo clippy`、`cargo test`、`flutter test`、`flutter build windows --release` 通过；`flutter analyze` 当前仅被**并发进行中的 R4-09 未提交文件** `lib/features/routing/routing_windows.dart` 阻断，R4-05 自身文件均干净；真实 GUI 退出→重开/交接端到端未实测）。

HEAD/baseline：`139fbea`（应用基线 `77c74ed`，上游冻结 `7d6a967`）。armed=false。无端口使用（本卡测试不监听端口，未碰 10808）。合成夹具；未改宿主系统代理/注册表/路由/TUN/自启；未读取用户凭据；未改 `main_shell.dart`/`app.dart`/frb_generated/`lib/bridge/api/**`/runtime/profiles/subs/settings/routing/monitor；未按进程名批量终止进程。

## 本次唯一用户流程

运行中真退出后立刻重开，或交接自更新；退出有界，重开不残留 lease/进程。

## 对照的缺陷与方案条目

- D29：快速退出重开或自更新交接，后台会话/平台恢复可能未结束；tray exit 路径不统一 stop/flush；自更新 500ms 后 `exit(0)` 绕开桌面退出；有 6s 失联兜底，不能说永久泄漏。
- 方案 §2 D29、§3 第 5 条；审计 RUN-08（退出/升级交接未统一；断连回收已有）。

## 实现概要

### Dart — 统一有界退出（`apps/desktop/lib/app/shell/desktop_integration.dart`）

- 新增 `runBoundedShutdown`：按序执行步骤，每步独立 `stepTimeout`，总预算 `totalBudget`；超时/抛错被记录且**继续后续步骤**（平台所有权一定恢复），预算耗尽则余下步骤标记 skipped，永不抛异常、永不挂死。
- `DesktopIntegration.runShutdown()` 固定顺序：`stop_runtime`（先停核，交接受理时无旧核文件锁）→ `flush_stats`（`monitor.monitorStartPolling()` 触发 Rust drain/flush）→ `restore_platform`（`stopPac` + `restoreOnExit(desiredMode)`）→ `stop_scheduler` → `unregister_hotkeys`。默认 `shutdownStepTimeout=5s`、`shutdownTotalBudget=15s`，`flushStats` 可注入。
- `exitApp()`：统一走 `runShutdown()`，未完成的清理用 `setMessage` 如实提示，再 destroy 窗口；`_exiting` 防重入。
- `exitForUpdate()`：自更新交接跑同一 `runShutdown()`（先停核、flush），再 `exit(0)` 供 runner 换包重启；清理失败仅记录、不阻断退出。
- 抽出 `DesktopLifecycle` 接口（`hideToTray/exitApp/exitForUpdate/removeListener`）供测试替身；`DesktopIntegrationHolder.value` 收敛到该接口（`main_shell` 的 `hideToTray` 调用不变）。
- `hideOnClose(isWindows, hide2TrayWhenClose)`：把「隐藏≠停止」的窗口关闭决策抽为纯函数并用于 `onWindowClose`（Windows 恒隐藏；其他平台按 `Hide2TrayWhenClose`）。

### Dart — 自更新交接（`apps/desktop/lib/features/update/update_controller.dart`）

- `applyAppUpdate` 成功后改为 `await handoffExit()`：有真实集成时先跑有界关机再退出；无集成（测试）回退到注入的 `AppExit`，保持 `recheck_rr04_app_update_test.dart`/`t16_update_test.dart` 语义不变。

### Rust — 退出时统计落盘（`crates/bridge_api/src/api/monitor.rs`）

- 新增 `persist_stats`（锁内取 `store_handle`+`snapshot_rows`，锁外写盘，沿用 D25）与 `#[frb(sync)] stats_flush()`（失败返回结构化 `SimpleResult.error`，不谎报成功）。
- `monitor_start_polling()` 在同步会话后执行 drain/flush，使已有 Dart 绑定即可在退出时落盘；失败写入 `store_error`。

### Rust — host 幂等回收与空闲退出（`services/net_host/src/{server.rs,session.rs}`）

- 新增纯决策 `idle_exit_due` 与 `HostState::should_exit_idle`：仅当**无客户端、无受管会话、无 TUN lease、无测试会话**且空闲 ≥ `max(grace, 15s)` 时为真。
- `watchdog` 在回收判定后检查空闲退出并 `notify_one`，host 进程随后正常退出 → 真实退出不残留服务进程；重开在空闲窗口内仍复用同一 host。新增单测 `idle_exit_requires_no_client_and_no_work_after_min`。

### Rust — 重连不猜 desired/Xray（`crates/application/src/engine.rs`）

- `reconcile_applied_session`：`Running` 快照下仅当本进程确有 `apply_target` 才发布 applied session；新 engine 重连已活 host（无 `apply_target`）时**不**回退到持久化 desired 节点，也不据默认值猜 Xray API。避免把实际 A 的流量记为 desired B / sing-box 当 Xray 轮询。

## 上游对照结论

上游 `AppManager.AppExitAsync`（`ServiceLib/Manager/AppManager.cs:122-148`）：`UpdateSysProxy(restore)` → `AppExitRequested`(等 50ms) → `SaveConfig` → `ProfileEx.SaveTo` → `Statistics.SaveTo` → `CoreManager.CoreStop` → `Statistics.Close` → `Shutdown`。
本卡按 R4-05 明示合同改为 `stop → drain/flush → 平台恢复`：**先在退出前停核**以满足「自更新 runner 无旧核文件锁」，其余步骤（平台代理/PAC 恢复、统计落盘、会话终止）与上游一一对应且有界。此顺序差异是修复方案的有意设计（上游先恢复代理），已登记。托盘退出（Windows `StatusBarView.xaml.cs:91`、`MainWindow.xaml.cs:203/302`）与菜单退出都走同一 `AppExitAsync`，本卡把 tray/菜单/自更新三条退出统一到同一 `runShutdown`。

## 断言复现

`apps/desktop/test/r4_05_contract_test.dart`（7 条，全部通过）：

1. 有界退出按 `stop → flush → restore → hotkeys` 合同顺序执行并全部完成。
2. stop 抛错不跳过平台恢复（失败隔离，代理不残留）。
3. 挂起步骤被 `stepTimeout` 截断，不挂死。
4. 总预算耗尽时余下步骤标记 skipped。
5. 隐藏不等于停止：Windows 恒隐藏、其他平台按 opt-in。
6. 自更新交接在有集成时走有界生命周期。
7. 无集成时回退到注入的 `exitApp`。

Rust 新增单测：`idle_exit_requires_no_client_and_no_work_after_min`（含连接中/运行中/有 lease/有测试会话/从未连接各分支）。

## 实际命令与结果

见 `commands.md`。摘要：`flutter test test/r4_05_contract_test.dart` → 7 passed；`cargo clippy --workspace --all-targets --locked -- -D warnings` → EXIT=0；`cargo test --workspace --locked` → EXIT=0；`flutter build windows --release` → EXIT=0（`apps/desktop/build/windows/x64/runner/Release/v2rayn_desktop.exe`，150016 bytes，SHA256 `6B3F8D1B…38EF0`）。R4-05 自身 Rust 文件 `rustfmt --check` EXIT=0。

## 构建前实例检查

`Get-Process v2rayn*|net_host|xray|sing-box|upgrade`：`net_host` PID 32340（`dist\v2rayN-R-1.0.0+1-windows-x64`，非本卡启动、无会话记录、未锁 `apps/desktop/build\`）；`v2rayN` PID 11728 + `xray` PID 18156（用户桌面上的上游程序，未触碰）。均未停止、未按名批量终止。

## 未完成 / 未验证

- 真实 GUI「运行→托盘退出→立刻重开」「自更新交接换包重启」未端到端运行（无 armed 包/未启动受管内核）。
- 真实 OS 效果（系统代理/PAC 恢复、core 进程消失、host 退出、数据目录可重开、单实例锁释放）需授权隔离机，未实测。
- `cargo fmt --all -- --check` 首轮曾在并发 R4-09 文件 `crates/application/src/routing.rs:620` 失败，其作者格式化后本卡末次运行 EXIT=0；`flutter analyze` 当前仅剩并发 R4-09 文件 `lib/features/routing/routing_windows.dart`（`_deleteScheme` 未引用 + `use_null_aware_elements`）。R4-05 文件本身干净。
- 接口缺口（登记，不绕过）：
  - **FRB 绑定**：`crates/bridge_api/src/api/monitor.rs::stats_flush` 已实现，但 `lib/bridge/api/monitor.dart` / `frb_generated` 未重生成，Dart 现经 `monitor_start_polling()` 的 drain 路径落盘；需接口整合者重生成。
  - **host 显式退出 IPC**：`net_host` 已有 `IpcOperation::Shutdown`（停会话 + `notify_one` 退出），但 `RuntimeClient`/`bridge_api`/FRB 无对应调用，Dart 当前依赖空闲退出 + 显式 `stop_runtime`；建议整合者补 `shutdown_runtime()`。
  - **host 发布 actual 事实**：重连仅能避免猜测（不发布 applied session），无法给出 actual node/core/端点；需在 `runtime` wire/`RuntimeDetail` 增补（属整合者所有权，`crates/runtime` 本卡未授权）。
