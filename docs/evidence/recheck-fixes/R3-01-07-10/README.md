# R3-01 / R3-07 UI / R3-10 修复证据（2026-10-04）

状态：`implemented`。开始 HEAD `593e289`；修复合同见 `docs/tasks/R3-01.md`、`docs/tasks/R3-PROXY-UI.md`、`docs/tasks/R3-10.md`；复核 `docs/evidence/parity-recheck-2026-10-04/round3-runtime.md` 的 R3-01/R3-07/R3-10。未提交（未 `git add`/`commit`）。

本轮只在 Windows 本机跑 Dart format/analyze/单元测试；未写系统代理、未启动 PAC 监听、未碰 10808、未读用户凭据/Custom secret；平台状态全部经 `FakePlatformBridge`，运行状态全部为合成 `RuntimeView`。

## 改动文件

产品代码：
- `apps/desktop/lib/features/settings/proxy_settings_view.dart`：新增 `ProxyProtocolKind`、`LocalProxyInbound`、`primaryLocalProxyInbound`（优先 SOCKS inbound）、`buildSystemProxyServer`（协议感知 ForcedChange 串）、`buildPacProxyRule`（`PROXY`/`SOCKS5 host:port;DIRECT;`）。保留 `buildProxyServer` 供既有 ForcedChange/模板测试。
- `apps/desktop/lib/features/settings/platform_controller.dart`：`_appliedProxyPort` → `_appliedProxyInbound`（AppliedSession 端口 + 协议）；`ForcedChange` 用 `buildSystemProxyServer`，`Pac` 用 `buildPacProxyRule`；无会话仍 `E_NO_RUNNING_SESSION`。
- `apps/desktop/lib/features/routing/routing_controller.dart`：`setDefaultAndReload` 在 `reload()` 后按 `RuntimeView.error` / `hasUnappliedChanges` 如实呈现成功或具体失败，不回滚“先保存默认再 Reload”。

测试：
- `apps/desktop/test/r3_01_07_10_test.dart`（新增，9 项）：PAC renderer 指令串、协议区分、无会话提示、reload 成功/失败。
- `apps/desktop/test/support/fake_platform_bridge.dart`：新增 `lastPacProxyRule` 记录 `pacStart`/`pacStartFromFile` 的 proxyRule。

未改 Rust：`crates/platform/src/pac.rs::render_pac` 的 `__PROXY__` 占位替换本就正确，缺陷在 Dart 入口传错字符串，故按“如需”不改 `pac.rs`。

## 上游对照

| 行为 | 冻结原版（7d6a967） | 本轮实现 |
|---|---|---|
| PAC `__PROXY__` 替换 | `PacManager.cs:49` → `PROXY 127.0.0.1:{httpPort};DIRECT;` | `buildPacProxyRule`：HTTP/mixed → `PROXY host:port;DIRECT;`；SOCKS → `SOCKS5 host:port;DIRECT;`（协议区分，见 R3-PROXY-UI） |
| ForcedChange 命名代理串 | `SysProxyHandler.cs:86-108` `GetWindowsProxyString`（无模板则裸 `loopback:port`） | `buildSystemProxyServer`：有高级模板照旧；无模板且 SOCKS → `socks=loopback:port`，否则裸地址 |
| 代理端口来源 | `SysProxyHandler.cs:19` `GetLocalPort(EInboundProtocol.socks)` | `_appliedProxyInbound`：以 `RuntimeView.ports` 为准，优先匹配持久化 SOCKS inbound 端口 |
| 默认路由重载 | `StatusBarViewModel.cs:391-417` `SetDefaultRouting` + `ReloadRequested` | 保持先保存默认再 `reload()`；成功提示以新 applied 事实为准 |

## 实际命令与结果

| 命令 | 结果 | 证据 |
|---|---|---|
| `dart format --output=none --set-exit-if-changed lib test` | exit 0（0 changed） | `format-check.log` |
| `flutter analyze` | 1 warning：`lib/features/profiles/group_editor_dialog.dart:57 _isProfileValid` 未引用（**本卡范围外**，为并行编辑文件，未触碰） | `analyze.log` |
| `flutter test test/r3_01_07_10_test.dart test/t13_platform_models_test.dart test/recheck_rr02_03_test.dart test/fix15c_pac_resolve_test.dart` | exit 0，32 passed | `r3-tests.log` |
| `flutter test test/t13_statusbar_test.dart test/fix15_tray_pac_test.dart test/recheck_r01_runtime_reload_test.dart test/runtime_controller_test.dart test/t18b_runtime_ui_test.dart test/recheck_rr08_tray_sync_test.dart test/t15a_statusbar_test.dart` | exit 0，33 passed | `regression-tests.log` |

R3-01 断言：合成 Running runtime（port 11808）下 `applyModeFromConfig(Pac)` 传入 bridge 的 proxyRule 为完整 `SOCKS5 127.0.0.1:11808;DIRECT;`（SOCKS inbound），HTTP/mixed 为 `PROXY 127.0.0.1:11808;DIRECT;`；`buildPacProxyRule(port:11808)` = `PROXY 127.0.0.1:11808;DIRECT;`。

R3-07 断言：`Inbound Protocol:0` → `socks=127.0.0.1:11808` + PAC `SOCKS5 ...`；`Protocol:6` → 裸 `127.0.0.1:11808` + PAC `PROXY ...`；无会话 → `E_NO_RUNNING_SESSION` 且 PAC 未启动。

R3-10 断言：成功保存 + 成功 reload → `已切换默认路由并重载`；成功保存 + `applyActive` 失败（保留 rev 7 旧 applied）→ 状态含 `重载失败`/`E_CONFIG_CHECK`，默认路由仍切换，旧 applied 保留。

## 未验证 / 边界

- 真实 WinINET/WinHTTP 系统代理写入、真实 PAC HTTP 监听与浏览器执行未运行（硬约束：仅在批准隔离环境可做）。
- 真实 `codegen`/config-check 导致的路由 reload 失败未端到端运行；仅合成失败。
- 真实 Custom 内联配置（端口/协议不在 `Inbound`）只能退回首个应用端口 + 默认 HTTP；见接口缺口。
- `flutter build windows`、全量 `flutter test` 未运行。

## 接口缺口（登记）

1. `RuntimeView`/`SnapshotDto` 仅发布 `runtime_ports`，无已应用端点协议（`ProxyProtocol` scheme/listen/auth）。当前从持久化 `Inbound` 推导协议并校验端口；建议 snapshot 增加 `applied_endpoint` 字段后由注册流程统一再生成 `frb_generated`（本卡未改生成物）。
2. `RuntimeController.reload()` 返回 `void`；调用方只能读 `RuntimeView` 推断结果。若需精确操作结果，应返回结构化 `RuntimeActionResult`。

## 铁律核对

- 未占用/修改 127.0.0.1:10808；测试端口 <11808 未使用，合成数据无真实监听。
- 未改宿主系统代理/PAC/注册表；未按名杀进程；未读/写用户凭据与 Custom secret。
- 未改 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/profiles/**` 等禁止清单。
- `compat/` 仅追加注释条目，未删行；`work/`、`outputs/` 只读。
