# 完整稳定移植版：详细实施方案

日期：2026-10-06。状态：identified。**本交付是修复设计和执行卡，不是已经完成修复的软件。**

应用审计基线：`a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。原版冻结基线：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。执行前核对当前 HEAD、未提交改动和已完成任务；新提交须追加差异复核，不机械套用旧行号。本方案不扩大到审计日最新上游版本，后续升级另作基线迁移。

## 1. 修复目标与文档入口

目标是普通用户能从正式发行包的正常界面完成原版适用流程，失败后能恢复，持久化和重开一致，内核与系统的真实状态可解释，响应与视觉达到原规格。Xray、sing-box、Mihomo 等继续作为外部受管理内核，不重写其代理数据平面。Flutter 负责界面，Rust 负责业务、配置生成、存储、运行控制和平台适配。

| 文件 | 实施用途 |
|---|---|
| [审计与原始证据](../../evidence/complete-port-audit-2026-10-06/README.md) | CP-01..17 当前根因、真实失败、证据范围及已修行为 |
| [运行与 TUN 实施细则](RUNTIME_TUN_SOLUTION.md) | 顺序、实际目标、进程观察、租约、提权、恢复、事件通道 |
| [设置、数据与更新实施细则](SETTINGS_DATA_UPDATE_SOLUTION.md) | 数据安全、保存/生效、消费者、备份、发行信任链 |
| [180 字段逐项实施映射](SETTINGS_IMPLEMENTATION_180.csv) | 每个原字段的最终消费者、生效时机、证据与前置 |
| [界面流程与性能实施细则](UI_FLOW_PERFORMANCE_SOLUTION.md) | 原生窗口、路由、导入、右键、底栏、连接表、分页 |
| [任务入口](tasks/README.md)、[执行依赖](execution-manifest.json) | 按唯一流程领取小卡；子代理文件所有权与整合顺序 |
| [完整验收矩阵](ACCEPTANCE_MATRIX.md) | 正常、失败、取消、重开、核心、平台和正式包退出条件 |
| [接口及任务所有权](INTERFACE_AND_OWNER_MAP.md) | 共享合同、SD/RT子包对应SP卡、Windows阶段与总门 |
| [给执行模型的开始指令](START_IMPLEMENTATION.md) | 可直接交给下一个模型的操作约束 |

原方案 `outputs/V2RAYN_FLUTTER_RUST_PLAN.md`、四份台账和旧 R4 卡仍有效；本方案补足当前失败合同，不覆盖历史、不降低库存分母。全范围包括 110 feature、185 action、180 设置字段（155 叶子、23 容器、2 内部）、158 实体字段、36 布局条目、3 主布局、53 窗口、14 核心及六 OS×架构单元。不同维度不能相加当功能数量或完成率。

## 2. 先解决的用户流程

1. 导入合成节点 A/B，选择 B，普通按钮启动，确认实际 B 的配置、PID、端点和流量；故意让本项目自己的核心退出，界面应及时显示退出及可重试错误。
2. 快速启动 A→停止→启动 B，最终结果符合最后一次用户意图；界面不因旧回包、超时或队列优先级回到错误状态。
3. 保存设置成功而核心/平台应用失败，显示“已保存、未生效”的分阶段结果；重试应用不再提交旧版本，也不重复成功步骤。
4. 路由删除部分成功后出现失败，随后关闭/确定不复活已删除规则；读取失败不被包装成空列表继续写入。
5. All 和有组导入都必须纯预览、一次提交；取消不写文件，批次失败不产生半批节点；Custom 配置文件与数据库有可恢复的提交关系。
6. 默认 B、当前组 G，临时选中 C 后备份；改动再恢复并彻底重开，默认 B 和组 G 恢复正确，表格选择遵循原版刷新规则；实际运行核心身份另行确认，不因恢复配置就声称已切换。
7. TUN、系统代理、自启的界面显示以实际确认结果为准；清理失败保留归属和恢复入口，不能报成功后遗忘资源。
8. 10k/100k 真实数据库操作时仍能移动选择、打开右键、取消查询；后台查询和界面绘制都受测，不用虚拟表格掩盖同步读全表。

前四类数据与实际状态阻断未关闭时，不发布“完整移植版本”。P2 的原版交互、设置与布局差距同样必须完成，只是实施顺序在安全合同之后。

## 3. 共享合同：先定义，再并行接线

本节类型与方法均为**拟新增或拟扩展的设计合同，当前不能当作已有接口调用**。SP-00 将它们对照现有 `JobState`、FRB、IPC 和存储模型收敛为编译可用的签名。分册命名有差异时，以经整合测试固定的合同为准；不得让两个 DTO 表达相互矛盾的事实。

### 3.1 四种节点身份与三类版本

必须分开保存和展示：`selectedProfileIds`（表格选择）、`currentGroupId`（视图筛选）、`desiredDefaultProfileId`（默认配置）、`actualDescriptor.targetProfileId`（真正运行的冻结目标）。点击选择不能暗改默认；保存默认不能假称核心切换；退出后历史 applied 信息不能继续表示端点活跃。

- `desiredRevision`：已保存的目标配置版本，已有概念继续保留。
- `appliedRuntimeRevision`：被某次成功运行计划实际应用的版本，来自冻结计划；失败、取消和仅保存不更新它。
- `actualGeneration`：会话/主核/sidecar 事实代次。退出或资源变更必须推动事实更新，即使 desired 未改变。
- 设置域/路由域/数据集的 revision 可以独立；DTO 必须标明域，禁止把一种 revision 当另一种使用。
- `datasetEpoch`：整库恢复/备份候选替换激活时创建的新数据代次，普通设置、文本导入和订阅增量提交保持它；保存、查询、窗口请求和重试必须携带此值，防止恢复后revision计数重复让旧请求生效。
- 跨文件提交用 `mutationId/commitId`、可选同代次内 `commitEpoch` 及恢复日志；commit身份不是datasetEpoch。不能声称 SQLite 事务能原子提交 JSON、ZIP 或 OS 注册表。分册里单独写epoch的字段统一指datasetEpoch，逐mutation journal身份另列。

### 3.2 用户命令与实际运行描述

```text
RuntimeIntent {
  clientCommandId, clientInstanceId, intentSeq, operationId, datasetEpoch,
  action: Start | Reload | Stop,
  expectedDesiredRevision?, explicitTargetId?,
  frozenPlanHash?, acceptedGeneration?
}
OperationReceipt {
  operationId, jobId?, acceptedIntentSeq, hostInstanceId, hostAdmissionSeq, status,
  targetId?, plannedRevision?, newRevision?, error?, actualSnapshot?
}
RuntimeActualDescriptor {
  sessionId, actualGeneration, operationId, intentSeq,
  targetProfileId, targetCore, coreVersion, planHash,
  appliedRuntimeRevision, mainPid?, sidecarPids[], readyEndpoints[],
  mainState, sidecarStates[], tunLeaseFacts, lastExit?, lastError?
}
RuntimeSnapshot { desiredRevision, datasetEpoch, latestAcceptedIntentSeq, actualDescriptor?,
  activeOperation?, pendingCleanup[], eventEpoch, eventSeq }
```

`operationId` 由Rust应用入口在发送前分配并登记；`jobId` 是已有任务设施身份；`sessionId` 是实际核心会话；三者不拼接成一个字符串再靠 UI 解析。host按(clientInstanceId, clientCommandId)+payloadHash去重，同键不同内容明确冲突。readyEndpoints为带scheme/owner/core/API类型/auth-required标记的typed端点，秘密只留Rust私有context。host自身快照只持实际事实；这里带desiredRevision的RuntimeSnapshot是application组合快照。已有 `JobState` 保留，新的 operation 状态若需要 `Accepted/Executing/Succeeded/Failed/Cancelled/Superseded`，作为单独版本化合同接入，不直接破坏历史 Job 枚举。Start/Reload在host映射Apply，Shutdown/RetryCleanup按RT分册的枚举统一收敛，不并存两个无映射action枚举。

提供方：application/net_host。调用方：bridge_api、runtime_controller。拟方法：`submitRuntimeIntent`、`queryOperation`、`getRuntimeSnapshot`。落点：`crates/application/src/engine.rs`、`crates/ipc_contract/src/lib.rs`、`services/net_host/src/session.rs`、`crates/bridge_api/src/api/engine.rs`、`apps/desktop/lib/features/runtime/runtime_controller.dart`。

### 3.3 设置保存与生效

```text
SettingsSaveReceipt {
  mutationId, datasetEpoch, commitId, commitEpoch?, newRevision?, savedDocumentToken?,
  save: NotStarted | Rejected | Committed | CommitUnknown | RecoveryRequired,
  coreApply: NotRequired | Pending | Succeeded | Failed | Unknown,
  platformApply: 同上,
  phaseErrors[], appliedContentHashes, operationIds[]
}
```

提供方：application/persistence/platform；调用方：主窗、独立设置窗。拟方法：`saveSettings(datasetEpoch, expectedRevision, mutationId, patch)`、`querySettingsMutation(datasetEpoch, mutationId)`、`retrySettingsApply(datasetEpoch, mutationId, savedDocumentToken, phases)`。未提交的拒绝没有newRevision；DB已提交但文件发布失败返回CommitUnknown/RecoveryRequired，阻止新写并先恢复确认，不能向用户声称配置完全未改变。保存确认Committed返回新 revision/token 给所有发起窗口。核心或 OS 应用失败不抹掉已成功保存的事实，也不把 desired 当 applied。重试仅执行未成功阶段，参数和内容 hash 必须指向已保存版本；验证它仍是当前可应用版本。

生效时机保持上游：有些立即保存、有些确定提交、有些需 reload/重启；不能统一成“每次控件变化马上重启核心”。窗口是否关闭、何时 reload 依据该原版入口，业务结果则始终可见。

### 3.4 原生窗口请求

```text
WindowRequestEnvelope { windowId, windowGeneration, requestId,
  mutationId?, datasetEpoch, domain, expectedRevision?, payload }
WindowSaveOutcome { windowId, windowGeneration, requestId,
  datasetEpoch, status, saveReceipt?, newRevision?, error? }
```

MethodChannel 的运输 ACK 只表示请求送达。真正提交结果通过结构化 Outcome；异常也必须产生 Outcome。迟到、重复、关闭后回复按 windowGeneration/requestId 过滤。有限等待到期进入“结果待确认”，查询 mutation/operation；不能自动重放不可幂等的保存。新窗口不得接收旧窗口回包。原版先关闭后 reload 的入口保留，在主窗提供可见的结果和重试。

### 3.5 查询、批提交与事件恢复

拟 `QueryProfilesPageAsync(filter, sort, cursor, datasetRevision, requestGeneration, limit)` 返回 `items/nextCursor/datasetRevision/total?/error`。查询在 Rust 后台或明确的 worker 执行，不能在 UI isolate 同步循环拉完所有页。排序必须稳定，包含 ID tiebreaker；数据修订变化时明确让游标失效并重查，不能悄悄跳行/重复。

拟 `PreviewImport` 不写 DB/受控文件；`CommitImport(previewToken, expectedRevision, mutationId, targetGroup?)` 一次提交整批。All 不再逐条调用旧保存方法。受控 Custom 文件 staging、DB 引用发布、失败回滚、崩溃恢复具有明确所有权和 journal。

控制事件包含 `eventEpoch/seq/operationId/generation`；订阅 Lagged 返回 `ResyncRequired`，之后取权威快照、更新订阅起点。日志/速率/测速进度使用独立可合并通道；生命周期和最终结果不能被高频日志挤掉。IPC与helper协议同包升级V2/握手，旧host/helper不兼容报可读错误；旧helper仅受控只读/旧Stop清理兼容，不承接新TUN写入。只关闭本项目确认为自己拥有的会话；不能按进程名杀掉其他代理。

## 4. 运行状态和命令的实现顺序

### 4.1 一个权威命令序列

UI 立即给出 pending 反馈；后端在会话维度串行协调 apply/stop/cleanup，不再用“pendingApply 总优先于 pendingStop”的分支拼队列。先验证输入并冻结目标、计划、revision，再接受 intent。只合并尚未开始且合并不改变原版语义的apply；所有Stop/Shutdown都是不可supersede的清理barrier，包括尚未开始的项。对不同 clientInstance 做后端全局接受序号，不拿各窗口私有 seq 作跨窗口排序。

A 在途→Stop→B：若 A 不可安全中断，完成当前安全阶段后按接受顺序 stop，再启动 B；若可取消，则补偿 A 的已拥有资源，再执行 B。旧 A/stop 完成回包可以记录历史，不能覆盖最新 B 的当前视图。取消只请求取消，直到实际补偿结束才成为 Cancelled。

网络超时不等于业务失败。UI 先查 command/operation/session，Running 就呈现实际 Running；执行中继续观察；确认为 Failed 或有证据NotSubmitted后允许新尝试。查询不排生命周期写队列、不持Inner长锁，使用独立可取消且有界IO，不能又被正在apply的操作堵住。自动重试必须复用幂等 ID 或仅查询，禁止生成新 ID 重复写入。

### 4.2 进程持续观察

ready 检查只证明启动时成功。net_host 持有主核/sidecar 的进程身份与退出观察者；自然退出、自己发起 stop、提权 helper 退出分别建事实。主核退出清空活跃 PID/端点、保留历史 applied 信息、产生结构化退出原因；不能保留 cached Running。sidecar 异常按依赖图决定主核暂停/降级/停止，并给出实际失败阶段。

普通与提权进程统一支持 ready/exit/query/stop。提权 RunProcess 的成功只表示派发，不是端点 ready。记录 PID 与创建时间/随机身份及会话归属，禁止复用 PID 后误杀。apply 的一次性 job 在 apply 成功后结束，持续运行核心由 session 状态描述，不保留永远 Running 的启动 job。

### 4.3 TUN 归属、心跳和清理

用 leaseId/sessionId/generation/owner、每项资源状态和恢复 journal 记录本项目创建的 adapter/routes/DNS/process。活动保活由真正持有会话者执行，idle timeout 只回收失联会话；不能用更长 24h 代替心跳。保活和清理各自有超时、鉴权和去重，崩溃重开从 journal 恢复。

清理顺序按依赖反向执行，并逐资源确认。某步失败保留 PendingCleanup 和资源归属，传播结构化错误，下次重试只处理未确认项；不能先 take/clear lease、删 journal、closed=true 再吞异常。AlreadyAbsent 可幂等成功，但“查不到/权限错误”不是 AlreadyAbsent。journal 写盘失败也必须可见，不能宣传恢复保障。

TUN 标签由实际 lease 和主/sidecar ready 事实计算；dry-run 只能表示“计划检查通过，未建立会话”。desired=false 但存在待清理租约不能显示“已关闭”。IPv4/IPv6、process protection、adapter 名称与地址归属统一从冻结 RuntimePlan 和平台探测来，禁止环境变量偷偷换名字导致回收定位错位。

所有 OS 副作用验收在授权隔离环境；普通端口不隔离默认路由、DNS 或管理员操作。未取得环境时继续做纯合同、配置、文件 journal 和本项目自己拥有的普通核心会话。

## 5. 数据与设置修复

### 5.1 不再把坏配置默认为一份新配置

区分不存在文件（首次初始化）、存在但空/截断/null/类型错（结构化损坏错误）、缺少可默认字段（仅补该字段）、未知键（保留）。错误配置进入只读恢复流程，不启动默认 10808、不覆盖源文件。兼容迁移先写备份与 migration journal，校验成功再发布；日志只记字段 ID/错误类型，不能记录节点凭据。

JSON 与 SQLite 使用可恢复提交：校验输入及当前datasetEpoch→生成mutation/commit身份与staged文件→写恢复记录→DB事务→发布JSON/文件→确认完成。普通mutation不更换datasetEpoch，整库恢复/替换激活才建立新代次。失败或崩溃按journal的真实阶段回滚/roll-forward；文件刷盘/替换失败可观测。恢复期间不向UI提供一半旧一半新的可写快照。具体算法以设置分册的迁移合同为准。

### 5.2 canonical 数据双向一致

把原版 `IndexId/SubIndexId/UiItem` 与现有存储中同义值建立唯一读写适配。缺字段和旧值冲突有确定的迁移优先级；正常保存、上游 ZIP 导出、本地恢复和 WebDAV 使用同一 canonical 映射。不能只给新私有字段写 B，却留旧 `IndexId=A` 给原版导出。

规则删除、订阅更新、组切换、节点移除要检查引用；158实体字段包括枚举/内部引用，不能仅比较节点URL。临时selected C不要求写入原版备份：冻结ProfilesViewModel.cs的RefreshServersBiz（361–375）先消费内存pendingSelectIndexId，否则选择可见默认IndexId，再否则首行；重开时按此规则。在同进程分页/排序仍按ID保留选择；ui_state不得跨重开覆盖原版默认选择合同。每次迁移/备份验收包括独立进程重开和冻结原版可读性；敏感生产备份不作为测试夹具。

### 5.3 180 字段逐项接消费者

从 CSV 每次领取一个叶子 ID 或不可分割的原版生效组，派生 SP-23 实例；容器/内部字段仅按结构、迁移和引用验收，不造 23 个设置开关。每项证据必须包括：原控件/默认/合法范围→UI 草稿→真实 FRB→Rust 保存→重开→最终 core JSON、应用行为或 OS 确认。只做 round-trip 的字段仍不能标 verified。

优先修 Happy Eyeballs 的 enabled 门控、MaxSplit 合法范围及原版取下界的配置语义，随后补应用日志、RootCertProvider、Mihomo 合并、外部模板、本地 SRS 等生产消费者。RootCertProvider 是应用 HTTPS 使用的信任来源，不是往 Windows 安装证书；它的验收使用受控 TLS 服务器和合成 CA。

硬件加速必须证明 Flutter Windows runner/引擎真正支持所需渲染行为。关闭 Impeller 或选择低功耗 GPU 不等于软件渲染。先做可行性卡，给出实际设备/renderer 证据及重启语义；若须扩展 runner/engine，独立 ADR 和实现任务，不能把勾选框存下来当完成。

### 5.4 平台生效按内容去重

系统代理 applied key 包含规范化的模式、会话、端口、bypass、PAC 内容与高级选项；同端口改 exceptions 必须重新应用。OS 失败不更新 appliedContentHash。查询实际平台结果与 desired 分开，重开后不能从 desired=true 推导自启已写成功。

更新和 Clash 选项保存失败要回滚临时视图或明确显示未保存草稿，不能默默前进。平台写入和核心 reload 使用已保存版本，旧错误不永久挡住新重试。自定义 PAC 缺失按上游 fallback 行为处理，不能擅自向用户指定文件路径写入默认内容。

## 6. 原版界面与实际操作

节点单击即时选择，双击/Enter/设置默认按各自上游入口；Ctrl/Shift 状态在事件发生时捕获，不拖到异步回调再读取。右键多选保持、空白点击、菜单 target 冻结、失焦/滚动/窗口失活/子菜单/Esc 等逐项对照冻结原版实际窗口。已有 target 冻结修复保留；未实际对照的消失/重现行为列未验证，不靠 Flutter 默认行为猜。

路由读取错误不能变成空的可写草稿。即时提交与子编辑确定提交保持原版，每次提交成功更新 revision 和本地基线；部分删除失败重查权威快照，关闭仅按 Modified 触发 reload，不拿打开窗口时的旧全集回写。

原生设置/路由窗带新 revision、主题、字体、语言和 DPI 快照；新组订阅和编辑当前订阅直达对应对象，不把入口统一变成总列表。当前组重开恢复 canonical SubIndexId；状态栏显示真正运行节点，同时保留原版测速入口和适用性。

状态栏最小宽度按原版左右分区：固定操作左区、伸缩中区、固定速率右区。先核验原版约束与最小窗宽，再定控制宽度/留白；800px 下关键左右区同屏，不能靠横向滚动才看见速率。连接表使用真实虚拟数据源，补列宽/列序/排序/右键与持久化，不先全量构建 DataTable。

消息按当前操作、产生时间、严重性、是否仍有效管理。旧平台错误保留到历史区，不能永久占主反馈位置遮住最新保存/测速/启动结果。需要用户处理的失败仍醒目，不能清空错误来掩盖未生效。

## 7. 性能实现与量测

先把 SQLite/文件/解析/全量归并从 UI isolate 的同步 FRB 调用迁出，再优化绘制。首屏只取可视页加有限预取；ID 是选择/焦点身份，不能用排序后的 index。排序、过滤在 Rust 查询层，参数化过滤并对现有索引做 query plan 验证；稳定游标/版本、取消 request generation、迟到响应丢弃共同接线。

测速/速率/日志 overlay 用增量和合并更新，只刷新相关可见行；缓存按 datasetRevision/filter/sort/pageKey/locale 失效，不给每个事件全量重新排序。导入、测速、连接与日志的大 I/O 分别审查背压、批次大小、取消安全点，避免把阻塞迁移到另一个共享锁。

100k 的约 2982ms 是实际 release DLL+SQLite 同步全读观测，不是最终 GUI 启动指标。修复必须测正式 release GUI，而不是只比较 async API 名称。固定相同原版/本项目内核、配置、合成节点和端点，记录 p50/p95/p99、原始样本、线程阻塞、帧、内存、队列等待及数据库规模。

门槛来自原规格：普通反馈 p95≤100ms；1000 节点冷启动可交互 p95≤2s；热启动≤1s；托盘显示≤150ms；10k 搜索排序 p95≤200ms；10k UI/raster 各 p95≤16.7ms，超预算≤1%。冷启动至少20样本、热启动至少30样本，公开 warm cache、硬件、DPI 和后台负载；不得只报两次均值。

## 8. 测试基础设施、核心和发行

当前门禁不全绿：Rust 1410 pass/1 fail/5 ignored；Flutter 批跑失败，三个文件补跑仍异常未完成；Dart 三文件格式需修。单列 SP-29，先明确 helper request/idle/heartbeat 合同并修测试的配置，不通过加长等待或删除预期装绿。Flutter 异常记录退出码、测试名、原生 dump/栈、加载顺序和资源占用，区分断言失败、fixture 错误和测试基础设施退出；根因未明不能算通过。

14 核资产存在和版本命令成功不等于14核产品接线验收。逐核覆盖正式界面选择、适用协议生成、真实本项目 host ready、合成 loopback/TLS 流量、日志、退出和恢复。自动更新核集合按原版实际支持；其它核保留手工安装/选择路径，不自造原版没有的自动更新要求。新上游 checksum、exe SHA 和下载验签证明来源，不能只重复本地 hash。

本产品自更新使用本产品发布仓库和独立信任根。当次 UI flags 显式传给 Rust，不能复用历史 check flags。已有 PGP/GPG 校验实现保留并验证，不写成仍无验签。需要可重现真实发布端、公钥/轮换与撤销策略、下载验证、stage、安装/自替换、原文件锁、失败回滚与重开。产品输入未确定标 blocked，并完成可独立执行的解析、拒绝和回滚合同。

正式交付从固定干净提交重新构建，build-info 记录源码/锁文件/协议/FRB/内核锁/工具链/armed=false。对 runner、Dart AOT `data/app.so`、Rust DLL、net_host、helper、updater 及资源逐文件 SHA256；只检查 runner exe 不能证明 Flutter 界面是新版本。现有旧 ZIP 不能作为验收新修复的包。

解压到干净路径，使用独立合成数据目录，通过未武装普通 GUI 入口验收。`AUTO_SMOKE`、开发环境核心路径、预置 active、直接 controller/harness 只作辅助，不替代产品流程。默认启动不自动拉核、不自动启代理/TUN；安装核入口及缺核重试必须真可用。整套证据绑定同一 package hash。

## 9. 执行依赖与子代理协作

推荐顺序：SP-00合同→数据 SP-01..03 与 runtime SP-04..10 并行→窗口/保存/路由/导入 SP-11..15→原版交互 SP-16..20 与字段 SP-23..27→异步 SP-21..22→原核/测试 SP-28..29→正式 GUI/SP-30、性能/SP-31、平台/SP-32..33→持续/SP-35→最终包/SP-34。manifest 是具体有向无环依赖；代码合并后的验证仍串行。

每个执行槽每次只领取一张唯一流程卡。下列共享文件须整合者独占或预约写锁：`crates/application/src/engine.rs`、`crates/bridge_api/src/api/engine.rs`、`crates/ipc_contract/src/lib.rs`、`apps/desktop/lib/bridge/bridge_port.dart`、`settings_controller.dart`、`runtime_controller.dart`、`services/net_host/src/session.rs`。领域代理可先写专属模块和故障回归；接口缺口登记提供方/调用方/参数/错误/版本/生效点，不私改别人的文件。

FRB codegen 与生成物提交由单一整合者执行，版本三处2.13.0一致，生成后二次 no-diff。协议 schema 变更、数据库迁移、核心锁变更和 dependency upgrade 分卡评审；不能用一次全面重写绕过失败合同。严禁再次运行旧 execution cards 生成器覆盖已执行记录；本目录新卡也只追加证据。

完成卡需要红合同→修复→正确合同绿→真实入口→持久化/重开/最终消费者证据。未运行写未运行，未实测写未验证；状态只用 identified/implemented/verified/preserved_only/blocked/not_applicable。代码变更后只跑相关回归与要求门禁；无新变化不反复全量刷测试。

## 10. 不能简化的边界与最终结论标准

- **当前包不可验收为稳定完整移植。** 构建成功不说明实际流程可用；状态合同、数据风险、消费缺口、测试和包身份仍待修。
- 本机可做：上述纯合同、真实合成磁盘/SQLite/FRB、普通进程 ready/exit、UI原版对照、异步/性能、普通发行包入口。OS副作用验收另用已授权隔离环境，不阻止独立修复。
- 需要外部输入：本项目真实发行源/信任公钥；授权的 TUN/系统代理/自启隔离环境；其它 OS/架构构建和真机条件。不是通过勾选“待测”就能称完成。
- 每个适用库存项都有当前正式入口证据；P0/P1为零，完整移植P2差距关闭；持续24h负载/500次切换及故障恢复通过；六平台各自声明证据与未验证。Windows阶段通过可单独交付，不能冒充全平台全部完成。

本轮只新增方案、映射、执行卡和文档一致性验证，没有修改生产源码、生成桥、核心、冻结源码、台账状态或发行包。历史审计的测试结果保留，不把本轮文档检查说成产品测试。运行及设置两个领域子代理交付并交叉复核，界面委派中止后由根代理补全和整合。Oracle技能此前未找到，未调用付费API，不冒称第二模型Oracle结果。
