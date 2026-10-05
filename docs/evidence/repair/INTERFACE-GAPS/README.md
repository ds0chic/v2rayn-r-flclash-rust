# INTERFACE-GAPS 接口整合子代理证据（2026-10-05）

起点 HEAD `8651e19`（工作树含并行卡改动，本子代理只动接口整合范围内的 Rust 文件）。
本回合只新增函数/DTO/字段，不改现有签名语义，未改两处 `frb_generated`、未改 `apps/desktop/**`、
`crates/updater/**`、`work/**`、`outputs/**`；未占用/修改 10808；单测只用合成数据与本地 loopback。

## 缺口实现状态

| 缺口 | 卡 | 状态 | 说明 |
|---|---|---|---|
| 1 `commit_import_text` | R4-16 | implemented | 无组/有组统一单事务落库入口 |
| 2 `GetOperation` 结构化状态 | R4-04 | implemented | IPC→应用→bridge 只读查询 |
| 3 `applied_inbound_protocol` | R4-24 | implemented | 新只读 DTO `AppliedInboundDto` |
| 4 资源任务桥函数 | R4-34 | implemented | `resource_auto_update_now` / `resource_update_status` |
| 5 app update spec 参数化 + 重启/回滚 | R4-29 | implemented | 参数化入口 + 重启/回滚 DTO |

## 改动文件（本回合）

- `crates/ipc_contract/src/lib.rs`：`OperationStatus::is_terminal()` + 单测。
- `services/net_host/src/session.rs`：`HostState::operation_status()` 只读查询。
- `services/net_host/src/server.rs`：`GetOperation` 分支改用该只读查询（语义不变）。
- `crates/application/src/runtime_client.rs`：`OperationStatusView` + `RuntimeClient::operation_status` 默认实现（未支持时结构化 not_found）。
- `crates/application/src/net_host_client.rs`：`NetHostClient::operation_status`（发 `GetOperation`）。
- `crates/application/src/engine.rs`：
  - `AppEngine::operation_status`、`applied_inbound_protocol`、`applied_inbound_port`；
  - `AppEngine::auto_update_now`、`last_resource_report`、`last_resource_report` 字段；
  - `run_resource_pass` 抽 `run_resource_pass_inner(force)`，记录最近一次报告。
- `crates/bridge_api/src/api/contract.rs`：新 DTO（见下）。
- `crates/bridge_api/src/api/engine.rs`：`get_operation`、`applied_inbound`、`resource_auto_update_now`、
  `resource_update_status`、`profile_from_dto`（pub(crate)）+ 映射/单测。
- `crates/bridge_api/src/api/subs.rs`：`commit_import_text` + 单测。
- `crates/bridge_api/src/api/t16.rs`：`t16_apply_app_update_spec_with_flags`、
  `t16_rollback_app_upgrade`、`t16_app_restart_command` + 单测；保留无参 `t16_apply_app_update_spec` 包装（见约束）。

## 约束说明（R4-29 签名）

`frb_generated.rs` 当前内联调用无参 `t16_apply_app_update_spec()`。为满足“不改两处 frb_generated”且工作区可编译，
本回合保留无参函数作为包装器，新增参数化入口 `t16_apply_app_update_spec_with_flags(prerelease, via_proxy)`。
根代理统一重生成后，Dart 侧改用参数化入口，包装器可删除。

## FRB 重生成需求清单（新增 pub 项）

新增函数（供根代理 `flutter_rust_bridge_codegen generate` 绑定）：

- `crates/bridge_api/src/api/engine.rs`
  - `get_operation(operation_id: String) -> OperationStatusDto`
  - `applied_inbound() -> AppliedInboundDto` `#[frb(sync)]`
  - `resource_auto_update_now() -> ResourceUpdateReportDto`
  - `resource_update_status() -> ResourceUpdateReportDto` `#[frb(sync)]`
- `crates/bridge_api/src/api/subs.rs`
  - `commit_import_text(profiles: Vec<ProfileDto>, subid: Option<String>) -> ImportResult` `#[frb(sync)]`
- `crates/bridge_api/src/api/t16.rs`
  - `t16_apply_app_update_spec_with_flags(prerelease: bool, via_proxy: bool) -> ExternalSpecDto`
  - `t16_rollback_app_upgrade() -> AppRollbackResultDto` `#[frb(sync)]`
  - `t16_app_restart_command() -> AppRestartCommandDto` `#[frb(sync)]`

新增 DTO（`crates/bridge_api/src/api/contract.rs`）：

- `OperationStatusDto { found, operation_id, job_id, state, cancel, error }`
- `AppliedInboundDto { protocol: Option<String>, port: Option<u16> }`
- `ResourceFailureDto { url, code, detail }`
- `ResourceUpdateReportDto { ok, due, attempted, downloaded, failed, error }`
- `AppRestartCommandDto { program, working_dir }`
- `AppRollbackResultDto { ok, restored, error }`

未改动的既有 pub 项：`SnapshotDto`、`ExternalSpecDto` 及其原有构造函数签名均未变（避免破坏未重生成的绑定）。

## 单测（合成数据/本地 loopback，无外网）

- `ipc_contract`: `operation_status_terminal_is_read_only`.
- `application::engine`:
  - `applied_inbound_facts_report_protocol_and_port_only_when_running`
  - `auto_update_now_forces_pass_and_records_status`（127.0.0.1 本地 tiny_http，OS 分配端口）
- `bridge_api::api::engine`:
  - `get_operation_unknown_reports_structured_not_found`
  - `operation_status_dto_maps_view`
  - `applied_inbound_is_empty_before_apply`
  - `resource_update_status_empty_and_auto_update_needs_store`
- `bridge_api::api::subs`: `commit_import_text_no_group_is_single_transaction`
- `bridge_api::api::t16`:
  - `app_update_spec_with_flags_via_proxy_without_port_is_structured`
  - `app_restart_command_is_exposed`
  - `rollback_without_previous_is_structured_not_found`（临时目录，不碰真实安装根）

## 未完成/接口缺口

- 未运行 Flutter/`apps/desktop` 任何命令（Dart 由根代理/各卡后续消费）。
- 未做真实 GUI/真实 net-host 进程的跨进程 `GetOperation` 端到端（Rust 单测覆盖映射与 not_found 分支；正路需真实命名管道）。
- 未实测真实发行源/真实安装根的回滚与重启（无授权隔离环境，R4-29 仍 blocked）。
- 未改 `frb_generated`，Dart 尚无上述绑定的编译期签名。
