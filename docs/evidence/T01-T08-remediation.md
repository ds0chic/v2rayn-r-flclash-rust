# T01–T08 审计整改记录（config_codegen 之外）

- 执行：deepseek-v4.1-flash（T01–T08 审计整改子代理）
- 基准 HEAD：`2fbe9f1`（`2fbe9f17437e0c25560de36fa68c37e29785e5e2`）
- 输入审计：`docs/evidence/audit/T01-T08.audit.gemini.md`、`docs/evidence/audit/T01-T08.audit.muse.md`
- 边界：不碰 `config_codegen`（归 T06b）；未 commit；未触碰 `127.0.0.1:10808`；未改系统代理；
  未读取任何用户凭据/订阅。

---

## 1. 整改项状态

| 编号 | 状态 | 说明 |
|---|---|---|
| **M-001** T03 用例 a PID 矛盾 | **已修** | `docs/evidence/T03.md` §4 表格与 a 时间线按 `T03.runs/a-normal.json` 修正为 net_host `39116` / xray `24264`；其他用例不动。 |
| **ISSUE-06** job.assign 失败静默 | **已修** | `services/net_host/src/session.rs`：失败即中止会话 + `start_kill`/wait + 结构化 `E_JOB_ASSIGN_FAILED`；新增 `job_assign_failed` 单测。新增错误码 `domain::codes::JOB_ASSIGN_FAILED`。 |
| **ISSUE-02** 导入指纹绑绝对路径 | **已修** | `crates/persistence/src/upstream_db.rs::source_identity` 改为内容寻址（`kind:content_hash[..16]`），去除 canonical path；新增回归测试 `same_content_from_another_directory_is_idempotent`。 |
| **M-003 / M-010** 明文秘密载体 | **部分修 + 已登记** | staged `config.json`/`core.log` 在会话结束/回滚/恢复时删除（`journal::remove_staged_artifacts`）；staged 目录与文件设置仅当前用户 DACL（`net_host::dacl::restrict_to_current_user`）。备份 bundle/`.bak`/`raw_records` 明文风险**未实现加密**，登记到 `docs/decisions/T04-storage.md` §7.2 与本文 §4。 |
| **ISSUE-05** 丢弃 epoch/seq | **已修（最小）** | `RuntimeEvent` 携带 `epoch/seq`；`RuntimeController` 维护 `lastEpoch/lastSeq`，乱序/断流记录 `debugPrint` 警告并写入 `RuntimeView.sequenceWarning`；状态栏新增 `rev: desired/applied` 与 `epoch/seq` 小文本。完整重连回放**登记未决**。 |
| **ISSUE-07 / M-021 / M-022** | **已修** | 删除状态栏死字段 `runningNode` 及 `运行信息:` 双源文本；`SyntheticRuntimeBridge` 移入 `apps/desktop/test/support/synthetic_runtime_bridge.dart`，`lib/` 内无假 Running 实现。 |
| **ISSUE-08** `V2RAYN_R_AUTOSTART` 后门 | **已修** | `apps/desktop/lib/app/app.dart` 自动启动钩子仅 `kDebugMode` 生效；release 门禁已复验构建通过。 |
| **M-006/M-007/M-008** T02 漂移 | **已登记 + 补测** | `docs/evidence/T02.md` 追加"修订注记"（契约冻结于 `1bc45e5`，HEAD 已被 T03 改写）+ HEAD 实测测试计数表；`crates/domain/src/revision.rs` 新增 5 项单测（含 `Empty/Ahead/check_expected E_REVISION_STALE`）。 |
| **M-020 / D12** 台账登记 | **已登记** | `compat/fields.entities.yaml` 追加 `remediation_implementation_records`（MIG-ENT-002/003/004 + REF-ENT-005 偏离）；`compat/features.yaml` 追加 F-CORE-005 / F-CORE-011 实现登记（纯追加，不删行）。 |
| **M-017** FRB 第三腿 | **已登记** | 新增 `docs/evidence/toolchain/frb-codegen-version.txt`、`generated-sha256.txt`；`docs/decisions/T01-deps.md` 追加注记。 |
| **M-018/M-019** 门禁可重放 | **已登记（本轮）** | 本文 §2 记录 4 条 Rust 门禁与 Flutter 门禁实跑输出摘要；`flutter_tester 0xC0000005` 处置口径见 §3。 |
| **M-023** 文档死引用 | **已修** | `T01.compat-refs.md`：`profiles_widget_test.dart` → `apps/desktop/test/profiles_pointer_test.dart`；补 `apps/desktop/` 前缀；`T06a.md` 追加复制后缀上游 `-clone` 语义核对注记。 |

---

## 2. 门禁结果（本轮实际重跑）

### 2.1 Rust workspace（`--locked` 全程可用）

| 门禁 | 命令 | 结果 |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | exit 0（无 diff） |
| clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0（0 warning） |
| test | `cargo test --workspace --locked` | **357 passed / 0 failed**（40 个测试二进制；含 subscriptions/config_codegen 等并行代理产出） |

关键 crate 结果行（摘要）：

```
domain            unittests  -> ok. 28 passed; 0 failed
ipc_contract      unittests  -> ok.  7 passed; 0 failed
application       unittests  -> ok. 27 passed; 0 failed
persistence       unittests  -> ok. 62 passed; 0 failed
persistence       tests/upstream_import.rs -> ok. 8 passed; 0 failed
runtime           unittests  -> ok. 18 passed; 0 failed
net_host          unittests  -> ok. 11 passed; 0 failed
subscriptions     unittests  -> ok. 61 passed; 0 failed
```

原始日志：`target/remediation_cargo_test.log`。
说明：整改开始时 `--locked` 因并行代理（subscriptions/sing-box）半成品与 lockfile 漂移一度失败；
其收敛后 `--locked` 全程通过，故不再依赖 `--offline`。

### 2.2 Flutter（`apps/desktop`）

| 门禁 | 命令 | 结果 |
|---|---|---|
| format | `dart format --output=none --set-exit-if-changed lib test` | exit 0（0 changed） |
| analyze | `flutter analyze` | No issues found |
| test（逐文件串行） | `flutter test test/<each>` ×18 | **18/18 可复现通过**（其中 `t05_shell_chrome_test.dart`、`t06a_editor_form_test.dart` 需重试；见 §3） |
| build | `flutter build windows --release` | exit 0（`build\windows\x64\runner\Release\v2rayn_desktop.exe`） |

> `tools/flutter_test_retry.ps1` 的**全套批跑**在本机当前负载下 12 次尝试均因引擎级崩溃未取得单次全绿；
> 逐文件串行重跑可全部通过（见 §3），确认无测试内容确定性失败。

---

## 3. `flutter_tester` 0xC0000005 处置口径（M-019）

- 现象：锁定的 Flutter 3.47.5 `flutter_tester` 在重度 widget 建树后随机/累积崩溃
  （`did not complete` / `Connection closed before test suite loaded` / exit `0xC0000005`），无 Dart 栈。
- 判定：**环境/引擎级**，与整改内容无关。证据：同一文件多次运行结果在"通过"与"进程死亡"间抖动；
  `flutter test test/<file>` 逐文件运行 18/18 可复现通过。
- 处置：维持 `tools/flutter_test_retry.ps1` 包装重试 + `dart_test.yaml: concurrency: 1`；
  **CI/多机复测为待办**（见 §4-R6）。

---

## 4. 新增未决项登记（D12）

| ID | 未决项 | 现状 / 下一步 |
|---|---|---|
| R1 | **DLL 架构显式报错缺口** | 非 x64/架构不匹配时未给出显式领域错误（`T01.md` 已记）。T12a/发布前补结构化诊断。 |
| R2 | **干净机 + 安装/便携双模式** | 仅在开发机验证；T12a 需定义 data dir/端口/自启动的干净机行为。 |
| R3 | **T02 契约漂移** | `T02-contracts.md`/`T02-revisions.md` 冻结于 `1bc45e5`，HEAD 已由 T03 改写；已在 `T02.md` §7 注记，契约以 `T03-runtime.md` 为准。 |
| R4 | **T04 自产库 ≠ 上游兼容** | 端到端输入全部由 `make_fixtures.rs` 自产，从未打开真实 sqlite-net `guiNDB.db`；T16 前补真实形状导入测试。 |
| R5 | **staged / 备份明文** | staged 已最小加固（删除 + 仅当前用户 DACL）；备份 bundle/`.bak`/`raw_records` 仍明文，未加密。T16 决定加密/ACL/轮转。 |
| R6 | **flutter_tester 崩溃与 CI 复测** | 见 §3；CI/多机崩溃率与重试语义待定义。 |
| R7 | **T07/T08 偏差跟踪 ID 统一** | config_codegen 侧偏差（protocol 静默丢弃、悬空回退 proxy、`v2ray.cool:10086` 占位、transport 矩阵、`endpoints:[]`、reality 校验过窄、模板合并顺序、内部导入覆盖缺口等）由 **T06b** 承接；统一跟踪 ID：`CGX-01`..`CGX-10`（本表登记，不在本代理修改 `config_codegen`）。 |
| R8 | **完整重连/回放** | ISSUE-05 只做到记录 `epoch/seq` 与告警；断流后的补发/重取快照/重放为 T09+。 |
| R9 | **复制后缀 `(副本)` vs 上游 `-clone`** | 已在 `T06a.md` §8 与 `compat/features.yaml` F-PROFILE-004 注记；代码对齐归 T06b/T10。 |

---

## 5. 回归测试清单（本轮新增/修订）

| 测试 | 位置 | 断言 |
|---|---|---|
| `same_content_from_another_directory_is_idempotent` | `crates/persistence/tests/upstream_import.rs` | 复制同一 `guiNDB.db` 到另一目录再导入 → `AlreadyImported`，`ProfileItem` 计数不变（7）。 |
| `job_assign_failure_is_structured_and_non_retryable` | `services/net_host/src/session.rs` | 失败错误码 `E_JOB_ASSIGN_FAILED`、`message_key`、`operation_id`、`retryable=false`。 |
| `staged_artifacts_are_removed_but_journal_survives` | `services/net_host/src/journal.rs` | 清理后 `config.json`/`core.log` 不存在、`journal.json` 保留。 |
| `recovery_deletes_staged_config_after_finalizing` | `services/net_host/src/journal.rs` | 恢复后明文配置被删、journal 变 `Finalized`。 |
| `restrict_to_current_user_applies_to_file_and_dir` | `services/net_host/src/dacl.rs` | 目录/文件 DACL 收紧可成功应用（Windows）。 |
| revision 状态机 5 项 | `crates/domain/src/revision.rs` | `Empty`/`InSync`/`Pending`/`Ahead`/`check_expected`(`E_REVISION_STALE`)/`next`。 |
| `revision label renders ...` | `apps/desktop/test/runtime_controller_test.dart` | `rev: 4/2`、空态 `rev: -/-`。 |
| `controller records epoch/seq and flags out-of-order events` | 同上 | 保存 `lastSeq`；乱序时 `sequenceWarning != null`。 |

既有测试：`cargo test --workspace --locked` 357 passed；Flutter 逐文件 18/18。

---

## 6. 台账/文档更新路径

- `docs/evidence/T03.md`（PID 修正）
- `docs/evidence/T02.md`（§7 修订注记 + 测试计数）
- `docs/evidence/T06a.md`（§8 复制后缀注记）
- `docs/evidence/T01.compat-refs.md`（死引用修正 + 修订注记）
- `docs/decisions/T01-deps.md`（M-017/M-018/M-019 注记）
- `docs/decisions/T04-storage.md`（§7 REF-ENT-005 偏离 + 明文未决项）
- `docs/evidence/toolchain/`（FRB 版本与 SHA256 清单）
- `compat/fields.entities.yaml`（追加 `remediation_implementation_records`）
- `compat/features.yaml`（追加 `remediation_implementation_records`）
- `docs/evidence/T01-T08-remediation.md`（本文件）
