# RR-07 / RR-10 修复证据

日期：2026-10-04
开始 HEAD：`842520b`（工作树干净）
环境：Windows 11 25H2，Rust 1.98.1，真实内核 `tools/cores/xray/v26.3.27/xray.exe`

## 结论

- **RR-07**：完整 Custom 配置的端点改为从配置内容解析真实 inbound（socks/http/mixed）与 API/telemetry 端口；运行计划、ready 检查（net-host 用计划端口）、`applied_session.proxy_port`、监控 API 端口与 via-proxy 下载 scheme 均使用真实值；解析失败返回结构化错误，不假装 ready。普通节点路径不变。
- **RR-10**：切换预检（adapter/locator/hash/端口/`test_args`/TUN）在停止旧会话之前执行；预检失败保留旧会话运行并结构化报错；预检通过但启动失败时恢复上一良好会话（复用其解析后的 exe），无法恢复则如实 `Stopped` 且清空端点。

## 改动

- 新增 `crates/runtime/src/endpoints.rs`：`parse_custom_endpoints(core, body)` 解析 Xray `inbounds`/`metrics.listen`（回退 `api` dokodemo-door tag）与 sing-box `inbounds`/`experimental.clash_api.external_controller`；返回 `CustomEndpoints{inbounds, api_port}`、`ProxyProtocol{Http,Socks,Mixed}`（`scheme()`）。
- `crates/runtime/src/lib.rs`：导出 `endpoints` 模块与类型。
- `crates/application/src/engine.rs`：
  - `build_runtime_plan` 对 `ConfigType::Custom` 解析真实端点并据此构造 `plan.ports`（含 API 非独占 claim）；解析失败 → `E_INVALID_PLAN` / `error.custom_endpoint_parse_failed`。
  - `apply_facts` 由 `(CoreType,u16,u16)` 升级为 `AppliedFacts{core,state_port,state_port2,scheme}`；Custom 时 API 端口与 scheme 取真实值。
  - `local_proxy_url()` 使用事实 scheme（socks→`socks5`）。
  - `monitor_session()` 读 `AppliedFacts`。
- `services/net_host/src/session.rs`：`PreparedPlan` + `precheck_plan(_with_exe)`、`start_prepared`、`try_restore`、`Inner.{last_plan,last_exe}`、`Session.exe`；`fail_operation` 清死端点；`stop_managed` 清恢复源。
- 测试：`crates/runtime/src/endpoints.rs` 6 用例；`crates/application/tests/t18b_runtime_plan.rs` 5 用例；`crates/application/src/engine.rs` 1 inline 用例；`crates/application/tests/t18b_runtime_e2e.rs` 1 真实 xray 用例；`crates/application/tests/fix13_tun_presocks_plan.rs` 夹具补真实 inbound；`services/net_host/src/session.rs` 5 个 RR-10 用例。

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p runtime -p application -p net_host -- --check` | 0 改变 |
| `cargo clippy -p runtime -p application -p net_host --all-targets --locked -- -D warnings` | 0 警告 |
| `cargo test -p runtime --lib endpoints --locked` | 6 passed |
| `cargo test -p application --lib --locked` | 204 passed |
| `cargo test -p application --tests --locked` | 全绿（含 t18b_runtime_plan 16、fix13 6） |
| `cargo test -p application --test t18b_runtime_e2e --locked rr07 -- --nocapture` | 1 passed（真实 xray，配置端口 11991 提供 SOCKS，计划不含 11990） |
| `cargo test -p net_host --bin net_host --locked` | 49 passed（含 RR-10 5 用例） |
| `run_rr10_real_xray.ps1` | `RR10_REAL_XRAY_OK` |

## RR-10 真实 xray 证据（`rr10-real-xray.log`）

- `apply` good（xray socks）→ `Running`，ports `[11808]`，pid 42876。
- `apply-bad`（`not-a-real-protocol`）→ 预检 `xray run -test` 拒绝：`E_INVALID_PLAN` / `error.config_check_failed`；旧会话仍 `Running`（snapshot `state=running`，ports `[11808]`）。
- `apply` good 于新端口 11809 → 真实切换成功，`Running`。
- `stop` → `STOPPED`。

## 边界

- 未占用/修改 `127.0.0.1:10808`；测试端口均 ≥11808 且先探测。net_host 默认 readiness 端口来自计划，未触碰 10808。
- 未改宿主系统代理 / 注册表 / 路由 / TUN；只停止本脚本启动的 `net_host` PID（37932）。
- 未读/写用户凭据；夹具均为合成配置。
- 未跑全量 workspace 测试、未跑 flutter build、未 git commit。
- 遗留：`AppliedSession` 未携带 scheme（用 engine 内部 `apply_facts` 补，遵守不改 FIX-07 结构）；Custom API 端口为声明值，未做独立就绪探针。

## 上游对照

- `ConfigHandler.GetPreSocksItem`（`ConfigHandler.cs:1555`）：区分普通/TUN LegacyProtect 的 SOCKS sidecar 与 Custom 自定义 `PreSocksPort`；本卡保留该语义，仅让完整 Custom 主端点来自配置本身。
- `CoreConfigContextBuilder`（`CoreConfigContextBuilder.cs:34-183`）：Custom 直通消费；本卡据此确认“完整 Custom 配置”应直通其真实端点而非套用 `GetLocalPort`。
