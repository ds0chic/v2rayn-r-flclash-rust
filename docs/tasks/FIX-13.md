# FIX-13 — TUN 开关主链 + 预 SOCKS / LegacyProtect plan 接线

状态：`implemented`（Rust 计划图/端口/启动序与 Flutter 开关“落盘→应用”闭环已实现并由合成测试验证；真实 TUN 端到端与预 SOCKS 进程执行登记 `blocked`，预 SOCKS/其它核执行拆到 FIX-13B/C）。

任务 ID：FIX-13

本次唯一用户流程：状态栏打开 TUN 开关 → 保存设置 → 重新应用运行计划；授权/取消与 helper 不可用明确反馈且不假成功；关闭开关同样落盘并重新应用。同一卡完成“预 SOCKS/LegacyProtect 主辅进程/端口图”写入 `RuntimePlan`（顺序按冻结 `CoreManager`）。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `b638821`（干净基线，未回退已修好的菜单/关闭/17 根条目/中文列头/间距）。审查结论见 `docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 38 行、`runtime-report.md` RT-05/06/07/08。FIX-07 已引入 `AppliedSession`（proxy_port/session_id/active_index_id/applied_revision）与端点发布/撤回，本卡复用。

对应 feature / field / action / layout ID：`F-CORE-005`（启动/停止/重载）、`FLD-CFG-102`（TunModeItem.EnableLegacyProtect / EnableTun）、`RT-05/06/07/08`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
`v2rayN/ServiceLib/Handler/ConfigHandler.cs:1555`（`GetPreSocksItem`：非 Custom 且 core≠sing-box 且 TUN+Legacy→sing-box SOCKS on `GetLocalPort(socks)`；Custom 且有 `PreSocksPort`→对应端口）、
`v2rayN/ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs:145`（`BuildAll`：有 pre-socks 时主核心 `IsTunEnabled=false`，主/辅共享用户 SOCKS 端口）、
`v2rayN/ServiceLib/Manager/CoreManager.cs:65`（`LoadCore`：`CoreStop → CoreStart(主) → WaitForProxyPort → CoreStartPreService(前置)`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：状态栏 TUN 开关的布尔期望；`AppSettings.TunModeItem`（EnableTun / EnableLegacyProtect / IPv4Address / Mtu / RouteExcludeAddress）；活动节点；`TunPlanHints`（adapter_name、interface_index、routes）。
- 输出：`TunModeItem.EnableTun` 落盘；`apply_runtime` 重新构建 `RuntimePlan`；TUN 计划携带 `network_policy.tun_enabled=true` 且存在 `tun` 过程节点；预 SOCKS 计划携带 `pre-socks` 过程节点、`主→pre-socks` 边、共享非独占端口。
- 错误：TUN 已开启但缺少 interface index → `E_INVALID_ARGUMENT`（`field=interface_index`）；helper 不可用/授权被拒 → `E_TUN_HELPER_UNAVAILABLE`（net-host，TUN 应用在核启动前回滚）；设置保存失败 → 不改运行状态、不报成功。
- 取消：UAC/授权取消按 `E_TUN_HELPER_UNAVAILABLE` 处理，运行状态保持原样；开关“取消”路径即关闭并经同一 persist→apply。
- 权限：TUN 需 `RequiredPrivilege::Tun`（helper 提权）；预 SOCKS 侧车为本地代理，无提权。
- 持久化：`TunModeItem`（guiNConfig JSON）；重开由 Engine 读回。
- 生效：保存成功后才 apply；UI 实际标签来自运行时快照，不来自开关。

允许修改的模块：`services/privileged_helper/**`、`services/net_host/**`、`crates/application/src/**`（`engine.rs` 仅 TUN/plan 最小改动）、`crates/runtime/**`、`crates/domain/src/runtime_plan.rs`、`crates/bridge_api/src/api/**`、`apps/desktop/lib/features/runtime/**`、`apps/desktop/lib/features/settings/**`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`docs/evidence/UX-PARITY-FIX-13/**`、本卡、compat 台账（仅追加）。
实际改动：`crates/application/src/{engine.rs,tun_plan.rs,lib.rs}`、`crates/application/tests/fix13_tun_presocks_plan.rs`、`apps/desktop/lib/features/runtime/tun_toggle.dart`、`apps/desktop/lib/app/shell/status_bar_view.dart`、`apps/desktop/test/fix13_tun_toggle_test.dart`。未新增 FRB 函数，未改两处 `frb_generated`。

禁止改变的已有行为：`main_shell.dart`、两处 `frb_generated`、`features/profiles/**`、`features/subs/**`、`features/monitor/**`、`features/update/**`、`features/backup/**`、`crates/updater/**`、`crates/subscriptions/**`；不删入口、不降分母、不伪造运行事实；不占用/修改 10808。

测试夹具和原版预期：合成 vless/vless-xray 节点 + 独立内存引擎，所有端口经 `TcpListener::bind` 探测且 ≥11808（本次 11808/11818/11828/11838/11848/11858/11868）；无真实适配器、路由、helper 或网络。原版预期：TUN 开启时计划必须带 `tun` 节点；LegacyProtect 计划主核心先启动、前置后启动，主/辅共享用户 SOCKS 端口。

本次必须通过的命令/真实场景：
- `cargo fmt -p application -- --check`、`cargo clippy -p application --all-targets --locked -- -D warnings`、`cargo test -p application --locked`
- `dart format`（本次改动文件）、`flutter analyze`
- `flutter test test/t13_statusbar_test.dart test/fix13_tun_toggle_test.dart`
- 真实 TUN 场景未运行（安全约束，见 blocked）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-13/`（`README.md`、`observations.json`）。

完成条件：Rust 单测覆盖 TUN 关闭无节点、缺 interface 明确报错、带 hint 附加可回读 `tun` 节点、LegacyProtect 写入主辅节点/端口/启动序、无 LegacyProtect 时无侧车、Custom `PreSocksPort` 入图；Flutter 单测覆盖保存失败不应用不假成功、保存成功再应用、关闭落盘、helper 拒绝显示“失败已回滚”；门禁通过。真实 TUN 未安全实测，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记 FIX-13B）：TUN 适配器启动后的 OS interface index 发现缺失；当前需 `TunPlanHints`/env 显式提供，否则结构化报错。建议在 net-host 启动核后探测适配器索引并二次下发 helper。
- 接口缺口（登记 FIX-13B）：预 SOCKS 侧车执行缺失（第二进程启动、用户端口监听交接、WaitForProxyPort、失败清理与旧会话处理）。本卡只写计划拓扑。
- 接口缺口（登记 FIX-13C）：Custom/其它代理核的候选 exe/参数/env/原始配置/YAML mixin 未逐个补齐；`adapter_for` 仍仅 Xray/SingBox。v2rayN=99 为应用标识，不作核。
- 接口缺口（登记）：helper 的 UAC 授权/取消未在本机隔离授权路径实测；仅以 `E_TUN_HELPER_UNAVAILABLE` 契约与合成 link 覆盖。

本轮实际结果：`build_runtime_plan` 现委托 `build_runtime_plan_with_hints`，TUN 开启时用 `tun_spec_from_settings`+`attach_tun_to_plan` 附加真实 `tun` 节点并 `plan.validate()`；缺少 interface index 返回 `E_INVALID_ARGUMENT`。`pre_socks_of` 结果写入 `pre-socks` 过程节点与 `主→pre-socks` 边，共享端口以非独占记录。新增 `tun_hints_from_env`。Flutter `tun_toggle.dart` 提供 persist→apply 接缝；`status_bar_view.dart` 开关改为保存 `TunModeItem.EnableTun` 后 apply，实际标签来自运行时快照。测试：Rust `fix13_tun_presocks_plan` 6/6，`cargo test -p application` 全绿；Flutter `fix13_tun_toggle` 7/7 与既有 `t13_statusbar` 2/2；`flutter analyze` 无问题；fmt/clippy 干净。
