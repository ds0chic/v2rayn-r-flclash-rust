# R4-25 生产 TUN 和拒绝恢复 — 证据

- 固定 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`（应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`）
- `armed=false`；无内核、无网络、无宿主系统代理/TUN/路由/注册表写入；合成夹具；未占用 `127.0.0.1:10808`，本卡未监听任何端口。
- 平台：Windows 11 25H2 (build 26220) x64；Flutter 3.47.5 / Dart 3.13.4；Rust 1.98.1。
- 真实 auto-route / 全局接管 / IPv6 / 排除 / 崩溃睡眠恢复：**blocked**（需授权隔离 VM；本机不得执行）。

## 结论

状态：**implemented（真实 OS 效果 blocked）**。

本卡只做两件事：核对 TUN 代码路径完整性、补合成合同测试；不写宿主 TUN/路由，不做真实内核会话。

## 代码路径核对（开关 → 授权/取消 → 核心创建接口 → helper 配置 → ready/失败清理）

| 环节 | 位置 | 结论 |
|---|---|---|
| TUN 开关（期望态） | `status_bar_view.dart:36,66-67,376-381`（`_desiredTun` 读 `TunModeItem.EnableTun`，`tun-toggle`）| implemented |
| 提交顺序（先持久化后应用） | `status_bar_view.dart:399-418` → `tun_toggle.dart:26-39` `toggleTunDesired`；保存失败不 `apply` | implemented |
| 授权/取消与失败保旧 | `tun_toggle.dart:45-50` `tunActualLabel`；`status_bar_view.dart:419-429`（denied 时 `tunDenied`，绝不显示成功/已关闭）| implemented |
| 核心创建接口（deferred） | `crates/application/src/tun_plan.rs`（`TUN_DEFERRED_PROCESS_ID`、`tun_deferred_spec_from_settings`、`attach_deferred_tun_to_plan`）；R3-04 | implemented（stub 测试；真实设备创建 blocked） |
| helper 配置（发现后回填索引） | `services/net_host/src/session.rs`（`resolve_deferred_tun`、`apply_tun_spec`）；`tun_lease.rs`、`helper_client.rs`、`services/privileged_helper/src/{backend,windows}.rs` | implemented（stub 测试；真实 helper 会话 blocked） |
| ready / 失败统一清理 | `session.rs`（`rollback`、`release_tun_lease`、`finalize_journal`）；R3-05 | implemented（stub 测试） |

Rust 侧既有测试未在本卡重跑（未改 Rust 生产代码），引用：
`crates/application/src/tun_plan.rs` 单测（禁用返回 None、CIDR 校验、deferred 描述符、attach 幂等）、
`cargo test -p net_host`（R3-04 发现顺序/超时、R3-05 helper 成功后失败统一清理，见 `docs/tasks/R3-04.md:29-39`、`R3-05.md:27-37`）。

## 本卡改动（仅测试）

新增 `apps/desktop/test/r4_25_contract_test.dart`（8 用例）：

1. 开启：先 `persist:true` 再 `apply`；
2. 关闭：先 `persist:false` 再 `apply`；
3. 保存失败：不 `apply`、`ok=false`、`error.settings_save_failed`，不动运行会话；
4. 保存后授权拒绝/取消（`apply` 抛错）：不返回 `ok`（绝不假造成功）；
5. 开关关闭即使运行时 Running 也读 `未启用`；
6. 期望开启但已停止读 `未启用`；
7. helper 拒绝读 `失败已回滚`；
8. 运行中读 `已请求(未验证)`（不假造成功）。

## 命令与结果

| 命令（cwd=`apps/desktop`） | 结果 |
|---|---|
| `flutter analyze` | `No issues found! (ran in 3.7s)`（`flutter_analyze.log`） |
| `flutter test test/r4_25_contract_test.dart` | `All tests passed!`（8/8，`flutter_test_r4_25.log`） |

Rust 未改动，故未运行 cargo 门禁；无原生生产改动，未运行 `flutter build windows`。

## blocked（未实测，登记）与解除条件

1. **真实 auto-route TUN、全局接管、DNS、IPv6、路由排除**：blocked。
   - 需要：授权隔离 VM/主机（可快照回滚、可损坏网络），管理员/UAC 与真实 sing-box TUN 设备创建。
   - 最小步骤：隔离快照基线 → 从可见 UI 开启 TUN → 记录核心先起、设备出现、helper 写入地址/路由、`ready` → 抓取默认路由/连通性变化 → 关闭 TUN 断言适配器/路由/lease/journal 全部回收 → 崩溃/睡眠唤醒后恢复 → 复位快照。
2. **helper 真实授权/拒绝会话**：blocked（同一隔离环境）。
3. **失败保旧的真实语义**：当前 Dart 侧先持久化期望态再 apply；apply 失败时持久化已变更。合成测试只证明“不返回成功、不动已成功会话”；“回滚持久化期望态 / 保持旧运行态”的完整事务语义未在真实 FRB/SQLite 链路验证 → 登记接口缺口。

## 接口缺口（提供方/调用方/输入输出/错误/生效点）

- 缺口 A：真实 lease 未贯穿 FRB。`RuntimeView`（`runtime_bridge.dart:26-86`）无 per-lease TUN 字段，`tun_toggle.dart:41-44` 注释确认“bridge carries no per-lease TUN facts”。
  - 提供方：`crates/bridge_api/src/api/engine.rs` SnapshotDto（只读、脱敏 lease 摘要：helper session 存在性/适配器名/索引/路由数，**不含凭据**）。
  - 调用方：`FrbRuntimeBridge._toView` → `RuntimeView` → `tunActualLabel`。
  - 输入输出：追加只读字段 `tun_lease_present`、可选 `tun_adapter`/`tun_interface_index`/`tun_route_count`；错误：无（只读）。
  - 生效点：每次成功 apply 后的 snapshot；缺失时维持 `已请求(未验证)`，不伪造。
  - 需接口整合者改 FRB 生成；本卡禁改 `frb_generated`/`lib/bridge/api/**`。
