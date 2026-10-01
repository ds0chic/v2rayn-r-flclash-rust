# ADR T16 — 备份/恢复/WebDAV/更新接线决策

状态：已采纳（实现轮，未 verified）
范围：`crates/application`、`crates/updater`（最小追加）、`crates/bridge_api`、`apps/desktop`。

## 1. 本地备份采用“版本化 bundle 目录”，原版 ZIP 走识别+导入

- 自有备份输出为目录 bundle：`manifest.json`（`format_version`、`created_at`、`app_source_commit`、db/config sha256、资源清单、表计数）+ SQLite 一致性快照 + `guiNConfig.json` + 顶层引用资源。由 `persistence::backup::create_backup` 生成，`BackupService::create_local` 负责收集资源与表计数。
- **偏离原版**：原版 `LocalBackup` 直接生成 `backup_*.zip`。本实现的新格式使用独立命名/目录与版本标识，不覆盖旧客户端唯一可恢复的备份（PLAN §15）。WebDAV 上传时才用 `zip_bundle` 打包为 `backup.zip`。
- 原版 `guiConfigs/` ZIP 只读识别（`recognize_archive`）并经由 T04 候选流程导入（`import_from_path`），不在本地解压覆盖现有配置。

## 2. 恢复“先验证后写候选”，配置最后原子落盘

- `BackupService::restore` 先 `verify_backup`（manifest 与逐文件 sha256），失败直接返回结构化错误，不触碰现有数据。
- 随后 `restore_backup` 走 T04 的 `commit_candidate`（同卷 rename 保留 `target_backup`），再以 temp+rename 覆盖 `guiNConfig.json`。任一步失败都不会留下半恢复状态。

## 3. WebDAV：显式凭据/目录，直连默认，凭据不落日志

- 端点为 `WebDavItem` 的 Url/UserName/Password/DirName；目录缺省 `v2rayN_backup`，文件 `backup.zip`。
- HTTP Basic 认证按请求附加；错误 detail 只含状态码/网络原因，绝不含用户名/密码。
- 默认 `no_proxy()`（loopback 测试必须可达）；仅在显式传入代理端点时经代理。请求支持 PROPFIND（含 MKCOL 建目录）、PUT、GET。

## 4. 更新：经代理必须显式，绝不静默直连

- `via_proxy=true` 时取当前运行会话的本地端口（`engine().local_proxy_url()`）；无端口立即返回 `E_PROXY_UNAVAILABLE`，不直连（与 T09 订阅更新一致）。
- 渠道：stable/prerelease；仅 `v2rayN`/`xray` 跟随预发布。sing-box 锁定 `≤1.14.*`（`pick_with_limit` 回退到上限内的最高版本）。
- 校验顺序：sha256（GitHub digest 或 `.dgst` 的 `SHA2-256`，`.dgst` 优先）→ 安全解包（拒绝穿越/符号链接/炸弹）→ PE/ELF/Mach-O 架构检查 → 同卷原子替换并保留 `<core>.previous` 回滚。
- 托管核心布局为 `cores/<core>/` + `install-manifest.json`；回滚用 `rollback_core` 换回 `<core>.previous`。`installed_version` 同时兼容 dev `tools/cores/<core>/v*` 布局（只读探测）。
- **应用自身更新不执行**：`app_update_spec` 只下载/校验/解包并返回 `ExternalUpgradeSpec`（helper、source、install_root、wait_for_pid、args），不启动任何进程。真实替换需未来独立外部进程验证。
- 更新目标列表包含本构建不更新的核心（v2fly/hysteria2/tuic/naiveproxy/juicity/brook/overtls/shadowquic/mieru），在 UI 明确禁用并标注“不支持更新”。

## 5. 清理为手动触发；定时循环未接线

- `cleanup_logs_tmp(root, now, log_age_days=7, test_age_hours=1)` 复刻原版判定（`Test*` >1h、日志/临时/`.partial` >7天）。目前仅在“备份与还原/维护”提供手动按钮；原版每小时 SCH-003 定时循环尚未接线（保留为 identified）。

## 6. UI 使用路径文本框而非原生文件选择器

- 桥接 API 只接受显式路径；窗口用路径输入框 + 动作按钮，便于确定性 widget 测试。原生保存/打开对话框留待后续接线。

## 7. 未决

- 签名方案未定：仍使用 `UnsupportedSignatureVerifier`，不冒充“已验证”。
- 外部升级实跑未验证（仅 spec）。
- 真实 WebDAV 服务器未实测（仅 loopback mock）。
