# T14 提权 Helper 决策记录（操作白名单、管道安全、会话租约与路径校验）

- 关联方案：`outputs/V2RAYN_FLUTTER_RUST_PLAN.md` §5（进程结构与唯一所有权、helper 只支持有限操作枚举、
  Windows 管道显式 DACL、net-host/helper 职责边界）、§13（启动/切换/退出状态机：进程树约束、资源所有权与幂等恢复）。
- 关联台账：`compat/features.yaml`（TUN/路由/提权相关条目）。本回合只落地 helper 与 IPC 合同，不接线到 net-host。
- 冻结上游（commit `7d6a967`）参照点：原版以管理员进程直接实施 TUN/路由/网卡操作；本方案改为
  “普通权限 net-host + 最小提权 helper”的拆分，语义以本方案 §5/§13 为准。
- 相关代码：
  - `crates/ipc_contract/src/helper.rs`（协议、校验、路径规范化）
  - `crates/ipc_contract/src/lib.rs`（`SessionIdentity`、帧上限、超时常量）
  - `services/privileged_helper/src/{main,server,backend,windows,audit,pipe_security}.rs`

## 1) HelperOp 白名单与理由

`HelperOp` 是**闭合枚举**（`#[serde(tag = "op", rename_all = "snake_case")]`），未知操作无法被反序列化表示，
因此“任意命令”无法从协议层夹带进来：

| 操作 | 载荷 | 作用 |
|---|---|---|
| `Ping` | 无 | 存活探测，回传 `ElevationStatus` |
| `GetElevationStatus` | 无 | 查询是否提权与会话数 |
| `AddRoutes` | `entries: Vec<RouteEntry>` | 批量添加路由（按条目全有或全无） |
| `RemoveRoutes` | `entries: Vec<RouteEntry>` | 批量删除路由 |
| `SetTunAdapterAddress` | `config: TunAddressConfig` | 按接口索引设置地址/MTU |
| `RunElevatedCore` | `spec: ElevatedCoreSpec` | 从受控目录提权启动白名单内核 |
| `StopElevatedCore` | `handle: u64` | 按不透明句柄停止内核 |
| `Shutdown` | 无 | 清理后停用 helper |

理由（对应 §5）：

1. **有限枚举 = 最小攻击面**：不提供 shell、不提供任意程序执行、不提供接收任意命令行的操作。
2. **参数结构化**：路由是字段化 `RouteEntry`（对应 `MIB_IPFORWARD_ROW2` 子集），TUN 是字段化
   `TunAddressConfig`，内核启动是“白名单 core + 参数数组”，**从不接收单条 shell 字符串**。
3. **两段式校验**：所有输入先由 `ipc_contract` 校验（路径边界、枚举、参数上限），server 的 `dispatch`
   再分发；`windows::WindowsBackend` 在调用 OS 前**再次**校验（纵深防御）。
4. **独立协议版本**：`HELPER_PROTOCOL_VERSION = 1`，与冻结的 net-host 消息集互不影响
   （`ipc_contract/src/lib.rs` 只把 `helper` 作为向后兼容模块追加）。

## 2) 命名管道 DACL 与会话 SID 校验

两层独立控制，缺一不可：

- **DACL（资源层）**：`pipe_security::PipeSecurity::current_user_only()` 取当前用户 SID，构造 SDDL
  `D:P(A;;GA;;;<sid>)`（保护型 DACL，仅当前用户 `GENERIC_ALL`），经
  `ConvertStringSecurityDescriptorToSecurityDescriptorW` 转为安全描述符，作为 `SECURITY_ATTRIBUTES`
  传给 `ServerOptions::create_with_security_attributes_raw`。同时 `reject_remote_clients(true)`、
  `max_instances(16)`、`PipeMode::Byte`、`first_pipe_instance(first)`。
  理由：tokio `ServerOptions` 不暴露安全描述符，必须显式建 DACL，不能依赖宽泛默认权限（§5）。
- **SID 校验（会话层）**：daemon 以 `--serve` 启动时把 `expected_sid = current_user_sid_string()`、
  `require_sid_match = true` 写入配置。每个连接在 `main.rs` 中经
  `GetNamedPipeClientProcessId → OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION) →
  OpenProcessToken(TOKEN_QUERY) → GetTokenInformation(TokenUser) → ConvertSidToStringSidW`
  得到对端 SID，交给 `serve_connection`。`sid_matches` 为非空且 `eq_ignore_ascii_case` 比较；
  不匹配在**处理任何请求之前**以 `Unauthorized` 拒绝。
- **共享令牌（应用层）**：`V2RAYN_R_HELPER_TOKEN` 为空则 daemon 直接退出、不建管道；
  `check_helper_session` 逐请求校验协议版本与非空 token。令牌**从不写日志**。

失败即拒绝：DACL 决定“谁能连”，SID 决定“连上的是不是预期用户”，token/version 决定“会话是否合法”。

## 3) 会话租约与断连清理

`ConnectionLease` 逐连接保存本会话**自有**资源：`routes`、`tun_interfaces`、`cores`。server 的
`dispatch` 只在成功后把这些资源登记进租约；`on_disconnect` 只清理租约里的条目，因此
**会话之间天然隔离**（`sessions_are_isolated` 用例验证 A 断连不会动 B 的路由）。

`LeasePolicy` 三档：

| 策略 | 断连行为 | 用途 |
|---|---|---|
| `CleanOwned`（默认，daemon 采用） | 删自有路由、重置自有 TUN 地址、停自有内核 | 正常会话 |
| `StopCoresOnly` | 只停内核，保留路由/TUN | 需要瞬时保留网络设置的特例 |
| `LeaveRunning` | 不做任何清理（显式 opt-in） | 明确要求资源存活过会话 |

`on_disconnect` 通过 `lease.closed` 标志保证**幂等**：重复调用不产生第二次后端动作
（`lease_cleanup_is_idempotent`）。清理后写一条 `lease_cleanup` 审计，记录所用策略。
对应 §5“helper 在特权会话期间保持可用并监督 net-host，失联时清理自己拥有的特权资源，避免授权后立即
退出导致后续无权回滚”，以及 §13“资源已经恢复时重复恢复应安全”。

## 4) `RunElevatedCore` 的路径/参数校验策略

校验入口是 `validate_elevated_core(spec, allowed_run_roots)`，逐条拒绝：

1. **core 白名单**：`ALLOWED_ELEVATED_CORES` 为闭合名单（xray、v2ray、v2fly、sing-box、mihomo、
   hysteria、hysteria2、tuic、naive、naiveproxy、brook、overtls、shadowquic、mieru、juicity）；
   比较大小写不敏感，`.exe` 可省略。
2. **绝对且规范化**：`normalize_windows_path` 拒绝相对路径、NUL、以及任何 `..` 段，只接受
   `X:\...` 或 `\\server\share\...`；同时把 `/` 归一为 `\`、折叠 `.` 与空段。
3. **可执行文件名匹配 core**：取规范化后的文件名词干，须与 `core` 大小写不敏感相等
   （`naive ⇄ naiveproxy` 互认为别名），且扩展名必须是 `.exe`。
4. **exe 必须直接位于 run_dir 内**：`exe_parent == run_dir`（不允许嵌套目录）。
5. **run_dir 必须位于受控根之下**：`allowed_run_roots` 来自 daemon 启动环境变量
   `V2RAYN_R_HELPER_RUN_ROOTS`（`;` 分隔）；`path_under` 做大小写不敏感的词法前缀判定。
6. **参数边界**（`validate_args`）：条目数 ≤ `HELPER_MAX_ARGS(64)`，单参数 ≤
   `HELPER_MAX_ARG_BYTES(4096)`，命令行总长 ≤ `HELPER_MAX_COMMAND_LINE_BYTES(32000)`，
   且**不允许 NUL/控制字符**（制表符除外）。

通过后才允许进入后端。`windows::run_elevated_core` 再次校验，然后以 `application_name`（exe 路径）与
独立构造的 `command_line` 调 `CreateProcessW`，`CREATE_NEW_PROCESS_GROUP | CREATE_SUSPENDED` 启动，
建匿名 Job Object 并 `AssignProcessToJobObject`，最后 `ResumeThread`——对齐 §13“Windows 普通子进程由
net-host 持有 Job Object 等合适的进程树约束”。

> 注意：这是**词法**归一化而非文件系统规范化：不解析符号链接/重解析点/8.3 短名，也不抵抗
> 校验与 `CreateProcessW` 之间的 TOCTOU。此项列入未决项。

## 5) 审计日志字段与脱敏

`AuditRecord { ts_ms, session_id, operation, summary, outcome }`：

- `ts_ms`：Unix 毫秒；`session_id`：`s1`、`s2`… 单调计数器；`operation`：`op_name` 稳定小写名。
- `outcome`：`Ok` / `Rejected`（版本、帧超限、畸形、未授权、非白名单、越界路径、未知句柄）/
  `Error`（后端失败、超时）。
- `summary`：**仅由计数与白名单标识拼装**。例如 `run_elevated_core core=xray args=3`、
  `add_routes count=1`、`set_tun_address addresses=1 mtu_set=true`；拒绝时只记错误**标签**
  （`error: not_allowlisted`），不记 detail 字符串。

脱敏约束（`audit.rs` 注释与 `audit_summary_excludes_args_and_paths` 用例共同保证）：

- 不写配置内容、不写可执行文件路径、不写参数内容、不写会话令牌、不写凭据。
- 日志**内存有界**：`DEFAULT_AUDIT_CAPACITY = 4096`，超限从头部逐条淘汰（FIFO），避免长生命周期无界增长。

## 6) 与 net-host 的职责边界（§5 / §13）

| 维度 | net-host | helper |
|---|---|---|
| 身份 | 普通权限，唯一内核进程持有者，运行时状态权威 | 最小提权，仅执行需权限的有限平台操作 |
| 拥有的资源 | 普通 core 进程句柄/进程号、系统代理、恢复日志 | 提权启动的进程/句柄、受管路由、TUN 网卡地址 |
| 发起方式 | 由 net-host 通过会话租约**管理** helper 资源，不重复直接拥有（§5 所有权表） | 不主动实施网络变更，只响应枚举操作 |
| 配置传递 | 接收不可变 `RuntimePlan`，在私有运行目录按哈希引用配置 | **不接收** `RuntimePlan`；只接收字段化的小操作载荷，任意路径禁止交给提权 helper（§5） |
| 恢复/退出 | §13 逐字段所有权恢复、资源只清理本应用创建/明确拥有的部分、日志重读 OS 事实；持有核心 Job Object | 断连即按租约 `CleanOwned` 清理自有路由/TUN/内核；重新幂等 |

要点：

- helper 的租约是“路由/TUN/提权内核”所有权的落地点；net-host 通过会话在自身恢复流程中编排它，
  两者共同满足 §13“TUN 路由/DNS/网卡只清理本应用创建或明确拥有的资源”。
- helper 活到特权会话结束、失联即清理，避免 §5 所述“授权后立即退出导致后续无权回滚”。
- 当前**尚未**把 helper 客户端接入 net-host/bridge：会话令牌下发、租约交接、断连检测与编排脚本均未做，
  列入未决项。
