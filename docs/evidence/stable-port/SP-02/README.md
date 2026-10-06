# SP-02 可恢复持久化提交 — 证据

状态：implemented（Rust 门禁全绿；真实 OS/正式包验收未做，不写 verified）。
基线：`92b57b0`（SP-00/SP-01 已完成）。未 commit，工作树仅含本卡 6 个文件。
合成数据与专用 scratch 目录（`tempfile::tempdir`，用后即删）；无真实用户数据、
无 socket/端口、无宿主代理/路由/TUN/DNS/Run-key 操作。

## 1. 改动文件

- `crates/persistence/src/commit.rs`（新增）：两阶段提交 journal。
  `begin`（staging + journal 原子写）→ `mark_db_committed` →
  `publish_staged`（fsync + rename，旧文件或完整新文件二选一）→
  `finish`（done 收据 + 清理）/`abandon`（DB 前回滚）。
  `pending`/`load`/`load_receipt` 查询；`recovery_required` 标记阻塞新写。
  journal 记录只含身份/哈希/阶段（单测锁定键集合），文档字节只在
  staging 载荷；`CommitFault` 为测试专用故障注入（沿 `ImportFault` 前例，
  生产恒传 `None`）。Windows 只读句柄不能 flush（os error 5），fsync 改用
  可写句柄。
- `crates/persistence/src/lib.rs`：注册 `pub mod commit`。
- `crates/application/src/recoverable_commit.rs`（新增）：纯辅助层，直接复用
  SP-00 冻结合同 `ipc_contract::stable`（`SettingsSaveReceipt`、
  `SettingsSaveState`、`PhaseApplyState`、`ContractError`、`DatasetEpoch`），
  未新增矛盾 DTO。`commit_id = derived_id("commit", mutation_id)`（幂等），
  `settings_hash` 为幂等键（规范 settings）、`doc_hash` 为发布完整性键
  （暂存字节精确哈希——曾用紧凑串哈希对 pretty 文件，恢复必 mismatch，已修）。
- `crates/application/src/engine.rs`：`AppEngine` 新增 `dataset_epoch()`（=
  `restore_epoch`，普通提交保持，恢复/替换才 bump）、`save_settings_commit`、
  `query_settings_mutation`、`recover_pending_commits`、
  `pending_commit_recovery`、`set_commit_test_fault`；旧 `save_settings` /
  `save_settings_group` 在恢复未决时拒绝（`error.recovery_required`），杜绝
  经旧路径写入一半快照。提交成功才进内存 revision/desired，文件为真相源。
- `crates/application/src/lib.rs`：注册 `pub mod recoverable_commit`。
- `crates/application/tests/sp02_recoverable_commit.rs`（新增，8 项正确预期，
  先红后绿）：stale revision/epoch 拒绝无 newRevision；成功提交返回
  newRevision/token 且独立重开可读、原参重放同一收据；未知 mutation 查询为
  NotStarted；发布失败 → CommitUnknown → 新写 RecoveryRequired → 重开仍阻塞
  → 恢复 roll-forward 同一 commitId 且只提交一次；DB 前崩溃 → 重开回滚、
  旧快照完整、新写可用；同 mutation 不同内容判冲突拒绝；tracking 记录与
  receipt 不含秘密探针。

## 2. 真实命令与 exit

| 命令 | exit | 结果 |
|---|---|---|
| `cargo fmt --all -- --check` | 0（初检有本卡 import 顺序 diff，`cargo fmt` 后复检 0） | 通过 |
| `cargo clippy -p persistence -p application -p domain --all-targets --locked -- -D warnings` | 初次 1（`manual_ok_err` 一处，已修）→ 0 | 通过 |
| `cargo test -p persistence --locked` | 0 | 100 passed（74+5+8+5+8，含 commit 6 新单测） |
| `cargo test -p application --locked` | 0 | 478 passed（lib 278 + 30 个集成文件全 ok，含 sp02 8/8） |
| `cargo test -p domain --locked` | 0 | 49 passed |
| `dart format --output=none --set-exit-if-changed lib test`（apps/desktop） | 1 | 4 个文件为基线既有格式问题（本卡零 Dart 改动，`git status` 为证），未动 |
| `flutter analyze`（apps/desktop） | 0 | No issues found |
| `flutter test test/repair/r4_27_repro_test.dart` | 0 | 4 passed（相邻存储合同无回归） |
| `flutter test` 全量 | 未运行 | 本卡允许模块无 Dart 范围（见阻塞 §7），全量批跑历史有基础设施异常，留 SP-29 |

## 3. 正确合同（摘）

- 拒绝（stale revision/epoch、校验失败、空 mutation、mutation 冲突）：
  `save: Rejected`，`new_revision: None`。
- 成功：`save: Committed`，`new_revision/token/commit_id/commit_epoch=1`，
  `applied_content_hashes=[settings_hash]`，core/platform 为 `NotRequired`
 （持久化事实；生效分阶段由 SP-12 链处理，不在此冒充 applied）。
- DB 已提交而发布未确认：`save: CommitUnknown`，无 revision，不谎称“未改变”；
  之后一切新写（含旧 `save_settings` 路径）→ `RecoveryRequired` 直至
  `recover_pending_commits` 确认（roll-forward 得同一 commit 的 Committed，
  staged-only 得 Rejected 回滚记录）。
- 查询从不重放写；未知 mutation → `NotStarted` + `E_UNKNOWN_MUTATION`。

## 4. 正式入口 / 崩溃与重开 / 最终消费者

- 正式入口：`AppEngine::open_with_runtime(data_dir, _)` 真实 SQLite +
  真实 `guiNConfig.json`（`NullRuntimeClient`，无内核/网络/OS 副作用）；
  提交经 `save_settings_commit`，重开为全新 `open_with_runtime` + 显式
  `recover_pending_commits`（重开自动感知阻塞，不自动改数据）。
- 崩溃注入：`CrashAfterStage`（仅 journal）→ 回滚；`FailFilePublish` /
  `CrashAfterDbCommit`（DB 已提交）→ roll-forward；drop engine 模拟崩溃，
  独立重开验证。
- 最终消费者：本次为 Rust 持久化快照（重开读回 + query 收据）；FRB/Dart
  接线未做（共享 DTO 由根整合者负责，见阻塞）。

## 5. owned 回收

- 全部测试目录为 `tempfile::tempdir`（进程外 scratch，测试结束自动删除）；
  无 10808/宿主资源触碰；`work/`、`outputs/` 只读未动；`compat/` 未动；
  `git status` 仅本卡 6 文件，无多余产物。

## 6. 未验证与下一前置

- 未验证：真实 OS 副作用（本卡无）、正式未武装包端到端（SP-30/SP-34）、
  其它 OS/架构、24h/500 切换（SP-35）。
- 下一前置：SP-03（canonical 往返，本提交的 revision/journal 可直接复用）、
  SP-12（已保存未生效重试：`retrySettingsApply` 语义与本收据对接）、
  SP-14（All 批导入提交走同一 journal 模式）。
- 阻塞（需根整合者）：Dart 侧 `sp_02_commit_contract_test.dart` 与
  `saveSettings(datasetEpoch, expectedRevision, mutationId, patch)` /
  `querySettingsMutation` / `retrySettingsApply` 的 FRB 接线未做——本卡允许
  模块仅 `crates/persistence` + `crates/application`，FRB/共享 DTO 归整合者
  独占；Rust 侧收据已是冻结 `stable` 类型，可直接映射。
