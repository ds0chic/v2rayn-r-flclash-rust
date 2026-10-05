# R4-28 备份 WebDAV 与恢复 — 证据

状态：implemented（本卡 Dart 控制器修复完成并有复现/合同证据；真实 TLS 分支 blocked；真实 Windows 端到端重开未实测）。
基线 HEAD：`7e9a4b49de8c287abc920b5a3eff1c1b2cb74a1b`（工作树上有其他并行子代理的未提交改动，见下）。
本卡仅改 `apps/desktop/lib/features/backup/backup_controller.dart`，未改 Rust 主代码、未改 `frb_generated`、未动宿主代理/路由/TUN/注册表/自启、未使用 10808。

## 1. 缺陷与修复

- D23（P1）：WebDAV/本地备份恢复的 async 控制器只在成功路径处理结果；`await` 周围没有统一的 `catch/finally`。桥接/原生抛异常时 `busy` 永久为真，按钮永久禁用；`cancel()` 只对本地备份/恢复生效，WebDAV 操作不检查 generation，取消后晚到的上传/恢复结果仍会提交并显示“成功”。
- 修复（`backup_controller.dart`）：
  - 每个 async 操作统一 `try { await ... } catch { 可读错误 } finally { busy=false（仅当 generation 仍当前）}`；同步操作（保存配置、打开目录、清理）也加 `try/catch`。
  - `webdavCheck/webdavList/webdavBackup/webdavRestore/recognize/reloadBundles` 全部纳入 `_generation` 取消语义；取消后晚到结果被丢弃，状态保持“已取消”。
  - 新增单项在途（task drain）：`busy` 期间第二次操作被拒绝，避免两个恢复并发交换数据库。
  - 恢复成功后重载 provider 失败不再被伪装成恢复失败，而是明确“恢复已写入但重新加载失败”。
  - 失败后 best-effort 状态刷新自身再异常也不会逃逸卡住窗口；错误文本截断且不引入凭据。
- 未改动的既有正确语义保留：本地包 restore 前置 verify（损坏包直接拒绝，未改现有数据）、Rust 侧候选/回滚事务、WebDAV 401/403、404、超时、不可达的 `E_*` 映射。

## 2. 复现（先失败后通过）

命令：`flutter test test/repair/r4_28_repro_test.dart -r expanded`

- 修复前（`git stash` 仅回退 backup_controller.dart 到 HEAD）：3/3 失败
  - 抛异常的 webdavCheck：`Bad state: native bridge exploded ...`，busy 卡住。
  - 抛异常的 localBackup：`Bad state: disk copy failed`，busy 卡住。
  - 取消慢速远程备份：实际显示 `远程备份完成：1024 字节`（期望 `已取消`）。
  - 日志：`r4_28_repro_prefix.log`
- 修复后：3/3 通过。日志：`r4_28_repro_postfix.log`

## 3. 合同测试

命令：`flutter test test/r4_28_contract_test.dart -r expanded` → 9/9 通过（`r4_28_contract.log`）

覆盖：异步桥接面；本地备份提交清单；本地恢复失败保留原 bundles 并报错；WebDAV 401/404/超时/不可达四分支映射且不含合成口令；上传失败绝不假成功且不刷新远端列表；上传成功刷新远端列表；webdavRestore 抛异常后 busy 清除且重试成功；取消丢弃晚到上传且窗口可继续；并发第二次操作被拒绝。

## 4. 回归

命令：`flutter test test/t16_backup_test.dart test/fix14c_webdav_backup_test.dart test/sr02_04_backup_import_test.dart test/sr03_restore_lifecycle_test.dart test/fix14_backup_activate_test.dart test/fix14b_backup_restore_test.dart test/r4_10_contract_test.dart -r compact` → **27/27 通过**。首次批量运行曾出现 fix14b 整文件 “did not complete”，单文件重跑与二次批量均 5/5、27/27 通过，属并发构建/测试资源抖动（与并发子代理 cargo 同时运行），非本卡逻辑回归。

## 5. 必过命令结果

- `flutter analyze`（全量）：2 errors，均在 `lib/app/shell/tray_menu_model.dart`（并行子代理 R4-26 的在途改动，非本卡范围）。日志 `flutter_analyze_full.log`。
- `flutter analyze lib/features/backup test/r4_28_contract_test.dart test/repair/r4_28_repro_test.dart`：No issues found。日志 `flutter_analyze_scoped.log`。
- `dart format`：本卡 3 个文件已格式化。
- `cargo fmt --all -- --check`：EXIT=0（`cargo_fmt.log`）。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：EXIT=0（`cargo_clippy.log`）。
- `cargo test --workspace --locked`：EXIT=0，全部通过（`cargo_test.log`，含 `crates/application/tests/t16_webdav.rs` 的 loopback 正常/401/404/超时/上传失败/上游布局往返）。
- `flutter build windows --release`：未运行。本卡无 Rust/桥接/FRB 改动，且全量构建会被并行子代理在途的 `tray_menu_model.dart` 编译错误阻断，不属本卡合同。

## 6. Rust 侧 WebDAV 证据（既有，未改动）

`crates/application/tests/t16_webdav.rs`（loopback，端口探测 11808..11900，直连无代理）已覆盖：check/list/upload/download 正常往返；目录缺失 MKCOL；401 分类且 detail 不含口令；下载 404；上传 500 可重试；慢服务器超时 `E_TIMEOUT`；`guiConfigs/` 上游布局上传→下载→recognition。故真实 HTTP 分支不依赖假桥。真实 TLS 服务器登记 blocked（无隔离真实 TLS 端点，未使用用户凭据）。

## 7. Blocked / 未完成

- 真实 TLS WebDAV 端点与真实网络：blocked（无授权隔离真实端点；只用本地 loopback 与合成凭据）。
- Windows 端到端真实重开（备份→恢复→进程重开→主题/组/节点/热键/runtime 一致）：未实测（未跑真实桌面进程；本卡只到控制器+合成桥）。
- “断连”单列分支：Rust 侧网络错误归入 `E_UNAVAILABLE`（connrefused/timeout 区分），Dart 分支已映射；未加独立断连用例。

## 8. 改动文件

- `apps/desktop/lib/features/backup/backup_controller.dart`（修复，本卡唯一生产改动）
- `apps/desktop/test/r4_28_contract_test.dart`（新增）
- `apps/desktop/test/repair/r4_28_repro_test.dart`（新增）
- `docs/repair/tasks/R4-28.md`（状态更新）
- `docs/evidence/repair/R4-28/**`（本目录）
