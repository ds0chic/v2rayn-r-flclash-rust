# 审计修复记录：Phase 1 全量 + Phase 2 部分（2026-10-05）

基线：`docs/evidence/tun-settings-audit-2026-10-05/README.md`。本记录只覆盖已实施并已跑门禁的修复。

## Phase 1：保存与结果合同（6 条正确预期断言全绿）

| 编号 | 修复 | 位置 |
|---|---|---|
| AUD-ROOT-01 | 整树保存成功后重读权威设置，同步 `groupRevisions`；下次组保存不再 stale | `apps/desktop/lib/features/settings/settings_controller.dart`（`_refreshAuthoritativeRevisions`） |
| AUD-ROOT-02 / TUN-A03 | `applyActive` 返回真实应用结果；`toggleTunDesired.apply` 返回 `Future<bool>`，void 完成不再算 applied；状态栏按结果如实提示 `tunDenied` | `runtime_controller.dart`、`tun_toggle.dart`、`status_bar_view.dart` |
| AUD-ROOT-03 | 加载异常/空 JSON 记为 `loadFailed`，不再生成可编辑默认文档；失败态禁止保存 | `settings_controller.dart`（`load`、`loadFailed`、保存守卫） |
| AUD-ROOT-04 | `save_settings_group` 补 `guard_storage`：存储不可用时组保存不落盘、不推进修订 | `crates/application/src/engine.rs` |
| AUD-ROOT-05 / AUD-DESK-03 | `saveAndApply` 增加平台阶段（系统代理/PAC），失败保持“已保存、平台未应用”并回传结果；经静态 hook 驱动，避免 settings↔platform provider 环 | `settings_controller.dart`、`platform_controller.dart`（`applySavedMode`） |
| AUD-DESK-01 | 自启写入失败后再次保存会重试（`_autostartApplied` 只记录 OS 已确认值） | `settings_controller.dart` |
| AUD-DESK-02 | 草稿记录创建时 revision，保存按草稿 revision 提交；独立设置窗口经 `expectedRevision` 传递 | `settings_controller.dart`、`settings_actions.dart` |
| AUD-DESK-12 | 更新检查：`null`=全部、空数组=全不选，不再把空数组扩成全部目标 | `crates/bridge_api/src/api/t16.rs`（`Option<Vec<String>>`，FRB 已重生成） |

回归测试：`apps/desktop/test/repair/audit_tun_settings_contract_test.dart`（6/6 通过，原审计复现断言未放宽）。

## Phase 2：TUN 可运行合同（部分）

| 编号 | 修复 | 位置 |
|---|---|---|
| TUN-A01 | helper 空闲回收不再用请求超时：新增 `idle_timeout`（默认 24h），空闲会话不再 5s 后清理租约/停止提权内核；仅管道断开/Shutdown/超长空闲结束 | `services/privileged_helper/src/{server.rs,main.rs}` + 回归测试 `idle_session_survives_past_the_request_timeout` |
| TUN-A02 | LegacyProtect 拓扑只保留一个 TUN provider：主核配置不再生成 tun 入站（sidecar sing-box 独占）；主核为 sing-box 且无 sidecar 时保留自身 tun | `crates/application/src/engine.rs` + 断言并入 `pre_socks_legacy_sidecar_uses_base_port_and_real_socks_config` |
| TUN-A03 | 见 Phase 1（toggle 不再假成功）；真实租约事实已贯穿 FRB：`SnapshotDto.runtime_tun`（adapter/if/route_count/dry_run）→ Dart `RuntimeView.tun` → 标签显示 `已启用 (v2rayn-tun if=N)`，无租约才显示 `已请求(未验证)` | `crates/bridge_api/src/api/{contract.rs,engine.rs}`、`apps/desktop/lib/features/runtime/{runtime_bridge.dart,tun_toggle.dart}` |
| 附加 | TUN 适配器名统一 `v2rayn-tun`（codegen 与 net-host 发现一致）；net-host 提权 sidecar 启动（helper `RunElevatedCore` + 受控目录 staging）；helper `MIB_UNICASTIPADDRESS_ROW` 镜像字段序/对齐修正（`CreateUnicastIpAddressEntry` 87 修复）；helper `HelperServerConfig.allowed_run_roots` 由 `--run-roots` 正确填充 | `codegen.rs`、`services/net_host/src/{session.rs,helper_client.rs}`、`services/privileged_helper/src/{main.rs,windows.rs}` |

真机观察（受控，未改宿主路由）：提权 sidecar 启动、`v2rayn-tun` 适配器创建、helper 地址写入成功（租约 `dry_run=false`，if=84）。

## 已跑门禁

- Rust：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked` 全 0。
- Flutter：`dart format`、`flutter analyze` No issues；审计回归 6/6；受设置/TUN 影响的 82 个测试文件全通过（2 例已知 flutter_tester 偶发崩溃重跑通过）。

## 未完成（按审计顺序）

1. TUN-A04：主核自身带 TUN 时的提权路径（当前只接 sidecar）；A05：提权 sidecar 的就绪/退出检查（目前依赖 deferred 适配器发现超时兜底）。
2. TUN-A06：生产上下文的全局 IPv6 探测与内核进程路径保护输入（`codegen.rs`、`xray/inbound.rs` 等始终 false/空）。
3. Phase 3 设置消费者：Happy Eyeballs、Fragment 合法取值、TLS 根来源、HWA/日志、Mihomo merge、外部路由模板、本地 SRS 组装。
4. Phase 4 UI 状态统一；Phase 5 受控平台验收（真实 TUN 路由接管仍在授权环境执行）。
