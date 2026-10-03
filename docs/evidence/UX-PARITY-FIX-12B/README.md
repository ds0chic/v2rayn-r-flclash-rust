# UX-PARITY-FIX-12B evidence — 应用自身升级：退出→替换→重启接线

场景：应用自身升级 检查→下载→验签→退出替换→重启。本卡只接线隔离目录下的 stage→替换（含 `.previous` 回滚）→重启命令构造，真实重启由外部 runner（`v2rayN-upgrade.exe`）执行，测试用 stub runner，不启动任何进程。

- 基线：HEAD `da7df0d`（工作树含根代理桥接改动，未回退）。
- 关键改动：`crates/updater/src/app_upgrade.rs`（新增）、`crates/updater/src/lib.rs`、`crates/application/src/update_service.rs`、`crates/bridge_api/src/api/t16.rs`、`apps/desktop/lib/features/update/update_controller.dart`、`apps/desktop/test/t16_update_test.dart`、`crates/updater/tests/app_upgrade.rs`、`crates/application/tests/t16_update.rs`。
- 证据文件：`observations.json`（结构化）、`cargo-test.log`、`flutter-test.log`（原始输出）。

## 验收对照

1. 发行来源/安装根/runner 一致（acceptance 1）：
   - `UpdateService` 增显式 `install_root`（`with_app_install_root`）；桥接 `t16.rs` 取 `current_exe` 父目录或 `V2RAYN_R_APP_ROOT`，不再用 `cores_root.parent()` 近似。
   - 复用 FIX-12 冻结的 `updater::install::apply_atomic`/`UpgradeCoordinator` 与 PGP 语义（`enforce_detached_signature` 未改）。
   - runner = `install_root/v2rayN-upgrade.exe`（`AppInstallLayout::runner_exe`），`AppInstallLayout::coordinator()` 把 staged source 交给它。
   - 应用/核心分流不变：`apply_core("v2rayN")` 仍拒 `error.update_app_not_core`。
   - 原版 WPF 包护栏：`apply_app_upgrade` 经 `verify_payload` 要求载荷含当前应用 exe 名；合成 `v2rayN.exe` 包被 `InstallConflict` 拒绝。证据：`updater/tests/app_upgrade.rs::synthetic_wpf_zip_is_rejected`、`t16_update.rs::app_update_rejects_wpf_payload_for_flutter_binary`。
2. 退出替换重启（acceptance 2）：隔离 `app_root` + 合成 zip，`safe_unpack_zip` stage → `apply_app_upgrade`（原子替换，保留 `app.previous`）→ `rollback_app_upgrade` 恢复旧载荷 → `restart_command` 指向 `app/<exe>`，工作目录 `app/`。stub runner 文件仅作为存在性标记，未被创建/执行。证据：`updater/tests/app_upgrade.rs::synthetic_zip_stage_replace_rollback_restart`、`t16_update.rs::app_update_stage_replace_rollback_restart_isolated`。
3. 真实远端发行源（acceptance 3，`blocked`）：本 build 未配置自己的发布仓库；`UpdateService.app_repo` 默认 `None`，`check_app_update` 报 `error.update_app_source_unconfigured`（`E_UNAVAILABLE`）。未伪造任何 GitHub 端点。证据：`t16_update.rs::app_update_requires_configured_source`、`t16_update_test.dart::unconfigured application source is reported as blocked`。
4. 不改生成文件（acceptance 4）：未改两处 `frb_generated`、`bridge_api/src/api/contract.rs`；新增 FRB 需求登记在任务卡。
5. 不改 FIX-12 内核语义：`CoreInstallLayout`、`install_core_from_dir`、`enforce_detached_signature` 未改；`check_core` 仅抽出 `check_core_inner` 支持 repo 覆盖。

## 命令

见 `cargo-test.log` 与 `flutter-test.log`。未跑全量 workspace 测试，未跑 `flutter build windows`。未改宿主系统代理/注册表/路由/TUN，未占用 10808；合成下载走 loopback mock（`127.0.0.1:0`），应用替换走临时隔离目录。

## 上游对照

- 上游 `CheckUpdateViewModel.UpgradeN`（`:332`）：取下载文件名 → `Utils.UpgradeAppExists` → `ProcUtils.ProcessStart(upgradeFileName, fileName, StartupPath)` → `AppExitAsync(true)`。本卡把“取文件名/存在检查/启动 runner/退出”冻结为 `AppInstallLayout` + `ExternalUpgradeSpec` + `AppRestartCommand`；实际 spawn/退出仍由外部 runner 路径承担（本卡不真实重启宿主）。
- 上游 `AmazTool/UpgradeApp.Upgrade`：`Waiting(5)` → 结束旧进程 → 解压时把当前 exe `File.Move` 到 `.tmp` → `Waiting(2)` → `Utils.StartV2RayN()`。本项目用 `apply_atomic` 的目录交换 + `app.previous` 取代逐文件覆盖，回滚语义更明确；`StartV2RayN` 对应 `AppRestartCommand{program: app/<exe>, working_dir: app/}`（上游 `FileName=v2rayN`、`WorkingDirectory=StartupPath`）。
- 上游应用更新从 `2dust/v2rayN` 取包并验签；本项目为独立 Flutter 产物，发行源未定，故 `app_repo` 默认空并以 `with_app_repo`/`V2RAYN_R_APP_REPO` 接入真实源，不硬编码伪造端点。
