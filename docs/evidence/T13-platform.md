# T13 平台后端证据（crates/platform）

- 任务：T13-platform（系统代理 / PAC / 自启动 / 单实例 / 自定义脚本字段承载）
- 范围：仅 `crates/platform/**`；未改其他 crate、apps、compat、work、outputs；未 commit。
- 环境：Windows 11 x64，Rust 1.98.1（MSVC）。本机存在用户正在运行的代理（10808）与真实系统代理设置。

## 1) API 概览

### 1.1 系统代理（`platform::sysproxy`）

逐字段模型与所有权恢复，语义对齐方案 §13 与上游
`ServiceLib/Handler/SysProxy/SysProxyHandler.cs`（`ESysProxyType` 四态）。

- `SysProxyMode { ForcedClear, ForcedChange, Unchanged, Pac }`（serde 名称即上游枚举名）。
- `ProxyState { enabled, server, bypass, auto_config_url, auto_detect }` —— `snapshot()` 的逐字段视图。
- `ProxySettings { server, bypass, auto_config_url, auto_detect }` —— `apply()` 的输入。
- `ProxyField { Enabled, Server, Bypass, AutoConfigUrl, AutoDetect }` + `ProxyField::ALL`。
- `AppliedChange { field, before, after }` —— 本应用实际写入的每个字段（`before`=旧值，`after`=本应用写入值）。
- `restore_if_owned(applied, current) -> RestoreReport`（纯函数）。
- `RestoreReport { restored: Vec<RestoreAction>, conflicts: Vec<RestoreConflict> }`，含
  `is_clean()` / `restored_fields()` / `conflicted_fields()`。
- `trait SystemProxyBackend`：
  - `snapshot() -> Result<ProxyState>`
  - `set_field(field, Option<&str>) -> Result<()>`
  - `notify_changed() -> Result<()>`（默认 no-op）
  - `apply(mode, &ProxySettings) -> Result<Vec<AppliedChange>>`（默认实现，派生自 `snapshot`+`set_field`）
  - `restore(&[AppliedChange]) -> Result<RestoreReport>`（默认实现）
- `FakeSystemProxyBackend`：内存实现，记录 `writes()`（写入序列）与 `notify_count()`。
- Windows：`WindowsSystemProxyBackend`（`#[cfg(windows)]`，编译但本回合不执行）。
  - 第 1 层：`InternetQueryOptionW` / `InternetSetOptionW` +
    `INTERNET_OPTION_PER_CONNECTION_OPTION(75)`，选项 FLAGS/PROXY_SERVER/PROXY_BYPASS/AUTOCONFIG_URL。
  - 第 2 层（current user）：`HKCU\...\Internet Settings` 的 `ProxyEnable` / `ProxyServer` /
    `ProxyOverride` / `AutoConfigURL` / `AutoDetect`。
  - 变更通知：`INTERNET_OPTION_SETTINGS_CHANGED(39)` + `INTERNET_OPTION_REFRESH(37)`（对齐上游）。
  - 辅助：`default_proxy_string(port)`、`windows_bypass(exceptions, not_proxy_local_address)`（含 `<local>`）。
- 其他平台：trait 骨架缺失（无 unsupported 结构体）；本 crate 未导出 Linux/macOS 系统代理实现，
  相关分支记为 unresolved（见 §4）。

### 1.2 PAC 本地服务（`platform::pac`）

- `PacSource { Inline(String), File(PathBuf) }` + `read()`（文件不存在 → `NotFound`）。
- `render_pac(template, proxy_rule)`：替换 `__PROXY__`（对齐上游 `PacManager.InitText`）。
- `PacConfig { host, port, path, proxy_rule }`：默认 `127.0.0.1`、端口 `0`（自动从
  `DEFAULT_PAC_PORT_BASE = 11808` 起选）、路径 `/pac`。
- `PacServer::new/start/stop/is_running/bound_addr/port/url`：
  - 强制 loopback（非 loopback host → `Invalid`）；`Drop` 自动 `stop`。
  - `start` 幂等：运行中再次调用仅刷新内容、端口不变。
  - 端口占用 → `PlatformError::PortInUse`。
  - `GET /pac`（忽略 query）返回 `application/x-ns-proxy-autoconfig`；其他路径 404。
  - 连接读写 500ms 超时，accept 非阻塞 10ms 轮询，`stop` join 有界。

### 1.3 自启动（`platform::autostart`）

- `trait AutoStartBackend { query, set, remove, is_enabled, enable, disable }`。
- `encode_run_command(exe, args)` / `decode_run_command(cmd)`（引号编解码）。
- `run_value_name(startup_path)` = `v2rayNAutoRun_<md5(startup_path)>`（`AUTO_RUN_NAME` 前缀）。
- `FakeRegistry`：内存 `BTreeMap`，支持 `snapshot()`。
- Windows：`WindowsRunKeyBackend`（`#[cfg(windows)]`，编译但本回合不执行），写
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 的 `REG_SZ`，`remove` 用 `RegDeleteValueW`（幂等）。
- `crate::hash::md5_hex`：自实现 RFC 1321 MD5（避免新增依赖、保持 `--locked`），含标准向量测试。

### 1.4 单实例（`platform::single_instance`）

- `SingleInstanceGuard::acquire(name)` / `acquire_default()` / `name()`；`Drop` 释放内核对象。
- Windows：命名互斥体 `CreateMutexW` + `GetLastError == ERROR_ALREADY_EXISTS(183)`，与 T01 探针
  (`crates/application/tests/single_instance.rs`) 同一机制；`DEFAULT_INSTANCE_NAME = Global\v2rayN-R-single-instance`。
- 非 Windows：`Unsupported`。

### 1.5 自定义脚本字段（`platform::script`）

- `CustomSystemProxySetting { pac_path, script_path }` + `validate()/pac_file()/script_file()`。
- `validate_existing_file(Option<&str>)`：未设置→`Ok(None)`；文件存在→`Ok(Some)`；
  不存在→`NotFound`；存在但非普通文件→`Invalid`。
- 只做字段承载与路径存在性校验，**不执行脚本**（执行归 T13 接线阶段，用户明确环境）。

## 2) 测试统计（真实运行，全部 fake/loopback）

| 目标 | 文件 | 用例数 |
|---|---|---|
| lib 单元（MD5 向量、restore 纯逻辑、字段归一化、脚本校验） | `src/lib.rs` unittests | 8 |
| 系统代理所有权/恢复矩阵 | `tests/sysproxy_ownership.rs` | 13 |
| PAC 服务 | `tests/pac_server.rs` | 10 |
| 自启动编解码/fake 注册表 | `tests/autostart.rs` | 9 |
| 单实例 | `tests/single_instance.rs` | 3 |
| 模型序列化/脚本校验 | `tests/models_and_script.rs` | 8 |
| **合计** | | **51** |

命令与结果（Rust workspace 根，`C:\Users\Colby\.cargo\bin\cargo.exe`）：

```
cargo fmt     -p platform              -> FMT=0
cargo fmt     -p platform --check      -> CHECK=0
cargo clippy  -p platform --all-targets --locked -- -D warnings -> EXIT=0
cargo test    -p platform --locked     -> 51 passed; 0 failed
```

- 单实例测试仅同进程两次获取判定（第二个 `AlreadyRunning`；drop 后可再获取）；`Global\` 命名空间。
- PAC 测试使用显式空闲端口或自动 ≥11808 端口，连接 127.0.0.1；每个 server 用 `stop` 或 `Drop` 收尾。
- MD5 采用 RFC 1321 标准向量（空串 / `a` / `abc` / `message digest` / fox 句）。
- workspace 级门禁受并行任务影响：本次只跑 `-p platform`；`Cargo.lock` 被并行的
  `core_adapters` 任务修改（新增 bytes/tiny_http/tokio-tungstenite 等），与本任务无关，未触碰。

## 3) 所有权恢复矩阵结论

`apply` 只登记“本应用实际改动的字段”（`before != after`）；未改动的字段不登记，避免把未写过的值
误判为自有。

`restore_if_owned(applied, current)` 逐字段：

- 当前值 == 本应用写入值（`after`）→ 自有 → 恢复为 `before`（记入 `restored`）。
- 当前值 == 恢复目标（`before`）→ 已处于原值（幂等重复恢复 / 用户手动回退）→ 静默跳过，无冲突。
- 其他 → 外部修改 → 保留现值并记入 `conflicts`。

矩阵（测试覆盖）：

| 场景 | 结果 |
|---|---|
| 本应用值未变 | 全部恢复，`is_clean()` |
| 用户改过某字段 | 该字段保留 + 冲突；其余原字段恢复 |
| 用户清除某字段（值缺失） | 该字段保留（None）+ 冲突 |
| 部分字段改变（应用前后相同字段） | 只登记/恢复真正改动的字段 |
| 重复恢复 | 第二次无 restored、无 conflict（幂等安全） |

## 4) 真实后端未验证清单（本回合只编译，未执行）

以下代码路径在测试中**从未运行**，仅在 Windows 目标编译通过，标 `未验证`：

- `WindowsSystemProxyBackend`：`InternetQueryOptionW`/`InternetSetOptionW`（per-conn）、
  HKCU 注册表两层读写、`SETTINGS_CHANGED`/`REFRESH` 通知；`query_wininet` 的字符串缓冲区
  布局与 RAS/多连接枚举（上游 `EnumerateRasEntries`）在真实系统上的行为。
- `WindowsRunKeyBackend`：`RegOpenKeyExW`/`RegQueryValueExW`/`RegSetValueExW`/`RegDeleteValueW`
  在 `HKCU\...\Run` 的读写删；`Global\` 互斥体在受限令牌/多用户会话下的可见性。
- `windows_bypass` 的 `<local>` 与去空格规则未与真实 `ProxyOverride` 往返验证。
- Linux/macOS 系统代理与自启动后端：未实现。

## 5) 未决项

1. **`windows` crate 未采用**：任务卡建议 `windows` crate，但新增依赖会使 `Cargo.lock` 变更，
   而 `Cargo.lock` 不在 T13 写边界内且门禁要求 `--locked`；故改用零依赖手写 FFI（语义等价）。
   若后续允许更新 lock，可切换为 `windows` crate（见 decisions 决策 D3）。
2. **RAS/多连接（PPPoE）未复现**：上游对 LAN + 拨号连接逐个写代理；本实现只做 LAN/current
   connection 的 per-conn 选项 + 注册表，多连接枚举留待接线阶段。
3. **PAC `__PROXY__` 规则端口**：`PacConfig.proxy_rule` 由调用方提供（HTTP 入站端口在
   net-host/接线层），本 crate 不决定该端口，避免与 10808 冲突。
4. **自启动 value 命名的启动路径**：`run_value_name` 的 MD5 输入为“启动路径”，具体路径来源
   （`Utils.StartupPath()` 语义）由接线层提供。
5. **`remove` 语义微差**：上游 `ClearTaskWindows` 写空串，本实现用 `RegDeleteValueW` 删除；
   `is_enabled` 对空串亦判 false，语义等价（见 decisions D4）。
6. **提权（RunLevel=Highest）计划任务**：上游管理员时用 TaskScheduler；本 crate 未实现该分支。
7. **非 Windows 单实例**：目前返回 `Unsupported`，后续可用文件锁/抽象套接字补齐。
