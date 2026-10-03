# FIX-12B — 应用自身升级：退出→替换→重启接线（发行来源/安装根/runner）

状态：`implemented`（应用安装布局、显式安装根、check_app_update 发行来源护栏、原子替换+.previous 回滚、重启命令构造、原版 WPF 包拒绝，均在隔离目录 + 合成 zip 下验证；真实远端发行源未配置，`blocked` 登记）。

任务 ID：FIX-12B（来自 `docs/tasks/FIX-12.md` 文末接口缺口，及 `docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 37 行“自身升级另卡”）。

本次唯一用户流程：应用自身升级：检查→下载→验签→退出替换→重启。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；启动应用 HEAD `da7df0d`（工作树含根代理桥接改动 `customImportFile` 等，未回退）。FIX-12 已冻结 `runtime::CoreInstallLayout`、`updater::install::UpgradeCoordinator`/`apply_atomic`/`restore_previous` 与 `updater::signature::v2rayn_app_verifier`（PGP）。审查结论见 `repair-queue.md` FIX-12 行、`root-report.md` ROOT-05、`settings-report.md` SET-20、`runtime-report.md` RT-04/15。上游基准：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `AmazTool/UpgradeApp.cs`、`AmazTool/Utils.cs`（`StartV2RayN`/`Waiting`/`GetExePath`/`StartupPath`）、`ServiceLib/ViewModels/CheckUpdateViewModel.cs:332`（`UpgradeN`）、`ServiceLib/Common/ProcUtils.cs:12`（`ProcessStart helper fileName startupPath`）、`ServiceLib/Common/Utils.cs:890`（`UpgradeAppExists`）。

对应 feature / field / action / layout ID：`F-CORE-003`、`ACT-UPDATE-001`/`ACT-UPDATE-002`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：应用发行源 slug（`UpdateService.app_repo`，本 build 默认未配置）、显式应用安装根（`install_root`，桥接取当前进程目录或 `V2RAYN_R_APP_ROOT`）、签名 URL（`.sig`）、已下载校验和的 asset/url/version。
- 输出：`AppInstallLayout`（安装根/`app` 载荷目录/`app.previous`/runner `v2rayN-upgrade.exe`）；`AppUpgradeOutcome{app_exe, kept_previous, restart}`；`AppRestartCommand{program,args,working_dir}`。
- 错误：未配置发行源 → `error.update_app_source_unconfigured`（`E_UNAVAILABLE`，retryable=false）；载荷不含当前应用 exe 名 → `UpdateError::InstallConflict`（拒绝原版 WPF 包 `v2rayN.exe`）；spec 安装根不匹配 → `error.update_app_install_root`；缺签/错签沿用 FIX-12 `enforce_detached_signature`（`error.update_signature_missing` / `SignatureInvalid`）。
- 取消：下载沿用 `CancellationToken`。
- 权限：仅写显式应用安装根的 `app/`、`app.previous`、`.staging/`；不启动进程、不写系统代理/TUN、不监听端口。
- 持久化：替换返回 `InstallManifest`（内存结构，未落盘）。
- 生效：重启命令指向 `app/<exe>`，工作目录 `app/`（对应上游 `StartV2RayN` 的 `FileName=v2rayN`、`WorkingDirectory=StartupPath`）。

允许修改的模块：`crates/updater/**`、`crates/application/src/update_service.rs`、`crates/bridge_api/src/api/t16.rs`、`apps/desktop/lib/features/update/**`、`apps/desktop/test/**`、`docs/evidence/UX-PARITY-FIX-12B/**`、本卡、`compat/features.yaml`（仅追加证据/测试）。

禁止改变的已有行为：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`features/{settings,profiles,subs,runtime,monitor,backup,routing}/**`、`engine.rs`、`crates/application/src/{dns,routing,speedtest,subs,monitor}.rs`、`crates/subscriptions/**`；FIX-12 已提交的内核更新布局/验签语义（`CoreInstallLayout`、`install_core_from_dir`、`enforce_detached_signature`）不改。

本轮实际结果：
- 新增 `crates/updater/src/app_upgrade.rs`：冻结 `AppInstallLayout`（`install_root`/`app`/`app.previous`/`v2rayN-upgrade.exe`）、`apply_app_upgrade`（经 `install.rs::apply_atomic`，保留 `.previous`）、`rollback_app_upgrade`、`verify_payload`（载荷须含当前应用 exe，拒绝 WPF 包）、`AppRestartCommand`。`lib.rs` 增模块与再导出。
- `update_service.rs`：`UpdateService` 增显式 `install_root`/`app_repo`/`app_exe_name`/`runner_name` 与 `with_app_install_root`/`with_app_exe_name`/`with_app_repo`；新增 `check_app_update`（未配置源报 `error.update_app_source_unconfigured`）、`app_layout`、`apply_app_upgrade`、`rollback_app_upgrade`、`app_restart_command`；`check_core` 抽为 `check_core_inner` 支持 repo 覆盖；应用暂存改到 `install_root/.staging`；`APP_REPO_ENV`/`test_app_repo_override`。
- `t16.rs`：`update_service()` 显式设置应用安装根（`current_exe` 父目录或 `V2RAYN_R_APP_ROOT`）；`t16_apply_app_update_spec` 改用 `check_app_update` 与 `app_layout().runner_exe()`。
- `update_controller.dart`：应用升级状态文案改为退出后由外部 runner 替换并重启；未配置源显示“应用自身发行源未配置”。
- 测试：`crates/updater/tests/app_upgrade.rs`（合成 zip：stage→replace→`app.previous`→rollback→restart；WPF zip 拒绝）；`crates/application/tests/t16_update.rs` 新增 `app_update_requires_configured_source`、`app_update_stage_replace_rollback_restart_isolated`、`app_update_rejects_wpf_payload_for_flutter_binary`；`apps/desktop/test/t16_update_test.dart` 更新文案并新增未配置源错误用例。

本次必须通过的命令/真实场景：
- `cargo fmt -p updater -p application -p bridge_api -- --check`
- `cargo clippy -p updater -p application -p bridge_api --all-targets --locked -- -D warnings`
- `cargo test -p updater --locked`
- `cargo test -p application --lib update_service --locked`
- `cargo test -p application --test t16_update --locked`
- `dart format --output=none --set-exit-if-changed lib/features/update test/t16_update_test.dart`
- `flutter test test/t16_update_test.dart`

证据文件位置：`docs/evidence/UX-PARITY-FIX-12B/`（`README.md`、`observations.json`、`cargo-test.log`、`flutter-test.log`）。

完成条件与未完成项：
- 已实现并验证：acceptance 1（显式安装根 + `CoreInstallLayout` PGP 语义不变 + 应用/核心分流 + 载荷 exe 名护栏拒 WPF 包）；acceptance 2（隔离目录 + 合成 zip 的 stage→替换→`.previous` 回滚→重启命令构造，未执行真实 runner）；acceptance 4（未改任何 FRB 生成文件）。
- `blocked`（acceptance 3）：真实远端应用发行源未配置，本 build 无自己的发布仓库，因此 `check_app_update` 默认报 `error.update_app_source_unconfigured`；未伪造 GitHub 端点。待真实发布仓库与签名资产确定后，用 `with_app_repo`/`V2RAYN_R_APP_REPO` 接入即可，无需改布局/验签。
- 接口缺口（登记）：`ExternalSpecDto` 未携带“重启命令”；Dart 端不需要主动 spawn（由外部 runner 执行），但若要显示重启目标/在桥接层发起 runner，需新增 FRB 函数与 DTO（`bridge_api/src/api/contract.rs`），本卡不改生成文件，登记后由根代理重生成。
- 未完成（沿用 FIX-12 登记）：SET-20 的 `CheckUpdateItem`（`CheckPreReleaseUpdate`/`UpdateViaProxy`/`SelectedCoreTypes`）保存/加载 FRB 通道与后台每日检查点亮按钮仍需 FRB 重生成。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。
