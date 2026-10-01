# T02 — IPC / 领域 / FRB 契约

- 任务：T02 领域模型与 IPC 契约（代码已完成，本文件为契约固化文档）
- 上游基线：v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`
- 事实源（以代码为准，全部路径为仓库相对路径）：
  - `crates/ipc_contract/src/lib.rs` — net-host ↔ AppEngine 的 IPC 消息集
  - `crates/domain/src/{error,event,job,revision,runtime_plan,profile,entities,settings,reference,enums}.rs`
  - `crates/application/src/{engine,jobs,repository,snapshot,runtime_client}.rs`
  - `crates/bridge_api/src/api/{contract,engine}.rs` — FRB 表面与扁平 DTO
  - `compat/domain-map.yaml` — Rust 类型 ↔ FLD-* ↔ 存储 的映射大表
- 约定：本回合不运行上游内核、不下载内核、不触碰 `127.0.0.1:10808`；不修改任何代码/台账。

> 本文件描述的是"代码当前实际承诺的契约"，不是设计意图。行号对应本仓库当前提交状态。

---

## 1. IPC 契约（`crates/ipc_contract`）

### 1.1 协议与限额常量

| 常量 | 值 | 位置 |
|---|---|---|
| `IPC_PROTOCOL_VERSION` | `1` | `crates/ipc_contract/src/lib.rs:21` |
| `IPC_MAX_MESSAGE_BYTES` | `8 * 1024 * 1024`（单个 frame 上限，仅控制消息；配置按引用暂存） | `:25` |
| `IPC_REQUEST_TIMEOUT_MS` | `5_000` | `:28` |
| `IPC_APPLY_TIMEOUT_MS` | `60_000` | `:31` |
| `IPC_HEARTBEAT_INTERVAL_MS` | `2_000` | `:34` |
| `IPC_HEARTBEAT_MISS_THRESHOLD` | `3` | `:37` |

`timeout_for(op)` 对 `ApplyPlan` / `StopRuntime` / `Shutdown` 返回 `IPC_APPLY_TIMEOUT_MS`，其余返回
`IPC_REQUEST_TIMEOUT_MS`（`:238-244`）。

### 1.2 消息集

`IpcOperation` 是**封闭枚举**（`#[serde(tag = "op", rename_all = "snake_case")]`），注释明确说明
未知操作无法表达，杜绝任意命令走私（`:68-71`）。操作如下（`:72-87`）：

| 操作 | 载荷 | 语义 |
|---|---|---|
| `GetSnapshot` | — | 读取当前运行快照与启动恢复状态 |
| `ApplyPlan` | `plan: Box<RuntimePlan>` | 应用一个不可变运行计划 |
| `StopRuntime` | `operation_id: Option<String>` | 停止受管运行（幂等） |
| `GetOperation` | `operation_id: String` | 查询先前的操作/作业状态 |
| `SubscribeEvents` | `epoch: EventEpoch, from_seq: u64` | 从指定 epoch/seq 订阅事件流 |
| `Shutdown` | — | 清理后优雅关闭 net-host |
| `TestSession` | `TestSessionOperation` | 受限测试会话操作（仅测试内核） |

`TestSessionOperation`（`tag = "test_op"`，`:90-104`）：`Open { ports, max_duration_ms }` /
`Close { session_id }` / `Status { session_id }`，注释声明"无任意 shell"。

Proxy/TUN 变更被折叠进单一 `ApplyPlan`，保证任一时刻只有一个未协调写入者（`:6-7`）。

### 1.3 会话身份与信封

- `SessionIdentity { protocol_version, session_token, peer_pid, peer_created_at_ms }`（`:41-50`）；
  `peer_created_at_ms` 用于击败 PID 复用；注释声明 token 永不写日志（`:39`）。
- `RequestEnvelope { session, request_id, operation }`（`:54-59`），`request_id` 在响应中回显。
- `ResponseEnvelope { request_id, result }`（`:63-66`）。

### 1.4 结果与运行快照

- `IpcResult`（`tag = "kind"`，`:109-118`）：`Snapshot` / `Accepted { operation_id }` /
  `Operation(OperationStatus)` / `EventStreamOpened { epoch, from_seq }` / `Stopped` / `Shutdown` /
  `TestSession { session_id, state }` / `Error(DomainError)`。
- `RuntimeSnapshot`（`:123-138`）：`state`、`applied_revision`、`epoch`、`last_seq`、
  `active_operation`、`recovery`、`host_alive`。注释明确 **net-host 从不写业务数据库**（`:120-121`）。
- `RecoveryStatus`（`:142-151`）与 `RecoveryStage`（`Prepared/Applying/Applied/Finalized`，`:157-162`）。
  注释：恢复日志只是提示，不是系统真值，动作前始终从 OS 重新读取状态（`:154-156`）。
- `OperationStatus`（`:166-172`）：`operation_id`、`job_id`、`state`、`cancel: Option<CancelOutcome>`、`error`。

### 1.5 校验与错误桥接

| 函数 | 作用 | 位置 |
|---|---|---|
| `check_frame_size(len)` | 超过 `IPC_MAX_MESSAGE_BYTES` 返回 `MessageTooLarge` | `:210-219` |
| `check_session(session)` | 协议版本不符 → `VersionMismatch`；空 token → `Unauthorized` | `:222-235` |
| `timeout_for(op)` | 按操作返回超时 | `:238-244` |
| `check_test_ports(ports)` | 测试端口下限 `11_808`，低于则 `Malformed` | `:247-255` |

`IpcError`（`:177-182`）是 framing/校验错误，与领域错误分离；`IpcError::to_domain()`（`:185-206`）将其
映射为 `DomainError` 稳定码：`IPC_VERSION_MISMATCH` / `IPC_MESSAGE_TOO_LARGE` / `INVALID_ARGUMENT`
（malformed）/ `PERMISSION_DENIED`（unauthorized）。

---

## 2. 错误码表（`crates/domain/src/error.rs::codes`，`:17-38`）

错误码是 IPC 与 FRB 契约的一部分，改动需协议版本升级（`:15-16`）。

| 码 | 含义 | 图示位置 |
|---|---|---|
| `E_INVALID_ARGUMENT` | 参数非法 | `:18` |
| `E_INVALID_ENUM` | 枚举值无法识别 | `:19` |
| `E_FIELD_REQUIRED` | 必填字段缺失 | `:20` |
| `E_FIELD_RANGE` | 字段越界 | `:21` |
| `E_FIELD_FORMAT` | 字段格式错误 | `:22` |
| `E_REVISION_STALE` | 期望 revision 已过期 | `:23` |
| `E_NOT_FOUND` | 目标不存在 | `:24` |
| `E_CONFLICT` | 冲突 | `:25` |
| `E_INVALID_PLAN` | 运行计划非法（如重复 tag/进程） | `:26` |
| `E_GRAPH_CYCLE` | 依赖图成环 | `:27` |
| `E_PORT_CONFLICT` | 端口冲突 | `:28` |
| `E_DANGLING_REFERENCE` | 悬空引用 | `:29` |
| `E_CANCELLED` | 已取消 | `:30` |
| `E_NOT_CANCELLABLE` | 已过安全点不可取消 | `:31` |
| `E_TIMEOUT` | 超时 | `:32` |
| `E_IPC_VERSION_MISMATCH` | IPC 版本不符 | `:33` |
| `E_IPC_MESSAGE_TOO_LARGE` | IPC 帧超限 | `:34` |
| `E_PERMISSION_DENIED` | 权限拒绝 | `:35` |
| `E_UNAVAILABLE` | 运行端不可用 | `:36` |
| `E_INTERNAL` | 内部错误 | `:37` |

`DomainError`（`:42-55`）字段：`code`、`message_key`（UI 侧本地化键，绝不返回原始 panic/stdout）、
`field_path`、`retryable`、`operation_id`、`detail`（脱敏诊断，无凭据、无原始配置）。
构造助手：`with_field`/`with_operation`/`retryable`/`with_detail`（`:69-87`）、
`invalid_enum`（`:91-95`）、`invalid_argument`（`:97-99`）、`stale_revision`（`:101-104`）、
`not_found`（`:106-110`）。

---

## 3. 事件契约（`crates/domain/src/event.rs`）

### 3.1 epoch / seq

- `EventEpoch(pub u64)`（`:23`）：net-host 每产生一代新运行时递增（`:18-19`）。
- `EventSeq(pub u64)`（`:40`）：epoch 内单调序号；`is_contiguous_with` 判断是否恰好后继（`:48-50`）。
- `EventEnvelope::follows(previous)`（`:174-176`）用于检测断流。

### 3.2 分道与分类

`EventChannel`（`:57-62`）：`Control`（可靠、绝不因日志/流量压力被丢弃）与 `Telemetry`（尽力而为、
按族独立限流）。`EventKind::channel()`（`:139-147`）划分：

- Control：`RuntimeStateChanged`、`JobProgress`、`JobFinished`、`ErrorRaised`、`RevisionChanged`
- Telemetry：`TrafficDelta`、`LogBatch`、`ConnectionsBatch`、`SpeedTestBatch`

`EventKind` 共 10 个变体，含 `Other(String)`（`:67-88`）。未知 kind 反序列化为 `Other`，新生产端不会
击穿旧消费端（`:64-65`, `:117-136`）；`as_str()` 给出线格式 token（`:91-105`）。

### 3.3 载荷

- `EventEnvelope { epoch, seq, kind, payload: Value }`（`:156-161`）。
- `RuntimeStateChanged { state, applied_revision, message_key }`（`:181-186`）。
- `RuntimeState` 状态机（`:189-201`）：`Stopped`（默认）、`Validating`、`Preparing`、`Starting`、
  `Checking`、`Running`、`RollingBack`、`Degraded`。
- `JobEvent { job_id, state, percent: Option<u8>, stage_key, error }`（`:205-212`）。`percent`
  未知时为 `None`——绝不伪造（`:207-208`）。

---

## 4. 作业与取消语义（`crates/domain/src/job.rs`）

- `JobState`（`:37-46`）：`Running`、`Cancelling`、`Compensating`、`Done`、`Failed`、`Cancelled`；
  `is_terminal()` 判定 `Done|Failed|Cancelled`（`:49-53`）；`as_u8`/`from_u8` 用于原子存储（`:56-77`）。
- `CancelOutcome`（`:83-92`）：`Requested`（已受理，将在安全点停止）、`Compensating`（正在回滚已生效
  副作用）、`AlreadyFinished`（请求前已结束）、`NotCancellable`（已过安全点，必须跑完）。
- `CancellationToken`（`:98-157`）：`cancel()` 幂等且无锁（`swap` 原子位，`:122-128`）；克隆共享同一
  flag（`:184-188` 测试）；`check()` 在安全点返回 `E_CANCELLED`（`:135-144`）。
- `JobManager::cancel`（`:116-145`）判定优先级：
  1. 未知作业或终态 → `AlreadyFinished`；
  2. `past_safe_point` → `NotCancellable`（不触碰 token，作业继续跑完）；
  3. 首次请求 → `Requested`；已处于 `Cancelling` → `Compensating`。
- `mark_past_safe_point`（`:148-154`）标记越过不可取消的安全点。
- 运行相关动作串行收敛（`:3`）；本骨架不 spawn 线程，由调用方驱动并回调 `finish`（`:7-8`）。

> 取消是协作式的，只在安全点发生；越过 no-return point 的作业必须先补偿，UI 显示
> "cancelling"/"recovering"，绝不显示假的立即完成（`:4-6`）。

---

## 5. desired / applied revision 契约（`crates/domain/src/revision.rs`）

- `DesiredRevision`（`:15-35`）：AppEngine 每次持久化变更递增。
- `AppliedRevision`（`:44-60`）：net-host 报告实际运行到的版本。
- 单一 `activeRevision` 绝不用于同时表示数据库状态与运行状态（`:4-5`）。
- `RevisionPair::state()`（`:95-105`）：
  - 两者均为 0 → `Empty`
  - `applied > desired` → `Ahead`（被其他写入者/旧会话推进，必须协调而非盲目覆盖）
  - `applied == desired` → `InSync`
  - 否则（`desired > applied`）→ `Pending`（"已保存未应用"，合法且可显示）
- `check_expected(expected)`（`:110-122`）是 `save_profile`/`save_settings` 的乐观并发检查：不匹配返回
  `E_REVISION_STALE`。

`application::RevisionStore`（`crates/application/src/repository.rs:199-237`）持有 `DesiredRevision`，
`bump()` 递增（`:221-224`）、`check()` 校验（`:227-236`）。

---

## 6. RuntimePlan 契约与校验（`crates/domain/src/runtime_plan.rs`）

### 6.1 结构

`RuntimePlan`（`:425-442`）：`plan_id`（重试间稳定）、`desired_revision`、`target`、
`process_graph`、`outbound_graph`、`ports`、`privileges`、`network_policy`、`resources`。

- `RuntimeTarget`（`:415-421`）：`core_type`、`version`、`config: ConfigSource`、`config_sha256`。
- `ConfigSource`（`:44-54`）：`Inline { body }`（net-host 复制进私有运行目录）或
  `ControlledFile { staged_token, sha256 }`（**绝不接受调用方提供的绝对路径**，`:47-53`）。
- `PortRequest`（`:58-66`）：`port`、`transport`、`owner`、`exclusive`；
  `PortTransport::{Tcp,Udp,Both}`，`overlaps` 定义 `Both` 与 Tcp/Udp 重叠（`:88-100`）。
- `RequiredPrivilege`（`:105-114`）：`None`/`Tun`/`SystemProxy`/`PrivilegedPort`。
- `NetworkPolicy`（`:118-125`）：`system_proxy: Option<SysProxyType>`（`None` = 不动）、
  `tun_enabled`、`bypass`。
- `ContentHash`（`:23`）为十六进制编码的资源哈希。

### 6.2 OutboundGraph（in-core detour，`:150-260`）

`OutboundNode { tag, profile_id, protocol }`（`:132-139`）、`OutboundEdge { from, to }`（`:143-146`）
表示 `from` 经 `to` 出站。校验：

| 校验 | 结果码 | 位置 |
|---|---|---|
| `validate_unique_tags` 重复 tag | `E_INVALID_PLAN` | `:249-259` |
| `validate_references` 边端点无节点（悬空） | `E_DANGLING_REFERENCE` | `:225-246` |
| `validate_acyclic` DFS 找环（含完整环路径） | `E_GRAPH_CYCLE` | `:172-222` |

### 6.3 ProcessGraph（多进程拓扑，`:286-410`）

`ProcessNode { id, core_type, config, ports, privileges }`（`:264-273`）、
`ProcessEdge { before, after }`（`:278-281`）。

- `start_order()`（`:308-378`）：稳定拓扑序；重复 id → `E_INVALID_PLAN`；边引用未知进程 →
  `E_DANGLING_REFERENCE`；残留环 → `E_GRAPH_CYCLE`。
- `validate_ports()`（`:384-410`）：同一端口且 transport 重叠、且属不同 owner →
  `E_PORT_CONFLICT`（仅当 `exclusive`）。
- `RuntimePlan::stop_order()`（`:485-489`）：启动序的反序。

### 6.4 顶层校验顺序

`RuntimePlan::validate()`（`:446-454`）依次调用：
`outbound_graph.validate_unique_tags` → `validate_references` → `validate_acyclic` →
`process_graph.start_order` → `process_graph.validate_ports` → `validate_port_set`，
返回**第一个**结构化错误。`validate_port_set()`（`:458-482`）对计划级 `ports` 做同样重叠检测。

---

## 7. 领域模型概览与未知扩展保留

`domain` 不依赖 Flutter/窗口/平台 API（`crates/domain/src/lib.rs:1-15`）；`crates/domain/src/lib.rs:20`
显式 `#![allow(clippy::result_large_err)]`，理由为 `DomainError` 是刻意共享的错误契约类型。

| 模块 | 内容 | 关键位置 |
|---|---|---|
| `enums` | `ConfigType`(15)、`CoreType`(15)、`Network`(9)、`RuleType`、`MultipleLoad`、`SysProxyType`、`GirdOrientation`、`RuleMode`、`InboundProtocol`、`Security`、`SpeedTestAction` | `enums.rs:13,143,287,336,378,390,436,457,500` |
| `profile` | `Profile`(40) + `ProtocolExtra`(30) + `TransportExtra`(11) + `SecurityParams`(13) | `profile.rs:26,104,125,151` |
| `entities` | `Subscription`(17)、`RoutingProfile`(13)、`RoutingRule`(13)、`DnsProfile`(9)、`FullConfigTemplate`(8)、`TrafficStats`(6) 及 `MigrationRecord`/`TaskRecord`/`CoreInstallation`/`WindowState`/`ColumnDefinition`/`GlobalHotkey`/`CoreTypeBinding`/`InboundListener` | `entities.rs:18,92,61,131,166,198,212,228,241,258,273,284,304` |
| `settings` | `AppSettings` 根（25：2 ids + 23 items）及 23 个 `ConfigItems` 子类 | `settings.rs:430` 起 |
| `reference` | 六类引用语义 `REF-ENT-001..006` | `reference.rs:24-147` |

**未知扩展保留策略**（plan §11："不能为了强类型而删除未知扩展"）：每个领域 struct 均带
`#[serde(flatten)] extra: ExtraMap`（`ExtraMap = Map<String, Value>`，`profile.rs:19`）：

- `Profile.extra`（顶层未知键）：`profile.rs:183-184`
- `ProtocolExtra.extra` / `TransportExtra.extra`：`profile.rs:79-80,118-119`
- `entities` / `settings` 的每个 struct 同样带 `extra`（如 `entities.rs:40-41`、`settings.rs:32-33`）。
- 语义验证：`profile.rs:311-326` 证明未知顶层键往返不丢；`:329-335` 证明 proto extra 未知键往返不丢；
  `settings.rs:481-490` 证明 settings 未知键往返不丢。

> `compat/domain-map.yaml` 中 `fld: null` 的 `extra` 条目即对应这些保留槽；`Profile.extra`
> 还以**一条复合映射**承载 9 个已废弃 `FLD-ENT-*`（`domain-map.yaml:124,127`）。

### 引用表达式（`reference.rs`）

`ReferenceKind`（`:24-35`）：`StringRemarks`(001/002)、`CommaSeparatedIndexIds`(003)、
`SubscriptionIdPlusRegex`(004)、`IdRemap`(005)、`SentinelString`(006)。`ReferenceSource`
（`:53-60`）：`Imported`/`Native`/`Remapped`。`ReferenceExpr`（`:68-82`）保留 `raw` 原文、解析目标
`targets`、可选 `filter`、`is_self` 与来源；`to_raw()` 保证未修改的导入表达式原样回写（`:149-153`）。
`SELF_SENTINEL = "self"`（`:64`）表示"拥有该 group 的 subscription"。

---

## 8. FRB 表面（`crates/bridge_api/src/api`）

### 8.1 DTO（`contract.rs`）

DTO 与富领域类型刻意解耦：领域含 `serde_json::Map` 未知扩展包，FRB 无法翻译，因此 DTO 只携带 UI 所需
字段，Rust 侧领域仍是真值源（`contract.rs:1-6`）。DTO 列表：
`RevisionDto`、`JobDto`、`CapabilityDto`、`RecoveryDto`、`SnapshotDto`、`ErrorDto`、`ProfileFilterDto`、
`ProfileSortDto`、`ProfileDto`、`ProfilePageDto`、`SaveProfileResult`、`ApplyRuntimeResult`、
`CancelResult`、`EventEnvelopeDto`、`ApplyRuntimeRequest`（`:16-175`）。

关键点：
- `ProfileDto.extra_json: String` 以 JSON 字符串承载未知扩展（`:122-123`），在 `engine.rs:87-89` 反序列化回
  领域 `extra`，`:55` 序列化出去。
- `ErrorDto: From<DomainError>`（`:77-88`）逐字段透传稳定契约字段。
- `EventEnvelopeDto { epoch, seq, kind, control, payload_json }`（`:160-168`），`control` 标记
  控制/遥测分道。

### 8.2 FRB 函数（`engine.rs`）

单一进程级内存引擎 `static ENGINE: OnceLock<AppEngine>`（`:31`），`#[frb(sync)]` 同步函数：

| 函数 | 位置 | 说明 |
|---|---|---|
| `get_snapshot` | `:96-150` | 组装 `SnapshotDto`（revision/runtime/jobs/capabilities/recovery/profile_count） |
| `query_profiles` | `:154-187` | filter/sort/cursor/page_size |
| `save_profile` | `:192-208` | draft + `expected_revision`，返回保存实体与新 revision 或字段错误 |
| `apply_runtime` | `:242-262` | target_id + `expected_revision`，返回 operation id |
| `cancel_job` | `:266-272` | 幂等取消 |
| `subscribe_events` | `:323-336` | FRB stream；订阅首帧先发当前 epoch/seq（`stream_opened`），便于重连检测缺口 |
| `emit_test_event` | `:340-343` | 测试辅助：注入合成控制事件 |
| `subscriber_count` | `:347-349` | 诊断/测试 |
| `mark_job_past_safe_point` | `:353-355` | 测试辅助：取消流 |
| `seed_synthetic_profiles` | `:360-365` | 合成数据（RFC5737/example 域） |
| `profile_count` | `:369-371` | 当前内存 profile 数 |

`stub_plan()`（`:215-237`）是 T02 的占位计划构建器：`plan_id` 含 target 与 revision，`ports` 为空，
因此**永远不会触碰用户正在运行的代理端口**；注释明确 T03 用真实 `config_codegen` 输出替换（`:210-214`）。

### 8.3 事件流 fan-out

`broadcast()`（`:312-319`）向所有存活 sink 投递，关闭的 sink 被剔除；控制事件不因压力丢失
（`:291-299`）。FRB 层生成自增 epoch/seq（`:38-48,275-283`）。

---

## 9. 应用层契约（`crates/application`）

- `AppEngine`（`engine.rs:25-30`）组合 repository + revision store + job manager + runtime client；
  `in_memory()` 使用 `NullRuntimeClient`（`:34-36`）。T03 替换 runtime client，T04 替换 repository
  （`:1-6`）。
- `save_profile`（`:75-94`）：先 `check(expected)`（`:84`）→ `draft.validate()`（`:85`）→ `upsert`（`:91`）
  → `revisions.bump()`（`:92`）。
- `apply_runtime`（`:100-127`）：先 `check(expected)`（`:110`）→ `plan.validate()`（`:112`）→
  `runtime.apply`（`:114`）；`Accepted` 时启动作业并返回 `"{operation_id}:{job_id}"`（`:115-120`），
  `Unavailable` 时返回可重试的 `E_UNAVAILABLE`（`:121-125`）。
- `cancel_job`（`:130-132`）委托 `JobManager`。
- `snapshot`（`:135-156`）由 `assemble`（`snapshot.rs:58-77`）组装，运行字段一律来自 `RuntimeClient`，
  绝不从按钮点击推断（`snapshot.rs:4-6`）。
- `capability_table()`（`:178-230`）为静态能力表：Xray 支持 8 型、sing-box 支持 11 型、Mihomo 仅
  Custom/Outbound、V2fly 子集；仅 Xray 与 sing-box 标记 `structured_generation=true`。
- `RuntimeClient` 契约（`runtime_client.rs:38-50`）：`snapshot`/`apply`/`stop`/`cancel`；`NullRuntimeClient`
  （`:57-119`）仅记录最后一个 plan，不触进程。

---

## 10. 不变量（跨契约）

1. **单一写入者**：Proxy/TUN 变更只经一个 `ApplyPlan`，不存在两个未协调写入者（`ipc_contract/src/lib.rs:6-7`）。
2. **运行层与业务层分离**：net-host 从不写业务数据库（`ipc_contract/src/lib.rs:120-121`）；UI 不拥有运行状态。
3. **revision 分离**：`desired` 与 `applied` 是两个不同提交，`desired > applied` 合法且可显示（`revision.rs:4-10`）。
4. **乐观并发**：任何变更携带 `expected_revision`，过期即拒绝，绝不静默覆盖（`revision.rs:106-122`）。
5. **作业串行 + 幂等取消**：运行相关动作串行收敛；取消幂等且协作式（`jobs.rs:3-5`, `job.rs:80-92`）。
6. **信息不伪造**：进度未知即 `None`（`event.rs:207-208`、`jobs.rs:23-24`）；`allowInsecure` 等敏感项按各生成契约处理。
7. **未知扩展不丢**：所有持久化 struct 带 `extra` flatten（§7）。
8. **错误可机读**：稳定 `code` + `message_key` + `field_path` + `retryable`（§2）。
9. **安全端口**：测试端口下限 11808（`ipc_contract/src/lib.rs:247-255`、`engine.rs:227-236`）。

---

## 11. 复核命令

```powershell
# 契约类型与内联测试
Get-Content crates\ipc_contract\src\lib.rs
Get-Content crates\domain\src\error.rs, crates\domain\src\event.rs, crates\domain\src\job.rs, crates\domain\src\revision.rs, crates\domain\src\runtime_plan.rs
Get-Content crates\bridge_api\src\api\contract.rs, crates\bridge_api\src\api\engine.rs

# 以代码路径:行号核对（示例）
rg -n "IPC_PROTOCOL_VERSION|IPC_MAX_MESSAGE_BYTES|check_test_ports|pub enum IpcOperation" crates\ipc_contract\src\lib.rs
rg -n "pub mod codes|stale_revision|pub enum EventKind|pub enum CancelOutcome|pub fn validate" crates\domain\src
```
