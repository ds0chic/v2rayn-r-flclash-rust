# R3-06 / R3-PROF-05 / R3-PROF-07 修复证据

日期：2026-10-04。开始时 HEAD `593e289`（工作树非干净：存在并行的 R3-01/07/10 工作流未提交改动，见下方“并发边界”）。
固定对照原版 v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
测试端口一律 ≥11808 且先绑定探测；未触碰 `127.0.0.1:10808`；未改宿主系统代理/注册表/路由/TUN；未杀外部进程；未读取用户凭据（夹具为合成数据/RFC 5737 文档地址）。

## 改动摘要

- R3-06 `crates/bridge_api/src/api/monitor.rs`：`poll_loop` 改为 `rt.block_on(poll_loop_async(..))`，循环体用 `tokio::time::sleep` 让同一 current-thread runtime 持续调度 `SingboxTrafficSource` spawn 的 WS 任务；停核时 `source=None` 取消；`build_source` 去掉 `&Runtime` 参数。新增 std-only 合成 WS server + `production_hub_drives_singbox_ws_statistics` 走生产 `monitor_start_polling`。
- R3-PROF-05 `crates/application/src/speedtest.rs`：新增 `udp_ping_via_socks`（SOCKS5 UDP ASSOCIATE）、`socks5_udp_associate`、`socks5_udp_datagram`、`udp_via_socks_supported` 及受控 SOCKS5 UDP server 测试。`crates/bridge_api/src/api/speedtest.rs`：`NetHostTestSession::udp_ping` 改经 `session.port`；`speedtest_supported().udp` 改为能力门控。
- R3-PROF-07 `crates/application/src/groups.rs`：`resolve_sub_children` 增加 `p.is_valid()`；`apps/desktop/lib/features/profiles/group_editor_dialog.dart`：新增 `_isProfileValid` 并在订阅匹配中应用。
- 测试：`crates/application/src/groups.rs`、`crates/application/src/speedtest.rs`、`crates/bridge_api/src/api/monitor.rs`、`crates/bridge_api/src/api/speedtest.rs`、`apps/desktop/test/reprof10_group_preview_test.dart`。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test -p application --lib --locked` | exit 0，**224 passed / 0 failed**（含 `groups::tests::sub_children_drop_invalid_leaves`、`speedtest::re_prof_08_udp_tests::udp_ping_via_socks_routes_through_the_session_port`、`udp_via_socks_datagram_encodes_domain_targets`） |
| `cargo test -p bridge_api --lib --locked` | exit 0，**59 passed / 0 failed**（含 `api::monitor::tests::production_hub_drives_singbox_ws_statistics`、`api::speedtest::tests::udp_support_is_gated_by_the_socks_associate_path`、`api::speedtest::tests::udp_ping_routes_through_the_session_port`） |
| `cargo test -p core_adapters --test stats_singbox --locked` | exit 0，**11 passed / 0 failed**（含 `accumulates_streaming_deltas`、`reconnects_with_backoff_and_keeps_accumulating`） |
| `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` | exit 0，无告警 |
| `cargo fmt -p application -p bridge_api -- --check` | exit 0，无差异 |
| `dart format lib/.../group_editor_dialog.dart test/reprof10_group_preview_test.dart` | 2 files（1 changed），已格式化 |
| `flutter test test/reprof10_group_preview_test.dart` | exit 0，**9 passed / 0 failed**（含 `subscription children drop invalid leaves (RE-PROF-07)`） |
| `dart analyze lib/.../group_editor_dialog.dart test/reprof10_group_preview_test.dart` | `No issues found!` |

未运行：全 workspace 门禁、真实 sing-box/Xray 内核与真实 WS/API、真实 UDP 经内核、真实 Windows GUI/重开、`flutter build windows`（按约束不跑）、系统代理/注册表/TUN/路由写入。

## 上游对照结论

- R3-06：冻结 `StatisticsSingboxService.cs` 持续驱动 Run/WS 接收；修复后 `SingboxTrafficSource` 的 spawn 任务在持续 runtime 上获得调度，生产入口合成测试非零；`core_adapters` 既有的断线退避重连测试佐证恢复路径。真实内核未跑。
- R3-PROF-05：对照 `ServiceLib.UdpTest/UdpTestService.cs:91-153` 与 `Socks5UdpChannel`，实现经节点测试核本地端口 `UDP ASSOCIATE` 的探针；上游的响应内容校验（`VerifyAndExtractUdpResponse`）未迁移，已登记。
- R3-PROF-07：对照 `GroupProfileManager.cs:118-122` 的 `p.IsValid()`，Rust 订阅子项过滤新增 `is_valid`；Dart 预览同步。显式 `ChildItems` 路径保持上游语义不过滤。

## 并发边界（重要）

本回合开始时工作树并非 clean：`git status` 显示并行的 R3-01/07/10 工作流已修改 `routing_controller.dart`、`platform_controller.dart`、`proxy_settings_view.dart`、`update_service.rs`、`t16.rs`、`compat/*.yaml`，并新建 `docs/tasks/R3-01.md`、`R3-10.md`、`R3-PROXY-UI.md`、`docs/evidence/recheck-fixes/R3-01-07-10/`。其中 `crates/bridge_api/src/api/speedtest.rs` 被并行编辑（R3-PROF-06 原始 Custom 导出），期间一度处于不可编译的中间态；本卡仅在该文件 `udp_ping` / `speedtest_supported` 区域做精确字符串替换，未覆盖其改动。`cargo fmt -p application -p bridge_api` 可能对并行 in-progress 文件做了重排（无逻辑改动）。未对 `compat/` 追加（避免并行写冲突），登记给根代理合并。

## 未完成 / 接口缺口

- R3-PROF-05：未做响应内容校验；支持位为构建级门控而非逐节点内核能力（缺会话能力查询接口）。
- R3-PROF-07：Dart SS 仅校验密码非空，未复制 Rust `SS_METHODS_SINGBOX` 白名单。
- R3-06：未实测真实 sing-box 内核与真实 WS；`monitor_start_polling` 无 engine 会话时会清空 hub（合成测试按先启动再 configure 覆盖）。

## 下一步前置

- 隔离环境用真实 sing-box 内核 + 合成 WS 复验持续非零与断线恢复。
- 真实 net-host 测试核 + 受控 SOCKS5 UDP server 复验节点 UDP 路径。
- 与并行 R3-01/07/10 工作流合并后重跑 `application`/`bridge_api` 单 crate 门禁。
- 根代理按需把本卡证据追加进 `compat/` 台账。
