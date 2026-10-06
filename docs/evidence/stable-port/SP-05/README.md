# SP-05 冻结运行目标和 job 终态 — 证据

状态：implemented（Rust + Dart 门禁全绿；真实原生/正式包验收未跑，不写 verified）。
基线：`7466bee`（SP-00/01/02/03/04/11 已完成）。不要 commit，根整合者验收提交。
合成数据与 fake 桥仅用于故障注入与并发编排；真实入口走真实 FRB 函数
（`apply_runtime` / `stop_runtime` / `get_operation`，本次零新增 FRB 面、
零生成物变更、无二次生成事项）；Rust 侧计划携带探测端口 ≥11808
（11951 起探），Dart 侧无 socket；绝不占/改 10808；不改宿主系统代理/
路由/TUN/DNS/Run-key；只停本项目自有进程（net_host 侧 stub `.cmd`
经持有句柄回收）；`work/`、`outputs/` 只读未动；`compat/` 未动；
并行他卡文件（SP-23/SP-29、tools/gates、manifest 他卡块）未动。

唯一用户流程：A 计划提交后用户改默认 B，实际核心和 applied 目标仍正确表示 A。

## 0. 缺陷复现（红）

- 审计红合同（基线行为）：`docs/evidence/complete-port-audit-2026-10-06/runtime/
  applied-target-race-contract.log` —— A 在途时 desired 移到 B，
  `applied_target` 读成 B（`left: Some("synthetic-b")`）。
- 本卡红探针（基线 `7466bee` 上实测，仅用基线已有 API，临时文件已删，
  日志见 §2）：`sp05_red_probe_apply_a_then_default_b_still_reports_a`
  exit 101，
  `left: Some("node-b") / right: Some("node-a")` —— 与审计红合同一致。
  根因：`engine.rs` 在阻塞 `runtime.apply` 返回**之后**才读
  `active_profile()`（RUN-04）；`operation_status` 把复合
  `"<op>:<job>"` 原样透传给 runtime（查无此 operation）且 apply job
  永不终结（RUN-05）。

## 1. 改动文件

- `crates/domain/src/applied_target.rs`（新增）：
  `FrozenAppliedTarget`（提交时冻结：target_profile_id/plan_id/
  config_sha256/core/desired_revision/operation_id/intent_seq/
  actual_generation）+ 唯一映射 `stable_operation_name` /
  `job_state_from_stable_name`（`stable::OperationState` 蛇形名 ↔ 既有
  `JobState`；`Superseded` 无对应 job 终态，映射为 `Cancelled` 并文档化，
  不并存两套无映射枚举）。
- `crates/domain/src/lib.rs`：模块声明与重导出（仅此）。
- `crates/application/src/engine.rs`：
  `apply_target: Option<String>` 改为 `applied_frozen`（提交前冻结，
  永不重读 desired）+ `intent_seq` / `actual_generation` 单调计数 +
  `operation_jobs`（operation→job 关联）；新增
  `apply_runtime_for_target(plan, target_id, rev)`（冻结显式 target；
  旧 `apply_runtime` 保留并改为提交前预读默认，不再提交后读取）；
  `operation_status` 切分复合 ID、终态合并（任一侧终态即终态，
  runtime 补 job 关联）、未知保持 structured not-found；
  `reconcile_applied_session` 只发布冻结 target（空目标/无记录仍不伪造，
  R4-05 方向保留），首次 Running 发布推进 generation 并结束关联 job
  为 Done，失败带错结束 Failed、无错撤回结束 Cancelled、从未发布且无错
  保持 pending（读不杀死提交）；`stop_runtime` 成功才记账（撤回 live、
  有 live 才推进 generation、取消未达 Running 的 apply job、保留历史、
  持久化）；`guiNConfig.json` 新增引擎键 `applied_target` /
  `runtime_intent_seq` / `actual_generation`（进 `SETTINGS_META_KEYS`，
  不泄漏进设置解析），`open` / `reopen` 经 `restore_applied_history`
  恢复；新增 `operation_state_name` / `job_state_for_operation` 映射封装
  与 `applied_target()` / `actual_generation()` / `last_intent_seq()` /
  `operation_job()` 只读口。
- `crates/application/src/lib.rs`：重导出两映射封装（供 bridge/证据复用）。
- `services/net_host/src/session.rs`：
  `stop_managed_inner` 使用 `operation_id`（原 `_operation_id` 忽略）——
  有 id 的 stop 记终端 Done 条目，早先 apply 条目保留（stop 只追加历史，
  永不改写）；查询/快照路径未动。新增 `record_stop_operation`。
- `crates/bridge_api/src/api/engine.rs`：`apply_runtime` 改调
  `apply_runtime_for_target(&plan, &target, …)`（一行，目标不再丢失；
  FRB 面零变更，`get_operation` 经引擎切分直接支持复合 ID 查询）。
- 新增测试：`crates/application/tests/sp05_applied_target.rs`（7 项，
  见 §3）；`session.rs` 内 `sp05_*` 2 项（stub core，无监听端口）；
  `apps/desktop/test/repair/sp_05_target_contract_test.dart`（4 项，
  脚本化 bridge，无原生库：冻结转发/复合 ID 双形查询/stop 保历史/
  stale 拒绝）。
- FRB/共享 DTO：零改动（`stable.rs` 只作词汇复用；`SnapshotDto` 新增
  applied-target 字段与 `StopRuntimeResult.operation_id` 回传属整合者
  wire/regen 范围，已登记 §5，不私改）。

## 2. 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | 通过（中途本卡 2 处格式 diff，已随 `cargo fmt` 修） |
| `cargo clippy -p domain -p application -p bridge_api -p net_host --all-targets --locked -- -D warnings` | 0 | 通过（中途 `question_mark` 1 处，已改 `?` 早返） |
| `cargo test -p domain --locked` | 0 | 52 passed（含 applied_target 3/3） |
| `cargo test -p application --locked` | 0 | 509 passed（含 lib 288、sp05 7/7、sp04 4/4），0 failed |
| `cargo test -p bridge_api --locked` | 0 | 77 passed，0 failed |
| `cargo test -p net_host --locked` | 0 | 73 passed（含 sp05 2/2、sp04 3/3），0 failed |
| `dart format --output=none --set-exit-if-changed`（本卡 Dart 文件 + runtime 特征目录） | 0 | 通过（sp_29 他卡文件格式问题未动，归属 SP-29） |
| `flutter analyze`（apps/desktop） | 0 | No issues found |
| `flutter test test/repair/sp_05_target_contract_test.dart test/repair/sp_04_runtime_sequence_test.dart` | 0 | 11/11（sp05 4/4 + sp04 7/7） |
| 受影响 Dart 集（r4_01_repro/r4_01/r4_02/r4_04/r3_01_07_10/runtime_controller/fix11/fix13） | 0 | 53/53 |
| 红探针（基线 stash，仅基线 API）`cargo test -p application --locked --test sp05_red_probe` | 101 | 预期失败：applied B 而非 A（日志摘录见 §0） |
| `flutter build windows --release` / 正式包 GUI / 原生 FRB .dll 全链 | 未运行 | 留 SP-30/SP-34；未跑不得写 verified |

## 3. 正确合同（摘）

- 冻结：提交顺序 = intentSeq 顺序；目标/计划/revision/operation 在
  `runtime.apply` 调用前冻结；stale revision 拒绝且零 runtime 触碰、
  零冻结、零计数推进（sp05_stale_submit_never_switches）。
- 三者分离：desired（`active_index_id`）可先行改 B；applied（冻结历史）
  与 actual（live session + 端口 + generation）仍为 A；仅保存默认不
  触发切换、不伪造运行。
- job 终态：成功 apply 的 job 在首次 Running 对账后 Done，session 仍
  Running，无悬空 active apply；失败带错 Failed；stop/外部退出前未达
  Running 的关联 job 为 Cancelled；终端 job 永不改写；复合 ID 与裸
  operation ID 双向可查，未知保持 not-found。
- stop：撤回 live、保留历史 A（不断言 B）、有 live 撤回推进
  actualGeneration；失败 stop 不记账。
- 重开：独立进程同数据目录重开后 desired B 与历史 A 共存， live 事实
  仅来自后端，后端仍跑 A 会话则 applied 与 actual 一致为 A；计数单调。
- 映射：`Running/Cancelling/Compensating→executing、Done→succeeded、
  Failed→failed、Cancelled→cancelled`；反向 `accepted→Running、
  superseded→Cancelled`，未知名返回 None（domain 3 项 + 应用
  `sp05_operation_vocabulary_matches_stable_wire` 与 wire 序列化一致）。

## 4. 正式入口 / 重开 / 最终消费者

- 真实入口复用既有 FRB：Dart `applyTarget(targetId, expectedRevision)`
  → `rust.applyRuntime` → `AppEngine::apply_runtime_for_target`（真实
  `build_runtime_plan` 冻结 plan，合成节点 + 探测端口）/
  `stopRuntime` / `getOperation`（复合 ID 直查）/`getSnapshot`。
  应用集成测试走真实 plan 构建 + 真实 SQLite（重开用例
  `open_with_runtime` + 持久化 `guiNConfig.json` 含新键），fake 只做门控
  与后端事实脚本，不绑 socket、不起核、不碰管道/OS。
- 重开/恢复：`applied_target` + 双计数随文档持久化；`quiesce` 语义未动；
  net_host journal/恢复路径逐行保留（仅 stop 追加终端条目）。
- 最终消费者：monitor/applied-inbound 管线继续消费 `applied_session` +
  `apply_facts`（行为不变，fix11/fix13 回归全绿）；状态栏 actual 摘要
  需 FRB 字段，属 SP-17（见 §5）。

## 5. 未完成 / 阻塞（含整合者共享接口）

1. （需整合者 FRB regen）`SnapshotDto`/FRB 面尚未暴露
   `applied_target_profile_id` / `actual_generation` / `intent_seq`：
   本卡零 wire 改动，Dart 状态栏 actual 摘要（SP-17）在 Rust 侧事实就绪
   后仍需该字段。提供方 application（已就绪），调用方 bridge/Dart，
   版本 stable v1 不变。
2. （需整合者确认）stop operation-id 回传：`StopRuntime{operation_id}` /
   net_host 记录已就绪，但 `NetHostClient::stop()` 仍发 `None` 且
   `stop_runtime()` 返回 `()`（`StopRuntimeResult` 无 operation 字段）。
   按卡面要求先登记阻塞，不私改 wire：建议 `StopRuntimeResult` 加
   `operation_id` 可选字段 + FRB regen，或维持现状由调用方按 snapshot
   对账。
3. job 关联表是进程内事实（未持久化）：重开后前世 operation 查不到 job
   时如实 not-found，跨重启依据以冻结历史记录为准（已实现）。
4. 超时 unknown-apply（`apply` 本身报 `E_TIMEOUT`）仍无 operation id 可查：
   沿 SP-04 Dart 侧 reconcile 路径处理，引擎侧不伪造冻结（已实现）。
5. 真实原生 `.dll` 加载的 FRB 全链、正式包 GUI、隔离 OS 效果：未运行，
   留 SP-30/SP-34；覆盖 ID（F-CORE-005/006、R4-01/R4-04/R4-23 owner 集）
   不批量标 verified，本卡只锁定唯一流程机制。

## 6. owned 回收

- Rust 测试：内存引擎 / `tempfile` 数据目录（重开用例）/ net_host
  `std::env::temp_dir` run_root（测后 `remove_dir_all`）；stub `.cmd`
  经持有句柄回收；端口只探测不占用（应用测试 fake 不绑 socket，
  net_host 测试用 port 0 走 liveness 就绪，与既有 rr06 脚手架一致）。
- Dart 测试：脚本化 bridge + `ProviderContainer`（本文件未用 container，
  纯 bridge 单测）/ stream 关闭，无平台调用。
