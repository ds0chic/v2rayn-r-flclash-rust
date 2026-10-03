# FIX-12 — 更新选定内核→应用该版本（内核侧安装布局与签名管线）

状态：`implemented`（内核安装布局、runtime 解析、PGP 下载管线接入在本机真实 loopback 下载 + 合成归档下验证通过；应用自身替换/重启与检查窗口偏好持久化未完成，见文末缺口）。

任务 ID：FIX-12

本次唯一用户流程：在检查更新窗口选定内核 → 检查 → 应用该版本；应用后 runtime 实际启动的就是刚下载的那个版本。应用自身升级为另卡（登记 FIX-12B），本卡只做内核更新与应用/核心分流。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `59dfa97`。审查结论见 `docs/evidence/parity-review-2026-10-03/`：`repair-queue.md` FIX-12 行、`runtime-report.md` RT-04/RT-15、`settings-report.md` SET-20、`root-report.md` ROOT-05。

对应 feature / field / action / layout ID：`ACT-UPDATE-001`/`ACT-UPDATE-002`/`ACT-UPDATE-003`/`ACT-UPDATE-004`、`ACT-WIN-005`/`ACT-WIN-006`、`ENUM-002`（v2rayN=99 为应用标识）、`FLD-CFG-147/148/149`（CheckUpdateItem）。

必读上游文件、符号和固定 commit（只读）：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/Common/Utils.cs:1209`（`GetBinPath`：`<startup>/bin/<coreTypeLower>`）、`Utils.cs:1306`（`GetExeName` 追加 `.exe`）、`ServiceLib/Manager/CoreInfoManager.cs:32`（`GetCoreExecFile`）、`Manager/CoreInfoManager.cs:53/75/87`（`GetCheckUpdateCoreTypes`/`IsCheckUpdateSupported`/`GetCheckPreRelease`）、`Manager/CoreManager.cs:26/53`、`App/Updater`（`AmazTool` 外部替换、签名与来源）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：检查更新窗口选中的内核（`sing_box`/`xray`/`mihomo`）、prerelease、viaProxy；应用时复用检查结果的 asset/url/version/dgst。
- 输出：原子替换 `<cores_root>/<dir>/<version>/`，保留 `<dir>.previous`；`CoreInstallLayout` 解析安装根/目录名/版本目录/exe。
- 错误：`v2rayN` 走内核应用链时拒绝（`error.update_app_not_core`）；应用资产缺签名/验签失败在暂存阶段失败关闭。
- 取消：下载走 `CancellationToken`（沿用既有）。
- 权限：仅写 `<data>/cores`；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：`install-manifest.json` 记录版本；版本目录名即版本。
- 生效：runtime `CoreLocator` 解析到刚安装的版本目录。

允许修改的模块：`crates/runtime/src/{lib.rs,adapter.rs,install_layout.rs}`、`crates/application/src/{lib.rs,update_service.rs}`、`crates/bridge_api/src/api/t16.rs`、`crates/application/tests/t16_update.rs`、`docs/evidence/UX-PARITY-FIX-12/**`、本卡。未改 `updater` 内部（其 `InstallPlan` 本就路径无关），未改任何 FRB 生成文件。

禁止改变的已有行为：`main_shell.dart`、两处 `frb_generated`、`features/{subs,profiles,monitor}/**`、`engine.rs`、`crates/subscriptions/**`；不删入口/降分母。

冻结接口（先定义类型再实现）：`runtime::CoreInstallLayout`
- 安装根 `root()`；
- 核心目录名 `dir_name(CoreType)` / `dir_name_str(&str)`（`sing_box`/`sing-box` → `singbox`，全项目一处）；
- 版本目录 `version_dir(_str)`；
- exe 名 `exe_name(_str)`；
- 解析 `resolve_exe(_str)` 与最高版本 `latest_version(_str)`（数值化 `version_key`，`v` 前缀可选，忽略 `.previous`/`.staging`）。
`UpdateService` 与 `CoreLocator` 均只经该类型构造路径。

测试夹具和原版预期：`crates/application/tests/t16_update.rs` 的 loopback GitHub mock（端口 OS 分配，恒非 10808）+ 合成 `Xray-windows-64.zip`（最小 PE）；不下载真实 GitHub、不连接外网。原版预期：目录名与 exe 名由单一解析函数决定，更新后启动新版本。

本次必须通过的命令/真实场景：
- `cargo fmt -p runtime -p application -p bridge_api -- --check`（对改动文件）
- `cargo clippy -p runtime -p application -p bridge_api --locked -- -D warnings`
- `cargo test -p runtime --lib --locked`
- `cargo test -p application --lib update_service --locked`
- `cargo test -p application --test t16_update --locked`（真实 loopback 下载→安装→runtime 解析→回滚）
- `cargo test -p updater --locked`
- Flutter：本卡未改 Dart，无 Flutter 命令。

证据文件位置：`docs/evidence/UX-PARITY-FIX-12/`（`README.md`、`observations.json`、`cargo-test.log`）。

完成条件与未完成项：
- 已实现并验证：RT-04 安装布局统一（acceptance 1）；更新后 runtime 解析到新版本（acceptance 2，loopback 真实下载）；应用/核心分流拒绝 `v2rayN` 入 cores（acceptance 3 的护栏）；PGP 验证器接入应用暂存管线并验错签/缺签（acceptance 5，`enforce_detached_signature` + 真实 PGP 后端由 `signature.rs` 既有测试覆盖）。
- 未完成（登记）：acceptance 3 的“应用自身发行来源/安装根/runner/退出替换重启接线”属 FIX-12B；acceptance 4（检查窗口稳定/预览/代理/选中项保存并重开恢复、后台每日检查点亮更新按钮）需新增 FRB 函数（`CheckUpdateItem` 读写）与 Dart 侧接线，属 FRB 重生成后由根代理处理。

发现接口缺口时的处理：
- 接口缺口（登记 FIX-12B）：应用自身更新缺少真正的发行来源/sig 资产选择、install_root 与 `v2rayN-upgrade.exe` runner 的退出替换重启接线；当前 `app_update_spec_verified` 仅暂存+验签并返回 spec，不执行替换。
- 接口缺口（登记）：SET-20 的 `CheckUpdateItem`（`CheckPreReleaseUpdate`/`UpdateViaProxy`/`SelectedCoreTypes`）无保存/加载 FRB 通道，`update_controller.build` 仍返回内存默认；需新增 FRB 函数后由根代理重生成，Dart 侧再接。
- 接口缺口（登记）：`UpdateService::install_root` 默认取 `cores_root.parent()`，对应用升级只是近似；FIX-12B 需显式传入应用安装根。

本轮实际结果：
- 新增 `crates/runtime/src/install_layout.rs`（`CoreInstallLayout` + `version_key` + 测试）；`runtime::adapter::core_dir` 与 `CoreLocator::resolve` 改为经该类型（版本按数值比较，替换旧的字符串排序）。
- `crates/application/src/update_service.rs`：`core_dir_name`/`installed_version`/`installed_cores` 经 `CoreInstallLayout`；新增 `install_core_from_dir`（把解包树包进 `<dir>/<version>/` 后原子替换）；`apply_core` 拒绝 `v2rayN`；`CoreUpdateCheck` 增 `sig_url`；新增 `app_update_spec_verified`/`app_signature_verifier`/`enforce_detached_signature`/`download_signature`。
- `crates/bridge_api/src/api/t16.rs`：`t16_apply_core_update` 跳过 `v2rayN`；`t16_apply_app_update_spec` 用捆绑上游 OpenPGP 信任根调用 `app_update_spec_verified`。
- `crates/application/tests/t16_update.rs`：断言改为版本目录布局并新增 runtime 解析断言。
- 命令结果：`runtime` lib 41 passed；`application --lib update_service` 5 passed；`application --test t16_update` 3 passed（含真实 loopback 下载）；`updater` 全绿；`runtime/application/bridge_api` clippy `-D warnings` 通过；改动文件 rustfmt `--check` 通过。
