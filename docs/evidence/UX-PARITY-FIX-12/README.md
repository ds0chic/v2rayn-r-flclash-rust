# UX-PARITY-FIX-12 evidence — 内核更新安装布局 + PGP 下载管线

场景：检查更新窗口选定内核 → 应用 → runtime 解析并启动刚下载版本；应用（v2rayN）不得装进 cores。

- 基线：HEAD `59dfa97`；受保护来源见 `docs/evidence/parity-review-2026-10-03/`（RT-04/15、SET-20、ROOT-05）。
- 关键改动：`crates/runtime/src/install_layout.rs`（新增冻结类型）、`runtime/src/{lib,adapter}.rs`、`application/src/{lib,update_service}.rs`、`bridge_api/src/api/t16.rs`、`application/tests/t16_update.rs`。
- 证据文件：`observations.json`（结构化）、`cargo-test.log`（命令原始输出）。

## 验收对照

1. 共用安装根/目录名/版本结构（RT-04）：`CoreInstallLayout` 为唯一解析入口；`sing_box`/`sing-box` 统一为 `singbox`；`UpdateService` 与 `CoreLocator` 均经该类型。证据：`install_layout::tests::singbox_dir_is_singbox_everywhere`、`updater` 与 `runtime` 编译期共用同一类型。
2. 更新后启动刚下载版本：loopback mock 返回合成 `Xray-windows-64.zip`，`apply_core` 安装到 `cores/xray/26.4.0/xray.exe`，`CoreLocator` 无 pin 解析到该目录。证据：`t16_update.rs::check_apply_and_rollback_in_temp_dir`。
3. 应用/核心分流：`t16_apply_core_update` 跳过 `v2rayN`；`UpdateService::apply_core("v2rayN")` 返回 `error.update_app_not_core`。原版 WPF 包不会进入 Flutter cores 目录。证据：`update_service::tests::apply_core_refuses_application_target`。应用自身外部替换/重启未接线（FIX-12B）。
4. 检查窗口偏好持久化（SET-20）：未完成，缺 FRB 通道，登记后由根代理重生成（见任务卡）。
5. PGP 接入下载管线（RT-15）：`app_update_spec_verified` 在暂存阶段调用 `enforce_detached_signature`，缺签/错签失败关闭；真实后端 `updater::signature::v2rayn_app_verifier`（rpgp 或 GnuPG CLI，v5 信任根）。证据：`update_service::tests::detached_signature_enforcement_rejects_wrong_and_missing` 与 `updater` 的 `signature.rs` 既有 PGP 用例。

## 命令

见 `cargo-test.log`。未运行 Flutter（本卡未改 Dart）。未改宿主系统代理/注册表/路由/TUN，未占用 10808；mock 绑定 `127.0.0.1:0`。

## 上游对照

- 上游目录名来自 `Utils.GetBinPath(..., CoreType.ToString().ToLower())`（`bin/<coreTypeLower>`，exe 名由 `Utils.GetExeName` 加 `.exe`）。本项目保留 `cores/` 根与版本化 `<dir>/<version>/`（与 `tools/cores/singbox/v1.14.2/` fixtures 一致），仅冻结解析入口；`singbox` 与上游 `sing_box` 的拼写差异为本项目既有约定，已在类型内单点固化。
- 上游核心更新用 sha256/`.dgst`（`Xray ... .dgst`），应用自身用 `.sig` OpenPGP；本项目一致：核心走 dgst，应用走 `enforce_detached_signature`。
