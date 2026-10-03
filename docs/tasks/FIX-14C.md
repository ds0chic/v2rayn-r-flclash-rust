# FIX-14C — WebDAV 远端备份上传/恢复：上游 `backup.zip` 布局互通

状态：`implemented`（本地 loopback mock 已覆盖 PUT/GET/PROPFIND/MKCOL、Basic 认证、上游 `guiConfigs/` 布局识别、401/404/500/超时分支与恢复路由；真实 TLS 远端端点未实测，登记 `blocked`，故不写 `verified`）。

任务 ID：FIX-14C

本次唯一用户流程：WebDAV 远端备份上传/恢复：远端备份 → 远端出现 `backup.zip` → 远端恢复 → 重开使用。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；FIX-14（本地导入/恢复 quiesce→交换→reopen）与 FIX-14B（本地 `backup_*.zip` 选择器）已提交；`docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 39 行「当前备份恢复/WebDAV另卡」及 FIX-14 卡登记的 FIX-14C 缺口。开始应用 HEAD `a93949a`。

对应 feature / action / layout ID：`F-BACKUP-003`、`ACT-WIN-007`、`LAY-BACKUP-001`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
- `v2rayN/ServiceLib/ViewModels/BackupAndRestoreViewModel.cs:6`（`BackupFileName => backup_{yyyyMMddHHmmss}.zip`）、`:60/63/67`（`RemoteBackup`：`CreateZipFileFromDirectory`→`WebDavManager.PutFile`）、`:78/82/85`（`RemoteRestore`：`GetRawFile`→`LocalRestore`）、`:158`（`CreateZipFileFromDirectory` 把配置目录拷进临时 `guiConfigs/` 再整目录 ZIP）；
- `v2rayN/ServiceLib/Manager/WebDavManager.cs:13/14`（`_webDir = Global.AppName + "_backup"`、`_webFileName = "backup.zip"`）、`:98`（`CheckConnection`：MKCOL → PUT/DELETE `readme_test`）、`:128`（`PutFile` PUT 到 `{_webDir}/backup.zip`）、`:154`（`GetRawFile` GET `{_webDir}/backup.zip`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`WebDavItem`（Url/UserName/Password/DirName，目录默认 `v2rayN_backup`）；项目数据目录（`guiNDB.db` + `guiNConfig.json` + 子目录资源）。
- 输出：远端固定 `{dir}/backup.zip`（上游 `guiConfigs/` 布局 ZIP）；`WebDavOpDto{ok,bytes,message}`；恢复 `RestoreResultDto`。
- 错误：URL 空/非 http(s) 在客户端构造前拒绝；401/403→`E_PERMISSION_DENIED`；404→`E_NOT_FOUND`；其它 5xx→`E_UNAVAILABLE`（retryable）；超时→`E_TIMEOUT`；PUT 非 2xx 不算成功、报结构化错误；远端 ZIP 非法在写库前拒绝。
- 取消：窗口关闭即取消；上传/下载为异步桥接调用，无半提交。
- 权限：仅本机 UI + FRB/Rust；不启动内核、不写系统代理/TUN、不监听端口、不触碰 127.0.0.1:10808。
- 持久化：上传使用 T16 一致性 bundle 快照（`create_local`）重打包，不改本地配置；恢复经候选库/激活写盘。
- 生效：上传成功后 UI 立刻回列远端（`webdav_list`）显示文件出现；恢复成功后 `_refreshAfterRestore`，上游布局走 quiesce→候选+激活→reopen，项目 bundle 走 quiesce→交换→reopen。

允许修改并实际修改的模块：`crates/application/src/backup_service.rs`（新增 `zip_upstream_layout`、`UPSTREAM_GUI_CONFIGS`）、`crates/application/src/lib.rs`（导出追加）、`crates/bridge_api/src/api/t16.rs`（`t16_webdav_backup` 改上游布局并先确保目录；`t16_webdav_restore` 按识别结果路由并包裹 quiesce/reopen）、`crates/application/tests/t16_webdav.rs`、`apps/desktop/lib/features/backup/backup_controller.dart`、`apps/desktop/test/fix14c_webdav_backup_test.dart`、`docs/evidence/UX-PARITY-FIX-14C/**`、本卡、`compat/features.yaml`（仅 F-BACKUP-003 追加 notes）。未新增 FRB 函数，未改 `frb_generated`。

禁止改变的已有行为：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/{settings,profiles,subs,runtime,monitor,update,routing}/**`、`crates/application/src/engine.rs`、`crates/updater/**`；FIX-14/14B 已提交的本地恢复语义（`t16_backup_restore`/`t16_backup_import_upstream`/本地选择器）未改。

测试夹具和原版预期：本地 loopback mock WebDAV（端口在 `11808..11900` 逐个探测绑定，绝不使用 10808）；合成数据目录（`guiNConfig.json`、`guiNDB.db`、嵌套 `custom/sub/node.json`、`.work/` 临时文件）；合成凭据。原版预期：整目录 `guiConfigs/` ZIP、固定远端 `backup.zip`、PUT 前 MKCOL、GET 固定文件、认证头 Basic；丢失文件/无权限/超时明确报错。

本次必须通过的命令/真实场景：
- `cargo fmt -p application -p bridge_api`；`cargo fmt -p application -p bridge_api -- --check`
- `cargo test -p application --test t16_webdav --locked`；`cargo test -p application --lib --locked`；`cargo test -p application --test t16_backup --locked`
- `cargo test -p bridge_api --locked`
- `cargo clippy -p application -p bridge_api --locked --lib`；`cargo clippy -p application --locked --test t16_webdav`
- `dart format --output=none --set-exit-if-changed lib/features/backup test/fix14c_webdav_backup_test.dart`；`flutter analyze`；`flutter test test/fix14c_webdav_backup_test.dart test/t16_backup_test.dart test/fix14b_backup_restore_test.dart test/fix14_backup_activate_test.dart`
- 真实 TLS 端点：`blocked`（无凭据、硬约束禁止读用户秘密；需要真实 WebDAV 服务与合成账号）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-14C/README.md`、`observations.json`。

完成条件：远端 `backup.zip` 为上游 `guiConfigs/` 布局且可被原版识别恢复；上传先 MKCOL/PROPFIND 且 PUT 成功才报成功；恢复走 FIX-14 的 quiesce→交换/激活→reopen；401/404/超时/5xx 分支明确；凭据不落日志/证据；本地恢复语义未改。合成测试全绿。真实 TLS 未测故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 阻塞（登记）：真实远端 TLS WebDAV 未实测（无授权端点，且不得发送用户凭据）。建议根代理在隔离环境用合成账号复测一次真实 TLS PUT/GET。
- 接口缺口（登记）：`webdav.rs` 的 `parse_propfind` 仅解析 `D:`/无前缀 multistatus；若真实服务器返回其它命名空间前缀需扩展解析（本卡未造）。
- 接口缺口（登记）：上游 `CheckConnection` 会 PUT/DELETE `readme_test` 写权限探测；本项目 `check` 以 PROPFIND/MKCOL 代替，未做写探测（上传本身即写探测）。如需与上游逐事件一致，另卡。

本轮实际结果：新增 `BackupService::zip_upstream_layout`（T16 快照 → `guiConfigs/` 前缀 ZIP，丢弃 `manifest.json`）；`t16_webdav_backup` 改为上传上游布局并在 PUT 前 `check()`（PROPFIND/MKCOL）；`t16_webdav_restore` 下载后识别：上游布局走 `import_upstream`（quiesce→激活→reopen），项目 bundle 走 `restore`（quiesce→交换→reopen）；Flutter 控制器新增 WebDAV 错误分支映射、上传成功回列远端、远程恢复后重载。`t16_webdav` 7/7、`application --lib` 182/182、`t16_backup` 9/9、`bridge_api` 43/43、`fix14c_webdav_backup_test` 4/4、`t16_backup_test` 6/6、`fix14b_backup_restore_test` 5/5、`fix14_backup_activate_test` 3/3（首跑偶发 did not complete，重跑全绿）全绿；`flutter analyze` No issues；Rust fmt check 0 差异；clippy 本卡 0 warning（`bridge_api` 既有 `subs.rs` unused import 非本卡）。