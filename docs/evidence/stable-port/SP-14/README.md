# SP-14 All 纯预览批导入 — 证据

状态：implemented（定向 Rust + Dart 检查全绿；真实原生 GUI/正式包验收未跑，不写 verified）。
基线：`8da1452`。不要 commit，根整合者验收提交。
合成数据专用；不占/改 10808（无 socket 监听；受控端口仅出现在既有测试字符串中）；
不改宿主系统代理/路由/TUN/DNS/Run-key；`work/`、`outputs/` 只读未动；`compat/` 未动。
工作区另有并行兄弟卡未提交改动（SP-05/SP-06 等：`runtime_bridge.dart`、
`ipc_contract/stable.rs`、`services/net_host/*`、`services/privileged_helper/*`），
本卡未动其中任何文件。

唯一用户流程：用户在 All 预览导入后取消或提交失败，不产生文件或半批节点。

## 0. 缺陷复现（红）

- 审计红合同：`docs/evidence/complete-port-audit-2026-10-06/ui/ungrouped_import_failure_test.dart`
  —— All/无组提交走逐行 `saveImportedProfile`，第二条存储失败留下第一条
  （`saved=1/failed=1`，合成仓储 `before=0/after=1`）；预览走旧 `importFromText`
 （`materialize=true`），Custom 预览可落盘；`commitImportText` 已生成但 UI 未消费。
- 本卡红探针（基线 `8da1452` 上实测）：
  - Dart `test/repair/sp_14_import_batch_test.dart` 初版：`commitImport(bridge, preview)` 等
    新签名在基线代码上编译失败（`Too few positional arguments: 3 required, 2 given`），
    证明旧 `commitImport(bridge, text, preview)` 做第二次解析 + 无组逐行写；
  - Rust `crates/application/tests/sp14_import_batch.rs` 初版：
    `unresolved import application::import_batch` + `no method commit_import_batch`，
    证明应用层无 token/revision/mutation 批提交基础设施。

## 1. 改动文件

- `crates/application/src/import_batch.rs`（新增）：`preview_token`（FNV-1a 64 hex，
  与 Dart 同算法）/`ImportCommit`（`previewToken/expectedRevision/mutationId/targetGroup`，
  即 SP-00 `CommitImportRequest` 应用层形态）/`ImportReceipt`（`CommitImportResult` 形态）
  /`content_digest`/`staged_file_name`/`normalize_batch`/结构化空提交·token 失配·
  revision 过期错误；`commit_id` 复用 SP-02 `commit_id_for(mutation_id)`。
- `crates/application/src/engine.rs`：新增 `import_previews`（token→内容摘要，纯内存，
  注册预览零 DB/文件写）/`import_commits`（mutationId→收据，幂等重放）/`import_fault`
  （测试注入）；新增 `register_import_preview`/`check_import_token`/
  `set_import_commit_fault`/`commit_import_batch`（重放→revision→token→Custom 暂存→
  单 `replace_sub_profiles` 事务；失败删本批暂存文件；成功记收据）。
- `crates/application/src/lib.rs`：`pub mod import_batch;`（仅此）。
- `crates/bridge_api/src/api/subs.rs`：`commit_import_text` 补 Custom 暂存（内容寻址，
  与 `import_batch::staged_file_name` 同名复用）+ 单事务 + 失败清本批暂存文件 +
  测试注入 `COMMIT_IMPORT_FAIL_NEXT`；签名不变（FRB 免重生成）；新增 2 测试。
- `apps/desktop/lib/bridge/bridge_port.dart`：抽象新增 `previewImportText`/
  `commitImportText`；`FrbBridgePort` 接已生成绑定；`SyntheticBridgePort` 实现纯预览
  解析 + 直写单事务提交（一次 revision bump，经 `saveImportedProfile` 计数可证不再逐行）
  + `failNextCommit` 注入。
- `apps/desktop/lib/features/subs/import_persistence.dart`：`ImportPreview` 携
  `previewToken`（FNV-1a，与 Rust 同向量）/`expectedRevision`（预览时冻结）/
  `mutationId`；`previewImport` 走纯 `previewImportText`；`commitImport` 全部分组
  （含 All/无组）走单事务 `commitImportText`，token 失配→`E_PREVIEW_MISMATCH`、
  revision 过期→`E_REVISION_STALE`（均零写），同 mutationId 返回缓存收据；
  旧逐行 `persistImportedProfiles` 保留仅供既有直接调用者（T21-E 桥测试）。
- `apps/desktop/lib/features/subs/subs_actions.dart`：`_importPipeline` 预览后弹
  确认框（`import-preview-dialog`：数量/目标/取消零效果/失败无半批说明），取消吐司
  `已取消：未导入任何节点` 并零提交；提交失败吐司`导入未提交，已保留原数据（code）`；
  成功才 reload + 成功吐司。
- 测试（随改动同步，未列入“允许模块”生产约束）：
  新增 `apps/desktop/test/repair/sp_14_import_batch_test.dart`；
  更新 `test/r4_16_contract_test.dart`（新 seam 计数）、
  `test/repair/r4_16_repro_test.dart`、`test/recheck01_import_snapshot_widget_test.dart`、
  `test/t09_import_export_test.dart`、`test/t21e_import_ux_test.dart`（预览确认框点提交/取消）；
  新增 `crates/application/tests/sp14_import_batch.rs`。

## 2. 命令与 exit

Rust（workspace 根，`--locked`）：

- `cargo fmt --all -- --check` → exit 0。
- `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` → exit 0。
- `cargo test -p application --locked --test sp14_import_batch` → 8 passed / 0 failed
  （含 10k 有组/All 各一批：`elapsed=2.8641006s`（All）、`elapsed=2.9153779s`（有组），
  独立 `reopen` 后各 10000 行且 `subid` 全对）。
- `cargo test -p bridge_api --locked` → 79 passed / 0 failed
  （含新增 `sp14_preview_custom_leaves_no_files`、
  `sp14_commit_custom_materializes_and_fault_rolls_back`）。

Flutter（`apps/desktop`）：

- `flutter analyze` → `No issues found!`。
- `flutter test test/repair/sp_14_import_batch_test.dart` → 9 passed。
- 受影响既有（单文件分别运行；多真实桥文件同进程并发曾超时，见 §4）：
  `test/r4_16_contract_test.dart` → 9 passed；
  `test/repair/r4_16_repro_test.dart` → 1 passed；
  `test/recheck01_import_snapshot_widget_test.dart` + `test/recheck01_group_inheritance_test.dart` → 5 passed；
  `test/t09_import_export_test.dart` → 4 passed；
  `test/t21e_import_forms_test.dart` → 5 passed；
  `test/t21e_import_ux_test.dart` → 6 passed（含新增取消零效果用例）；
  `test/t21e_import_real_bridge_test.dart`（真实 `bridge_api.dll` + 临时 SQLite）→ 1 passed；
  `test/r4_16_matrix_test.dart`（真实桥 17 格式矩阵）→ 1 passed。

跨层 token 向量（Dart = Rust）：`"" → cbf29ce484222325`，`"a" → af63dc4c8601ec8c`，
两端同断言通过。途中发现 Dart `int.toUnsigned(64)` 定宽下恒等（高位仍打印负 hex），
改 `BigInt.toUnsigned(64)` 格式化解决。

## 3. 完成情况

- 预览零写入：`previewImportText`（Rust `materialize=false`；Dart 不再经旧 `importFromText`）
  + Custom 预览无文件断言 + 取消（粘贴框取消/预览框取消/无提交调用）三路 no-diff。
- 提交整批原子：Dart 全部分组单事务；Rust `replace_sub_profiles` 单事务 + Custom
  暂存失败回滚（注入错后行数不变、暂存目录零孤儿、重开一致）。
- `previewToken` 绑定内容；`expectedRevision` 过期零写；`mutationId` 同键重放零重写
  （Dart 缓存收据；Rust `import_commits` 注册表；重放先于 revision 复核，与 SP-02
  一致）。
- 组快照：粘贴/扫码入口仍在命令开始冻结 `groupSubId`，预览/提交沿用冻结值
  （组件测试切组不断言改目标）。

## 4. 未完成/阻塞（含整合者共享接口）

1. （阻塞·整合者）FRB `CommitImportRequest`（`preview_token/expected_revision/mutation_id/
   dataset_epoch/target_group`）正式接线：Rust `application::import_batch` 与 Dart
   seam 已按同语义实现且 token 算法跨层一致，但 `bridge_api` 现有 `preview_import_text`/
   `commit_import_text` 签名未带这些字段（本卡严守“FRB 仅整合者生成”，未改生成物）。
   需 SP-00 整合者新增/扩展 FRB 方法并二次 `no-diff`，届时 `commitImport` 改传参、
   `register_import_preview` 改由 Rust 侧登记。
2. （未验证）真实原生 GUI（含预览确认框截图/事件）与未武装正式包入口：本卡只有合成桥
   widget 测试与真实 `bridge_api.dll` 逻辑测试，无正式包证据。
3. （未运行）完整 AGENTS 门禁（workspace 全量 + `flutter build windows --release`）：
   按 `VALIDATION_POLICY.md` 留待发布候选；本卡仅定向检查。
4. （说明）多真实桥测试文件同进程一次批量运行曾超 600s 超时（各 DLL 初始化串扰），
   改单文件分别运行全绿；`t21e_import_real_bridge`/`r4_16_matrix` 覆盖的旧
   `importFromText` 行为保持不变。
