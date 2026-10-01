# T13 平台后端决策记录（所有权与四语义）

- 关联方案：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §13（启动/切换/退出状态机中的系统代理恢复）、
  §15（自定义系统代理脚本保留）。
- 关联台账：`compat/features.yaml` F-SYSPROXY-001（四态）、F-SYSPROXY-002（例外/高级协议）、
  F-SYSPROXY-003（PAC 服务）、F-SYSPROXY-004（退出恢复）、F-DESKTOP-003（自启动）；
  `compat/fields.yaml` FLD-CFG-137..142（SystemProxyItem 六字段）、FLD-CFG-095..105（TunModeItem）。
- 上游只读来源：`ServiceLib/Handler/SysProxy/SysProxyHandler.cs`、
  `ProxySettingWindows.cs`、`ServiceLib/Manager/PacManager.cs`、
  `ServiceLib/Handler/AutoStartupHandler.cs`、`ServiceLib/Global.cs`。

## 决策 D1 —— 逐字段所有权：只登记“真正改动”的字段

`apply(mode, settings)` 先 `snapshot()`，再对每个 `ProxyField` 计算目标值；**仅当
`before != after` 才写入并登记 `AppliedChange`**。若某字段应用前恰等于目标值，本应用不写该字段，
也**不声称拥有它**。

理由：方案 §13 要求“修改前存旧值，记录本应用写入值；恢复时当前值仍是本应用写入值才恢复”。
若把未改动的字段也当作自有，退出恢复时会覆盖掉用户在应用启动前就设置好的同值状态（尤其
`ForcedClear` 会把“本来就没开代理”误恢复成历史值）。

结果：`AppliedChange.after` 即所有权凭证；`restore_if_owned` 无需额外的外部状态。

## 决策 D2 —— `restore_if_owned` 三态判定与幂等

逐字段对当前 `ProxyState` 判定：

1. `current == after` → 仍是本应用写入值 → 恢复为 `before`，记 `RestoreAction`。
2. `current == before` → 已处于原值 → 静默跳过（既非恢复也非冲突）。
3. 其他 → 被外部改过 → 保留 `current`，记 `RestoreConflict`。

第 2 条同时满足方案 §13 “资源已经恢复时重复恢复应安全”：第二次 `restore` 不会产生假冲突。
冲突字段**绝不写回**，交由上层提示用户。

## 决策 D3 —— 四语义的目标映射

对齐上游 `SysProxyHandler.UpdateSysProxy` / `ProxySettingWindows.SetProxy`：

| 模式 | 目标状态 | 上游对应 |
|---|---|---|
| `Unchanged` | 不写任何字段、不发通知；`apply` 返回空 | 不调用 SetProxy |
| `ForcedClear` | `enabled=false; server=None; bypass=None; auto_config_url=None` | type 1（UnsetProxy） |
| `ForcedChange` | `enabled=true; server=settings.server; bypass=settings.bypass; auto_config_url=None` | type 2（DIRECT\|PROXY） |
| `Pac` | `enabled=false; server=None; bypass=None; auto_config_url=Some(url)` | type 4（DIRECT\|AUTO_PROXY_URL） |

- `Pac` 缺少 `auto_config_url` → `PlatformError::Invalid`（对应上游需 `pac` 端口与 URL）。
- `auto_detect` 默认强制四态下为 `false`（上游 flags 未含 AUTO_DETECT），除非 `settings.auto_detect`
  显式给出——保留未来开关。
- 上游 `forceDisable`（退出）：`Unchanged → ForcedClear`，非 `Unchanged` 不动。接线层实现该覆盖后
  再调用本 trait；本 crate 只提供四语义原语。

## 决策 D4 —— Windows 真实后端与依赖选择

1. **零新增依赖，手写 FFI**：任务卡建议 `windows` crate，但把 `windows` 加入 `crates/platform/Cargo.toml`
   会让 `Cargo.lock` 产生变更（`cargo tree -p platform --locked` 报
   “cannot update the lock file … because --locked was passed”），而 `Cargo.lock` 不在 T13 允许写入的
   路径内，且门禁明确要求 `--locked`。为同时满足写边界与门禁，Windows 后端采用
   `extern "system"` + `#[link(name = "wininet"/"advapi32"/"kernel32")]` 手写 FFI，语义与上游一致。
   若后续允许更新 lock，可无痛替换为 `windows` crate（trait 表面不变）。
2. **两层读取**：第 1 层 WinINET per-connection option；失败回退第 2 层 HKCU
   `Internet Settings`（current user）。写入优先 WinINET，失败回退注册表（对齐上游
   `SetProxy` catch → `SetProxyFallback`）。
3. **通知**：`INTERNET_OPTION_SETTINGS_CHANGED(39)` + `INTERNET_OPTION_REFRESH(37)`（上游同款）。
   任务提到的 `INTERNET_OPTION_PROXY_SETTINGS_CHANGED` 语义更窄，未采用；如需可追加。
4. **自启动删除用 `RegDeleteValueW`**：上游 `ClearTaskWindows` 写空串；本实现删除值，
   `is_enabled` 对空串/缺失均判 false，语义等价且更幂等（不残留空值）。
5. **单实例**：与 T01 探针一致使用命名互斥体 `CreateMutexW` + `ERROR_ALREADY_EXISTS`；
   默认名 `Global\v2rayN-R-single-instance`。`Global\` 命名空间与上游/探针保持一致。

## 决策 D5 —— PAC 服务边界

1. **强制 loopback**：`PacServer::new` 拒绝非 `127.0.0.1/localhost/::1`，保证 PAC 不对外暴露。
2. **默认端口 ≥ 11808**：`DEFAULT_PAC_PORT_BASE = 11808`，默认端口 `0` = 从 11808 起自动选空闲端口；
   **绝不使用 10808**（AGENTS.md 硬约束：用户正在运行的代理端口）。
3. **幂等与生命周期**：运行中重复 `start` 只刷新内容、端口不变；`Drop` 保证停止；连接/accept
   均有超时，避免测试卡死。
4. **`__PROXY__` 由调用方给规则**：`proxy_rule` 由接线层提供（HTTP 入站端口不在本 crate），
   避免把内核端口硬编码进平台层。

## 决策 D6 —— 自定义脚本字段只承载不执行

方案 §15 要求保留自定义系统代理脚本功能，但明确“在明确的用户权限和工作目录中执行”。
本回合只做字段承载（`CustomSystemProxySetting`）与路径存在性校验
（`validate_existing_file`），**不执行任何脚本、不读脚本内容**。执行、工作目录、权限与
“以管理员身份重启”入口留待 T13 接线阶段在用户明确环境中实现与验证。

## 门禁与并行影响

- `cargo fmt -p platform --check`、`cargo clippy -p platform --all-targets --locked -- -D warnings`、
  `cargo test -p platform --locked` 均通过（51 用例）。
- 期间 `Cargo.lock` 被并行 `core_adapters` 任务修改（新增 bytes/tiny_http/tokio-tungstenite 等）；
  `platform` 在 lock 中的依赖仍为 `serde`/`serde_json`，本任务未改 lock，也未计入 workspace 级结果。
- 未 commit。
