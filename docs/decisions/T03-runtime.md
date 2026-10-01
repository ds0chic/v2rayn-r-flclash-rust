# ADR T03-runtime — 运行时会话、进程所有权与客户端/服务端边界

- 状态：accepted（T03）
- 关联：`crates/runtime`、`services/net_host`、`crates/application`、
  `crates/bridge_api`、`apps/desktop/lib/features/runtime`
- 证据：`docs/evidence/T03.md`、`docs/evidence/T03.runs/`

## 背景

UI 不拥有内核状态；唯一内核进程持有者是 net-host。AppEngine 通过命名管道向
net-host 下发不可变 `RuntimePlan`，并只消费 snapshot + 事件流。需要在“GUI 可能
被强杀、net-host 可能被强杀、core 可能秒退”的前提下保证：核心不残留、状态不
伪造、恢复可审计、所有等待有上限。

## 决策

### D1. 进程所有权：Job Object + `(pid, 创建时间)`
net-host 为唯一持有者。每个 core 通过 `JobGuard::create_kill_on_close()`
（`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`）归属到 Job；net-host 即使被
`TerminateProcess`，句柄随进程销毁关闭，Job 自动终结 core 树。
任何终止/恢复都只针对 `(pid, creation_time)` 匹配的进程，绝不按名杀。
日志记录 PID + 创建时间（Windows FILETIME→Unix ms），跨重启防 PID 复用。

### D2. 租约与失联策略
客户端活性 = 连接存续（本地命名管道上 OS 即能感知对端关闭）。
- `active_connections == 0` 超过 `disconnect_grace`（默认 6s）→ 停止 managed core。
- 只要存在任一连接（含只读事件连接），租约保持，即使空闲也不回收。
- server→client 方向由 watchdog 周期性 `heartbeat` 事件承载“net-host 仍存活”。
不采用“要求客户端周期性发请求”的 client→server 心跳：本地管道下它既冗余，
又会因 GUI 空闲而误杀 core。

### D3. 恢复日志是提示而非事实
`run_root/<session>/journal.json` 记录 session/plan/revision/config hash/pid/
创建时间/port/stage，不写配置正文与凭据。
- Running 期间 journal 保持 `applied`；**只有 stop/rollback 才写 `finalized`**。
  早期版本在 Running 时写 `finalized`，导致崩溃后无法恢复（已修复）。
- 启动时 `recover_stale` 逐个核对 `(pid, creation_time)`：命中则按身份终止；
  若终止与“正在退出”的进程竞态失败，则复核身份，进程已消失即视为已清理。
  修复前后用例 d 表现差异见证据 §4。
- 恢复会打印 `[net_host] recovery session=... stage=... pid=... cleaned=...`。

### D4. Windows 同步命名管道的 I/O 纪律
同步管道句柄按 FileObject 串行，`File::try_clone` 得到的两个句柄共享同一
FileObject；一个线程阻塞读会卡住另一线程写。因此 NetHostClient：
- 控制请求：**每次请求一个独立短连接**，在同一线程内写请求、读响应，调用方
  用 `mpsc::recv_timeout` 限时；apply 上限 `IPC_APPLY_TIMEOUT_MS`(60s)。
- 事件：一条独立长连接，同一线程内先 `SubscribeEvents`、之后只读；断开按固定
  退避重连。该连接同时维持租约。
该模型避免死锁且保证每个调用有超时。

### D5. 管道安全
net-host 用当前用户 SID 构造 SDDL `D:P(A;;GA;;;<sid>)` 的
`SECURITY_ATTRIBUTES`，`reject_remote_clients(true)`，最大 16 实例。
`SessionIdentity` 校验协议版本与非空 token。管道名默认
`\\.\pipe\v2rayn-r-net-host`，测试用随机名隔离。

### D6. 端口与特权
测试/冒烟入站端口一律 ≥ 11808，**严禁 10808**（用户正在运行的代理）。
冒烟计划用 config_codegen 的 Xray 生成器：mixed/socks 入站
`127.0.0.1:11808` + `freedom` 出站；并可用 `xray run -test` 校验。
plan 校验、config hash 校验、端口预检（bind 后释放）在 spawn 前完成。

### D7. bridge/UI 边界
- `cfg!(test)` 下 bridge 使用内存引擎，生产使用 `NetHostClient`；单测不触碰
  进程/管道。
- `get_snapshot/apply_runtime/stop_runtime` 为 FRB 异步函数，避免阻塞 UI 线程。
- UI 的运行状态只来自 snapshot/events；apply/stop 失败保留结构化 `ErrorDto`，
  不被后续 snapshot 覆盖；未运行时显示 `未运行/Stopped/host=down`，绝不伪造
  Running。

## 后果

- 正向：进程树不残留、恢复可审计、失效路径全部有界、UI 无法伪造运行状态。
- 代价：控制请求为短连接（每次一次建连），事件长连接断开后不回放历史事件；
  apply 为“完成才返回”语义，启动阶段由事件驱动显示。
- 后续：如需取消 in-flight apply，可在 IPC 合同层增加 cancel 操作；如需
  多实例并发，可改 overlapped I/O 与真正的请求多路复用。
