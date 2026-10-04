# SR-02 / SR-04 修复证据（原版 ZIP 资源安装 + 提交后失败回滚）

状态：`implemented`。Rust 单 crate 与 Dart widget 合成用例通过；未在真实 Windows 窗口 + 真实 FRB 下肉眼对照原版恢复窗口，故不写 `verified`。

- 仓库 HEAD（开始）：`eeb2932`；工作树干净。
- 冻结原版：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
- 缺口来源：`docs/evidence/parity-recheck-2026-10-04/settings.md` SR-02 / SR-04 全节。
- 未改宿主代理/自启/TUN，未占用 127.0.0.1:10808，未读/写用户凭据；夹具全部合成。

## 结论

- SR-02：`backup_service::import_upstream` 在候选 DB 提交后新增“资源安装”阶段，按上游整目录语义把 `guiConfigs/` 下除 `guiNConfig.json`/`guiNDB.db` 外的所有成员（`config/`、`pac.txt`、脚本、嵌套子目录）复制到 live data 对应位置，逐文件 SHA-256 校验；重开后文件型 Custom 节点可经 `custom_file_text` 读到 live 文件并实际生成。
- SR-04：导入/恢复闭合成两阶段：候选 DB 提交后，配置写入、资源安装任一失败都回滚 DB（`target_backup`）、已写配置与已写资源；`restore` 中建立配置回滚副本失败也回滚已换 DB。错误文案不再声称“未修改现有数据”。

## 上游对照

- `work/.../ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:158` `CreateZipFileFromDirectory` 把 `Utils.GetConfigPath()` 整个配置目录 `CopyDirectory` 到 `guiConfigs/` 再打包；`:137` `FileUtils.ZipExtractToFile(fileName, toPath, "")` 把整目录解压回配置路径。资源随配置一起恢复（整目录语义）。
- 当前修复使原版 ZIP 导入的资源落盘与 `guiConfigs/` 布局一一对应：`guiConfigs/config/custom.json` → `<data>/config/custom.json`，与 `engine.rs:663` `custom_file_text` 的 `<data>/config/<Address>` 解析一致。

## 改动文件

- `crates/application/src/backup_service.rs`
  - `import_upstream` 改为两阶段：提交后调用 `activate_and_install`；失败返回带回滚说明的错误。
  - 新增 `activate_and_install`、`install_staged_resources`、`stage_upstream_resources`、`stage_file`、`sanitize_upstream_relative`、`copy_verified`、`with_rollback_note`、`StagedResource`。
  - `restore` 中 `backup_previous(config_path)` 失败分支补 `rollback_database`。
- `crates/application/tests/t16_backup.rs`：新增 4 个用例（见下）。
- `apps/desktop/lib/features/backup/backup_controller.dart`：`importUpstream` 失败文案由“导入失败（未修改现有数据）”改为“导入失败”，实际事实由错误 detail 承载。
- `apps/desktop/test/sr02_04_backup_import_test.dart`：新增导入失败文案用例。
- `docs/tasks/SR-02.md`、`docs/tasks/SR-04.md`、本目录。

## 实际命令与结果

```powershell
C:\Users\Colby\.cargo\bin\cargo.exe fmt -p application -- --check
# exit 0

C:\Users\Colby\.cargo\bin\cargo.exe clippy -p application --tests --locked -- -D warnings
# Finished `dev` profile; 无警告

C:\Users\Colby\.cargo\bin\cargo.exe test -p application --test t16_backup --test t16_webdav --test t11_routing_dns --locked
# t11_routing_dns 20 passed / 0 failed
# t16_backup      13 passed / 0 failed
# t16_webdav       7 passed / 0 failed

C:\Users\Colby\toolchains\flutter\bin\dart.bat format test/sr02_04_backup_import_test.dart lib/features/backup/backup_controller.dart
# 已格式化

C:\Users\Colby\toolchains\flutter\bin\dart.bat analyze lib/features/backup/backup_controller.dart test/sr02_04_backup_import_test.dart
# No issues found!

C:\Users\Colby\toolchains\flutter\bin\flutter.bat test test/sr02_04_backup_import_test.dart
# 1 passed / 0 failed

C:\Users\Colby\toolchains\flutter\bin\flutter.bat test test/fix14b_backup_restore_test.dart test/fix14_backup_activate_test.dart test/fix14c_webdav_backup_test.dart
# 12 passed / 0 failed
```

### 新增 Rust 用例

- `upstream_import_installs_resources_and_generates_file_node`：合成原版 ZIP（`guiConfigs/config/custom.json`、`pac.txt`、`scripts/check.js`）→ import → live 文件存在且哈希一致 → `AppEngine::open` 重开 → `build_codegen_input` 解析出 `custom-out`（证明 `custom_file_text` 读到 live 文件并实际生成）。
- `config_activation_failure_rolls_back_committed_import`：把 live `guiNConfig.json` 换成目录使配置写入失败 → DB 回滚为导入前行、资源未安装。
- `resource_install_failure_rolls_back_config_and_database`：`config` 处放普通文件使嵌套资源复制失败 → DB 回滚、配置文本恢复导入前。
- `restore_prior_config_snapshot_failure_rolls_back_database`：`guiNConfig.json.restore-prev` 换成目录使建立配置回滚副本失败 → 已换 DB 回滚、config 不变。

### 新增 Dart 用例

- `sr02_04_backup_import_test.dart`：注入导入失败（detail 含回滚说明），断言不再出现“未修改现有数据”，出现“导入失败”与回滚说明。

## 未完成 / 边界

- 未在真实 Windows 窗口 + 真实 FRB 组合下运行原版 ZIP 导入与资源型节点实机生成；`build_codegen_input` 为合成断言。
- 回滚为尽力而为（文件系统操作）；`with_rollback_note` 在回滚路径上统一附加说明，未逐项校验回滚后的字节。
- `apps/desktop/lib/features/backup/backup_and_restore_view.dart` 未改（文案由 controller 状态承载）。
- `crates/persistence/src/{upstream_db.rs,candidate.rs}` 未改：资源枚举在 application 层直接读取源目录/ZIP，未复用 persistence 的识别结构。
- compat 台账本轮未追加行（避免未经读取即改结构）；建议后续在 `compat/features.yaml` 追加 SR-02/SR-04 的 evidence 行。
