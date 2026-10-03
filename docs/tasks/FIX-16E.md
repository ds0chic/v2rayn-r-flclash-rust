# FIX-16E — 根证书来源（RootCertProvider）信任源与证书存储平台效果

状态：`implemented`（provider 归一化/信任源解析、`certutil` 命令预览与假存储已单测/桥接测试通过；本机 `CurrentUser\ROOT` 合成自签证书写读复原隔离实测通过并复位；下载客户端的实际接线、chrome/mozilla 内置 PEM 资源与 `LocalMachine` 提权写入登记为后续/blocked，不伪造）。

任务 ID：FIX-16E（来源 `docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 41/43 行 FIX-16 字段组「HWA/证书来源」；`docs/tasks/FIX-16.md` 后续卡表第 66 行）。

本次唯一用户流程：打开「参数设置」→「显示」页 → 「根证书来源 (RootCertProvider)」选择 `system / chrome / mozilla` →「保存」→ 关闭重开窗口后该值从持久化文档恢复；运行期按所选来源解析应用自身（下载/网络请求）的信任锚：`system` = OS/rustls 原生根存储，`chrome/mozilla` = 内置 PEM 集合。证书存储写入是受控扩展，默认假后端，真机写入仅在隔离测试显式构造。

前置任务及已验证证据：FIX-16 已补齐 `RootCertProvider` 控件并持久化（`field-matrix.md`、`fix16_settings_field_test.dart`）；FIX-08 草稿保存语义不回退。本卡基准 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

对应 feature / field / action / layout ID：`FLD-CFG-064`（GuiItem.RootCertProvider）、`SET-16/17/19/20`、`ROOT-07/09`、`ResUI.TbRootCertificateProvider`。

必读上游文件、符号和固定 commit：
- `v2rayN/ServiceLib/Manager/CertPemManager.cs:329-357`（`BuildTrustedCertificateCollection` / `IsSystemRootCertProvider` / `BuildCertificateChainPolicy`）
- `v2rayN/ServiceLib/Global.cs:758-763`（`RootCertProviders = [system, ChromeRootProvider, MozillaRootProvider]`）
- `v2rayN/ServiceLib/Handler/ConfigHandler.cs:98-100`（越界值强制回落第一项 `system`）
- `v2rayN/ServiceLib/ViewModels/OptionSettingViewModel.cs:68/201/376`（属性绑定）
- `v2rayN/ServiceLib/Resx/ResUI.zh-Hans.resx:1832-1835`（“仅用于 v2rayN 界面程序的下载及网络请求，不影响核心的证书验证。”）
- `v2rayN/ServiceLib/Services/DownloadService.cs:171/267`、`Helper/DownloaderHelper.cs:294`（唯一实际消费者）

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`guiNConfig.json` 的 `GuiItem.RootCertProvider`（字符串）。
- 输出：桥接 `cert_provider_info` 返回归一化 provider + 信任源（`system` / `bundled` + bundle 文件名）；`cert_install_command` / `cert_remove_command` 返回 `certutil` 命令预览（不执行）。
- 错误：越界值不报错，按上游回落 `system` 并置 `normalized=true`（对齐 `ConfigHandler.LoadConfig`）；未知 scope 命令预览返回 `E_INVALID_ARGUMENT`。
- 取消：`取消` 丢弃草稿，沿用 FIX-08。
- 权限：provider 解析与命令构造为纯函数；假存储默认；真机 `CurrentUser\ROOT` 写入无需提权，`LocalMachine` 需提权（blocked）。
- 持久化：`guiNConfig.json`（上游 SQLite 同构树），保存时对越界字符串就地归一到 `system`（`settings_controller._normalizeRootCertProvider`）。
- 生效：运行期信任锚按来源解析（`platform::cert::trust_source`）；下载客户端接线属后续卡。

允许修改的模块：`crates/platform/**`（新增 `src/cert/{mod.rs,windows.rs}`、`tests/cert_store.rs`、`tests/fixtures/synthetic-root.cer`）、`crates/bridge_api/src/api/platform.rs`（加函数/DTO/测试，未改 `frb_generated`）、`apps/desktop/lib/features/settings/{settings_defaults.dart,settings_controller.dart}`、`apps/desktop/test/**`、`docs/evidence/UX-PARITY-FIX-16E/**`、`docs/evidence/UX-PARITY-FIX-16/field-matrix.md`、本卡。未改 `main_shell.dart`/`app.dart`/两处 `frb_generated`/`lib/bridge/api/**`/其它 settings 文件/`features/{profiles,subs,runtime,monitor,update,backup,routing}`/`crates/application|updater`。

禁止改变的已有行为：FIX-16/16B/16C/16D 已提交语义；FIX-08 草稿保存；`settings-apply/save` 键；不删入口、不降分母、不伪造平台效果。

测试夹具和原版预期：合成自签证书 `tests/fixtures/synthetic-root.cer`（DER 718B，SHA-256 `6435cc53fe1b4e23bf3714482da5777fba2d8089f3767bbd1645728ab8e458fa`，无密钥、非用户凭据）；`FakeCertificateStore`（内存）。原版预期：`system` 用原生根存储、`chrome/mozilla` 用内置 PEM；越界回落 `system`；无 `InstallCert` 行为。

本次必须通过的命令/真实场景（实际运行见「本轮实际结果」）：
- `cargo fmt -p platform -p bridge_api -- --check`
- `cargo clippy -p platform -p bridge_api --all-targets --locked -- -D warnings`
- `cargo test -p platform --lib --locked`、`cargo test -p platform --test cert_store --locked`
- 隔离真机：`V2RAYN_R_CERT_STORE_TEST=1 cargo test -p platform --test cert_store --locked -- --ignored --nocapture`
- `cargo test -p bridge_api --lib --locked cert_`
- `dart format --output=none --set-exit-if-changed`（3 个目标文件）、`flutter analyze`（3 个目标文件）、`flutter test test/fix16e_cert_provider_test.dart`

证据文件位置：`docs/evidence/UX-PARITY-FIX-16E/{README.md,observations.json}`。

完成条件：字段保存重开一致；越界值按上游回落；信任源与命令构造有测试；Windows 证书存储写读复原在隔离真机通过并复位；`chrome/mozilla` 内置资源、`LocalMachine` 提权、下载客户端实际接线明确登记。已达成（真实下载客户端接线与内置 PEM 资源除外）。

接口缺口处理（登记，不自行削减）：
- 下载客户端实际接线：`RootCertProvider` 的真实消费者是应用自身 HTTPS 下载（上游 `DownloadService`/`DownloaderHelper`）。本卡只交付 `crates/platform::cert` 的信任源契约与桥接，接入 `crates/application` 下载/测速的 TLS 校验属 FIX-16E-2（`crates/application` 非本卡所有权）。
- `chrome`/`mozilla` 内置 PEM 资源：上游从嵌入资源 `Global.ChromeRootCertFileName`/`MozillaRootCertFileName` 载入；本仓无该资源与下载链 → 登记 blocked（信任源映射与文件名已交付，资源缺失）。
- `LocalMachine` 提权写入：`CertStoreScope::LocalMachine` 已可构造且标记 `requires_elevation()`；真机写测试仅覆盖 `CurrentUser`，`LocalMachine` 需提权 → blocked。
- `certutil -addstore ROOT` 弹 CryptUI 同意框（实测挂起），故真机后端改用静默 CryptoAPI；`certutil` 命令仅作预览/审计，不经其执行根存储写入。

本轮实际结果：
- `crates/platform/src/cert/mod.rs`：`RootCertProvider{system,chrome,mozilla}` + `ROOT_CERT_PROVIDERS` + `normalize`（上游回落）+ `trust_source`/`uses_system_store`；`CertStoreScope{CurrentUser,LocalMachine}`（`certutil_scope_flag`/`requires_elevation`）；`CertBlob`；`install/remove/verify_command`；`CertificateStore` trait + `FakeCertificateStore`。
- `crates/platform/src/cert/windows.rs`：`WindowsCertificateStore` 走 `crypt32` 手写 FFI（`CertOpenStore`/`CertAddEncodedCertificateToStore`/`CertEnumCertificatesInStore`/`CertDeleteCertificateFromStore`），静默无 UI。
- `crates/platform/tests/cert_store.rs`：`#[ignore]` + `V2RAYN_R_CERT_STORE_TEST=1` 的 `CurrentUser\ROOT` 写读复原。
- `crates/bridge_api/src/api/platform.rs`：`cert_root_providers` / `cert_provider_info`（`CertProviderDto`）/ `cert_install_command` / `cert_remove_command`（`CertCommandDto`，含越界错误分支）；未改 `frb_generated`。
- Flutter：`settings_defaults.dart` 增 `rootCertProviders`/`defaultRootCertProvider`/`normalizeRootCertProvider`；`settings_controller.saveDocument` 就地对越界 `RootCertProvider` 归一到 `system`。
- 测试：platform 单测 18/18（含 cert 5）；bridge cert 2/2；隔离真机 1/1（before `contains=false` → install → `true` → remove → `false`；前后 `CurrentUser\Root` 计数 84→84，thumbprint 前后均 not-found）；Flutter `fix16e` 2/2、`fix16` 3/3（重试一次，首次为已知 exit 79 抖动）、`fix08` 1/1。
- 门禁：`cargo fmt --check` 0 改变；两 crate `clippy -D warnings` 通过。

## 后续卡（未在第一批完成）

| 卡 | 范围 | 未完成原因 |
|---|---|---|
| FIX-16E-2 | 将信任源接入 `crates/application` 下载/测速 TLS 校验（chrome/mozilla 内置 PEM 载入） | `crates/application` 非本卡所有权；需内置 PEM 资源与下载链 |
| FIX-16E-3 | `LocalMachine\ROOT` 提权安装/回滚 + 授权隔离路径 | 需提权与平台授权，自动测试不可安全执行 |
