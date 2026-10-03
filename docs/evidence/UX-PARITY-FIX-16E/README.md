# UX-PARITY-FIX-16E evidence

FIX-16E：`GuiItem.RootCertProvider`（system/chrome/mozilla）信任源与证书存储平台效果。
基准 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；应用 HEAD `4017e9f`（未 commit）。

- `observations.json`：改动文件、实际命令与结果、状态统计、blockers、环境约束自证。

## 上游对照结论

`RootCertProvider` 在冻结上游 v2rayN 7.25.4 中**不是证书安装器**：

- `CertPemManager.BuildTrustedCertificateCollection`（`CertPemManager.cs:329-336`）在 `chrome`/`mozilla` 时返回内置 PEM 集合，`system` 时 `BuildCertificateChainPolicy` 返回 `null`（用 OS/rustls 原生根）。
- 唯一消费者是应用自身的 HTTPS 下载：`DownloadService.cs:171/267`、`DownloaderHelper.cs:294`。
- `ResUI.zh-Hans.resx:1832-1835` 明确：“根证书提供者 …… 仅用于 v2rayN 界面程序的下载及网络请求，不影响核心的证书验证。”
- `ConfigHandler.cs:98-100`：值不在 `Global.RootCertProviders`（`Global.cs:758`）时强制回落第一项 `system`。
- 全仓 `grep` 无 `InstallCert`、`X509Store`、`certutil`、`StoreLocation`：上游没有写系统证书存储的路径。

因此本卡交付的是**信任源选择**（`platform::cert::trust_source`），并额外提供一个受控的证书存储写入面（默认假后端、真机静默 CryptoAPI），不做冒名功能。

## 交付

- Rust `crates/platform/src/cert/{mod.rs,windows.rs}`：provider/scope/信任源/命令预览/`CertificateStore`（假 + Windows）。
- 桥接 `crates/bridge_api/src/api/platform.rs`：`cert_root_providers`、`cert_provider_info`、`cert_install_command`、`cert_remove_command`（未改 `frb_generated`，由根代理重生成）。
- Flutter `settings_defaults.dart` / `settings_controller.dart`：越界 provider 保存时就地归一到 `system`（对齐 `ConfigHandler.LoadConfig`）。
- 真机隔离实测：`CurrentUser\ROOT` 合成自签证书 写→读→删 复原通过，前后存储计数一致。

## 状态

- `verified`：provider 归一化、信任源映射、命令构造/错误分支（单测+桥接测试）；本机 `CurrentUser\ROOT` 合成证书写读复原。
- `implemented`：Windows 证书存储真后端（CryptoAPI 手写 FFI）、`certutil` 命令预览。
- `blocked`：`chrome`/`mozilla` 内置 PEM 资源缺失；`LocalMachine\ROOT` 提权写入；`certutil -addstore ROOT` 弹 CryptUI 同意框（不可自动）。
- 后续：FIX-16E-2 将信任源接入 `crates/application` 下载/测速 TLS 校验。
