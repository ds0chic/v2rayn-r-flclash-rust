# FIX-07 — 设为活动 → 正常生效 → 重开仍生效

状态：`implemented`（运行会话端点发布/撤回与正常启动恢复已在真实 Windows 窗口 + 真实 FRB/Rust/net-host/Xray 验证；击活入口的幂等一跳需 `features/profiles` 子代理落地最小补丁，见“接口缺口”）。

任务 ID：FIX-07

本次唯一用户流程：在节点表把某合成节点“设为活动”，运行受管核心并按冻结时机取最新 desired revision 后 apply，错误清楚反馈；正常重开（不设任何测试 env）后活动节点仍生效并恢复受管运行。保存活动节点、切换路由、F5 重载是另卡，不在本卡范围。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `c1c9c77`（干净基线，未回退审查已修好的菜单/关闭/17 根条目/中文列头/间距）。审查结论见 `docs/evidence/parity-review-2026-10-03/repair-queue.md` FIX-07 行、`profiles-report.md` PR-01、`runtime-report.md` RT-01/02/03、`root-report.md` ROOT-03、`all-items.json` 的 `F-PROFILE-003/006`、`ACT-PROF-002/005/030`、`ACT-MAIN-035` 与 `runtime-items.json` 的 `F-CORE-005`、`F-APP-007`、`CRM-003` 等。

对应 feature / field / action / layout ID：`F-PROFILE-006`（设为默认节点）、`F-PROFILE-003`（删除当前节点重载）、`ACT-PROF-005`（设为活动服务器）、`ACT-PROF-030`（双击活动）、`ACT-MAIN-035`（重载内核/F5，另卡）、`F-CORE-005`（启动/停止/重载）、`F-APP-007`（经代理检查更新，消费 applied 端点）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/ViewModels/ProfilesViewModel.cs:560`（`SetDefaultServer(indexId)`：空/相同直接返回，取 item 失败提示，`ConfigHandler.SetDefaultServerIndex` 成功后才 `RefreshServers(); Reload()`）、`ServiceLib/Handler/ConfigHandler.cs:406`（`SetDefaultServerIndex` 只写 `config.IndexId` 并 `SaveConfig`）、`ServiceLib/ViewModels/MainWindowViewModel.cs:319-342`（`StatusBarViewModel.SetDefaultServerRequested` 订阅 → `SetDefaultServer`；`Init()` 末尾无条件 `await Reload()`）、`MainWindowViewModel.cs:664-738`（`Reload` 取默认服务器→`BuildAll`→`LoadCore`→更新系统代理）、`ServiceLib/Manager/CoreManager.cs`（Stop→Start）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：节点表选中的单个节点稳定 ID；运行时 apply 的 target 为空 → 解析为持久化活动节点；期望 revision。
- 输出：活动节点 ID 落 `guiNConfig.json.active_index_id`；`apply_runtime` 返回操作 ID；`RuntimeView` 反映真实 `state/pid/ports/sessionId/desired/applied`。
- 错误：无活动节点时 `E_FIELD_REQUIRED error.no_active_profile`；revision 过期 `E_REVISION_STALE`；核心/生成失败结构化错误；控制器保留错误，不被随后 Stopped 快照覆盖。
- 取消：本卡无取消入口；停止受管核心为独立动作。
- 权限：无 TUN/系统代理/注册表写入；仅本机受管进程（net-host 唯一持有者）。
- 持久化：`active_index_id`（guiNConfig）+ profile（SQLite）；重开 Engine 读回。
- 生效：`applyActive()` 先 `refresh()` 取最新 desired revision 再 apply；`restoreActiveOnLaunch()` 在普通启动对持久化活动节点执行一次 apply。
- applied 与 desired 分离：`AppliedSession` 仅在 net-host 报 `Running` 且报告了绑定端口时发布；`Stopped/Degraded/RollingBack` 撤回；busy 状态保留旧事实。

允许修改的模块：`crates/application/src/{engine.rs,runtime_client.rs,lib.rs}`、`services/net_host/src/session.rs`、`apps/desktop/lib/features/runtime/{runtime_bridge.dart,runtime_controller.dart}`、`apps/desktop/lib/app/app.dart`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`docs/evidence/UX-PARITY-FIX-07/**`、本卡、`compat/features.yaml`/`actions.yaml`（仅 notes 追加）。

禁止改变的已有行为：`main_shell.dart`、`frb_generated.dart`/`frb_generated.rs`、`features/subs/**`、`features/profiles/**`（另一子代理在改）；不删入口、不降分母、不伪造运行事实；不占用/修改 10808。

测试夹具和原版预期：合成订阅 + 合成 `vless://` 节点（`127.0.0.1:11998`，不下载、不连接）；inbound 持久化为 ≥11808 的自由端口（本次 11841）；真实内核 `tools/cores/xray/v26.3.27`。原版预期：相同 ID 重复设默认不改变、不置空；不同 ID 切换；切换后 Reload 内核；重开后活动节点仍生效并恢复运行。

本次必须通过的命令/真实场景：
- `cargo fmt -p application -- --check`、`cargo clippy -p application --all-targets --locked -- -D warnings`、`cargo test -p application --locked`
- `cargo fmt -p net_host -- --check`、`cargo clippy -p net_host --all-targets --locked -- -D warnings`、`cargo test -p net_host --locked`
- `dart format`（本次改动文件）、`flutter analyze`
- `flutter test test/runtime_controller_test.dart`、`flutter test test/t18b_runtime_ui_test.dart`
- 真实窗口：`V2RAYN_R_FIX07_MODE=activate` 后同 data dir 跑 `reopen`。

证据文件位置：`docs/evidence/UX-PARITY-FIX-07/`（`observations.json`、`reopen-observations.json`、`activate-window.png`、`reopen-window.png`、`README.md`）。

完成条件：Rust 单测覆盖幂等设活动/切换、desired 不冒充 applied、失败候选不发布端点、Running 发布实际端口、停核撤回、applied 节点为 apply 目标；Flutter 单测覆盖 apply 前取最新 revision、错误不假成功、正常启动恢复；真实窗口“设为活动持久化→重开仍活动且恢复 Running（applied==desired，端口=11841）”。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：击活入口 `toggleActiveSelected`（`features/profiles/profile_actions.dart:181`）仍把“重复选同节点”写成 `null`，且 `setActive` 不触发 apply。最小补丁建议（不本卡改）：新增 `setActiveSelected`，`if (state.activeId == id) return;` 后 `controller.setActive(id); if (result.ok) await ref.read(runtimeControllerProvider.notifier).applyActive();`，并把 `profiles_table.dart:393/455/893` 的 `toggleActiveSelected` 调用替换为它。`test/user_journey_review_test.dart` 的 `repeat-set-default-keeps-node` 已编码该语义。
- 接口缺口（登记）：`AppliedSession` 目前只有单一 inbound `proxy_port`，没有独立 API/telemetry 端口字段（net-host 只 `ports: vec![port]`）；若后续需要“proxy/API 两类端点”，需要扩展 `RuntimeDetail`/`SnapshotDto` 并重生成 FRB。本卡未新增 FRB 函数，未改生成文件。
- 接口缺口（登记）：正常启动恢复会让官方打包冒烟 `tools/release/negative_unarmed.ps1`（种子含 active 节点、断言无核心）行为变化（现在会按持久化活动节点恢复运行）。这是 FIX-07 验收要求的有意行为，需根代理更新该负向脚本口径（改为断言“不因 env 变量武装”，而非“永不启动核”）。

本轮实际结果：`crates/application` 新增 `AppliedSession` 与 `AppEngine::{applied_session, reconcile_applied_session}`，`apply_target` 记录 apply 时的活动节点，`snapshot()` 时对账，`local_proxy_url()` 优先实际会话端口；`NullRuntimeClient` 增加 `mark_running_with/set_state`。`services/net_host/session.rs` 在 stop/rollback 清空 `ports/session_id/config_sha256`，使失败候选与停核不发布端点。Flutter `RuntimeBridge` 增加 `activeProfileId()`；`RuntimeView` 增加 `proxyPort/hasAppliedEndpoint`；`RuntimeController.applyActive` 先 refresh 再 apply，新增 `restoreActiveOnLaunch`；`app.dart` 普通启动调用它。证据：activate `set-active-persisted=true`；reopen `reopen-active-persisted=true`、`reopen-restore-attempted=true`、`state=Running`、`ports=[11841]`、`sessionId=s-…`、`desired=applied=3`。门禁见 README。
