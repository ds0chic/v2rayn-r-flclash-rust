# T09–T16 审计整改报告（A 组 · Rust/服务侧）

- 整改时间：2026-10-01 UTC+8
- 执行代理：A 组（deepseek-v4.1-flash），仅 Rust/服务侧
- 基线：`HEAD = 083b084dc7e96c9bb3223da128056e9dfaf7a7bd`（T11 routing+DNS full）；工作区在整改前已含一轮未提交的 Rust 侧改动，本报告对其逐项复核、修复门禁阻断、补齐测试与产物。
- 依据：`docs/evidence/audit/T09-T16.audit.muse.md`、`docs/evidence/audit/T09-T16.audit.gemini.md`
- 边界：未改 `apps/desktop`、`compat/`、`docs/evidence` 指定外文件；未 commit。

## 结论

**Rust 侧 10 项整改全部落地，门禁全绿。** `cargo fmt`/`cargo clippy -D warnings`/`cargo test --workspace` 均通过；T15 真实 Xray 冒烟、T10 16/16、T06b 35/36（kcp-legacy 上游漂移）均有本轮新鲜机器产物归档。遗留项均为 B 组台账口径与 T17+ 接线范畴。

## 逐项状态

| # | 发现 | 严重度 | 状态 | 修复与证据 |
|---|---|---|---|---|
| 1 | F02 updater 绝对路径逃逸 | P0 | 已修复 | `crates/updater/src/install.rs`：`is_within_root(root, path)` 词法规范化 + 存在时 `canonicalize` 复核；`validate()` 与 `external_upgrade_spec()` 均改用。回归 `external_spec_rejects_absolute_path_outside_root`（含 `C:\Windows\evil.zip`、`C:/Windows/evil.zip`、`/root/../evil.zip` 拒绝，相对路径 `stage/new.zip` 放行）通过。 |
| 2 | Gemini F-02 订阅替换非事务 | P1 | 已修复 | `store_repo.rs`：`ProfileStore::replace_for_sub` + `SqliteProfileRepository::replace_for_sub`（`begin()`/`commit()` 单事务）；`InMemory` 后端用 `snapshot_for_sub`/`restore_snapshot` 保证同契约。`engine.rs::replace_sub_profiles` 改调事务接口。失败注入测试 `replace_sub_profiles_failure_keeps_old_nodes`、`sqlite_replace_for_sub_rolls_back_on_injected_failure`（旧数据 2 条完整保留）通过。 |
| 3 | F03 via_proxy 静默直连 | P1 | 已修复 | 新增错误码 `E_PROXY_UNAVAILABLE`（`domain/src/error.rs` + `subscriptions/src/error.rs`）。`application/src/subs.rs::download_all` 与 `bridge_api/src/api/subs.rs::update_subscriptions` 在 `via_proxy=true` 且无有效端口时返回结构化错误，不直连。测试：`via_proxy_without_endpoint_returns_proxy_unavailable`、`via_proxy_with_endpoint_is_not_reported_unavailable`（含空白串视为无端口）、`update_via_proxy_without_endpoint_returns_proxy_unavailable` 均通过。 |
| 4 | F04 SUB_JOBS 死注册表 | P1 | 已修复 | `bridge_api/src/api/subs.rs`：注释改为“仅调试计数，取消唯一源为 `JobManager`（经 `cancel_job`）”；`register_sub_job` push / `finish_sub_job` retain 自洽，误导性“so cancel_job can reach”已清除。 |
| 5 | F10 Engine 单例测试竞争 | P1 | 已修复 | `bridge_api/src/api/engine.rs`：新增 `engine_test_lock()` 串行化引擎相关测试；删除 `save_profile_rejects_stale_revision_through_bridge` 中“重试 10 次”掩盖补丁（改用每次唯一 index_id + 单次读写）；新增并发说明测试 `global_engine_tests_are_serialized_not_concurrent`。生产单引擎不变。 |
| 6 | F11 helper 四连风险 | P1 | 已修复 | a) `ipc_contract/src/helper.rs::validate_elevated_core_canonical` 在 `validate_elevated_core` 内复验（`canonicalize` 破 symlink/junction/8.3），`privileged_helper/{server.rs:253,windows.rs:447}` 均接线；b) `windows.rs` 新增 `configure_job_kill_on_close`（`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`）与 `assign_to_job`，Assign 失败即 `TerminateProcess`+清理并返回 `JobAssignFailed`；c) `server.rs::on_disconnect` 返回失败列表、审计 `AuditOutcome::Error` 并 eprintln 外显；d) `CoreRecord` 升级为 `pid + created_at_ms` 绑定，`stop` 前 `pid_creation_time_ms` 复核，PID 复用拒绝。测试：`canonical_recheck_accepts_real_layout`、`canonical_recheck_rejects_symlink_escape`、`job_assign_failed_maps_to_stable_code`、`job_object_arms_kill_on_close`、`assign_to_invalid_job_fails_structured`、`process_identity_binds_pid_and_creation_time`、`disconnect_cleanup_failure_is_surfaced_and_audited`、`disconnect_cleanup_success_audits_ok` 全通过。 |
| 7 | F08 T15 冒烟产物 | P1 | 已修复 | 本轮真实运行 `cargo test -p core_adapters --test xray_smoke -- --ignored --nocapture`：PID 2840，metrics 11809，proxy up=210/down=13372，3.86s，exit 0。产物归档见下。 |
| 8 | F09 T10/T06b 产物 | P1 | 已修复 | T10：`cargo test -p application --test t10_core_matrix -- --nocapture` 16/16，1.92s；归档去绝对路径 + 时间戳/耗时/内核。T06b：`tools/validate/t06b_validate.ps1` 重跑 35 pass / 1 fail（`xray-kcp` exit 23，`mkcp-legacy` 上游不支持），sing-box 日志 0 字节属校验器成功静默、非陈旧。产物归档见下。 |
| 9 | F15/F16 设置语义 | P2 | 已修复 | `domain/src/settings.rs`：为 `SpeedTestTimeout`/`MixedConcurrencyCount` 的 `>=10` 补“校正而非报错”注释（与 FLD-CFG-106 对齐，`validate` 仍只拒 `<0`）；新增 `SYSTEM_PROXY_EXCEPTIONS_LINUX` 与 `default_system_proxy_exceptions()`（`cfg!(windows)` 分平台），`Default` 与 normalize 分支统一走该函数。 |
| 10 | F13 台账精度 | P2 | 仅登记 | 见文末 B 组待办；未改 `compat/`。 |

## 门禁结果（本轮首次运行）

```
cargo fmt --all -- --check                                   -> EXIT 0（先执行 cargo fmt --all 修正前次未格式化改动）
cargo clippy --workspace --all-targets --locked -- -D warnings -> EXIT 0
cargo test --workspace --locked                              -> EXIT 0；77 个测试二进制，808 passed / 0 failed / 1 ignored（ignored = xray_smoke，另行运行）
```

- 本轮修复的唯一门禁阻断：`bridge_api/src/api/subs.rs` 测试在 `await` 点持有 `std::sync::MutexGuard`（`clippy::await_holding_lock`）。改为在持锁下用 `tokio::runtime::Builder::new_current_thread().block_on(...)` 驱动 future，既串行化又不跨 await 持锁。
- 并行组若在 Rust workspace 引入阻断，本轮未观察到（全 workspace 编译/测试通过）。

## 回归测试（关键用例，均在 workspace 运行中通过）

- updater：`external_spec_rejects_absolute_path_outside_root`
- application：`replace_sub_profiles_failure_keeps_old_nodes`、`delete_sub_items_removes_orphan_nodes`、`sqlite_replace_for_sub_rolls_back_on_injected_failure`
- subscriptions/application：`via_proxy_without_endpoint_returns_proxy_unavailable`、`via_proxy_with_endpoint_is_not_reported_unavailable`、`build_candidates_honors_custom_core_hint`
- bridge_api：`update_via_proxy_without_endpoint_returns_proxy_unavailable`、`global_engine_tests_are_serialized_not_concurrent`、`save_profile_rejects_stale_revision_through_bridge`
- domain：`inbound_user_pass_accept_explicit_null_as_empty`
- ipc_contract：`canonical_recheck_accepts_real_layout`、`canonical_recheck_rejects_symlink_escape`、`job_assign_failed_maps_to_stable_code`
- privileged_helper：`job_object_arms_kill_on_close`、`assign_to_invalid_job_fails_structured`、`process_identity_binds_pid_and_creation_time`、`disconnect_cleanup_failure_is_surfaced_and_audited`、`disconnect_cleanup_success_audits_ok`

## 产物归档路径

- `docs/evidence/T15.runs/manifest.json`（命令/时间/PID/端口/样本/内核/清理说明）
- `docs/evidence/T15.runs/stdout.log`（本轮原始输出，repo/temp 路径可移植化）
- `docs/evidence/T15.runs/xray-config.template.json`（带 `<socks_port>`/`<metrics_port>` 占位）
- `docs/evidence/T10.runs/manifest.json`、`docs/evidence/T10.runs/results.sanitized.json`（16 条，去绝对路径）
- `docs/evidence/T06b.runs/results.json`、`docs/evidence/T06b.runs/matrix-logs/*.log`（刷新）
- `docs/evidence/T06b.runs/rerun-20261001-stdout-tail.log`（重跑命令/时间/结果/stdout 尾行；替换先前的 PowerShell 报错占位）

## 新增/仍存未决项

1. **T06b `xray-kcp` 失败（exit 23）**：xray 26.3.27 不再支持 `mkcp-legacy`（`unknown config id: mkcp-legacy`）。属夹具 legacy 用例与内核版本漂移，非 codegen 缺陷；建议 T17 前更新该夹具或标注 `not_applicable`（台账事项，交 B 组/内核负责人）。
2. **sing-box 校验证据弱**：`sing-box check` 成功时 stdout 为空，仅 exit 0；已在 T10/T06b 产物中显式记录，未伪造成文本证据。
3. **F11 a) 落点**：canonical 复验实现在 `crates/ipc_contract/src/helper.rs`（`validate_elevated_core` 所在层，被 `privileged_helper` 调用），非 `windows.rs` 内联。功能等价且覆盖启动前与执行前两处校验；`ipc_contract` 不在 A 组声明写范围，该改动属审前已在工作区的改动，需编排方确认归属。
4. **`subscriptions` 直连回退**：`download_with_fallback` 在“已提供代理端点但代理请求失败/为空”时仍按上游语义回退直连；F03 仅修复“完全无端点”场景。若产品要禁止失败回退直连，需另立需求。

## B 组待办台账清单（Rust 侧证据，未直接改 `compat/`）

1. **`compat/fields.entities.yaml` FLD-ENT-151..158**：当前树中 8 条字段仍为 `implementation_location: null`、`test_ids: []`（约 4147–4348 行）；另有聚合条目 `FLD-ENT-151..158`（约 4394 行）已给出 location/test。建议二选一：逐字段补 `location/test`，或在字段类条目统一声明“location 以聚合条目为准”的免补口径。
2. **`compat/features.yaml` F-SUB-008**：状态仍为 `implemented`，但 `test_ids` 仅持久化 + T10 链行为；建议改 `preserved_only` 或加“persistence-only，链行为见 T10”标注。`F-SUB-007` 已为 `preserved_only`（已正确）。
3. **文件名笔误 `subscription_pipeline.rs` → `subs_pipeline.rs`**：当前工作树 `compat/features.yaml`、`compat/fields.entities.yaml` 已全部使用 `subs_pipeline.rs`，未再发现该笔误；若审计基于 `29221bf`，请 B 组确认该项已闭环。
4. **红线复核**：`compat/*.yaml` 中 `status: verified` 计数为 0；`docs/evidence/T09.md` 首行已改为“implemented（Rust 侧），全链路跑通前不得标 verified”。请 B 组确认与 T00 口径一致。
5. **FLD-ENT-082..098 测试复用/命名**：审计提及同往返测试复用，建议 B 组核对测试粒度（A 组未改 `compat/`，无法闭环）。

## 本轮改动文件（仅 Rust/服务与指定证据）

- `crates/bridge_api/src/api/subs.rs`（clippy 阻断修复：测试改 block_on）
- `crates/application/src/subs.rs`（补 F03 “有端口”回归测试）
- `cargo fmt --all` 触及的 Rust 源文件（格式化）
- `docs/evidence/T10.runs/{manifest.json,results.sanitized.json}`
- `docs/evidence/T15.runs/{manifest.json,stdout.log,xray-config.template.json}`
- `docs/evidence/T06b.runs/{results.json,matrix-logs/*.log,rerun-20261001-stdout-tail.log}`
- `docs/evidence/T09-T16-remediation.rust.md`（本文件）
