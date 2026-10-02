# T21 — 安装器与外部自更新决策（ADR）

日期：2026-10-02
状态：accepted
范围：`tools/release/v2rayn-r.iss`、
`services/upgrade_runner`、
`crates/updater/src/install.rs`、
`crates/application/src/update_service.rs`。
证据：`docs/evidence/T21-install-update.md`。

## 背景

Windows 无法替换正在运行的 exe。T16 只产出
`ExternalUpgradeSpec`（“外部升级规格”），从不启动进程。T21 需要真正做出
安装器与外部自替换，并实跑安装/卸载与成功的自更新/失败回滚。

## 决策 1：外部 runner 独立为 `services/upgrade_runner` 二进制

- 应用内不 spawn 进程；由该独立二进制承担“等 PID 退出 → 加载计划 → 原子
  替换 → 回滚 → 重启 → 写结果 JSON”。
- 与 `net_host`/`privileged_helper` 同级放在 `services/`，不新增 crate 层级。
- 复用 `updater::apply_atomic`，不复制交换逻辑，保证同卷两次 rename 的
  崩溃安全语义单一来源。
- 结果 JSON 是唯一机器可读契约，供证据脚本与未来 UI 读取。

## 决策 2：`InstallPlan` 可序列化 + `restore_previous`

`InstallPlan` 原本只在进程内使用（无 serde）。外部进程需要同一计划，故：
- 给 `InstallPlan` 加 `Serialize/Deserialize`；
- 新增 `restore_previous`：current 缺失而 keep 存在时改回 current。
  这是 runner 的兜底——`apply_atomic` 已回滚自身 rename 失败，但 helper 若在
  两步之间崩溃，可能只留下 keep 目录；`restore_previous` 恢复它。
- 均为最小缺口，不改变 `apply_atomic` 的既有安全语义与测试。

## 决策 3：测试基址覆盖仅限 debug 构建

- 生产默认固定 `GITHUB_API_BASE = https://api.github.com/repos`。
- `test_api_base_override()` 读 `V2RAYN_R_UPDATE_API_BASE`，但整段包在
  `#[cfg(debug_assertions)]`；release 构建编译期返回 `None`，环境变量无效。
- 仅接受 `http(s)://` 前缀；摘要/签名/架构校验不放宽；不读取环境代理。
- 另有显式 `UpdateService::with_api_base()` 供测试直接设置，无需环境变量。
- 目的：让安装后的 **release** 二进制无法被环境变量重定向到伪造更新源。

## 决策 4：安装器使用 Inno Setup，每用户、portable 布局

- `PrivilegesRequired=lowest`：安装到 `{autopf}`（每用户），无需 UAC，卸载
  注册项落在 HKCU，便于测试清理并避免系统级残留。
- 文件按 portable 目录逐字节复制，不重排、不裁剪，保持与 zip 包一致。
- 快捷方式为可选任务；静默用 `/MERGETASKS`。
- AppId 固定 GUID，保证升级安装识别同一产品；`/DIR` 允许覆盖安装目录以
  隔离测试。

## 后果

- 正向：外部替换与回滚有独立进程、崩溃可恢复、结果可机读；安装/卸载可
  幂等复跑；生产更新源不可被环境变量篡改。
- 负向/未决：安装器与 exe 未签名；真实 GitHub 端点未端到端实跑；
  跨卷替换未覆盖；runner 真实 GUI 重启未跑（测试用 stub）。
- 不做：不改 `apps/desktop` 功能代码，不改其他 crate，不 commit。
