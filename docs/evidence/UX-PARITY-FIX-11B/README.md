# FIX-11B 证据 — Clash 面板：选择/关闭连接/模式/延迟 URL/重试

状态：`implemented`。Rust 侧（core_adapters / application / bridge_api）已实现并单测通过；
Flutter 侧（视图/控制器/桥接口）已实现，但 **新增 FRB 函数尚未重生成**，故 `flutter analyze`/`flutter test`/`flutter build` 本轮**未运行**，见「接口缺口」。

唯一用户流程：Clash 面板操作 选择代理 → 关闭单条/全部连接 → 切模式 → 延迟 URL/重试，与原版一致。

## 上游对照（冻结 `7d6a967`）

- `ServiceLib/Manager/ClashApiManager.cs`
  - `GetProxies`：每次同时请求 `/proxies` 与 `/providers/proxies`；两者都为 null 时 `Task.Delay(2000)` 并重试，最多 3 次 → 本仓 `ClashApiService::proxies_with_retry`（`PROXY_RETRY_ATTEMPTS=3`、`PROXY_RETRY_DELAY=2s`）。
  - `TestProxyDelay`：`GET /proxies/{name}/delay?timeout=10000&url=<encoded SpeedPingTestUrl>`，`delay` 非 JSON number 时为 `-1`。
  - `TestProviderProxyDelay`：`GET /providers/proxies/{provider}/{name}/healthcheck?timeout=10000&url=<encoded>`。
  - `SetActiveProxy`：`PUT /proxies/{group}`（name 在 header）。
  - `UpdateClashMode`→`UpdateConfig`：`PATCH /configs`，**mode 作为 header**（不是 body）。
  - `GetClashModes`：读 `mode-list`（否则 `modes`）；`GetClashMode`：读 `mode`。
  - `CloseConnection(id)`：`DELETE /connections/{id}`；`CloseConnection("")`（全部）→ `DELETE /connections/`。
- `ServiceLib/Global.cs:177`：`SpeedPingTestUrls.First()` = `https://www.google.com/generate_204`，即 `SpeedPingTestUrl` 配置默认值。
- `ServiceLib/ViewModels/ClashProxiesViewModel.cs`：`RuleModeSelected`（Rule/Global/Direct）→ `SetRuleMode`；`ClashModes` 来自 `GetClashModes`；`TestGroupProxiesDelay` 并行度 4；`SetActiveProxy` 仅当 group `type=="Selector"`。
- `ServiceLib/ViewModels/ClashConnectionsViewModel.cs`：`ConnectionCloseCmd`（选中，选中为空则禁用）、`ConnectionCloseAllCmd`；关闭后重新拉取。

## 本轮改动

- `crates/core_adapters/src/clash_api.rs`
  - 新增 `send_expect_success`（带自定义 header 的通用请求）；`update_mode`（`PATCH /configs`，`mode` header）；`get_modes`（`mode-list`→`modes`）；`get_mode`。
  - `close_all_connections` 改为 `DELETE /connections/`（对齐上游空 id 语义）。
  - 模块注释更新（`SetActiveProxy`/`UpdateClashMode` 归此层）。
- `crates/application/src/monitor.rs`
  - `DELAY_TEST_URL` 由 `gstatic` 改为上游默认 `https://www.google.com/generate_204`；新增 `PROXY_RETRY_ATTEMPTS/PROXY_RETRY_DELAY`。
  - `ClashApiService` 新增 `delay_url` 字段与 `set_delay_url`/`delay_url`，`proxy_delay`/`group_delay` 改用该 URL；新增 `proxies_with_retry`、`mode`/`modes`/`update_mode`。
- `crates/bridge_api/src/api/monitor.rs`
  - hub 新增 `delay_test_url`；新增 `monitor_set_delay_url`（frb）。
  - 新增 DTO `ClashModeDto`；新增 `clash_mode_state`、`update_clash_mode`（frb，待重生成）。
  - `clash_proxies` 改用 `proxies_with_retry`；`build_clash` 应用配置的延迟 URL。
- `apps/desktop/lib/features/monitor/monitor_bridge.dart`：接口与 `FrbMonitorBridge` 新增 `setDelayUrl`/`clashModeState`/`updateClashMode`。
- `apps/desktop/lib/features/monitor/monitor_controller.dart`：`MonitorState` 新增 `clashMode`/`clashModes`/`modeMessage`；新增 `refreshClashMode`/`setClashMode`/`setDelayUrl`。
- `apps/desktop/lib/features/monitor/proxies_view.dart`：新增模式下拉（`proxies-mode`，Rule/Global/Direct；核心提供 `mode-list` 时以其为准），init 拉取模式。
- `apps/desktop/lib/features/monitor/connections_view.dart`：关闭选中/全部失败时以 SnackBar 明确报错（不假成功）。
- `apps/desktop/test/support/fake_monitor_bridge.dart`：实现新接口；`apps/desktop/test/fix11b_clash_panel_test.dart` 新增（待重生成后可跑）。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo check -p application --locked` | exit 0 |
| `cargo check -p bridge_api --locked` | exit 0 |
| `cargo fmt -p core_adapters -p application -p bridge_api -- --check` | exit 0 |
| `cargo clippy -p application -p bridge_api -p core_adapters --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test -p core_adapters --test clash_api --locked` | 22 passed / 0 failed |
| `cargo test -p application --lib --locked monitor` | 18 passed / 0 failed |
| Flutter analyze/test/build | 未运行（见接口缺口，FRB 未重生成） |

新增 Rust 用例：`get_mode_reads_config`、`get_modes_prefers_mode_list`、`update_mode_patches_config_header`、`retry_returns_first_non_empty_after_failures`、`retry_gives_up_after_attempts`，以及 application 的 `clash_default_delay_url_matches_upstream_speed_ping_default`、`clash_delay_url_override_ignores_blank`、`proxy_retry_budget_matches_upstream`。

Mock 端点断言（core_adapters，端口 ≥11808，`127.0.0.1`，`no_proxy`）：
- `PATCH /configs` 且 header `mode: global`；
- `GET /proxies/{name}/delay?timeout=10000&url=<encoded>`；provider 走 `healthcheck`；
- `DELETE /connections/`（全部）/`/connections/{id}`（单条）；
- 重试：首次整轮 500 后第 2 轮成功；持续 500 则放弃并错误分类 `HttpError::Status(500)`。

## 未完成 / 接口缺口

- **接口缺口（阻塞，待根代理）**：新增 FRB 函数 `clash_mode_state`/`update_clash_mode`/`monitor_set_delay_url` 与 DTO `ClashModeDto` 未出现在 `apps/desktop/lib/bridge/api/monitor.dart` 与 `crates/bridge_api/src/frb_generated.rs`。重生成前 `flutter analyze`/`flutter test`/`flutter build windows` 无法编译。重生成后需跑：`flutter analyze`、`flutter test test/fix11b_clash_panel_test.dart test/t15a_proxies_test.dart test/t15a_connections_test.dart`。
- 未做真实 mihomo/sing-box 内核 + Flutter 同屏实机验收；仅 mock 端点 + Rust 单测证明请求路径/参数/重试语义。
- `setDelayUrl` 已暴露但未接入 `features/settings`（本卡禁改）；当前 hub 默认已等于上游 `SpeedPingTestUrls.First()`。
- 未改 FIX-11 已提交的统计轮询/`ServerStatItem` 语义；未占用/修改 10808；未变更宿主系统代理/注册表/路由/TUN。
