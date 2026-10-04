# R3-PROXY-UI — 系统代理/PAC 入口按协议区分 SOCKS/HTTP

状态：`implemented`（Dart 单元/控制器合成会话验证 SOCKS 与 HTTP/mixed 产生不同指令/命名代理串；无运行会话明确提示；真实系统代理/PAC 写入未运行）。

任务 ID：R3-PROXY-UI

本次唯一用户流程：状态栏/托盘/启动恢复选择系统代理模式（`ForcedChange` 或 `Pac`）。入口必须使用 `AppliedSession` 的实际端点端口与协议：SOCKS 端点不能被当成 HTTP 代理，且没有运行会话时必须明确提示而不是假装已应用。

前置任务及已验证证据：`docs/tasks/R3-07.md` 已登记“系统代理/PAC 入口仍只取端口、不区分 SOCKS/HTTP”为接口缺口（Rust 端 `ProxyProtocol` scheme/listen/auth 已产出）；本轮 UI 子代理按该登记补齐。复核见 `docs/evidence/parity-recheck-2026-10-04/round3-runtime.md` R3-07。

上游对照：`ServiceLib/Handler/SysProxy/SysProxyHandler.cs:19`（代理端口取 `GetLocalPort(EInboundProtocol.socks)`）、`:86-108`（`GetWindowsProxyString` 命名代理串）、`:110-117`（PAC 用 socks 端口启动 `PacManager`）；`PacManager.cs:49`。

对应 feature / field / action：`RR-03`、`RR-07`、`F-SYSPROXY-001`、`ACT-STAT-001`、`ACT-STAT-002`；`SystemProxyItem.SysProxyType`、`SystemProxyAdvancedProtocol`、`Inbound`（`LocalPort`/`Protocol`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：运行中 `RuntimeView`（`ports`）+ 持久化 `Inbound`（协议/端口）+ `SystemProxyItem`。
- 输出：SOCKS 端点 → `ForcedChange` 用 `socks=127.0.0.1:{port}`，PAC 用 `SOCKS5 127.0.0.1:{port};DIRECT;`；HTTP/mixed 端点 → 裸 `127.0.0.1:{port}`（或高级协议模板）与 `PROXY 127.0.0.1:{port};DIRECT;`。
- 错误：无运行会话 → 结构化 `E_NO_RUNNING_SESSION`，PAC 不启动、系统代理不写。
- 权限：`None`；secret 不进入任何日志/证据（本卡不读取任何 Custom secret）。
- 持久化：`SystemProxyItem.SysProxyType` 在成功/无会话路径均保存，重开保持选择。

允许修改的模块：`apps/desktop/lib/features/settings/{proxy_settings_view.dart,platform_controller.dart,platform_bridge.dart}`、`apps/desktop/lib/app/shell/status_bar_view.dart`、`apps/desktop/test/**`、本卡、证据目录、compat 台账（仅追加）。

禁止改变的已有行为：R3-01 的 PAC 渲染合同；RR-02/03 统一命令结构；FIX-15/15C 解析/自启；不改 `main_shell.dart`/`app.dart`/`frb_generated`/`lib/bridge/api/**`。

测试夹具与原版预期：合成 Running runtime（端口 11808）；`Inbound` 分别置 `Protocol:0`(SOCKS) 与 `Protocol:6`(mixed)；空会话。原版预期：代理端口取 SOCKS inbound；无会话不写系统代理。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze`
- `flutter test test/r3_01_07_10_test.dart test/recheck_rr02_03_test.dart`

证据文件位置：`docs/evidence/recheck-fixes/R3-01-07-10/`。

完成条件：SOCKS 与 HTTP/mixed 在 ForcedChange 与 PAC 上呈现不同且合法；无会话提示明确；`recheck_rr02_03` 既有诚实性测试继续通过；门禁通过。

接口缺口（登记）：`RuntimeView`/`SnapshotDto` 未发布已应用端点协议（仅 `runtime_ports`）。当前从生成该计划的持久化 `Inbound` 推导协议并校验端口；对纯 Custom 内联配置（端口/协议不在 `Inbound`）仍只能退回首个应用端口 + 默认 HTTP。建议 snapshot 增设 `applied_endpoint`（scheme/listen/port）字段，`frb_generated` 由注册流程统一再生成。

本轮实际结果：`buildSystemProxyServer`（协议感知的 ForcedChange 串）与 `buildPacProxyRule`（协议感知的 PAC 指令）落地；`primaryLocalProxyInbound` 优先取 SOCKS inbound；`_appliedProxyInbound` 用 `AppliedSession` 端口并在可用时匹配配置端口。`recheck_rr02_03_test` 的 PAC/无会话/启动恢复用例继续通过。
