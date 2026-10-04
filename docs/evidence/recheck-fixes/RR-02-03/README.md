# RR-02 / RR-03 修复证据（2026-10-04）

状态：`implemented`。修复合同见 `docs/tasks/RR-02.md`、`docs/tasks/RR-03.md`；定位复核见 `docs/evidence/parity-recheck-2026-10-04/runtime.md` RR-02/RR-03。

本轮未启动真实核心、未点击真实托盘、未写宿主 WinINET/WinHTTP 代理、未启动 PAC 监听端口。平台副作用只经 `FakePlatformBridge`，运行状态只经 `CountingRuntimeBridge`（合成）。

## 改动文件

生产：
- `apps/desktop/lib/features/routing/routing_controller.dart`：新增 `setDefaultAndReload(String)`（保存默认路由后按原版时机 `RuntimeController.reload()`）；`select` 保持编辑选中；`setDefault` 保持既有窗口语义。
- `apps/desktop/lib/features/settings/platform_controller.dart`：新增 `applyModeFromConfig` / `restoreAppliedModeOnLaunch` / `_appliedProxyPort` / `_configDir`；PAC 走既有 `startPacFromConfig`；无运行会话报 `E_NO_RUNNING_SESSION` 且仍保存模式。
- `apps/desktop/lib/app/shell/status_bar_view.dart`：系统代理下拉与路由下拉改走统一命令；删除只提示 PAC 的旧 `_applyMode` 与期望端口 `_firstInboundPort`。
- `apps/desktop/lib/app/shell/desktop_integration.dart`：托盘/热键代理模式改走统一命令；托盘路由勾选按 `RoutingItem.IsActive`；删除极简 `_defaultPacScript`、`_persistProxyMode`、`_firstInboundPort`。

测试：
- `apps/desktop/test/recheck_rr02_03_test.dart`（新增，5 项）
- `apps/desktop/test/t13_statusbar_test.dart`（补运行会话，适配 RR-03 真实端口合同）
- `apps/desktop/test/support/fake_platform_bridge.dart`（记录 `lastServer` / `lastAutoConfigUrl`）

台账（仅追加）：`compat/features.yaml`（RR-02/RR-03 条目）、`compat/actions.yaml`（ACT-TRAY-002..006、ACT-STAT-001/002 notes）。

冻结文件接线补丁（未落地）：见两张任务卡；分别为 `main_shell.dart` 的 `onSelectRouting -> setDefaultAndReload`，`app.dart` 在 `restoreActiveOnLaunch()` 后调用 `restoreAppliedModeOnLaunch()` 并新增 `platform_controller.dart` import。

## 上游对照

| 本项目 | 上游（7d6a967） | 结论 |
|---|---|---|
| `RoutingController.setDefaultAndReload` | `StatusBarViewModel.RoutingSelectedChangedAsync`（:400-419）：SetDefaultRouting→ReloadRequested→图标刷新 | 保存后按原版时机 Reload；勾选按 IsActive；`select` 仍为编辑选中 |
| `PlatformController.applyModeFromConfig` | `StatusBarViewModel.SetListenerType/ChangeSystemProxyAsync`（:362-389）：写 `SysProxyType`→UpdateSysProxy→保存 | 同一命令保存模式+应用；状态栏/托盘/热键统一 |
| `startPacFromConfig`（沿用 FIX-15C） | `PacManager.InitText`（:32-49）：custom 存在则用，否则 `<config>/pac.txt` | 不再用托盘内联极简模板；`__PROXY__` 由后端替换 |
| `restoreAppliedModeOnLaunch` + app.dart 补丁 | `MainWindowViewModel` LoadCore 后 `UpdateSysProxy`（:704-705） | 重开核心恢复后按保存模式重设系统代理/PAC |
| `_appliedProxyPort` 读 `RuntimeView.ports` | `StatusBarViewModel.ChangeSystemProxyAsync` 用实际本地端口 | 不再用期望第一入站；无运行会话明确提示 |

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format`（仅本轮改动文件） | 0 changed（已格式化） |
| `flutter analyze` | No issues found（见 `analyze.log`） |
| `flutter test test/recheck_rr02_03_test.dart` | 5/5 passed（见 `rr02-03-tests.log`） |
| 回归 `t13_statusbar_test` / `fix15_tray_pac_test` / `fix15b_startup_test` / `fix15c_pac_resolve_test` | 21/21 passed（见 `regression-tests.log`） |
| 另跑（逐文件）`fix08_routing_draft_test`、`fix08c_routing_draft_test`、`recheck_r01_runtime_reload_test`、`runtime_controller_test`、`t18b_runtime_ui_test`、`recheck02_active_apply_test` | 单独运行全部通过；多文件并发偶发 “did not complete”（已知 `flutter test` flake），重试通过 |

未运行：`flutter build windows`、真实托盘/状态栏窗口、真实核心 Reload、宿主系统代理/PAC 写读、Rust 门禁（本轮无 Rust 改动）。

## 未验证 / 边界

- 宿主系统代理/PAC 写入与回读：未验证（无既有授权路径，未执行；测试保持 fake backend）。
- 真实托盘点击与真实核心重启：未运行；托盘 wiring 需先由根代理落地 `main_shell.dart` 补丁。
- `app.dart` 重开恢复调用：补丁未落地（文件冻结）；合成层面的 `restoreAppliedModeOnLaunch` 已覆盖。
- 行为差异：无运行会话时本实现拒绝写代理并提示，原版按配置端口仍可写；如需预设置需另立卡。
- 接口缺口：`configDir` 为 Dart 侧 `<dataDir>/config` 推导；Rust `pac_resolve_script` 因 `frb_generated` 未重生成暂不可直调（FIX-15C 同缺口）。

## 清理事故记录（如实登记）

本轮创建证据日志时误将工作目录当作仓库根，先写入了 `apps/desktop/docs/...`；随后移动日志并 `Remove-Item -Recurse` 删除了 `apps/desktop/docs`（未跟踪目录）。该目录内含其它任务意外落在 `apps/desktop` 下的未跟踪副本（显示为 `UX-PARITY-FIX-05B/anal*`、`form*`、`scan*` 等）以及本任务日志的副本。仓库受跟踪的 `docs/evidence/UX-PARITY-FIX-05B/README.md` 与仓库 `docs/` 树未受影响（`git status` 无相关删除）。删除的未跟踪副本无法从 git 恢复，已向根代理报告。