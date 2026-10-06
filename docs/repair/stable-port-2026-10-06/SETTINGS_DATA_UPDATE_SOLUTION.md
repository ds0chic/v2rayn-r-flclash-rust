# 完整稳定移植：设置、数据与更新实施方案

本文是待实施方案，不是修复完成证明。基线为 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`，冻结原版为 v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`。主要解决总审计 CP-05/06/07/11/13，连同配置与 SQLite 修订、兼容备份、UI canonical 状态及更新交接。所有“拟新增”类型、方法、模块均尚未实现；不得把本文签收当作相应代码或验收通过。

实施入口是本目录主方案 SP 任务卡；本分册 SD-01..18 是内部工作包，可进一步拆成固定小任务卡。字段分母始终为 180：23 个容器、2 个内部状态、155 个叶子字段。`SETTINGS_IMPLEMENTATION_180.csv` 为逐 ID 的最终消费者、时机、前置和验收清单。容器不是额外的设置开关，内部 ID 不是让用户手输的新功能；无原版控件的字段按原来的内部行为迁移。

## 1. 先固定不变量与所有权

1. Rust application 持有设置文档、DB、revision、提交状态及运行请求；Flutter 只持有编辑草稿与可见状态。原生窗口传输 JSON/DTO，不自行打开 DB、改配置或修改代理。
2. `IndexId` 是已保存默认节点，`SubIndexId` 是当前订阅/组，表格 focus/selection 是另一种 UI 状态，`applied_target_id` 是当前实际运行节点。不得再用“当前节点”一个变量承载四种身份。
3. `settings_revision`、各组 revision、desired revision、runtime applied revision 各有用途；恢复操作另带 epoch。保存成功可以早于运行应用成功，结果必须如实表示。
4. 用户取消表单不保存、不重启内核；已保存后取消运行应用不能宣称“配置未变”。运行结果未知先查 operation/mutation，不能重写完整旧草稿。
5. 原版 defaults、null/empty/missing、枚举整数和字段顺序的兼容规则逐 ID 保留。未知 JSON 键、未知实体列、原生 Custom 文档不能因为 DTO 没列出而丢失。
6. 默认原版端口仍是产品兼容语义；本机测试另用独立数据与先探测的 ≥11808 端口。绝不启动合成默认文档占用用户 10808，不改宿主代理/路由/Run-key，不终止非本项目进程。

文件所有权建议：

| 包 | 主修改模块 | 依赖/边界 |
|---|---|---|
| SD-01 严格读取 | `persistence/upstream_config.rs`、`application/engine.rs`、`domain/settings.rs` | 保留现有 load defaults；去掉整树失败 default 回退 |
| SD-02 可恢复提交 | `persistence/store.rs`、拟新增提交日志模块、application 持久化协调器 | 唯一 writer；不由 UI 操作文件；与全局 mutation 队列整合 |
| SD-03 canonical ID | application active/sub selection、backup activation | 与运行/节点/订阅包约定四身份，不修改 applied 事实 |
| SD-04 保存收据 | settings controller/actions/host、bridge settings DTO | FRB 仅指定整合者统一生成 |
| SD-05 平台事实 | platform controller/service、autostart/PAC | 借用现有系统代理归属 ledger，不另建无归属写入 |
| SD-06 UI canonical | ui shell/profiles/connection 表、native geometry | 由 UI 总负责人实现显示；本包规定存储、迁移与结果 |
| SD-07 通用参数 | application codegen、config_codegen、speedtest、scheduler | 已有有效消费者优先补全链，不重写全部生成器 |
| SD-08 Happy Eyeballs | codegen xray DNS/sockopt | 一处统一 gating、保留 skip 语义 |
| SD-09 Fragment | domain/parser、Dart validator、xray/sbox outbound | 原始字符串与解析视图分离 |
| SD-10 应用 HTTPS | 拟新增公共 HTTP policy/factory、subscription/updater/WebDAV | 禁止专用库反向依赖 application；不触碰 OS 证书安装 |
| SD-11 Renderer | Windows runner、构建配置 | 先独立可行性研究，SDK/engine 变化需整合者批准范围 |
| SD-12 应用日志 | application 日志初始化/轮转、诊断入口 | 不混用内核 LogEnabled；不记录秘密 |
| SD-13 消息/统计 | log service/monitor UI、统计列展示 | 保留 backend 统计事实与 UI 显隐区分 |
| SD-14 Mihomo | custom runtime plan、Mixin merge | 同 native 文档/端点/监控包共同验收 |
| SD-15 路由资源 | routing/dns/resource updater/codegen context | 下载资源在正式计划构建消费，不只写文件 |
| SD-16 更新 | update service/API/controller、updater、upgrade runner | 本项目发行源/公钥与核心自动目标分开 |
| SD-17 备份/WebDAV | backup service/lifecycle/controller、persistence bundle | 真实库往返与 UI 重开；不用假桥代替整链 |
| SD-18 平台专属 | macOS/Linux adapter、hotkeys/activation policy | 单平台字段按冻结平台矩阵，六交付单元独立验收 |

同一文件只能有一个实施者；其它包先提交明确函数/DTO 修改建议，由文件所有者合并。一次 FRB 合同窗口汇总 SD-02/03/04/05/06/16/17 的接口后统一生成，避免代理各自修改生成文件。

## 2. SD-01：严格读取，缺字段默认与坏文件分开

现状 `engine.rs::read_config` 把已有空文件当 `{}`，`read_settings_state` 用 `serde_json::from_value(...).unwrap_or_default()`。Dart loadFailed 已修，但后端伪默认会让它失去作用。不要将失败改成又一个警告后继续可写。

拟新增 `LoadedSettingsDocument`：`raw_json`（原始树）、`typed_view`、`revision`、`group_revisions`、`epoch`、`source_schema`、`content_hash`、`normalization_report`。raw 是持久化基底；typed_view 仅供业务消费，不能无条件序列化回去覆盖 raw。`ConfigLoadError` 带安全 error code、JSON pointer、field ID、原因、可否重试；不带订阅或凭据原值。

读取按以下顺序：

1. 文件不存在：进入首次初始化流程，创建经过原版默认规则归一的文档。文件存在但空、仅空白、读权限拒绝、UTF 解码错误、JSON 截断或根不是对象：返回失败，保留文件，进入只读恢复页。
2. 验证已知组及叶子类型。对每个 field ID 查冻结台账：缺省用原版规则；允许 null 的字段保持 null；组允许 null 时仅按原版合成该组；空字符串不自动改 null，只有指定 TrimEx/默认规则允许时转换。
3. 数组元素逐项验证，错误指到如 `/Inbound/0/LocalPort`。未知 enum/未来字段采用原版支持或显式 preserved-only 策略，不能强行把整个对象转为当前默认。已知字段坏类型/范围错误必须失败或进入显式修复预览，不静默重置其它组。
4. 未知键留在 raw 的原层级、原类型与数组顺序中；保存 patch 只替换拥有的路径。读写支持的历史键名/别名需从冻结 Config/已有迁移查证；别名同时出现时必须报告冲突，禁止大小写不敏感地把任意未来字段吞掉。
5. 通过后执行可追踪的 load-default/migration，记录迁移 ID、版本、摘要；首次初始化与旧文档迁移是不同流程。原件留可恢复副本；正常日志只写摘要及 field ID。

不要为了“逐字段”在 155 个字段里复制 155 套反序列化器。保留强类型 serde 视图，先对 raw 用生成/静态字段 schema 做路径级类型检查；typed parse 仍返回 Result，可用受控路径解析错误，所有失败不能 `.unwrap_or_default()`。schema 的来源/修改与四台账对齐，但本次方案不自动改台账。

复用 `persistence/upstream_config.rs::FieldState` 的 Missing/Null/Value 三态，不能在强类型 String 转换时先把它们合并。台账某些字段原版保存入口有范围校验而旧 load 不校验，需要区分“兼容读取旧值”与“接受新编辑值”：旧值按冻结 reader/default 规则保留并记录诊断；新输入走冻结 editor 校验。不可用一个通用正整数校验器拒绝所有原版能读取的历史配置。真正坏类型/无法解释的文档仍不得伪造默认。

修复 UI：读取失败页面说明原配置未改，提供重试、选择有效备份、查看安全错误定位；“恢复默认”必须是明确用户动作和有备份的单独 mutation。主/子设置窗都不得打开可保存的空 draft。其它 provider 的 getSettings 失败不得各自假造一份默认再写回。

验收：missing 文件与 present-empty 的结果不同；同一合成文档的坏 TrayMenuServersLimit 不得抹掉 en/KeepOlderDedupl/11808；unknown nested object、null/empty/missing、坏枚举、坏数组元素逐行可解释；保存失败/重开不覆盖原件。当前审计 pure probe 要改为正确预期断言，保留旧失败证据。

## 3. SD-02：JSON 与 SQLite 的可恢复提交合同

整合后的epoch语义固定如下：本文请求、窗口、revision和restore处的`epoch`指`datasetEpoch`，仅整库/备份候选替换激活更换；普通设置、文本导入和订阅增量保留它。每次提交新建的是mutationId/commitId（可带代次内commitEpoch），不是新数据代次。所有query/retry及旧窗口回包同时验证datasetEpoch。DBCommitted后文件未确认发布属于commit_unknown/recovery_required，不是“未发生保存”的普通失败。

文件 rename 和 SQLite transaction 不能组成一个 OS 原子事务。本方案承诺“应用读取不暴露混合 revision、崩溃后可恢复”，不宣称跨文件瞬时原子性。不得先推进 revision/写一半 DB 后返回普通失败，再让 UI 认为没有发生过提交。

建议在 SQLite 的应用专用表增加 `AppCommit`（不改原版表字段）：`mutation_id`、`epoch`、`base_revision`、`new_revision`、`phase`、旧/新 config hash、候选 JSON 引用或保护的 payload、所属候选资源清单、完成时间、最后错误。若记录含配置值，它是数据目录的受保护恢复数据，绝不进入证据/日志/遥测；日志只记 ID/hash/phase。额外引入单 writer 的 `PersistentMutationCoordinator`，所有会同时改配置/实体/修订的入口进入它。

拟定提交阶段：

1. 在mutation队列冻结当前datasetEpoch、expected revisions、用户输入及目标身份，新建mutation/commit身份并验证业务/引用。生成候选JSON；自定义文件先生成到归属明确的staging，未提交不得进入可见live路径。候选资源完成写入、flush/hash核验后才允许DB commit；未提交候选仅按归属清单清理。
2. 同一 SQLite transaction 写实体变化、desired/global/group revision、候选 JSON 的受保护 payload 与 AppCommit=DBCommitted。小配置候选以 DB 中 payload 为可靠恢复来源，发布用临时 JSON 可在提交后重建；外部资源引用只能指向上一步已持久化的候选。不得把未 fsync 的临时文件当已准备。
3. 检查 live 配置仍匹配 base hash/epoch，将候选经受控原子替换写入 guiNConfig.json；处理 Windows ReplaceFile/rename 覆盖语义及锁定失败。保留必要 old snapshot；不删除用户在提交期间修改的新文件。
4. 事务标记 ConfigPublished/Complete，核对 hash，再发布权威 snapshot/revision 和 SettingsSaveReceipt。应用内读取从协调器拿完成视图；待恢复阶段不可读成新 DB+旧配置混合树。
5. 如果 DB 已提交但文件发布失败，返回 `recovery_required/commit_unknown` 及 mutation ID；阻止新的修改，保留日志。重试先查询该 mutation，执行幂等补发或明确 rollback，不能从 UI 重放一份旧全稿。

启动必须在 AppEngine/provider/runtime 初始化前恢复 pending journal。DBCommitted 且目标文件仍是 base hash：补发候选；文件已是 new hash：补记完成；两边不同且存在外部修改：进入 Degraded/冲突恢复，不覆盖第三方修改。每一步有幂等 stage、完整归属清单与错误。彻底退出/杀本项目测试进程后再启动要得到一致 revision。旧数据没有 AppCommit 时进行一次迁移，不把缺少 journal 当坏配置。

单改设置不需要复制完整 100k DB；小配置候选和 SQLite 同事务日志即可。备份/升级另用 SQLite backup API 做一致性快照。组保存只推进所属组及全局/desired 修订，完整保存推进相关组规则沿用当前合同；刷新权威版本失败必须保留“需对账”事实，后续 patch 先恢复对账。恢复 import 不能把旧包里的修订计数直接作为当前新 epoch 的计数。

验收在每个 stage 前后注入失败/进程退出：DB 未 commit 无效果；DB 已 commit 文件未发不能显示完成；重开只完成同一 mutation；旧响应不能写新 epoch；并发 node save/settings save/import/restore 不出现 lost update；外部改 JSON 不被无条件覆盖。证据不能只检查内存 revision 数值。

## 4. SD-03/06/17：canonical 状态及三种备份往返

`AppSettings.index_id/sub_index_id` 当前已存在，不需要另造第二份持久化 ID。逐步使 `set_active` 同一提交更新 typed/raw `IndexId` 与暂留兼容的 `active_index_id`；前者成为 canonical 默认身份。生产 open/reopen 按显式版本迁移，旧只有 active_index_id 时回填 IndexId；两个都存在却不同，识别本项目旧 metadata 的历史来源，显示迁移选择及记录，不能任取其中一个覆盖。确认历史本项目“切节点只写 metadata”的文件，以 meta 活动值迁移；正常原版文件只有 IndexId，按原版值和导入 ID map 处理。

`SubIndexId`由application的current-sub selection命令持久化。UI焦点、节点多选、滚动锚点可留运行中的ui_state，但当前组/默认节点不可同时以另一cache作为事实；跨重开选择遵循冻结ProfilesViewModel.RefreshServersBiz：没有内存pendingSelect时选可见默认IndexId，否则首行。临时C不作为原版备份必须载荷，也不能覆盖重开的默认B选择。import remap包含profile/sub/routing/DNS/template ID；按Remarks解析的引用仍保留原表达式，不全部转永久ID。运行已应用身份只从immutable descriptor/journal来，不能读live IndexId冒充。

UI canonical 规则：`UiItem.MainGirdHeight1/2`、MainColumnItem、WindowSizeItem、ClashUIItem.ConnectionsColumnItem 必须真实驱动分栏/列宽次序/窗口尺寸。迁移旧 ui_state/INI 一次：canonical 有有效值则优先；仅缺失时导入合法旧 cache；记迁移版本，再停止双份写入。保存实际位置/尺寸由 native 事件送 application，去抖；同一窗口 type/name 与 monitor/DPI 校正由 UI/native owner实现。失去屏幕/异常尺寸可回退可见值，但不得把 canonical 值暗改成其它窗口的默认。

备份生成从完成的 mutation snapshot 取得 canonical JSON 和 SQLite 一致性副本，manifest 带 schema、epoch/revision、引用资源/hash 与来源版本，不能简单复制可能落后的 guiNConfig.json。兼容 ZIP 的 `guiConfigs/guiNConfig.json` 中 IndexId/SubIndexId 必须是同一 DB 的 ID；engine metadata 不应改变旧客户端读取语义。新格式与兼容格式分别明确，WebDAV 仍使用原版可恢复目录/文件约定；不要覆盖旧客户端唯一可恢复包。

恢复顺序：下载/选择→识别→验证 hash/结构/资源→候选迁移/remap→冻结新 epoch并 drain 手工订阅/调度/保存任务→成功停旧 runtime和平台归属对账→可恢复交换 DB/config/resources→reopen→重载各 provider与native settings→按保存默认节点请求应用→独立返回 restore committed、runtime/platform apply 结果。清理/停核失败不能继续交换；交换后 runtime 失败不能谎报“恢复未发生”。保存窗口的旧 epoch 全部作废。

原版 ZIP import 当前确有 remap/activate，不要重写成纯复制。修当前 B→备份→恢复 A 的根因，在 export 和 set_active 消除 identity split，并使 restore 保留明确源语义。补测试：A→B 切换→本地新格式 restore、兼容 ZIP restore、WebDAV 上传后下载 restore，以及用原版读取兼容 ZIP，均默认 B；当前组非 All、列、主题、窗口尺寸、路由 default、DNS、Custom文件关联一致。无默认节点/空组和被移除节点按冻结回退规则。

关闭备份窗与取消 job 分开：当前 cancel 只丢 Dart结果，不能保证 Rust不写。拟新增真实 backup/restore job ID 和 commit checkpoint；未提交可取消，提交已开始则返回 already_committing 并继续对账，UI不得显示“已取消且没有变化”。关闭窗口不取消后台已提交操作，也不得隐去重要完成/失败事实。

## 5. SD-04：一次保存返回可重试的分阶段收据

统一拟新增`SettingsSaveReceipt`，字段至少：`mutationId`、`datasetEpoch`、`commitId`、`baseRevision/newRevision?`、`savedDocumentToken?`、权威group revisions、`savePhase`、`corePhase`、`platformPhase`、`restartAppFields`、`nextLaunchFields`、`normalizedPatch`、安全errors、可用retry action。未提交的rejected无newRevision；commit_unknown/recovery_required先恢复查询，不能假称未保存。确认committed则即便apply失败也返回newRevision/token，不能只返回bool/message。

建议阶段枚举分别定义而不混用：save=`not_started/rejected/committed/commit_unknown/recovery_required`；core=`not_required/pending/applied/failed/unknown`；platform=`not_required/pending/applied/failed/unknown`；“需重启”由字段时机与新 revision标记，而非平台已成功的替代值。

拟新增接口（由桥接整合者调整命名后统一生成）：

| 方法 | 输入 | 输出 / 用途 |
|---|---|---|
| `settings_open_editor` | windowType | snapshot、epoch、revision、group revisions、UI theme/font/locale 与 capability |
| `settings_submit_patch` | editorId、epoch、expected versions、owned field patch、requestId | SettingsSaveReceipt；保留未知/未编辑字段 |
| `settings_query_mutation` | mutationId/requestId、epoch | 权威保存收据，响应丢失后查询；不重写 |
| `settings_retry_apply` | committed mutationId、epoch、指定失败阶段 | 重试已有新 revision，验证它是否仍是当前 desired；不再次保存旧稿 |
| `settings_refresh_editor` | editorId | 当前权威snapshot，明确外部冲突；不自动覆盖用户草稿 |

`SettingsEditorOutcome`/native MethodChannel协议回传保存事实及 newRevision。当前 `_applyOptionDraft` 不再永久闭包捕获打开 revision；editor store记录本窗口已提交新版本，只有权威收据允许推进。用户修改后的重试有 patch/baseDiff；另窗/restore变化时报告冲突，列明当前与草稿有变化的字段，不用“取最新revision”强制覆盖。

ACK 仅表示送达，不表示保存完成。native requestId+window generation+epoch贯穿主/子engine；主回调捕获所有异常并回结构化错误；回复超时返回 unknown、保留请求记录并 query mutation。窗口关闭后回复只更新权威状态/通知，不串入下一个窗口。不要把32秒超时当事务已取消；具体时间预算由交互任务卡固定并实测。

FRB DTO 只传明确的字符串/整数/列表/枚举及安全错误；patch/raw JSON采用现有桥接的 JSON 字符串格式并附 owned paths，不让Flutter拥有任意落盘权限。Rust内部 `LoadedSettingsDocument` 的 raw树不直接作为可变桥对象。旧 SaveSettingsResult 入口可短期作为调用新 coordinator 的兼容适配器，生产所有入口接新收据后再移除；禁止保留一条旧直写旁路。Dart状态需分别有 persistedRevision、draftBaseRevision、pendingMutationId、save/core/platform阶段，`load()`不能把失败阶段或实际OS事实清零。

前端每次 option toggle（update/proxy sorting/autoRefresh等）都必须检查保存收据：持久化成功才提交 definitive UI state，或立即乐观显示并标未保存，失败回滚/提示/重试。空 selected cores 与 null默认的区别保留。组保存和全树保存不得有一个路径继续吞 bridge异常。

## 6. SD-05：OS desired、实际事实和内容 hash 对账

AutoRun desired从JSON读；`_autostartApplied` 禁止由 load赋成desired。拟新增读取实际注册状态/任务状态能力和 `AutostartAppliedFact`：enabled、targetExe、args、scope/backend、observedAt、appliedForRevision、error。未知保持unknown；失败后窗内/重开/重启都可幂等重试。同值但安装路径变了也要对账，外部删除/修改不能被旧缓存判已完成。

Windows冻结原版管理员计划任务与普通 Run-key的区别先核对 AutoStartupHandler；只实现 Run-key不能冒称管理员路径完整。安全权限不足/UAC取消需独立错误，保存的desired可以保留但UI说未应用。验收真实平台写入只在授权隔离环境。

系统代理/PAC去重的 `PlatformConfigHash` 由有效 mode、**实际运行**session/endpoint/protocol、exceptions、local bypass、advanced mapping、PAC路径及已读取内容hash、脚本路径/必要平台参数组成。秘密不作日志，内部hash不暴露原值。只有平台实际成功后写 applied hash；失败/未知不得堵后续重试。mode=unchanged遵循原版不改OS，不用“空变更”报已控制系统。

平台 apply与desired持久化分别返回，不再 `_persistMode` 忽略失败。临时平台效果已发生而保存失败，显示 actual≠desired并提供按归属 ledger撤回/重试保存；不得无条件把宿主改回一个猜测值。PAC缺自定义路径按冻结PacManager回退默认config/pac.txt；不可在用户错误路径悄悄造默认文件。仅拥有的PAC进程/文件可清理。

settings saveAndApply的顺序由原版时机决定：commit→immediate UI/App行为→需要的core apply→绑定真实applied endpoint的platform apply→return receipts。运行失败时不把desired端口写系统代理；仅 restart_app字段无需无意义重启core。某字段需要下次launch的提示跨关窗保留。

## 7. SD-07/08/09：网络参数、Happy Eyeballs 与 Fragment

已有 core basic/inbound/KCP/gRPC/Mux/Hysteria/DNS/speedtest字段优先走差异验收。每项至少有“改一个字段→保存→重开→生成前后差异只在允许位置→内核校验→可观察运行效果”。不是每项都造新的网络服务；mock只注入错误，最终生成器/SQLite用真实实现。core不支持某字段按冻结能力标 not_applicable，不能删掉另一个core的需求。

Happy Eyeballs：在 `fill_sockopt_domain_strategy` 或相应公共生成步骤显式输入 enable/skip条件，只有原版适用策略且 `EnableHappyEyeballs=true` 才发对象。TryDelayMs/PrioritizeIPv6/Interleave/MaxConcurrentTry原值从typed settings取，null/default处理按原版；关闭后仍保存参数，但所有适用outbound都不得残留happyEyeballs。按 `V2rayDnsService.FillSockoptDomainStrategy` 的skip路径做proxy/direct/dial分支用例，不能给所有outbound粗加开关对象。验收off→on→off和原参数非默认；真实Xray使用锁定兼容版校验，不以内部JSON包含一个键结案。

MaxSplit：拟新增domain纯 `parse_max_split(raw)->ParsedMaxSplit{from,to}`，无值/空白、单值、范围规则严格来自 `Utils.TryParseMaxSplit(0,10000)`。`1-3`合法；负数、倒序、>10000、多段非法。原始字符串保留，UI与Rust调用同合同（Dart可用相同纯规则并以Rust错误为最终权威）。冻结 `V2rayOutboundService.cs:843` **生成maxSplit只取第一整数**，所以当前兼容生成用from，不能自行发数组/随机范围或把to作为新wire语义。长度/延迟的列表和legacy Length/Interval迁移同样保留raw与按原版默认填充。未来支持其它core版本语义应单独能力卡，不能在本次偷偷改原版行为。

KCP/gRPC/其它整数字段的范围不要抄通用非负校验；逐ID对原版字段/default/单位和实际core语义，UI合法值与后端合法值一致。配置页面必须显示适用core的启用条件，不能用所有core都能保存替代真正支持。

## 8. SD-10/11/12/13：HTTPS、renderer、日志与统计

RootCertProvider只决定**应用自身HTTPS**根信任，来源system/chrome/mozilla；不安装OS证书、不改变代理内核TLS。订阅、更新metadata/asset、WebDAV、外部路由/Geo/SRS，以及原版使用此下载器的测速网络请求，统一接入配置policy。

拟新增低层 `net_http` factory或等效共享模块，供subscriptions/updater/application使用，避免subscriptions反向依赖application产生环。application构造 `HttpPolicy{trustSource,trustBundleVersion,proxyEndpoint,timeout,redirectRules,limits}` 后传入；专用库保留各自取消/体积上限/HTTP状态分类。`system`按当前选定TLS后端正确用系统根；chrome/mozilla使用受控公开根bundle并关闭不该混入的默认根，不能 `danger_accept_invalid_certs(true)` 伪造通过。bundle版本/来源/hash是发行资源，缺失/损坏返回可见错误，不静默降级system。运行中改provider重建或按policyHash取client，旧任务固定旧policy直到取消/完成，不污染连接池。

HTTPS验收用合成本地TLS服务器、两套公开测试CA及受控证书链，证明选A接受A拒B、选B反向、错误host/过期/不可信都拒绝；逐个实际客户端跑同规则。合成公开/临时测试材料不取用户秘密，证据不含私钥。远端账户只用于最终授权场景，功能接线无需等用户安装证书。

HWA必须先研究。锁定Flutter3.47.5本地Windows embedder头文件暴露GPU preference、Impeller与FlutterGPU设置，**没有可直接把EnableHWA映成SoftwareOnly的该层开关**。这并不能单独证明整引擎完全不支持软件路径；需要查锁定engine源码/实际embedder初始化/官方对应版本参数并做最小release实验。`DisabledImpeller`、低功耗GPU、禁FlutterGPU API不等于禁硬件渲染，不能作为假实现交差。

SD-11研究交付：可用启动前renderer选择途径、是否需自建embedder/engine、GPU不可用回退、multiwindow一致性、依赖锁/维护成本。若现成引擎支持正确软件路径，runner在创建任何Flutter engine前读取经过验证的启动设置并实际选择，所有子窗同policy；若需patch engine，先独立卡证明性能/发布可复现再纳入。确实不可实现时登记明确阻断与技术方案，不把开关改名降需求。最终以运行renderer事实、GPU/API观察和帧耗时验证两模式，不只看JSON或传了flag。

EnableLog是应用日志开关，与CoreBasic.LogEnabled分别实现。Rust、host、UI诊断logger初始化读取该字段、按原版restart_app时机应用，关后停止常规应用日志写入；安全恢复journal不是普通诊断日志，不能因关闭日志失去恢复能力。按原版日志清理周期轮转，清理只删除归属文件，敏感节点/订阅/header/cookie不入日志。

MsgUI.MainMsgFilter/AutoRefresh初始化和每次原版时机的更新均走canonical patch；过滤表达式语义与日志显示/复制原版一致，不能把旧regex换成任意contains搜索而仍叫对齐。自动刷新停止/隐藏窗口/队列上限的规则与日志服务配合，重开保留。EnableStatistics与DisplayRealTimeSpeed按原版启动时机控制真正采集/显示；表格统计列显隐、HideColumnIpInfo、EnableAutoAdjustMainLvColWidth要被实际组件读取，而不只是uiShell赋值。隐藏列不丢数据，关闭采集不制造零速率假事实。

## 9. SD-14/15：Mihomo、外部模板与本地 SRS

Mihomo custom runtime build目前绕过已有merge helper。正式接线在 native custom plan生成时调用 `mixin_options_from_app` + merge，而非生成后把任何原始配置JSON化。取同一settings revision的ipv6、mixin enabled、入站/PAC/Clash API相关参数，读受控Mixin文件；按冻结CoreConfigClashService的先后覆盖规则，保留未知YAML、数组顺序和用户配置非拥有区域。关闭Mixin停止合并而不删除用户Mixin文件。输出本次计划实际core消费的文件，然后core配置校验、真实启动、API端点/监控实际连通。解析/读取失败不应用一份猜测的默认配置。

外部RouteRulesTemplateSourceUrl增加真实异步job：有效URL→使用SD-10client获取→校验冻结模板schema/容量→候选路由集合→按原版replace/append、lock、default语义提交→刷新UI。失效URL、坏内容、取消、TLS失败保留现有规则；有外部source就返回unavailable的占位链必须被正式入口替换。Russia/Iran预设不能长期只存“待下载URL”；先提交可恢复的desired，再按原版要求完成下载，分别显示阶段，不称offline待下载等同全部应用。

本地SRS由application读取资源清单构建 `CodegenSettings.local_srs_files` 和基目录；不在纯生成器里扫盘。沿用已实现 `srs_request` 的 `{0}`类型/{1}文件命名映射和 `srss/geosite-*.srs` 布局，按冻结local存在/remote回退规则选择，不自行猜路径。文件下载→hash/格式验证→临时rename→资源manifest更新→下一计划使用同revision的snapshot。已有下载文件但没有填生产context不算功能实现。损坏本地文件要可解释地拒绝或按原版允许回退远端；离线模式不能还访问一个隐蔽remote URL。

RoutingIndexId旧迁移先对源routing ID remap，再设置对应IsActive及清理旧键，非法ID按冻结GetDefaultRouting回退。current普通set_default实现保留。验收原版旧设置/旧ZIP指定第二条路由，导入后实际生成用第二条，不能总取first。

## 10. SD-16：真正可发布的更新合同

14 个核心锁/资产/历史最小会话已存在，本方案不重做“新增12核”。冻结上游自动更新目标只有应用、Xray、Mihomo、sing-box；其它proxy core按原版人工提供native config/可执行文件。`v2rayN=99`是应用身份，绝不是可运行core。手工核下载主页/安装路径/缺核指导要清楚，但不要虚造原版没有的自动更新。

本项目应用更新必须有自己的发布repo、release版本/渠道、资产命名、metadata schema和可信公钥。当前生产repo=None、verifier内嵌上游v2rayN公钥；不能把上游源码项目发布包下载后覆盖Flutter产品。拟新增release构建配置 `AppReleaseSource`，含repo/允许域与资产、trust key IDs、schema/兼容区间；值来自本项目真实发行决策，未确定则blocked。runtime环境变量仅测试覆盖的规则保持，正式包不可被任意env重定向。

公钥为发行公开材料，签名私钥留外部发布系统。信任根轮换以旧可信key签署轮换清单/同时信任窗口或明确安装升级策略；不要把下载侧car里的任意公钥即时当可信根。优先固定纯Rust可支持的签名格式；若所选格式只能GPG验证，要把GPG作为真实发行依赖并验无GPG错误。当前上游LibrePGP key fallback不能自动等同本项目长期依赖选择。

UI当前flags显式送 `t16_apply_app_update_spec_with_flags(prerelease,via_proxy)`，将其接入BridgePort并统一生成FRB；保存flags失败先可见，不依赖上一次check的process-global选择。metadata/asset固定本次同一flags/policy/proxy session；校验渠道、版本上限、架构、来源、摘要、签名、解包路径与文件清单。hash来自同源不能替代可信签名；缺/错签拒绝替换。core若无signature资源按冻结及项目明确来源策略走checksum能力，不虚称所有14核都有上游签名。

下载、校验、staging、交接、安装、启动、回滚各有operation/phase。stage成功≠安装成功；runner已spawn≠新程序已启动。拟新增 `UpgradeReceipt`/查询接口，runner等待旧PID退出、检查明确install_root、锁定与归属、逐文件overlay日志、保留app.previous、启动新exe后等待健康确认，再把升级标Complete。

数据升级先记录DB/config一致性快照、data schema区间和恢复说明；新程序迁移失败或健康确认失败，rollback恢复匹配程序+匹配数据快照，不能启动读不懂新DB的旧程序。没有数据迁移也需证明旧版本能读；修改/新增schema要记录兼容界限。安装器独立验证portable vs installed路径、权限、快捷方式/注册项、卸载保留用户数据规则，不把flat文件copy测试当安装器实跑。

真实验收矩阵：未配置源明确不可用；stable/prerelease与直连/代理当次选择；真实发布签名与错签/缺签/篡改；平台架构错；下载中断/取消；文件占用/权限拒绝；runner交接旧程序实际退出；新程序启动/迁移失败回滚；已装core更新后旧会话停止/重启和缺核重试。普通未武装RC包最后验，不能只跑debug覆盖源、mock runner或自测试开关。

## 11. SD-18：平台专属能力不混成“等待授权”

macOS Dock activation policy和macOS/Linux自定义proxy脚本必须有正式consumer；只有路径校验不算实现。脚本执行的参数/cwd/超时/退出/副作用补偿按冻结ProxySettingLinux/OSX移植，不引入任意命令拼接。Windows隐藏/不适用该字段按WPF原版，不展示成有效Windows开关。

全局热键按冻结组合/多动作派发保存，注册与冲突是平台事实；候选编辑注册失败保留旧可用注册。原版时机表若记录next_launch，但当前已有即时重注册路径，先核对GlobalHotkeySettingViewModel/MainWindow正式流程并更新项目明确定义，不能由实施模型任选一个较方便时机。CSV保留冻结台账时机且标需要入口核定，避免把audit库存当最终事实。

每个OS×架构独立构建并实测相关API。权限缺失、缺代码、没有平台环境、功能原版不适用是四种不同状态。非OS写入的消费者补线/纯验证不需要等VM；真实路由/代理/自启等有副作用验收仍守仓库边界。

## 12. 拆小任务、实现顺序与验收提交

建议顺序：SD-01→SD-02→SD-03→SD-04/05→SD-06/17→SD-07/08/09/10/12/13/14/15→SD-16/18；SD-11可早期并行研究，但不能在未研究前宣布支持HWA。网络/运行TUN对接依赖主方案命令队列、实际session/lease、清理journal，其问题不通过设置包私改底层绕开。

每个小卡只走一个可见流程：例如“旧稿外部冲突拒绝”“保存成功而apply失败后原生设置窗重试”“坏配置加载拒绝”“切B兼容ZIP往返”“关Happy后实际Xray配置”“切CA后真实HTTPS”“Mihomo关Mixin后启动”“当次prerelease自更新”。字段相邻且共用同消费者可同卡验，但不把180个字段并成一次save/reopen测试。

小卡必须给：固定源文件/符号和commit、目标field/action/layout IDs、允许文件、拟新增合同、正常/错误/取消/重开/权限时序、合成数据、必须命令与实际用户入口、证据目录、退出条件。允许生产源码修改时才执行；本方案没有进行任何产品修改或测试运行。

三层证据分开：A=真实实现的纯库/文件/SQLite/codegen合同；B=正式未武装包的UI→FRB→后台→落盘→退出重开；C=实际core/HTTPS/native OS效果。fake仅作故障注入。有A没有B/C只能implemented，不得按测试数字/代码行数算完成度；领域未验证不能随意not_applicable。每次将owner任务与180CSV相应行附当前revision、运行环境、证据路径和限制，原版分母不减。

本分册关闭条件：CP-05/06/07的现有正确合同均通过，并补原生窗口/真实备份往返；每个适用叶子有最终消费者与当前效果证据；恢复/更新/失败重试无伪成功、不丢未知键、不复活旧draft；renderer研究结论和真实实现清楚；平台未验逐实例保留。还须主方案运行/性能/完整用户流程/发行门禁共同通过，单独设置包通过不能宣称整个软件稳定。

## 13. 给实施模型的首批接线定位

| 现有入口/符号 | 第一次修改的明确职责 | 接口所有权 |
|---|---|---|
| `crates/application/src/engine.rs::read_config/read_settings_state` | 返回严格 Result；已有文件空/坏类型失败，采用raw+typed完整文档 | SD-01 |
| `crates/persistence/src/upstream_config.rs::FieldState/ConfigStorage` | 保留三态与未知raw；历史别名/迁移不吞未来键 | SD-01 |
| `crates/application/src/engine.rs::save_settings/save_settings_group/persist_config` | 统一进入 coordinator，revision与候选共同提交、无旧直写旁路 | SD-02 |
| `crates/application/src/engine.rs::set_active` | canonical IndexId与兼容meta同revision，返回权威身份snapshot | SD-03 |
| `crates/application/src/backup_service.rs::BackupService/create_local/activate_upstream_config` | 完成视图快照、源语义remap、恢复epoch/已提交事实 | SD-17，与SD-03/02合同 |
| `crates/bridge_api/src/api/settings.rs`、`apps/desktop/lib/bridge/bridge_port.dart` | 新 SettingsSaveReceipt/mutation/query/retry；生成文件由整合者统一产生 | SD-04 |
| `apps/desktop/lib/features/settings/settings_controller.dart::saveAndApply` | 新revision始终登记，失败stage保留；load不确认OS，retry不重存旧稿 | SD-04/05；一个文件所有者 |
| `apps/desktop/lib/features/settings/settings_actions.dart::_applyOptionDraft` | 原生窗口保存callback真正消费收据，窗口自身newRevision与外部冲突分开 | SD-04 |
| `apps/desktop/lib/features/settings/platform_controller.dart` | 实际事实与内容hash对账，失败不更新成功缓存 | SD-05 |
| `apps/desktop/lib/app/shell/ui_shell_controller.dart` 与表/窗口消费者 | canonical字段真实读取，旧cache一次迁移，配置UI不是只赋状态 | SD-06；UI owner |
| `crates/application/src/codegen.rs::settings_to_codegen/dns_to_codegen/mixin_options_from_app` | 一份revision下投影设置；Happy开关、Mihomo options真正进入生产计划 | SD-07/08/14；一个文件所有者 |
| `crates/application/src/engine.rs::build_resource_requests` 与 native custom-plan builder | snapshot包括local SRS清单，native YAML实际merge | SD-15/14；application owner |
| `apps/desktop/lib/features/update/update_controller.dart`、`crates/bridge_api/src/api/t16.rs` | 当前flags显式进入Rust；保存失败阻止继续；后续stage可查询 | SD-16 |

顺序上先修存储和收据，再接具体字段，最后收口真实平台/发行证据。任何包若发现现有入口不支持本表合同，登记具体接口缺口并提交提案给所有者，不能把功能降为仅保存，也不能在旁边创建第二套事实源。
