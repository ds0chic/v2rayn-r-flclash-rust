# T02 — 架构决策记录与修订历史

- 任务：T02 领域模型与 IPC 契约
- 配套契约：`docs/decisions/T02-contracts.md`、`docs/evidence/T02.md`
- 上游基线：v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`
- 说明：每条决策给出背景、选择、被拒备选与代码证据（`路径:行号`）。凡代码注释已给出理由的，直接引用。

---

## D-T02-01 运行相关变更折叠为单一 `ApplyPlan`

- **背景**：代理端口、TUN、系统代理等在运行期都需要写 OS/网络状态，若有多条独立命令并发下发，会出现
  互不协调的写入者与竞态。
- **决策**：IPC 操作集中为 `GetSnapshot` / `ApplyPlan` / `StopRuntime` / `GetOperation` /
  `SubscribeEvents` / `Shutdown` + 受限 `TestSession`；Proxy/TUN 变更全部折叠进一个不可变 `ApplyPlan`。
- **理由（代码原文）**：*"Proxy/TUN changes are folded into a single `ApplyPlan` so there is never more
  than one uncoordinated writer."*（`crates/ipc_contract/src/lib.rs:6-7`）
- **备选（被拒）**：为 proxy/tun/dns 各开独立 IPC 命令——拒绝，会引入多写入者。
- **后果**：`RuntimePlan` 必须承载完整的进程图、端口集、特权与网络策略（见 D-T02-08）。

## D-T02-02 IPC 操作集是封闭枚举，测试会话是受限子枚举

- **背景**：调试/测试需要在内核上绑定端口，但普通通道不得变成任意命令执行面。
- **决策**：`IpcOperation` 为封闭 `enum`（`#[serde(tag = "op")]`），注释明确 *"Unknown operations
  cannot be represented; the enum is closed on purpose so no arbitrary command can be smuggled
  through."*（`crates/ipc_contract/src/lib.rs:68-71`）。测试能力单独建模为 `TestSessionOperation`
  （`Open`/`Close`/`Status`），注释声明 *"No arbitrary shell."*（`:89-104`）。
- **安全约束**：`check_test_ports` 强制端口下限 `11_808`，确保保留的用户代理端口不会被测试占用
  （`:247-255`）。
- **后果**：新增能力必须显式改枚举并升协议版本（`IPC_PROTOCOL_VERSION`，`:20-21`）。

## D-T02-03 会话身份携带协议版本与防 PID 复用信息

- **背景**：本地 IPC 也要防止版本错配、陈旧连接与 PID 复用导致的串话。
- **决策**：`SessionIdentity { protocol_version, session_token, peer_pid, peer_created_at_ms }`，
  `peer_created_at_ms` 用于击败 PID 复用（`crates/ipc_contract/src/lib.rs:41-50`）；`check_session`
  校验版本与空 token（`:222-235`）。注释：token 永不写日志（`:39`）。
- **备选（被拒）**：仅用 PID 标识对端——拒绝，PID 会被复用。
- **后果**：握手失败映射为 `E_IPC_VERSION_MISMATCH` / `E_PERMISSION_DENIED`（`:185-206`）。

## D-T02-04 `desired_revision` 与 `applied_revision` 严格分离

- **背景**：保存设置与把设置应用到运行内核是两个不同提交，历史上"单一 activeRevision"会混淆数据库状态
  与运行状态。
- **决策**：`DesiredRevision` 随 AppEngine 每次持久化递增；`AppliedRevision` 由 net-host 报告实际运行到的
  版本；`desired > applied` 是合法可显示状态（`crates/domain/src/revision.rs:4-10`）。
- **衍生状态**：`RevisionState::{Empty, InSync, Pending, Ahead}`；其中 `Ahead` 表示运行端领先，需协调而非
  覆盖（`revision.rs:70-105`）。
- **备选（被拒）**：用数据库版本号统一表示两者——拒绝，无法表达"已保存未应用"。
- **后果**：快照必须同时报告 `desired`、`applied` 与派生 `revision_state`（`crates/application/src/snapshot.rs:38-55`）。

## D-T02-05 变更走乐观并发，过期 `expected_revision` 一律拒绝

- **背景**：设置界面可能持有陈旧视图，直接覆盖会丢更新。
- **决策**：`save_profile` / `save_settings` / `apply_runtime` 均携带 `expected_revision`；
  `RevisionPair::check_expected` 不匹配即返回 `E_REVISION_STALE`（`crates/domain/src/revision.rs:106-122`）。
  应用层在 `save_profile`（`engine.rs:84`）与 `apply_runtime`（`engine.rs:110`）校验。
- **备选（被拒）**：last-write-wins——拒绝，会产生静默数据丢失。
- **后果**：UI 必须处理 `E_REVISION_STALE` 并重新拉取快照（`field_path` 定位字段）。

## D-T02-06 取消是协作式、幂等，且区分"可取消/需补偿/已结束/不可取消"

- **背景**：启动/停止涉及真实进程与网络副作用，无法原子回滚；UI 需要如实反馈。
- **决策**：`CancellationToken` 的 `cancel()` 幂等且无锁（`crates/domain/src/job.rs:122-128`），克隆共享
  flag；作业在安全点 `check()`（`:135-144`）。`JobManager::cancel` 按优先级返回四种 `CancelOutcome`
  （`:116-145`）：
  - 未知/终态 → `AlreadyFinished`
  - 越过安全点（`past_safe_point`）→ `NotCancellable`，不触碰 token，作业跑完
  - 首次 → `Requested`
  - 已 `Cancelling` → `Compensating`
- **理由（代码原文）**：*"Once a job has entered the point of no return (an OS/network side effect)
  it must compensate first; the UI shows 'cancelling' / 'recovering', never a fake immediate
  completion."*（`job.rs:4-6`）
- **备选（被拒）**：强制杀进程/立即返回成功——拒绝，破坏副作用一致性并欺骗 UI。
- **后果**：`JobState` 含 `Cancelling` 与 `Compensating` 两个中间态（`job.rs:37-46`）。

## D-T02-07 事件按控制/遥测分道，`epoch`+`seq` 支持缺口检测

- **背景**：状态迁移与作业进度必须可靠送达；流量/日志/连接等高频数据不能挤占控制通道。
- **决策**：`EventKind::channel()` 把事件分为 `Control`（可靠、绝不被压力丢弃）与 `Telemetry`
  （按族独立限流）（`crates/domain/src/event.rs:57-62,139-152`）。每条事件带 `epoch`（运行时换代递增）
  与单调 `seq`；`EventEnvelope::follows` / `EventSeq::is_contiguous_with` 检测缺口，客户端据此重取快照
  而不是猜测（`event.rs:8-10,48-50,174-176`）。
- **兼容性**：未知 kind 反序列化为 `Other(String)`，新生产端不击穿旧消费端（`event.rs:64-65,117-136`）。
- **备选（被拒）**：单一事件通道——拒绝，遥测洪峰会淹没控制事件。

## D-T02-08 `RuntimePlan` 不可变，校验环、端口、悬空引用并返回结构化错误

- **背景**：net-host 需要一份可校验、可重放、无调用方任意路径的计划。
- **决策**：`RuntimePlan` 为不可变数据；配置源仅 `Inline` 或 `ControlledFile`（**不接受绝对路径**，
  `crates/domain/src/runtime_plan.rs:47-53`）。`validate()` 依次检测重复 tag/悬空/环/进程拓扑/端口，
  返回**第一个**结构化 `DomainError`（`:446-454`）。
- **错误码映射**：重复 → `E_INVALID_PLAN`；悬空 → `E_DANGLING_REFERENCE`；环 →
  `E_GRAPH_CYCLE`；端口冲突 → `E_PORT_CONFLICT`（`:172-259,308-410`）。
- **备选（被拒）**：把校验推迟到内核启动失败——拒绝，无法在写入前给出字段级诊断。
- **后果**：`stop_order` 定义为 `start_order` 的反序（`:485-489`），保证逆序安全停止。

## D-T02-09 未知扩展一律保留（extra/flatten），禁止"为强类型删字段"

- **背景**：上游持续演进而本项目冻结在 7.25.4；若反序列化丢弃未知键，往返会损坏配置。
- **决策**：每个持久化领域 struct 带 `#[serde(flatten)] extra: ExtraMap`（`profile.rs:19,79-80,118-119,183-184`，
  `entities.rs`/`settings.rs` 各 struct 同理）。注释引用 plan §11：*"不能为了强类型而删除未知扩展"*
  （`profile.rs:8-10`）。
- **验证**：`profile.rs:311-326`/`329-335`、`settings.rs:481-490` 三个测试证明未知键往返不丢。
- **备选（被拒）**：用 `deny_unknown_fields` 强类型——拒绝，会拒绝并丢弃未来字段。
- **后果**：`compat/domain-map.yaml` 以 `fld: null` 的 `extra` 条目登记这些保留槽；`Profile.extra`
  另以一条复合映射保留 9 个废弃 `FLD-ENT-*`（`domain-map.yaml:124,127`）。

## D-T02-10 FRB DTO 与领域类型解耦，未知扩展以 JSON 字符串过桥

- **背景**：领域类型含 `serde_json::Map`，FRB 无法直接翻译。
- **决策**：`crates/bridge_api/src/api/contract.rs` 定义一组扁平 DTO，只带 UI 所需字段；领域仍是 Rust 侧
  真值源（`contract.rs:1-6`）。`ProfileDto.extra_json` 以 JSON 文本承载未知扩展，在桥层序列化/反序列化
  （`engine.rs:55,87-89`）。
- **备选（被拒）**：直接暴露领域类型给 FRB——拒绝，无法翻译且泄漏内部结构。
- **后果**：DTO 变更与领域变更解耦；`ErrorDto::from(DomainError)` 保持稳定契约字段（`contract.rs:77-88`）。

## D-T02-11 T02 以 `NullRuntimeClient` + 内存仓库落地，边界预留 T03/T04

- **背景**：T02 目标是契约与骨架，不应在此时引入真实进程/数据库，且必须避免触碰用户运行中的代理。
- **决策**：`AppEngine::in_memory()` 使用 `NullRuntimeClient`（`application/src/engine.rs:34-36`，
  `runtime_client.rs:57-119`）；仓库为 `InMemoryProfileRepository`（`repository.rs:79-192`）。
  FRB 的 `stub_plan()` `ports` 为空，因此**永远不会触碰 10808**（`bridge_api/src/api/engine.rs:210-237`）。
- **理由（代码原文）**：T03 用 net-host 客户端替换 runtime client，T04 用 SQLite 替换仓库，桥层表面不变
  （`engine.rs:1-6`）。
- **后果**：本回合测试不启动任何内核；真实运行验证属 T03/T06b。

## D-T02-12 恢复日志是提示而非系统真值

- **背景**：崩溃恢复的日志可能在写入中途中断，不能盲信。
- **决策**：`RecoveryStatus`/`RecoveryStage` 注释明确 *"The journal is a hint, not system truth:
  state is always re-read from the OS before acting."*（`crates/ipc_contract/src/lib.rs:154-156`）。
- **后果**：net-host 恢复时以 OS 实况为准，`restored`/`pending` 只作展示与协调线索。

---

## 被拒方案汇总

| 方案 | 拒绝理由 | 关联决策 |
|---|---|---|
| 每个运行面各开独立 IPC 命令 | 多写入者竞态 | D-T02-01 |
| 用通用 `command: String` 通道 | 任意命令走私面 | D-T02-02 |
| 仅用 PID 标识对端 | PID 复用 | D-T02-03 |
| 单一 `activeRevision` | 无法区分保存与应用 | D-T02-04 |
| last-write-wins 保存 | 静默丢更新 | D-T02-05 |
| 立即杀进程上报"已取消" | 破坏副作用一致性、欺骗 UI | D-T02-06 |
| 单一事件通道 | 遥测洪峰淹没控制 | D-T02-07 |
| 延迟到内核启动才校验计划 | 无字段级诊断 | D-T02-08 |
| `deny_unknown_fields` 强类型 | 丢弃未来字段 | D-T02-09 |
| 直接暴露领域类型给 FRB | 不可翻译、泄漏内部结构 | D-T02-10 |

---

## 未决项 / 后续任务前置

1. **T03**：用真实 `RuntimeClient`（net-host IPC 客户端）替换 `NullRuntimeClient`；本回合未验证传输层
   （命名管道/Unix domain socket）与心跳重连（`ipc_contract/src/lib.rs:9-11,33-37`）。
2. **T04**：用 SQLite 仓库替换 `InMemoryProfileRepository`；`domain-map.yaml` 已给出 `storage` 定位与
   `upstream_key` 大小写，但真实迁移未运行。
3. **T12a**：`AppSettings` 全字段 UI 接线与逐字段校验（`settings.rs:1-9` 声明为后续任务）。
4. **FRB 代码生成**：`frb_generated.rs` 的再生成与 Dart 绑定验证属 T03 轮统一验证；本回合未运行
   Flutter 侧门禁。
5. **workspace clippy**：`crates/domain` 曾存在 `result_large_err` 等 lint（见 `docs/evidence/T07-T08-codegen.md:129-142`）；
   `domain` 已通过 `lib.rs:20` 显式 `allow(clippy::result_large_err)` 处理该类型。本回合未重跑 workspace clippy
   （另一代理在并行构建，避免抢构建锁）。

---

## 文档修订历史

| 版本 | 日期 | 变更 | 依据 |
|---|---|---|---|
| v1 | 2026-10-01 | 首版：固化 T02 契约与 12 条架构决策，补 `domain-map.yaml` 统计与测试证据 | `docs/decisions/T02-contracts.md`、`docs/evidence/T02.md`、`crates/**`、`compat/domain-map.yaml` |

> 本文件为 T02 文档补全回合产物，未改动任何代码、配置或台账，未 commit。
