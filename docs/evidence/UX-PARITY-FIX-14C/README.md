# UX-PARITY-FIX-14C 证据

任务卡：`docs/tasks/FIX-14C.md`。唯一流程：WebDAV 远端备份上传/恢复：上传 → 远端出现 `backup.zip` → 恢复 → 重开使用。上游布局互通；上传成功才报成功；恢复走 FIX-14 的 quiesce→交换/激活→reopen；凭据不落日志/证据。

## 改动文件（本卡）

- `crates/application/src/backup_service.rs` — 新增 `UPSTREAM_GUI_CONFIGS` 与 `zip_upstream_layout`（T16 一致性快照 → 单个 `guiConfigs/` 根的 ZIP；丢弃 `manifest.json`、跳过 `.work`）。
- `crates/application/src/lib.rs` — 导出追加（仅追加）。
- `crates/bridge_api/src/api/t16.rs` — `t16_webdav_backup` 上传上游布局并在 PUT 前 `check()`（PROPFIND/MKCOL）；`t16_webdav_restore` 按 `recognize` 结果分流：上游布局 `import_upstream`（quiesce→候选+激活→reopen），项目 bundle `restore`（quiesce→交换→reopen）。
- `crates/application/tests/t16_webdav.rs` — mock 服务器改为在 `11808..11900` 逐个探测绑定；新增 MKCOL、404、500、上游布局上传/下载/识别用例。
- `apps/desktop/lib/features/backup/backup_controller.dart` — WebDAV 错误分支映射（401/403、404、超时、不可达）；上传成功回列远端；远程恢复后重载引擎设置。
- `apps/desktop/test/fix14c_webdav_backup_test.dart` — 新增上传成功/失败、恢复失败/成功与凭据不回显窗口测试。
- `compat/features.yaml` — F-BACKUP-003 追加 `notes`（未删行）。
- `docs/tasks/FIX-14C.md`、本目录。

未改：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/{settings,profiles,subs,runtime,monitor,update,routing}/**`、`crates/application/src/engine.rs`、`crates/updater/**`；FIX-14/14B 本地恢复入口与语义未改。

## 上游对照结论

| 上游（冻结 commit） | 本项目实现 |
|---|---|
| `BackupAndRestoreViewModel.cs:158` `CreateZipFileFromDirectory`：配置目录拷入临时 `guiConfigs/`，整目录 ZIP | `BackupService::zip_upstream_layout`：T16 快照的 DB/config/递归资源全部放入 `guiConfigs/` 前缀，`manifest.json` 丢弃 |
| `WebDavManager.cs:14` 远端固定 `backup.zip`；`:128` PUT `{dir}/backup.zip` | `webdav.rs` `backup_url()` = `{url}/{dir}/backup.zip`，`upload` PUT 同一路径 |
| `WebDavManager.cs:64/134` PUT 前 MKCOL 确保目录 | `t16_webdav_backup` PUT 前 `check()`：PROPFIND，404 时 MKCOL（405 视为已存在） |
| `WebDavManager.cs:154/164` GET `{dir}/backup.zip` 后本地恢复 | `t16_webdav_restore` GET 固定文件 → 识别上游布局 → FIX-14 候选+激活（quiesce→reopen） |
| `WebDavManager.cs:108` 写权限探测 PUT/DELETE `readme_test` | 未做（上传本身即写探测）；登记为缺口 |
| `BackupAndRestoreViewModel.cs:6` 本地文件 `backup_{yyyyMMddHHmmss}.zip` | 远端名固定 `backup.zip`（与上游一致）；本地命名属 FIX-14B，不在本卡 |

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `cargo fmt -p application -p bridge_api` / `-- --check` | ok / 0 差异 |
| `cargo test -p application --test t16_webdav --locked` | 7 passed / 0 failed |
| `cargo test -p application --lib --locked` | 182 passed / 0 failed |
| `cargo test -p application --test t16_backup --locked` | 9 passed / 0 failed |
| `cargo test -p bridge_api --locked` | 43 passed / 0 failed |
| `cargo clippy -p application -p bridge_api --locked --lib` | 本卡 0 warning（`subs.rs` 既有 unused import 非本卡） |
| `cargo clippy -p application --locked --test t16_webdav` | 0 warning |
| `dart format --output=none --set-exit-if-changed lib/features/backup test/fix14c_webdav_backup_test.dart` | 0 changed |
| `flutter analyze` | No issues found |
| `flutter test test/fix14c_webdav_backup_test.dart` | 4 passed |
| `flutter test test/t16_backup_test.dart` | 6 passed |
| `flutter test test/fix14b_backup_restore_test.dart` | 5 passed |
| `flutter test test/fix14_backup_activate_test.dart` | 3 passed（首跑 did not complete，重跑全绿） |

## 安全与硬约束

- mock 服务器端口在 `11808..11900` 逐个探测绑定（`bind_loopback`），测试不触碰 127.0.0.1:10808。
- 凭据一律合成（`user` / `secret-value-not-logged`）；错误只含 code/messageKey/HTTP 状态，断言不含秘密。
- 未改宿主系统代理/注册表/路由/TUN；未启动内核；未读写用户凭据。

## 未完成 / 接口缺口（含 blocked）

- `blocked`：真实 TLS WebDAV 端点未实测（无授权端点，且不得发送用户凭据）。建议根代理在隔离环境用合成账号复测真实 TLS PUT/GET。
- `identified`：`parse_propfind` 仅解析 `D:`/无前缀 multistatus，其它命名空间前缀需扩展。
- `identified`：上游 `CheckConnection` 的 PUT/DELETE `readme_test` 写探测未复刻（以 PUT 本身代替）。