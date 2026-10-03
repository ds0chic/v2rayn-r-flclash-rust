# FIX-11B — Clash 面板：选择/关闭连接/模式/延迟 URL/重试

状态：`implemented`（Rust core_adapters/application/bridge_api 已实现并单测通过；Flutter 视图/控制器/桥已实现。新增 FRB 函数未重生成，故 Flutter 门禁与测试本轮未运行；未做真实内核实机同屏对照，不写 `verified`）。

任务 ID：FIX-11B

本次唯一用户流程：Clash 面板操作 选择代理（selector 切换）→ 关闭单条/全部连接 → 切换模式（Rule/Global/Direct）→ 延迟 URL/重试，均经真实 API 调用生效并可回读，失败明确报错，与原版一致。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；拆卡自 FIX-11（`docs/tasks/FIX-11.md` 第 45 行），关联 `repair-queue.md` 第 36 行 PR-29/RT-18、`runtime-report.md` RT-18。FIX-11 已提交统计轮询/`ServerStatItem` 语义，本卡不改。证据见 `docs/evidence/UX-PARITY-FIX-11B/README.md`。

对应 feature / field / action / layout ID：`F-MONITOR-004`（Clash proxies）、`F-MONITOR-005`（Clash connections）、`LAY-CLASHPROXY-001`、`LAY-CLASHCN-001`；上游 `ClashProxiesViewModel`/`ClashConnectionsViewModel`/`ClashApiManager`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/Manager/ClashApiManager.cs`（`GetProxies` 3×/2s 重试、`TestProxyDelay` `timeout=10000&url=SpeedPingTestUrl`、`TestProviderProxyDelay` healthcheck、`SetActiveProxy`、`UpdateClashMode`→`PATCH /configs` header、`GetClashModes`/`GetClashMode`、`CloseConnection`）、`ServiceLib/Global.cs:177`（`SpeedPingTestUrls.First()=https://www.google.com/generate_204`）、`ServiceLib/ViewModels/ClashProxiesViewModel.cs`、`ClashConnectionsViewModel.cs`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：hub 中 applied 会话的 `state_port2`/`secret`（Clash 控制器端口）；设置的 `SpeedPingTestUrl`（延迟 URL）；UI 选择的 group/name/mode/connection id。
- 输出：`ClashProxiesDto`/`ClashModeDto`/`DelayResultDto`/`GroupDelayDto`/`ClashConnectionsDto`/`MonitorActionResult`；模式经 `PATCH /configs` 生效，选择经 `PUT /proxies/{group}` 生效，关闭经 `DELETE`。
- 错误：无 Clash 支持时 `supported=false` 并给出「当前内核不提供 Clash API」；构建失败/请求失败返回 `ErrorDto(code=E_CLASH, retryable=true)`；延迟失败为 `-1`（上游约定），UI 显示 `--`。
- 取消：无独立取消入口；关闭/选择即为即时动作。
- 权限：仅 loopback `127.0.0.1`，`no_proxy`，绝不接触用户 10808；不写系统代理/注册表/TUN。
- 持久化：无（模式由内核持有；面板状态为瞬时）。不改 FIX-11 的 `ServerStatItem` 落库。
- 生效：模式/选择变更后重新拉取 `clash_proxies`/`clash_mode_state` 回读；连接关闭后重新拉取连接表。

允许修改的模块：`apps/desktop/lib/features/monitor/**`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`crates/application/src/monitor.rs`、`crates/bridge_api/src/api/monitor.rs`（新增函数，未改 `frb_generated`）、`crates/core_adapters/src/clash_api.rs`、`crates/core_adapters/tests/clash_api.rs`、`docs/evidence/UX-PARITY-FIX-11B/**`、本卡、`compat` 台账（仅追加）。

禁止改变的已有行为：`main_shell.dart`、两处 `frb_generated`（含 `crates/bridge_api/src/frb_generated.rs`）、`features/settings/**`、`features/profiles/**`、`features/subs/**`、`features/runtime/**`、`features/update/**`、`features/backup/**`、`features/routing/**`、`crates/application/src/engine.rs`、`crates/updater/**`；不删入口/不降分母；不伪造成功；不占用/修改 10808。

测试夹具和原版预期：loopback mock Clash API（端口 ≥11808，`127.0.0.1`，`no_proxy`）；合成 proxies/providers/connections 夹具。原版预期：重试 3 次 2s；延迟 `timeout=10000&url=SpeedPingTestUrl`；模式 patch header；关闭全部 `DELETE /connections/`。

本次必须通过的命令/真实场景：
- `cargo check -p application --locked` / `-p bridge_api --locked`
- `cargo fmt -p core_adapters -p application -p bridge_api -- --check`
- `cargo clippy -p application -p bridge_api -p core_adapters --all-targets --locked -- -D warnings`
- `cargo test -p core_adapters --test clash_api --locked`
- `cargo test -p application --lib --locked monitor`
- 重生成后：`flutter analyze`、`flutter test test/fix11b_clash_panel_test.dart test/t15a_proxies_test.dart test/t15a_connections_test.dart`

证据文件位置：`docs/evidence/UX-PARITY-FIX-11B/README.md`。

完成条件：选择/关闭单条/全部/模式切换经真实 API 生效并可回读、失败明确报错；延迟 URL/重试按上游；mock 端点验证请求路径与参数；无运行会话时入口禁用或明确提示。Rust 门禁与测试通过；Flutter 门禁待 FRB 重生成。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（阻塞）：新增 FRB 函数 `clash_mode_state`/`update_clash_mode`/`monitor_set_delay_url` 与 DTO `ClashModeDto` 需根代理重生成（不改生成文件）。重生成前 Flutter 无法编译。
- 接口缺口（登记）：`setDelayUrl` 已暴露但未接入 `features/settings`（本卡禁改）；hub 默认已等于上游 `SpeedPingTestUrls.First()`。
- 接口缺口（登记）：`SetActiveProxy` 上游仅允许 `type=="Selector"`；本仓 `select` 未在 Rust 层强制该判断（UI 选择性展示）。若需严格一致，建议后续在 Rust/UI 层加 group type 校验并明确报错。

本轮实际结果：core_adapters 新增 `update_mode`/`get_modes`/`get_mode` 与 `send_expect_success`，`close_all_connections` 改 `DELETE /connections/`（22/22 通过）；application 的 `DELAY_TEST_URL` 改为上游默认、新增重试常量与 `delay_url`/`proxies_with_retry`/`mode`/`modes`/`update_mode`（18/18 通过）；bridge_api 新增 hub `delay_test_url`、`monitor_set_delay_url`、`ClashModeDto`、`clash_mode_state`、`update_clash_mode`，`clash_proxies` 用重试；Flutter 新增模式下拉、失败 SnackBar、桥/控制器接口与假实现。clippy/fmt/测试全过。Flutter 门禁因 FRB 未重生成未运行，保持 `implemented`。
