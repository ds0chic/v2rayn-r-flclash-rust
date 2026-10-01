# T14 提权 Helper 证据（services/privileged_helper + ipc_contract::helper）

- 任务：T14-helper（最小提权 helper 与 IPC 合同）收尾；只补文档 + 跑门禁，未改代码。
- 范围：仅 `docs/evidence/T14-helper.md` + `docs/decisions/T14-helper.md`；
  实现期已落在 `crates/ipc_contract/src/{helper.rs,lib.rs}` 与
  `services/privileged_helper/src/{main,server,backend,windows,audit,pipe_security}.rs`。
  本回合未改 `apps/`、`compat/`、`work/`、`outputs/` 或其他 crate，未 commit。
- 环境：Windows 11 x64，Rust 1.98.1 stable（MSVC）。helper 平台代码**仅编译、未执行**（见 §3 声明）。
- 硬约束遵守：全程未触碰 127.0.0.1:10808，未改系统代理/注册表，未提权，未改路由/TUN，未杀任何用户进程。

## 1) 协议与后端概览

### 1.1 协议（`ipc_contract::helper`）

- 常量：`HELPER_PROTOCOL_VERSION = 1`、`HELPER_PIPE_NAME = \\.\pipe\v2rayn-r-helper`、
  `HELPER_MAX_MESSAGE_BYTES = IPC_MAX_MESSAGE_BYTES(8 MiB)`、
  `HELPER_MAX_ROUTE_ENTRIES=256`、`HELPER_MAX_TUN_ADDRESSES=64`、`HELPER_MAX_ARGS=64`、
  `HELPER_MAX_ARG_BYTES=4096`、`HELPER_MAX_COMMAND_LINE_BYTES=32000`。
- 消息：`HelperRequest { session, request_id, operation }` /
  `HelperResponse { request_id, result }`；`HelperOp` 为闭合枚举（see decisions §1）。
- 结果/错误：`HelperResult`（Pong/ElevationStatus/RoutesAdded/RoutesRemoved/TunAddressSet/
  CoreStarted/CoreStopped/Shutdown/Error）与 `HelperError`
  （VersionMismatch/MessageTooLarge/Malformed/Unauthorized/NotAllowlisted/PathOutOfBounds/
  UnknownHandle/Backend/Timeout），`HelperError::to_domain()` 映射到稳定 `domain::codes`。
- 校验函数：`check_helper_session`、`check_helper_frame_size`、`helper_timeout_for`、
  `parse_cidr`、`validate_route_entries`、`validate_tun_address`、`validate_args`、
  `validate_elevated_core`、`normalize_windows_path`、`path_under`/`path_equal`、
  `is_allowed_core_name`。
- `ALLOWED_ELEVATED_CORES`：xray、v2ray、v2fly、sing-box、mihomo、hysteria、hysteria2、tuic、
  naive、naiveproxy、brook、overtls、shadowquic、mieru、juicity。

### 1.2 服务端（`privileged_helper::server`）

- 帧格式沿用 net-host 约定：`u32 LE 长度 || JSON`，上限 `HELPER_MAX_MESSAGE_BYTES`。
- `HelperServer::handle` 先 `check_helper_session`，再 `dispatch`，并写审计；未知/非法输入在到达后端前拒绝。
- `serve_connection`：SID 不匹配→先回 `Unauthorized` 再断开；逐帧带 `request_timeout`；超时回 `Timeout`；
  帧超限回 `MessageTooLarge`；坏 JSON 回 `Malformed` 且继续；`Shutdown` 处理后收尾断开。
- `on_disconnect` 按 `LeasePolicy` 清理本会话自有资源，`closed` 标志保证幂等。

### 1.3 后端

- `trait HelperBackend`：`is_elevated`、`add_routes`/`remove_routes`、`set_tun_address`/`reset_tun_address`、
  `run_elevated_core`/`stop_elevated_core`、`shutdown`。
- `FakeBackend`：内存记录每个调用、可注入失败（`fail_on`）与提权状态，供全部测试使用。
- `WindowsBackend`（仅 Windows 编译）：`IsUserAnAdmin`；路由经 `CreateIpForwardEntry2`/
  `DeleteIpForwardEntry2`；TUN 地址经 `CreateUnicastIpAddressEntry`/`DeleteUnicastIpAddressEntry`；
  内核经 `CreateProcessW`（挂 Job Object）与 `TerminateProcess`；内核启动前**再次**校验路径/参数。
- `pipe_security`：当前用户专属 DACL + `current_user_sid_string`；`main.rs` 以
  `get client SID → sid_matches` 做会话校验，`reject_remote_clients(true)`、`max_instances(16)`。
- `audit`：内存有界（4096）结构化审计，summary 仅计数/白名单标识（see decisions §5）。

## 2) 门禁与测试结果（本机真实运行）

工作目录：仓库根；命令使用显式路径 `C:\Users\Colby\.cargo\bin\cargo.exe`。

| 命令 | 结果 |
|---|---|
| `cargo fmt -p ipc_contract -p privileged_helper --check` | 通过（exit 0，无 diff） |
| `cargo clippy -p ipc_contract -p privileged_helper --all-targets --locked -- -D warnings` | 通过（exit 0，`Finished dev profile in 3.01s`，0 warning） |
| `cargo test -p ipc_contract -p privileged_helper --locked` | 通过（exit 0，**64 passed / 0 failed / 0 ignored**） |

测试数量明细（`cargo test` 输出逐 target 统计）：

| target | 用例数 | 结果 |
|---|---|---|
| `ipc_contract` 单测（`src/lib.rs`，含 `helper::tests`） | 25 | passed |
| `privileged_helper` 单测（`src/lib.rs`：audit 1 + windows 3） | 4 | passed |
| `privileged_helper` bin（`src/main.rs`） | 0 | passed |
| `tests/dispatch.rs` | 28 | passed |
| `tests/loopback.rs` | 7 | passed |
| doc-tests（两 crate） | 0 | passed |
| **合计** | **64** | **0 failed / 0 ignored** |

其中 **`privileged_helper` = 4 + 28 + 7 = 39**（即任务简报中的 39 测试），`ipc_contract` = 25。

覆盖点：

- 协议：请求/响应 JSON 往返、结构化错误、版本不匹配、空 token、帧上限、内核操作长超时、白名单闭合。
- 校验：路由族/接口/前缀/条目上下限；TUN 名称/接口/地址/MTU；参数上限与控制字符；
  绝对路径规范化、`..` 与相对路径拒绝、前缀 `path_under` 边界；白名单/别名/`run_dir` 边界。
- 服务端 dispatch：ping/状态/增删路由/设 TUN/启停内核/shutdown；停止幂等；未知句柄 NotFound；
  非法输入**先于后端**拒绝（`fake.calls()` 为空）；错误分类与审计 outcome。
- 审计：成功字段、拒绝标记、summary 不含参数与路径（脱敏）。
- 租约：路由/TUN/内核清理、三档策略、幂等、会话隔离、调用顺序。
- 传输（`tokio::io::duplex`）：ping、坏 JSON→Malformed、超帧→MessageTooLarge、空闲超时→Timeout、
  shutdown 收尾、SID 不匹配先拒、SID 匹配放行。

## 3) 明确声明：真实提权/TUN/路由变更未在本机执行

- **未提权运行**：从未以管理员身份启动 helper daemon，从未执行 `privileged_helper --serve`；
  `V2RAYN_R_HELPER_TOKEN`/`V2RAYN_R_HELPER_RUN_ROOTS` 未在真实特权会话下发。
- **未创建真实命名管道**：全部传输测试走内存 `tokio::io::duplex`；`PipeSecurity`/DACL 创建、
  `client_sid`、`current_user_sid_string` 在该环境**未被调用**。
- **未做真实路由/适配器配置**：`CreateIpForwardEntry2`、`DeleteIpForwardEntry2`、
  `CreateUnicastIpAddressEntry`、`DeleteUnicastIpAddressEntry` 从未对 OS 调用；
  `WindowsBackend` 的路由/TUN 路径**编译但未执行**。
- **未提权启动内核**：`CreateProcessW`/`CreateJobObjectW`/`AssignProcessToJobObject`/`TerminateProcess`
  未对真实内核调用。
- `windows.rs` 单测只覆盖**纯函数**：`quote_arg`/`command_line`、`sockaddr_from_ip`、`forward_row`
  （不触达 OS）。
- 结论：本回合为**协议/校验/编排逻辑的离机验证**；平台真实行为未验证（对应状态：实现 implemented，
  未实测部分未验证）。

## 4) 未决项清单

1. **未在提权上下文运行**：daemon 的 `--serve` 生命周期、真实管道并发/多实例、SID 拒绝的真实取值均未验证。
2. **未做真实路由/适配器配置**：IP Helper API 的成败码、路由优先级/度量、TUN 地址冲突与回滚、
   IPv6 前缀行为未验证；`reset_tun_address` 依赖本地 registry 记忆，跨进程重启后无法回滚未登记项。
3. **非 Windows 后端未实现**：`main.rs` 在非 Windows 只打印“不支持”；`windows` 模块 cfg(windows)；
   Unix domain socket 后端、权限模型与恢复策略均缺失。
4. **与 net-host 的租约联调未做**：`services/net_host`/`bridge_api` 目前**没有** helper 客户端；
   会话令牌下发、`expected_sid` 与 token 的握手、资源所有权交接、net-host 侧断连检测/恢复编排
   （§13 日志重读）均未接线，helper 租约尚未被任何上层驱动。
5. **路径校验是词法而非规范化**：不解析符号链接/重解析点/8.3 短名/硬链接，校验与创建之间存在 TOCTOU；
   `args` 只约束长度与字符，不约束参数语义（如 `--config` 指向 `run_dir` 之外的路径）。
6. **Job Object 约束不完整**：未设置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，未忽略
   `AssignProcessToJobObject` 的失败码，`CreateJobObjectW` 失败时仍会继续启动内核；
   helper 崩溃不能保证提权内核随之终止。
7. **租约清理为 best-effort**：`on_disconnect` 内对删除/重置/停止的返回值一律 `let _ =` 忽略，
   失败不回滚、不重试、不外显；helper 内**没有**持久恢复日志（§13 的恢复日志属 net-host 职责）。
8. **审计仅为内存**：4096 上限、FIFO 淘汰、无落盘/轮转/外部 sink、无限流；重启即丢失。
9. **进程身份**：helper 以内核句柄 `u64` 标识，不绑定 PID+创建时间（§13 要求 PID 加创建时间防止 PID 复用）；
   真实身份校验留给 net-host 侧。
10. **字段布局**：`SockaddrInet` 等 `#[repr(C)]` 结构仅在当前 x64 编译下测试，未在 ARM64/32 位验证。
11. **未 commit**。
