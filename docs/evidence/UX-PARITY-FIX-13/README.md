# UX-PARITY-FIX-13 — TUN 开关主链 + 预 SOCKS plan 接线

状态：`implemented`（Rust 计划图/端口/顺序与 Flutter 开关落盘闭环均已实现并用合成测试验证；真实 TUN 端到端与预 SOCKS 进程执行登记 `blocked`）。

对应审查：`docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 38 行 FIX-13、`runtime-report.md` RT-05/06/07/08。
上游基准：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ConfigHandler.GetPreSocksItem`、`CoreConfigContextBuilder.BuildAll`、`CoreManager.LoadCore`。

## 本轮改动

Rust：
- `crates/application/src/engine.rs`
  - `build_runtime_plan` 改为委托 `build_runtime_plan_with_hints(..., tun_hints)`；env 提示见下。
  - TUN：只有能构造出合法 `TunSpec` 时才 `attach_tun_to_plan`，因此计划不再出现“`tun_enabled=true` 但无 `tun` 节点”的非法组合（原 RT-05 缺陷）。缺少 interface index 时返回结构化 `E_INVALID_ARGUMENT`（`field=interface_index`），不静默丢弃。
  - 预 SOCKS：`pre_socks_of` 结果真正写入 `ProcessGraph`——新增侧车 `ProcessNode(id="pre-socks")`、启动序边（主核心→前置服务）、以及共享用户端口的**非独占**端口记录；`plan.validate()` 通过。
  - `crates/application/src/tun_plan.rs`：新增 `tun_hints_from_env()`（`V2RAYN_R_TUN_ADAPTER` / `V2RAYN_R_TUN_INTERFACE_INDEX`），供隔离/dry-run 显式提供适配器与接口号。
  - `crates/application/src/lib.rs`：导出 `PRE_SOCKS_PROCESS_ID`、`tun_hints_from_env`。

Flutter：
- `apps/desktop/lib/features/runtime/tun_toggle.dart`（新）：纯函数 `toggleTunDesired(persist→apply)` 与 `tunActualLabel(desired, runtime)`，作为可单测接缝。
- `apps/desktop/lib/app/shell/status_bar_view.dart`：TUN 开关不再只改 Dart 内存——先写 `TunModeItem.EnableTun` 落盘，成功后再 `applyActive()`；开关值来自已保存设置，实际标签来自运行时快照（helper 拒绝→`失败已回滚`，未运行→`未启用`，不伪造成功）。

## 上游对照

- `CoreManager.LoadCore`：`CoreStop → CoreStart(主) → WaitForProxyPort → CoreStartPreService(前置)`；本卡计划边 `主 → pre-socks` 一致（启动序 `start_order` 中主核心在前、前置在后，停止序相反）。
- `CoreConfigContextBuilder.BuildAll`：使用预 SOCKS 时主核心不直接处理 TUN、由前置 sing-box 承担；主/辅共享同一用户 SOCKS 端口，故计划中以**非独占**端口记录，真实监听交接留给 FIX-13B。
- `ConfigHandler.GetPreSocksItem`：既有决策函数现被 `build_runtime_plan` 真正消费（此前只做纯测试）。
- v2rayN 应用标识不作核，未删任何核心候选，分母不变。

## 实际运行的命令

| 命令 | 结果 |
|---|---|
| `cargo fmt -p application -- --check` | 无差异 |
| `cargo clippy -p application --all-targets --locked -- -D warnings` | 通过，无告警 |
| `cargo test -p application --test fix13_tun_presocks_plan --locked` | 6 passed |
| `cargo test -p application --locked` | 全部通过（含 157 lib tests） |
| `dart format ...tun_toggle.dart ...status_bar_view.dart ...fix13_tun_toggle_test.dart` | 2 changed |
| `flutter analyze` | No issues found |
| `flutter test test/t13_statusbar_test.dart test/fix13_tun_toggle_test.dart` | 9 passed |

端口：测试全部 ≥ 11808 且先 `TcpListener::bind` 探测；未触碰 10808、宿主系统代理、注册表、路由与 TUN 设备。

## blocked / 接口缺口

- 真实 TUN 端到端（适配器创建、路由下发、UAC 授权/取消）未实测：需既有授权的隔离主机写入，本卡只走合成/dry-run 路径。
- TUN 适配器启动后的 OS interface index 发现未实现；`build_runtime_plan_with_hints` 需要调用方/env 显式提供，否则明确报错。→ FIX-13B。
- 预 SOCKS 侧车的**执行**（第二进程、用户端口监听交接、失败时清理旧会话）未实现，本卡只写计划拓扑/顺序/端口。→ FIX-13B。
- 各 custom core 候选 exe/参数/env/原始配置/YAML mixin 与其它非 Xray/SingBox 适配。→ FIX-13C。
- Flutter 开关仅在纯接缝与状态标签层验证；窗口内点击→FRB 保存→net-host apply 的真机闭环本轮未跑。
