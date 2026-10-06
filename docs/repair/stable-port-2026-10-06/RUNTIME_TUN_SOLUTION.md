# 运行与 TUN 详细实施方案

状态：`identified`，方案交付。覆盖 CP-01/02/03/04/12/17；本文件中的新接口、新文件、新测试均为**拟新增**，没有声称已经实现。本轮只阅读和写方案，未修改生产源码、未执行测试或应用、未写 OS 设置。

编写时重新核对的 HEAD：`a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。工作树变化为上一轮审计目录和 repair 文档，`crates/ services/ apps/` 未有源码改动。实施前必须再次记录 HEAD/diff；若源码变化，先重核对本文件的现有符号，不能覆盖别人的修复。原版基准仍是 v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

本文件是主方案 SP-00..35 的运行/TUN分册。下文 RT-00..RT-14 是本领域拆分建议标签，不是新 feature/field ID，也不替换主任务 manifest。所有适用功能分母、原版设置默认、原版入口与布局必须保留。

## 1. 实施必须保持的当前基础

| 现有模块/符号 | 当前用途 | 实施方式 |
|---|---|---|
| `crates/domain/src/runtime_plan.rs::RuntimePlan/RuntimeTarget/ProcessGraph/NetworkPolicy/RequiredPrivilege` | 不可变配置计划、进程依赖与权限 | 扩展计划身份和每节点执行要求；domain不引入Flutter/窗口/平台IO/IPC依赖 |
| `crates/application/src/engine.rs::build_runtime_plan(_with_hints)/apply_runtime/stop_runtime/operation_status/reconcile_applied_session` | 计划生成、业务修订、运行调用 | 准备与提交分离；实际事实来自host descriptor，删除accepted后再读desired default的归属逻辑 |
| `crates/application/src/runtime_client.rs::RuntimeClient/RuntimeSnapshot/AppliedSession/OperationStatusView` | 应用到运行服务的边界 | 扩展类型并映射IPC，UI/业务模块不直接持有核心进程 |
| `crates/application/src/net_host_client.rs::do_request/run_request/event_loop/RequestGate` | Windows管道、超时与事件线程 | 支持快速admission、独立只读对账、真正可终止的IO等待；保留有界worker要求 |
| `crates/ipc_contract/src/lib.rs::IpcOperation/IpcResult/OperationStatus/RuntimeSnapshot` | 现有v1闭合IPC | 一次协议整合升级，不由各子代理分别增加不一致分支 |
| `crates/runtime/src/wire.rs::RuntimeDetail/RuntimeTunDetail/ServerFrame` | 当前PID/端口事实放在额外detail事件中 | V2 snapshot原子携带实际descriptor；legacy detail只作兼容投影，不作第二事实源 |
| `services/net_host/src/session.rs::HostState/Inner/Session/SidecarSession/apply_plan/precheck_plan/start_prepared/try_restore/stop_managed/release_tun_lease` | 核心的唯一持有者 | 分步抽出生命周期调度与进程持有任务；保持先预检、失败保旧/恢复旧的正确基础 |
| `services/net_host/src/server.rs::serve_connection/watchdog`、`events.rs::EventBus` | 按连接处理命令、host心跳/断联回收、事件广播 | 所有写命令汇入一个host生命周期队列，事件有lag恢复合同 |
| `services/net_host/src/helper_client.rs::HelperLink/PipeHelperLink/TunLease`、`tun_lease.rs::*` | 提权核与TUN资源租约、journal | 持续保活、明确release确认和待清理记录；不再把drop连接视作清理成功 |
| `services/privileged_helper/src/{backend,server,windows}.rs` | 枚举式最小提权操作、OS资源持有 | 新增自有进程状态查询和资源释放结果；按资源记录成功/失败与所有权 |
| `apps/desktop/lib/features/runtime/{runtime_controller,runtime_bridge,tun_toggle}.dart` | 前端pending、操作结果、实际状态 | 使用本次命令自身结果；desired/actual/error/pending分开，不从全局error==null判断成功 |
| `apps/desktop/lib/app/shell/status_bar_view.dart` | TUN开关与标签 | 保持原版入口与布局；改读实际状态及分阶段结果，不只是换提示文案 |

旧5秒helper回收、Legacy普通节点双TUN provider已经修过；bool apply结果与lease DTO已经贯通。这些要保留。当前残余缺口见 `docs/evidence/complete-port-audit-2026-10-06/runtime/README.md`，实施无需重新进行一遍历史审计。

## 2. 先锁定共享身份与结果合同（RT-00）

### 2.1 各身份只做一件事

| 身份 | 生成/持有者 | 规则 |
|---|---|---|
| `client_instance_id` | Rust应用入口，每次进程启动新值 | 不是节点ID或用户设备ID；同一GUI进程的toolbar、托盘、热键、子窗共用。不同进程不能共用本地seq空间 |
| `intent_seq`、`command_id` | Rust应用统一入口 | 入口收到明确动作即分配；u64经FRB使用BigInt，不经过double。command_id用于重复提交对账 |
| `operation_id` | Rust应用入口，**发送前**分配并登记 | 不再等host响应后才知道ID；禁止拼接job_id。由host验证长度/字符并按client scope登记 |
| `job_id` | 现有JobManager | 独立字段，映射operation；保留现有JobState和CancelOutcome |
| `host_instance_id` | 每次net_host boot | host重启后不同；不能只用PID判断是否同一host |
| `host_admission_seq` | 单个host命令调度器 | 定义多个控制来源在host的全局受理顺序；不拿不同进程的wall-clock/本地seq冒充全局点击顺序 |
| `generation`、`session_id` | host | 新候选会话和恢复会话都分配新generation/session；旧退出/日志/回复必须带原generation |
| `target_id`、`plan_id`、`desired_revision`、各config hash | 不可变RuntimePlan | 显式目标在动作时冻结；accepted后不得读取后来的default补归属。恢复旧plan仍带旧target/revision/hash，使用新generation |
| `process_node_id`、`ProcessIdentity` | host/helper实际spawn | PID+creation time；helper还持有自己的opaque handle。PID为0/creation未知不提交Ready |
| `lease_id`、`resource_id`、adapter identity | helper/host | TUN每资源单独归属；interface index和adapter显示名不能单独当永久身份 |

### 2.2 拟新增/扩展的类型草案

整合补充：所有与已保存文档/revision关联的intent、operation查询、重试、window结果需携带datasetEpoch（整库恢复/替换激活才换代次），与host_instance/generation/eventEpoch不同。普通mutation新建command/operation/commit身份而不换datasetEpoch，旧数据代次请求不能作用于新库；SettingsSaveReceipt还包含CommitUnknown/RecoveryRequired及可选newRevision/token，未确认提交先恢复查询。

下面是接口形状，不是可直接粘贴编译的已有代码。公共字段放domain/IPC的纯类型，application与FRB使用镜像；不能让ipc_contract依赖runtime（当前runtime已经依赖IPC）。进程identity在wire层用平坦字段，向现有`runtime::ProcessIdentity`显式转换。

```rust
// 拟新增。UI传入动作；Rust入口填身份，不接受UI任意填权限/执行路径。
struct RuntimeIntent {
    client_instance_id: String,
    intent_seq: u64,
    command_id: String,
    operation_id: String,
    origin: RuntimeOrigin,
    action: RuntimeAction, // Apply、Stop、Shutdown、RetryCleanup
    target_id: Option<String>,
    expected_revision: Option<u64>,
    saved_document_token: Option<String>,
    enqueue_budget_ms: u64, // duration，不能传跨进程不可比较的monotonic timestamp
}

// 拟新增。admitted/queued不是运行成功；job_id和operation_id独立。
struct OperationReceipt {
    operation_id: String,
    job_id: String,
    command_id: String,
    admission: AdmissionState, // LocalQueued、HostQueued、Duplicate、Rejected
    host_instance_id: Option<String>,
    host_admission_seq: Option<u64>,
    error: Option<DomainError>,
}

// 拟新增。唯一实际会话事实；不含节点凭据/Clash secret/完整配置正文。
struct RuntimeActualDescriptor {
    host_instance_id: String,
    descriptor_version: u64,
    generation: u64,
    session_id: String,
    plan_id: String,
    target_id: Option<String>, // None只用于明确标记的legacy未归属会话
    applied_revision: u64,
    main_core: CoreType,
    config_hashes: Vec<NodeConfigHash>,
    processes: Vec<ActualProcess>,
    endpoints: Vec<ActualEndpoint>,
    tun: ActualTunState,
    observation: ObservationState, // Verified、ReconcileRequired、Unknown
}

// 现有RuntimeSnapshot的拟扩展V2合同，名称可保留；必须一次原子返回。
struct RuntimeSnapshot {
    host_instance_id: String,
    state: RuntimeState, // 保留已有Stopped/Validating/.../Running/Degraded
    historical_applied_revision: u64,
    actual: Option<RuntimeActualDescriptor>,
    candidate: Option<CandidateSummary>,
    cleanup_pending: Vec<PendingCleanupSummary>,
    last_operation: Option<RuntimeOperationStatus>,
    last_failure: Option<OperationFailure>,
    epoch: u64,
    last_seq: u64,
    host_alive: bool,
}
```

`ActualEndpoint`至少有node_id、core、用途Proxy/Statistics/ClashApi、协议http/socks/mixed、listen/port、UDP支持、authentication_required、ready、system_proxy_eligible。公开descriptor不带认证密码。Rust监控/下载确实需要的secret放拟新增`AppliedRuntimePrivateContext`，通过受控资源token/仅Rust接口获取；不经普通SnapshotDto/Flutter，不写日志。用实际API endpoint的owner/core，不能重连后猜Xray默认端口。

host的RuntimeSnapshot只陈述运行事实；AppEngine/FRB的组合snapshot另外保留当前`desired_revision`和相关设置组revision。不要把host的historical_applied_revision写回desired，也不要在Stopped时让历史修订看起来仍Applied。host收到duration预算后以本进程monotonic时钟建立deadline；客户端和host各自记录剩余预算，不直接比较跨进程绝对monotonic值。

`RuntimeOperationStatus`另增operation层stage/state：Queued、Preparing、Prechecking、WaitingAuthorization、StoppingPrevious、StartingCandidate、Checking、Committing、Cleaning，以及Succeeded/Failed/Cancelled/Superseded终态。未知结果是**客户端知识状态**，不是把host已Succeeded改成Unknown。清理达到等待预算后operation可终结为Failed+cleanup_pending；持久化资源继续由恢复流程持有，不能让job永远Running。

`OperationFailure`分开candidate_error、rollback_result、cleanup_errors、preserved_actual_generation。候选失败且旧会话仍Running是合法组合；不能用candidate错误覆盖actual实际状态。

### 2.3 拟新增公共接口

| 层 | 拟接口 | 语义 |
|---|---|---|
| application/FRB | `begin_runtime_intent(request) -> OperationReceipt` | 快速受理并记录operation，重生成/管道/核心等待在后台；立即反馈pending，不能ok=Running |
| application/FRB | `get_runtime_operation(operation_id) -> RuntimeOperationStatus` | 先查应用已登记的local operation，再查host已受理operation；未知不能变NotFound默认空DTO |
| application/FRB | `get_runtime_snapshot() -> RuntimeSnapshot` | 一次response内返回actual/candidate/cleanup/epoch watermark |
| application/FRB | `cancel_runtime_operation(operation_id) -> CancellationReceipt` | 返回CancelledBeforeDispatch、CancelRequested、PastSafePoint、AlreadyTerminal；不能只cancel应用job |
| application/FRB | `retry_runtime_cleanup(cleanup_token) -> OperationReceipt` | 只重试本应用拥有且仍pending的资源；不重新apply同一配置 |
| RuntimeClient/IPC | `SubmitRuntimeCommand { intent, plan: Option<RuntimePlan> }` | Apply必须带已固化plan；Stop/Shutdown无任意配置。短admission response后host后台执行 |
| RuntimeClient/IPC | `GetCommand { client_instance_id, command_id }` | admission ACK丢失时按发送前已知键找操作；不靠新apply试探 |
| helper | `RenewLease`、`GetOwnedCoreStatus`、`ReleaseOwnedResources`、`GetLeaseStatus` | 只作用于已鉴权client/lease的枚举资源；都返回generation/lease/resource状态，不能加任意shell |

现有`apply_runtime/stop_runtime/get_operation`保留兼容wrapper一轮：wrapper调用新流程并等待本次终态，返回同一operation_id；Apply只在Succeeded且实际candidate descriptor匹配时`ok=true`，Stop只在Succeeded且该操作要求释放的资源全部确认清理时`ok=true`。Unknown/cleanup_pending明确错误或未完成，不能靠全局snapshot.error推成功。旧bool消费者必须逐入口迁至typed结果；兼容wrapper不能成为永久双调度器。

## 3. 命令调度和未知结果（CP-02、CP-03，RT-01..03）

### 3.1 应用入口和host都统一，但只host写运行资源

1. Flutter所有入口调用同一`RuntimeController`；子窗通过现有主窗通信入口转发，热键/托盘不能直接旁路写native状态。controller同步显示“已请求/排队”，自身不造Running。
2. Rust应用入口分配intent/operation/job，先写operation元数据，再进行可取消的计划准备。拟新增`application/src/runtime_commands.rs`管理本应用请求顺序和对账；它不spawn核心、不改路由。
3. 显式start/设默认携带冻结target和authority revision；设置“保存并应用”携带保存后的revision/document token。F5/启动恢复按原版在其实际准备步骤读最新持久化default，随后立刻固定target/revision；不能混成“所有apply都晚读default”。明确记录origin，原版sameactive普通激活no-op和显式启动重试仍分开。
4. 计划输入应来自同一修订快照：SQLite用一致读事务；memory仓库复制前后核对revision。所有影响runtime的写入都必须推进同一修订。复制期间revision变了则结构化Stale，不把A身份套到B正文；设置saved_document_token只能使用确实保存的版本。无关纯布局修订不能导致运行计划错误串版本。
5. 每个host新建一个拟`LifecycleActor`（建议`services/net_host/src/lifecycle.rs`）。`serve_connection`所有Apply/Stop/Shutdown/恢复写操作只向actor发命令。当前`HostState::apply_plan/stop_managed`降为actor唯一调用的内部实现，外部不再直接并行进入。
6. application dispatcher按同一client的入口顺序向host提交；计划准备尚未发出可在证明无host dispatch时取消，晚到的准备结果不得再次发出。多个独立client以host_admission_seq定义全局受理顺序。不能按两台窗口机器的时钟排序，也不能仅靠每个NetHostClient自己的RequestGate声称全局串行。
7. actor状态锁只用于短提交；不持`Inner`锁等待core退出/UAC/网络握手。执行任务必须带operation/generation，完成消息回actor才提交状态。进程退出/断联/cleanup结果可高优先级通知actor；读snapshot/get_operation始终能及时响应。

### 3.2 队列规则写进合同

- 使用一个有界FIFO序列，替换`_pendingApply ?? _pendingStop`固定优先级。启动A在途→Stop→启动B，必须A安全收尾→Stop清理→B，或A在允许取消点取消→Stop清理→B。
- 只允许supersede**尚未开始、明确可合并、相邻且没有barrier**的apply（例如同origin的排队reload或选节点B被C替代）。每个被替代调用返回Superseded+superseding_operation_id；不能共享新调用的bool成功。
- Stop/Shutdown/备份退出/更新交接是清理barrier，不能被后来的start丢掉。相邻重复stop可加入同一操作waiter，但仍保留清理结果；中间有start时不可跨start合并。
- active操作不是Superseded。explicit Stop可请求取消其尚未产生不可逆资源的准备/预检；进入spawn/资源应用后，先到安全点并清理候选，才执行Stop。取消申请≠已取消；不允许为了响应快遗留后台core。
- 队列满时拒绝新的普通apply并反馈Busy；Stop保留控制槽或先淘汰尚未开始的可合并apply，不能让stop被普通请求淹没。每client pending、全host pending、操作历史均有上限。
- 使用monotonic deadline；排队/准备/执行预算分别可观测，UI等待到期不等价于服务端执行失败。拟默认：admission等待2s、普通查询5s、config check15s、授权等待60s、ready10s、整体操作120s；先作为可注入配置，按真实场景收敛，不把这些建议值冒称原版规范或已测性能。

### 3.3 重复请求和response丢失

1. `command_id + client_instance_id`为幂等键，附command payload hash（target/revision/action/config hashes，不能记录凭据正文）。同键同hash返回同operation；同键不同hash拒绝冲突，不再执行。
2. host在副作用前把受理记录写自己的journal（不是业务SQLite）：operation、command key、stage、plan摘要、candidate/previous generation。query可返回Queued/Preparing，也可返回终态；终态历史有界但保留足够重连窗口。
3. admission超时/管道断开：UI进入reconcileNeeded，查GetCommand/GetOperation + atomic snapshot。不能自动重新生成operation并apply；确认NotSubmitted后才允许显式重试生成新operation。
4. query也失败：UI显示“结果待确认”，保留最后实际descriptor并标过期，禁用依赖确切事实的危险操作或要求先对账；不能把view重置成Stopped。读请求用独立有界通道，不受一个60秒旧写worker永久占住。
5. 修改管道IO等待：采用可取消/有deadline的async named pipe或持有句柄并实现确定性取消的IO；仅`recv_timeout`让等待线程退回而读线程继续阻塞不够。超时后能确认worker/句柄终结，worker数不累积，也不能把唯一gate永久占住。
6. operation终态只结束该job。当前host Ready后Done而应用才start job的顺序必须消除；job应入口先start，Done/Failed/Cancelled/Superseded发生时finish一次。临时测速job仍使用自己的归属，不能stop主会话。

## 4. 实际会话、退出监视与回滚（CP-01/03，RT-04..05）

### 4.1 统一每个进程的持有方式

拟新增`ManagedProcess`与`ManagedGeneration`，逐步替换`Session.child`必为普通child、SidecarSession三种Option的组合；不要一次重写所有业务模块。

- `ManagedProcess::Normal`：专门的ProcessTask持有`tokio::process::Child`和JobGuard，接收Stop请求并等待退出，发`ProcessExited { host_id,generation,node_id,identity,exit_status }`。不让两个任务同时wait同一个Child。stdout/stderr读任务只拿stream和固定generation；不能借EventBus当前epoch给旧core日志重新贴新generation。
- `ManagedProcess::Elevated`：helper持有实际进程句柄/Job；host持有lease/core handle、完整PID+creation身份和查询/退出通道。host不能通过普通PID调用擅自终止helper以外的进程。
- 在Ready前和Ready后使用同一owner记录。所有handle缺失、Job绑定失败、creation time无法确认都不能提交Ready，按已获得的自有资源逆序回滚。
- Windows通过实际handle等待退出；非Windows使用其process group/进程身份能力。现有identity在非Windows的能力不得被假定完整，适用平台须各自实现与验收，Unavailable明确暴露。

### 4.2 状态迁移

| 输入/阶段 | actual/candidate与运行状态 | operation结果 |
|---|---|---|
| Apply受理/预检 | actual旧会话继续可用；candidate=Prechecking | 尚未成功；字段/核检查失败=Failed，actual保旧 |
| 预检成功但授权未通过 | 在能安全提前授权/确认helper时先完成授权；旧actual仍保留 | UAC取消=Cancelled/Denied；不能先停旧核再弹权限请求 |
| 开始切换、停止旧会话 | 旧endpoint一旦确认停止即撤；candidate=Starting；已有资源pending另列 | 保存上一良好不可变plan/实际exe引用，不保存later desired |
| 候选spawn/ready成功 | 原子提交完整actual新descriptor，candidate清空，Running | Succeeded + job Done |
| 候选启动失败且旧可恢复 | 候选自有资源清理；旧plan启动为新generation，actual target仍旧 | 当前候选operation Failed，rollback_result=Restored；不能因恢复旧核而把候选ok=true |
| 候选失败且不能恢复 | 无live endpoint；Stopped或Degraded，pending cleanup仍可见 | Failed，root error+cleanup errors都保留 |
| 主核/必需sidecar退出 | 先按generation确认，撤不可用endpoint；通知整代停止/降级并清理 | 之前Succeeded是历史；新增runtime failure/cleanup操作，不伪改过去操作结果 |
| Stop成功 | actual=None，candidate=None，所有自有资源已确认释放 | Succeeded/Stopped；历史applied revision可保留为历史字段 |
| Stop核心已停但资源未清 | live endpoint撤回，cleanup_pending存在，Degraded | Failed+cleanup_pending，job终态；后台恢复记录继续持有 |
| 只有host存活 | host_alive=true；不推导core alive | UI不显示Running除非actual observation有效 |

正常核心退出的实际事实目标：退出通知后1秒内更新snapshot/事件（建议验收阈值，需测）；不能靠用户F5/手动刷新才修正。必需sidecar/TUN provider退出使整代不再完整Ready。明确可选监控进程退出可降级，但不能凭实施模型习惯把前置代理/TUN provider标成可选。

### 4.3 重开与实际归属

1. host仍在：新AppEngine请求atomic snapshot，使用descriptor里的actual target/core/endpoints/hash/revision；不再要求该engine自己曾设置`apply_target`。节点已删除时显示“运行节点已删除/未在当前列表”，保留实际opaque ID与端点；不改成新default。
2. host已重启：journal是恢复线索，先核对PID+creation+owned Job/资源；不能仅凭journal Applied恢复Running。按已确认死亡/存活/Unknown分别处理。
3. 能恢复上次良好plan时需有受控config/exe资源及hash/版本；恢复是新的operation/generation。无完整资源或旧v1未带target时显示legacy未归属/待恢复，不猜desired来填空。清完旧资源才启动新desired，避免同端口和TUN旧路由争用。
4. 真退出先Stop并等待清理结果，再flush统计/持久化、恢复仅自己拥有的代理/PAC、自启无退出副作用、最后host shutdown；每步有界。cleanup待确认时把durable pending交接给下次boot，不能删journal后退出。
5. 更新交接复用同一Stop/cleanup receipt。runner只有在所有相关进程/文件句柄已关闭或显式报告交接未完成后才替换；不得把失联watchdog当正常退出成功。

## 5. TUN provider、权限和就绪（CP-12，RT-06..08）

### 5.1 计划表达权限，主核与侧核走同一路径

1. 拟新增`ProcessExecutionSpec { node_id, roles, privilege, executable_ref, config_ref, readiness }`。roles是MainProxy/PreProxy/TunProvider的集合；同一sing-box主进程可以同时是MainProxy与TunProvider，不额外生成一个TUN进程。最终生成配置与明确的provider_node_id对应唯一provider。net_host不再仅用`body_has_tun_inbound`猜sing-box sidecar权限。
2. 保持已有Legacy非sing-box普通节点“前置sing-box拥有TUN、主核无TUN”拓扑；sing-box自身TUN或非Legacy主核TUN也使用同一`spawn_managed_process`，Tun privilege意味着走helper，普通节点走normal owner。禁止整体把GUI提权或所有核心强行提升。
3. 各CoreAdapter拟增加`execution_capabilities()`、`readiness_spec()`默认合同；按14核心/原版适用表显式登记，未知能力不等于支持。native Custom不能因JSON未解析到type=tun就被当普通无权限。保留原版raw config语义，适配器验证其有效provider；无法判断时报告具体缺口，不能悄悄删Custom字段。
4. 预检完成后，在停旧核前获得helper可用/授权确认；取得权限不是Ready。helper已存在但版本/ownership不匹配应拒绝，不能绕到普通spawn。取消与拒绝返回不同结构化结果，UI可区别“不授予权限”和“内核配置失败”。
5. 受控staging继续遵守现有RunElevatedCore的exe/run_dir/allow-list约束。计划生成前分配受控artifact_id，拟纯`CoreExecutionLayout`给application/host同一稳定最终路径；host验证该布局、核心hash和boundary后才copy/stage。这样TUN保护规则可以包含真正提升执行路径，不能只包含原安装路径。不要为了减少copy放宽成UI任意exe路径。

### 5.2 就绪是多项事实，不是“RunElevatedCore返回了handle”

拟`ReadinessSpec`按adapter/拓扑组合必要条件：

- Normal/elevated process实际存活且identity匹配；启动100ms后/每个等待阶段都检查早退。
- 必需本地代理入口完成对应协议probe（SOCKS5 greeting、HTTP/mixed本地协议响应等），并核对该监听属于本代节点；只TCP connect不能证明是自己的服务。认证入口用Rust内部凭据/协议能力，不能在Flutter暴露密码。
- TUN provider已创建符合指定name的实际adapter；取得interface GUID/LUID或平台稳定identity，index只是当前属性。旧同名adapter或旧index不能直接满足本代Ready。
- 要求的地址/MTU/显式route与helper资源回执相符；auto-route由核心产生的路由属于核心控制范围，helper不再重复写相同default route。route/DNS语义按适用核验证，不能只以route_count>0认为TUN流量工作。
- 当前Sing-box pre-proxy必须在其依赖main proxy Ready后启动；graph按拓扑启动/逆序停止，保持冻结原版`CoreManager`的主核→代理端口→前置核逻辑。
- 无本地代理端口的合法native Custom使用明确ProcessOnly就绪等级，UI不宣称可设置系统代理/远端可用；不能借port=0省略一切ready并假造入口。

helper新增`GetOwnedCoreStatus`返回自己的handle、PID+creation、running/exit_code/observation及lease。可先采用250ms左右有界轮询，再接push事件；轮询必须持续，不能只启动时查一次。查询失联不等价于进程死亡，进入Unknown/ReconcileRequired并核对/清理；实际core死亡则发同一ProcessExited事件。没有adapter不创建、不写地址，超时如实回滚并记录pending。

### 5.3 adapter名称和重新发现

从`TunPlanningContext`一次决定requested adapter name、平台命名策略、provider node。当前Windows默认`v2rayn-tun`保留；`V2RAYN_R_TUN_ADAPTER`若继续支持必须传入`codegen.rs::settings_from_app`的tun.name和helper descriptor同一值，不能一个fixed默认一个env覆盖。macOS/Linux依原版/平台适配器命名要求，不把Windows字符串强套到所有平台。

弃用“计划之前发现旧index、停旧核后仍使用此index”的写入路径。每个新provider创建/Ready后重新查询name+stable identity+ownership，取得当代index再写。显式test override也必须校验，不能作为真实OS验证口径；发现与写入之间若identity变了则重查/失败，不向复用index写别人的网卡。

## 6. helper长会话、资源归属、失败恢复（CP-04/12，RT-09..11）

### 6.1 保活与不同超时分开

1. 每个活跃helper link由拟`HelperSessionActor`唯一持有pipe；序列化apply/query/renew/release，不能timer和apply两个线程同时读同一帧。所有sidecar-core lease和address/route lease都要保活；只向Flutter发host heartbeat不够。
2. 拟固定初值：renew每15s，请求/frame-progress timeout5s，连续3次renew失败进入对账，lease期限90s；值由双方version协商/配置注入。同一活跃连接有正常请求也续期。完整帧读取超过预算终止该请求/连接并进入owned cleanup；不能让半帧占24h。
3. 长时间没有用户点击但核心正常运行：renew继续，24h/48h也不会按“用户无操作”清活跃lease。断管道/owner死亡与无UI动作分开。旧24h timeout不能作为最终解决方案。
4. 请求执行有单独budget；超时不能放弃一个仍在后台写route的线程。backend持有operation token，完成后回报已发生的具体资源，cancel到安全点清理；无法停止的OS调用按Unknown结果记录并只读核对，不执行第二次盲重试。
5. 睡眠/恢复：resume后先读host/helper/process/adapter/resource身份，成功renew后重建observation；lease真的失效就不能继续标Enabled。只用wall-clock跳变判owner死亡不合格。保活线程停止/恢复本身也要有时间线证据。

### 6.2 资源粒度与两个journal

拟新增`OwnedResourceRecord`：resource_id、lease_id、owner(host/client/generation)、kind(Process/Address/Route/MTU)、stable OS identity、Created/Adopted/ExistingForeign、expected value digest、Applied/CleanupPending/Released/Unknown、last_error。禁止只记录“interface index=7”的一整组模糊资源。

- 创建前读取现状。已存在的同值资源区分“确为本lease创建/已journal归属”和“外部存在”；外部资源不记Created，不在cleanup删除。原版core自己添加的地址/auto-route不能被helper重建后冒认归属。
- 多地址/多route应用按项提交：每成功一项即写owner journal；第N项失败逆序清理前N-1项，未清项仍pending。不能全部成功后才插WindowsBackend.tun registry。
- helper维护自己的durable ownership journal，存放在受控目录与正确ACL下；host保留自己的lease/command journal引用。双方记录用于重开定位，事实须OS只读核对；不能host以为helper负责、helper以为host负责，然后双方都删记录。
- 删除每项时先保留record，调用OS删除并核对Gone或仍存在；Gone视幂等Released，identity不匹配视Foreign/Conflict而非删除。确认成功后再删record。backend registry、ConnectionLease列表和host journal同步收敛。
- `reset_tun_address`改为按owned address记录释放，不能先remove整个registry再逐条删。`remove_routes`不能第一个失败就丢失其余项进度。所有失败返回per-resource结果，不靠count整数表示全成功。

### 6.3 显式释放结果与reopen

拟新增helper `ReleaseOwnedResources { lease_id, resource_ids, expected_owner } -> ReleaseReport`，结果为Released/AlreadyGone/Pending/Conflict+errors；现有`cleanup()`签名改返回该结果或typed DomainError+pending resources。PipeHelperLink不得吞RemoveRoutes错误、drop连接后立即Ok。

1. Stop/rollback首先撤不可用的live endpoint，但actualTun/resource状态转Cleaning，而不是立刻None。
2. host调用显式release，在预算内等待确认；不支持显式release的旧helper不可承接新TUN写入。
3. 全部Released才删除对应journal、发布Disabled。任一Pending/Unknown保留可重试信息、发布Degraded/清理待恢复，并结束本次操作为Failed+cleanup_pending。
4. `HelperServer::on_disconnect`负责同一release执行器兜底；失败资源复制到durable pending后才能关闭connection bookkeeping。不能closed=true+mem::take后不留任何owner。
5. 重开扫描pending：核对host/owner身份、资源stable identity和expected value；自己的资源安全重试，Foreign冲突只报告，不删。helper不可用保留pending；普通backend error也不能算cleaned并删journal。
6. retry_cleanup是单独operation，只清理对应lease；不得重新apply旧TUN来“试一下是否清完”。禁止启动冲突的新TUN generation直至旧自有资源已释放/明确可复用且所有权转移被确认。
7. 自动后台重试有退避和上限，保留用户可见“重试清理/诊断”入口；待恢复资源不能制造无限Running jobs。多个retry同lease幂等，不并发重复写OS。

## 7. IPv4/IPv6、DNS、进程保护与原版语义（CP-12，RT-12）

### 7.1 拟TunPlanningContext

扩展`CodegenOptions`或增加独立`TunPlanningContext`，避免再从`CodegenSettings::default()`得到false/空上下文。它至少含platform、IPv6Probe(KnownHasGlobal/KnownNoGlobal/Unknown)、provider node、统一adapter name、原版Xray/sing-box保护集合、各节点**最终实际执行路径**、bound interface/send-through及strict-route策略、requested地址/MTU/route excludes、最终DNS配置来源。

只读能力探测建议在Rust平台适配/运行准备层实现，不放Flutter：新增`platform/network_capabilities.rs`或runtime内只读probe边界，返回具体能力与Unknown错误；domain/config_codegen只消费纯上下文。不得以Windows一次成功证明macOS/Linux功能。探测结果在计划时记录版本/时间，拓扑执行前核对必要身份变化。

### 7.2 每个设置的最终消费者

| 设置/上下文 | 实施要求 |
|---|---|
| FLD-CFG-095 EnableTun | 决定计划provider与RequiredPrivilege；actual只能由完成的provider/lease事实得到 |
| 096 AutoRoute / 097 StrictRoute | 分清core-owned auto routes与helper显式routes；按原版Legacy+strict时清主核BindInterface/SendThrough（冻结Builder:168-181），不要清用户持久化原值，只改本次resolved main context |
| 098 Stack / 099 Mtu | 映射指定核合法枚举/默认与最终adapter；MTU应用/回滚记录old/new+ownership，不能只写配置字段就算OS有效 |
| 100 EnableIPv6Address / 104 IPv4Address / 105 IPv6Address | 地址family/CIDR/范围验证；按核原版默认生成；IPv6地址开关与HasGlobalIPv6Address是不同输入，不能混成一个bool |
| 101 IcmpRouting | 原版rule/direct/drop逻辑到指定核配置和真实ICMP效果，非法值按原版合法范围处理 |
| 102 EnableLegacyProtect | 保留已修单provider拓扑；原版保护列表、主/前置配置/端口依赖一起验 |
| 103 RouteExcludeAddress | CIDR合法、IPv4/IPv6分别覆盖；最终core配置/route拆分与helper资源规则一致；排除网段不能被auto-route重抓 |
| HasGlobalIPv6Address | 冻结Builder:52与Xray inbound的全局IPv6/autoSystemRoutingTable规则；Probe Unknown不是静默false，TUN相关计划给可读诊断，不伪称IPv6覆盖 |
| protect_core_executables | 冻结Builder:53及routing消费者；Xray `process`与sing-box `process_path`使用对应核需要的标识。包括真的staged提升核路径和installed普通核路径，不只写`xray.exe`样例 |
| TUN DNS | 使用最终选定DNS/路由与53端口hijack规则；DNS消费者接实际运行核，不根据UI当前default猜。远端DNS/本机DNS/泄漏/失败fallback逐适用设置验证 |

计划生成时依原版保护Xray/sing-box范围和实际拓扑补充需要的受管provider；不要把所有进程一律direct。不能保护整个目录或任意用户应用以绕过TUN。Custom/natively merged config的原版适用范围先按台账定位，不能为了方便重写raw全文或假称GUI字段必覆盖所有Custom。

IPv6验证分两层：纯配置矩阵能证明传入了::/0、排除/地址/保护规则；真实隔离机的受控IPv6路径、DNS与出口检查才能证明实际接管/不泄漏。没有IPv6网络只能记该场景未验证，不能删项或把Enabled显示当验收。

## 8. TUN UI与保存/权限取消（CP-12，RT-13）

1. desired toggle仍来自持久化`TunModeItem.EnableTun`；下方actual label只从`ActualTunState`/cleanup/observation生成。候选错误在独立操作反馈区域显示，不夺走actual租约显示。
2. 拟ActualTunState：Disabled、Preparing、Enabled、Cleaning、CleanupPending、Unknown、Simulated。`dry_run=true`只能Simulated，不得“已启用”。“已请求未验证”用于没有足够actual证据，不能称Enabled。
3. 用户关闭、保存false、apply失败但旧lease仍活跃：toggle可以反映desired=false，actual必须“仍启用，关闭失败”并提供重试；不能一看到!desired就未启用。cleanupPending不显示Disabled。
4. 保存失败不调用apply，不推进revision。保存成功但core/helper失败：结果包括saved=true、runtimeApplied=false、actual未改变或待清理、operation error；不能整件任务弹“已关闭/已启用”。
5. 用户开启时UAC取消：依据原版取消回false语义，拟`TunToggleTransaction`记录before value、保存后groupRevision和operation。仅在同一Tun组revision/intent仍匹配时CAS撤销本次EnableTun；其他用户随后修改了Tun组则不覆盖，保留当前desired并明确取消旧operation。未涉及的设置组不得被回滚。
6. 授权拒绝/取消、helper不可用、地址/route失败、内核配置错误分开message key；必须能在同一GUI修好原因后显式重试，不靠重开/后台弹新UAC。
7. 现有`applyActive()->bool`过渡到`RuntimeCommandResult`（operation/terminal/saved/applied/noop/superseded/unknown/error）；`profile_actions.dart::_applyRuntime`不能继续忽略返回值后读取全局error。旧调用返回值由适配层按本次operation转换，不能其他操作的error污染本次结论。
8. 状态栏/toolbar/托盘/设置子窗共用同一个实际descriptor，不拥有第二份内核状态；标签布局与原版左右分区保持，不能把恢复信息挤进一串小字。

## 9. 控制事件lag与快照一致性（CP-17，RT-14）

1. `EventBus`分控制事件与高频log/traffic通道。control包含operation阶段、actual descriptor改变、退出、cleanup/recovery；log采用有界buffer/截断或coalescing，不能阻塞控制状态。UI日志裁剪不能改变控制事件完整性。
2. epoch绑定host/control世代策略并在协议文档固定；所有core事件另外带generation。seq只给广播控制事件分配。请求私有的snapshot/detail回复不得消耗同一个广播seq制造假gap（当前`bus.make`私有detail投影必须迁移）。
3. `serve_connection`显式match `RecvError::Lagged(n)`：回发ResyncRequired并发送atomic snapshot watermarked version，或关闭订阅连接促可靠重连。不能默默退出forwarder又保留活连接。
4. 拟新增有界control replay ring。Subscribe(epoch,from_seq)若可重放则按顺序重放；超出窗口/host改变则返回ResetRequired+snapshot+start watermark。订阅请求和snapshot必须以同一actor/watermark防“快照后、订阅前”丢事件；慢客户端再次lag时重来，不无限缓存。
5. net_host_client收到reset/缺seq/连接断开先标observation过期，再拉snapshot、重建订阅；RuntimeController必须主动refresh，不能只debugPrint sequenceWarning。旧epoch/generation数据只能补历史日志，不能覆盖actual。
6. refresh storm继续合并，但关键进程退出/清理失败无需等全部log settle；keep最后一份authority descriptor_version，迟到snapshot若版本更老则淘汰。heartbeat只更新host liveness，不改变actual核心健康。
7. admission/query与event的帧要校验request_id/command scope；从同一pipe读到event不能把它当query结果。取消订阅要关闭持有句柄/可取消读取，使旧event线程能终结。

## 10. 协议、journal与发行兼容

- 当前IPC_PROTOCOL_VERSION=1、HELPER_PROTOCOL_VERSION=1。上述闭合enum/结果语义升级属于不兼容变更，拟升各自V2，并增加Hello/Capabilities/version handshake；不是“加serde default就能保证旧exe一起工作”。application/net_host/helper/FRB必须同一发行包更新。
- 停旧v1会话使用v1受控stop和身份核对；确认清理/进程归属完成后启动v2。未知旧helper状态先恢复/对账，不能为强行升级按进程名kill，也不能让v2 TUN请求落回v1假成功路径。只读v1诊断可保留，不承接新的TUN写入。
- journal新增schema_version，先写兼容reader再写v2。读取v1 lacking target/ownership时标LegacyUnattributed/Unknown，不填desired；保留原文件供恢复，成功迁移后原子rename/备份。坏JSON/坏类型是结构化恢复失败，不用默认空记录覆盖。
- v2 command/resource journal写temp→flush/atomic replace，目录/路径只由受控layout生成；不把任意operation/adapter/display字符串拼成路径。保留原版业务配置/SQLite未知键，运行journal升级不变更用户节点协议字段。
- FRB Rust/Dart/codegen仍固定2.13.0，一个接口整合者重生成，二次生成no-diff。不得各子代理手写generated文件或让两份RuntimeTunDto字段不一致。
- 旧发行ZIP属于旧source/dirty包，不能替代此次改完后的验证；最终重新固定干净HEAD/所有二进制和Dart AOT hash，用未武装普通入口验收。

## 11. 小任务拆分、依赖和文件锁

每行是一条主用户流程；实施者按主方案§19格式另立卡，先引用本文件合同，再实施。建议每个任务只做表内事项，发现缺口上报整合者，不自己删需求或扩大到全部UI重写。

| 标签 | 唯一流程/产出 | 依赖 | 修改范围/唯一owner |
|---|---|---|---|
| RT-00 | 固定共享身份/operation/actual/cleanup schema、V2迁移合同 | 主SP-00 | 合同整合者：domain runtime_plan、ipc_contract、runtime wire、application mirrors、bridge DTO；先schema/纯序列化测试，无OS |
| RT-01 | 启动请求快速排队并得到自身receipt/job | RT-00 | application runtime_commands新模块；engine.rs入口仅engine整合者落地 |
| RT-02 | A→Stop→B顺序与safe-point cancel | RT-01 | host lifecycle新模块、server.rs转发；session.rs仅host owner修改 |
| RT-03 | response丢失→query同command，不重复执行 | RT-02 | net_host_client、command journal、operation mapping；不与RT-02并行改server/session |
| RT-04 | A apply期间default变B，actual仍A | RT-00/02 | RuntimePlan target/actual提交、engine reconcile、monitor私有context；contract/engine owner整合 |
| RT-05 | 自有主核/必需sidecar退出→状态撤回 | RT-02/04 | managed_process新模块、session ownership、exit events；先normal core，后helper分支 |
| RT-06 | 主核自身TUN也从统一helper启动 | RT-00/05 | node execution/readiness specs、CoreAdapter、host spawn统一；与generator owner固定边界 |
| RT-07 | 提升sidecar等待真实ready且持续退出监视 | RT-06/09 | helper GetOwnedCoreStatus、host elevated ProcessTask；helper接口只由合同owner改 |
| RT-08 | 新provider发现当代adapter，name/index一致 | RT-06 | tun_plan、CoreExecutionLayout、codegen.name投射、host discovery；generator owner与host owner分文件 |
| RT-09 | 长时间不点击仍维持helper lease | RT-00 | helper session actor、RenewLease、独立frame/request/lease deadlines；先pure虚拟时间，再真实长会话 |
| RT-10 | 第N项地址/route写失败仍能恢复 | RT-00/09 | helper backend/server/windows资源journal；一个helper owner，禁止与RT-09并发改同文件 |
| RT-11 | 关闭/崩溃→清理失败→重开重试 | RT-05/10 | tun_lease、release_tun_lease、recovery/exit交接；host与helper结果桥接后整合 |
| RT-12 | IPv6与protect路径/strictLegacy到最终配置 | RT-06/08 | application codegen context、platform只读probe、指定核generator；不改其他设置消费者 |
| RT-13 | TUN开启/关闭/拒绝/取消UI反馈真实 | RT-03/04/07/11 | runtime_controller/bridge/tun_toggle/statusbar + settings组CAS入口；Dart owner串行整合 |
| RT-14 | slow event subscriber lag后恢复实际状态 | RT-00/04/05 | events/server、net_host_client、RuntimeController；依赖稳定descriptor/version合同 |

强文件锁：`engine.rs`由应用整合者统一修改，其他代理提交新模块或最小补丁建议；`session.rs/server.rs/helper_client.rs`由host owner统一；helper server/windows由helper owner；`contract.rs/api/engine.rs`与所有FRB生成物由接口owner；runtime_controller/status_bar由Dart运行owner；`codegen.rs`由设置/生成整合者。允许并行的是不共享文件的新模块、纯测试夹具和证据，依赖实现与schema合入后才接线。

禁止为了并行让两人分别实现主核和sidecar两套不同类型。建议host/helper任务串行、小步保留可构建状态；每次卡完成就记录实际命令、当前commit、未验证边界和下一卡前置。

## 12. 正确合同测试与真实验收

下列路径除已经存在的审计文件外均为**拟新增**，不是声称文件已创建或测试已运行。

### 12.1 无OS的正确合同

- `crates/application/tests/runtime_identity_contract.rs`：用实际build_runtime_plan(A)与gate，在A在途时default B；actual必须A；explicitTarget!=default、B失败保A、rollback旧plan、节点删除、重连actual身份均覆盖。
- `crates/application/tests/runtime_operation_contract.rs`：raw operation_id→get_operation可往返；成功/失败/取消/Unknown reconciliation后job终态；same key same payload只有一次，same key不同payload冲突；持久化受理点前/后注入失败。
- 拟`services/net_host/src/lifecycle.rs::tests`：多连接/多来源Apply/Stop全局FIFO；只能淘汰未开始的允许apply；Stop barrier不丢；准备取消晚结果不submit；旧generation退出不能清新会话。
- 拟`services/net_host/src/managed_process.rs::tests`：normal/core early/postready退出、侧核退出、Job失败、身份不明、bounded stop；只启动自己的stub，不改OS网络。
- `services/privileged_helper/tests/lease_renewal.rs`：实际server+duplex+FakeBackend+虚拟时钟，持续renew跨48h仍owned；无renew失联进入cleanup；半帧/坏帧独立短timeout，cleanup error不清ownership；resume/reconcile模型。
- `services/privileged_helper/tests/resource_ownership.rs`：按第N项create/delete注入错误、foreign既有资源、不匹配identity、重复release、crash journal reload；每个失败资源有durable pending，成功项不重复删。
- 本轮已有`docs/evidence/complete-port-audit-2026-10-06/runtime/rust/src/lib.rs`中三个正确合同：target race、cleanup journal、helper ownership。修复后迁入正式测试，不把应保留journal改成应删除；观察测试中24h expiry/job常驻的断言要转成正确行为，不能以“3passed”关闭缺陷。
- 拟`services/net_host/tests/control_stream_recovery.rs`：实际pipe/duplex slow subscriber、burst/lag、跨epoch、私有snapshot不消耗broadcastseq、snapshot+subscribe无gap；无核心、无OS写入。
- 拟`apps/desktop/test/repair/runtime_operation_result_test.dart`：A→Stop→B最终B、A→B→Stop最终Stopped、superseded旧waiter不是新ok、Unknown不Stopped、late reply不串operation。
- 本轮`runtime/runtime-contracts_test.dart`中的actual label、顺序、Unknown正确断言全部保留；追加desired false+实际lease活跃、candidate失败但旧会话活跃、dry-run、cleanupPending、UAC取消组CAS冲突。使用实际controller/purelabel，替身只注入故障。

### 12.2 安全真实loopback与跨进程验收

在检查harness无host代理/TUN调用后，以新的独立数据/run根、独立pipe、预探测端口≥11808，真实指定核跑：启动→真实协议握手→坏配置保旧→成功切换→持身份handle故障退出→snapshot/事件及时撤回→重试→Stop→host关闭→重开归属。只停止本次managed identity/handle，不按名称kill。

本轮`runtime/real-loopback.ps1`可改为正式正确断言入口，不能继续只打印REPRODUCED然后exit0；改完后断言postready退出后非Running/无旧endpoint，并测readiness/实际绑定owner。再增加response丢失/多connection/slow subscriber fault transport，不需要TUN也能验CP-01/02/03/17。跑正式Flutter→FRB→SQLite→net_host入口，不能一直用audit client替代GUI。

### 12.3 有OS副作用的TUN真实验收

生产auto-route/默认路由/地址/MTU/DNS更改只在已获授权隔离环境执行；本机没有该环境则标未验证并继续无副作用修复。VM准备属于部署工作，不应阻止上述代码合同先完成。

验收矩阵至少：Windows普通用户与管理员；Xray Legacy前置sing-box、sing-box自身TUN、适用的非Legacy主核TUN/native Custom；IPv4-only、双栈、无global IPv6、指定IPv6前缀/route excludes；stack/MTU/ICMP/strict-route/bind/send各合法值和默认；helper授权取消/拒绝/丢失、主/侧核退出、cleanup部分失败、睡眠恢复、连续开关、24h/48h持续真实会话。

每场景记录授权环境/OS/DPI/构建hash/锁定core/原版预期、before/after资源快照、实际流量/DNS/IPv6出口、owned资源回收和重开结果；合成远端由测试方控制、不要用用户订阅/凭据。结束恢复隔离机自己的原网络状态，且只清本项目记录资源。权限对话出现、ifIndex存在、route_count>0、函数返回Ok、FakeBackend通过都不能替代真实数据面与恢复验收。

### 12.4 实施阶段门禁（本方案未执行）

修改Rust后：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`。当前idle_connection_times_out fixture必须按新增idle合同明确配置，另保留活跃lease/renew正确合同，不删除或放宽失败预期。

修改Flutter后在`apps/desktop`：Dart format检查、flutter analyze、受影响正确合同、完整flutter test、flutter build windows --release；FRB二次生成no-diff。引擎异常退出需保留证据、查根因并复跑，不能计为业务通过。

最终条件：CP-01/02/03/04/12/17正确合同关闭；actual/operation/desired/source身份跨重开可靠；OS自有资源失败可恢复；普通未武装发布包的正式用户流程通过，且每个TUN适用核/OS/架构有明确实测或未验证结论。没有完整原生/OS证据时只能标`implemented`或`blocked`相应场景，不能写`verified`或宣称完整稳定。
