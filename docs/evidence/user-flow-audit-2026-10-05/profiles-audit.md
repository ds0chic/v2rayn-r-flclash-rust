# 节点完整使用流程审查与修复方案（2026-10-05）

状态：`identified`。当前生产代码基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`；冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。下文 `UP/` 指 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`，原版 Windows 行为以 WPF 为准。

本轮转为只读生产代码审查。根代理后续授权两项无副作用的短时序Flutter widget复现，实际执行均触发预期的失败断言，见UF13；未运行Rust测试、未启动内核、未操作宿主代理/TUN/自启、未读取个人节点或订阅数据。本轮“源码确认”仅证明路径/合同差异，不证明真实窗口已经复现。历史修复卡/测试记录只作为已实现线索；不能据此宣布当前完整用户流程 verified。

初始授权曾允许修改节点动作，收到只读要求后已撤回自己的 `profile_actions.dart` 临时补丁。`git diff` 与 `git diff --numstat` 均为空，文件 `git hash-object` 与 `HEAD:.../profile_actions.dart` 同为 `e3be0e30b15b773ddf6b0a9e6c74ed8daf04d868`。状态中该文件的 `M` 标记未用 reset 处理，以免影响外部工作。本轮交付本报告及证据目录中的两项复现源码/日志/observations；复制到apps/desktop/test的两个临时测试已在哈希核对后删除，不修改生产代码/原有测试/台账。

## 1. 首批守门问题

### UF-PROF-01 — P0：订阅成功更新删除同组手工节点

证据层级：源码确认，实机/真实 SQLite 场景未验证；状态 `identified`。

最小证实用户触发：在订阅组 A 中粘贴/扫码批导入节点 M（原版该入口 IsSub=false），或迁移/恢复一个已有 IsSub=false 的 M；随后成功更新 A，下载结果仅包含订阅节点 S。当前代码会按 Subid 删除 M 所在的整组，而不区分其来源。即使修好批导入的 IsSub，仓储删除范围仍会删 M。

原版合同：

- `UP/ServiceLib/ViewModels/MainWindowViewModel.cs:484-502` 剪贴板走 `AddBatchServers(..., _config.SubIndexId, false)`。
- `UP/ServiceLib/Handler/ConfigHandler.cs:1672-1673` 普通手工批导入写 `Subid=subid`、`IsSub=isSub`，这里为 false。
- 同文件 `2246-2259`，订阅更新调用 `RemoveServersViaSubid(..., true)` 时只删除 `isSub = 1 and subid = ...`；删除整个组才删除所有该组节点。
- 同文件 `2043-2049`，更新前保留原组与活动对象快照。
- 来源边界：冻结 `ProfileItem.cs:163` 的 IsSub 初始化值为 true；普通手工新建 `AddServerCommon:1201-1247`、普通复制 `CopyServer:372-392` 没有统一改为 false。不能据批导入规则断言原版所有手工入口都受保护。若产品决定统一保护所有手工新建/复制，须单列行为改善决策；本P0移植守门首先保证批导入/已存 IsSub=false 的节点。

当前路径：

- `crates/bridge_api/src/api/subs.rs:533-540`，通用 `import_from_text` 不论调用来源均写 `profile.is_sub = true`；节点页手工导入正使用该 API。
- `crates/application/src/subs.rs:737-760`，先读 existing、下载并生成 candidates，再 `replace_sub_profiles_at_epoch(..., true, epoch)`。
- `crates/application/src/subs.rs:631-636` → `crates/subscriptions/src/merge.rs:194-212`，existing 只用于计数，候选集仅由 incoming 构成，没有并入手工节点。
- `crates/application/src/engine.rs:2103-2136` → `crates/application/src/store_repo.rs:74-92`，仓储按 `Subid` 查询并删除整组，无 `IsSub` 条件。内存仓储 `store_repo.rs:562-574` 相同。

修复原则与范围：

1. 在 application 定义导入来源（manual_batch / subscription / restore），由来源决定 IsSub；不要仅靠 subid 是否非空推断来源。手工批导入绑定当前组但 IsSub=false，真实订阅候选 IsSub=true，备份恢复保留其来源。普通新建/复制分别对照冻结入口，不默认套用批导入值。
2. 把“追加手工节点”“替换订阅来源节点”“删除整个组”拆成不同用例；当前 `remove_existing: bool` 不能表达三者合同。订阅事务删除范围限定 `Subid=A AND IsSub=1`，手工节点与其他组一律保留。
3. 候选解析/校验/过滤/匹配在提交前完成；空结果、下载失败、解析失败、取消、restore epoch 冲突都不得删除旧数据。原版有部分先删后写行为，迁移不能因照搬该细节造成数据丢失。
4. 在一个事务中提交替换及相关身份/活动映射；若失败保证旧节点、活动引用、统计与排序不半更新。实际持久化边界由 application/persistence 协同处理，不由 Flutter 逐条补救。

安全验收：合成 A/B 两组，A 中 M(IsSub=false)、S-old(IsSub=true)，B 中 B-old。用本地隔离订阅返回 S-new 更新 A：M/B-old 原 ID 与所有适用字段完整保留；S-old 按匹配规则更新/替换；成功、空结果、错误、取消、中途存储失败、restore epoch 变化分别断言数据库与重开后结果。补“手工 URI 导入到 A 后再更新 A”真实 FRB 场景。测试可用本地 HTTP 合成内容，端口须先探测且 ≥11808，不能使用真实订阅。

### UF-PROF-02 — P1：订阅更新后活动节点与统计没有映射

证据层级：源码确认，实际更新后重启未验证；状态 `identified`。

用户触发：A 订阅中的 S 是活动节点，更新仍包含同一传输身份的 S；或更新删除 S。当前 candidate 未沿用旧身份时会取得新 ID，原 active 引用可悬空；节点表更新完成不等于下一次“应用/重启服务”可用。

原版：`UP/ServiceLib/Handler/ConfigHandler.cs:2109-2117` 更新后 `FindMatchedProfileItem` 匹配旧活动对象并写新的默认 ID；`:2120-2129` 匹配旧节点并克隆流量统计。`ProfilesViewModel.cs:398-402` 刷新还调用 `ConfigHandler.SetDefaultServer` 校正不存在的默认对象。

当前：`application/subs.rs:645-650` 对空 ID 分配新 ID；`:631-636` 的 refresh 不匹配旧对象；`:760` 整组提交。`subs_actions.dart:451` 仅 profiles.reload；`profiles_controller.dart:496-507` 读回旧 active，不调用回退。当前 `merge.rs:207-211` 的 existing_count 不能替代身份匹配/统计迁移。

修复范围：application 订阅提交用例 + persistence/monitor 的身份映射，Flutter 消费提交结果。优先沿用稳定 ID；若必须换 ID，按冻结 `CompareProfileItem/FindMatchedProfileItem` 的两级匹配合同建立旧→新映射，在提交中更新活动引用，保存/转移 ProfileEx、ServerStat 适用数据。旧 active 无匹配时执行原版可用节点回退规则并明确是否需要 Reload，不能只清 UI 高亮。不要把 IP/延迟/统计胡乱复制到传输身份已变的节点。

验收：相同节点更新、仅备注变化、凭据/传输变化、旧 active 消失、手工 active 保留、策略组引用旧节点、重复节点、多组更新、统计关闭等矩阵。检查活动标记、持久化默认 ID、重开、下一次生成配置、受管内核应用目标一致。匹配合同须直接对照冻结源，不能用字符串备注代替身份。

### UF-PROF-03 — P1：顶部“应用”的目标与用户当前选择分裂

证据层级：源码确认 + 用户报告入口失效；本轮没有实测用户节点；状态 `identified`。

用户触发：选中 B 后按顶部“应用”；后端无 active 则没有可应用节点，后端 active=A 则应用 A。用户仅选中节点并不能建立后台默认节点。

当前：`apps/desktop/lib/app/shell/main_shell.dart:653-694` 的 `_RuntimeToolbar`，key `runtime-start`、标签“应用”，`:677` 直接调用 `controller.applyActive`。`profiles_controller.dart:905-923` 点击只维护 selected/primary，不持久化 active；`:582-588` 才能 setActive。UI 名称与实际目标必须明确一致。

原版：`UP/ServiceLib/ViewModels/ProfilesViewModel.cs:560-591` 设置默认使用 `SelectedProfile.IndexId`，成功后 Refresh+Reload。原版 `MainWindowViewModel.Reload:664-739` 使用持久化默认节点；因此**选择≠设置默认≠重启服务**，不能为修顶部入口而把所有入口重定向到 selected。

修复合同：建立明确 `startSelectedAndApply` 用例，解析 explicit target → primary → single selected，捕获稳定 ID；没有 UI 目标但已有 active 时是否回退 active 必须由主方案固定，不得静默取第一条。成功设置 active 后显式应用，设置失败立即展示具体错误。顶部若保持“应用当前节点”语义，应接该用例；若坚持只应用后台默认，应把对象展示清楚，并提供用户可找到的“启动所选节点”。原版“重启服务/F5”仍使用持久化 active，不随选择漂移。

验收：无节点、列表有节点但无 active、A active/B selected、Ctrl 多选且 primary=B、过滤隐藏 primary、菜单 explicit=A、A active 且运行停止/失败、活动节点保存失败、端口占用、重复点击。检查 UI 目标、engine active、runtime plan profile id、applied revision、运行阶段及真实错误一致。

### UF-PROF-04 — P1：幂等设置默认与显式启动/失败重试混为一体

证据层级：源码确认；运行阶段推断未实测；状态 `identified`。

原版边界必须保留：`UP/ServiceLib/ViewModels/ProfilesViewModel.cs:569-577` 空 ID/同 active ID 直接返回。`UP/v2rayN/Views/ProfilesView.xaml.cs:192-201` 双击由 DoubleClick2Activate 控制；false 打开编辑器，true 设置默认，`:263-265` Enter 设置默认。**同 active 的 Enter/双击不 Reload 本身不是原版差异。**

当前路径与缺口：

- `profiles_table.dart:422-425` Enter、`:477-489` 双击都走 `setActiveSelected`。
- `profile_actions.dart:553-555` sameactive 提示后返回，符合设置默认的幂等边界。
- `profile_actions.dart:514-515` sameactive 直接 `ActivationOutcome(persisted:true, applied:true)`，但未调用 runtime，不能证明“已应用”。该对象把 noop 写成运行成功。
- `profile_actions.dart:350-352` 以当前 error==null 判断 apply 成功，不能表达“请求接受”“运行中”“已应用目标/版本”。
- `features/runtime/runtime_controller.dart:111-126` apply 清 UI error 后 refresh，旧 backend error 再次带回即可阻断请求；根/runtime 审查负责该链，节点入口不能靠再 setActive 绕过。

修复方案：设置默认保留 sameactive no-op；显式 Start/Apply/Retry 走独立用例（允许 sameactive 重新应用），与保留入口共用目标捕获/持久化/错误处理，不共用“sameactive就成功”的状态判断。结果至少区分 no_change、persist_failed、apply_rejected、accepted/pending、running/failed；最终成功由 runtime session/applied profile+revision 证据判定。编辑活动节点后 Reload 仍维持原版，不应因 sameactive 幂等门控吞掉编辑生效。重试清除旧错误的展示可做，但不能由 UI 清理伪造后端运行状态。

验收：Enter sameactive=不新建会话；显式 Start sameactive stopped/error=实际发起新申请；失败原因已修复后 Retry=再次校验并应用；旧错误保留在历史记录但不永久阻断；同 active 但 desired_revision 改变，显式 Apply 必须应用新版本；双击设置 false 必须编辑，不能强制启动。

### UF-PROF-05 — P1：“更新当前订阅”更新了另一个窗口的对象

证据层级：源码确认，真窗待验；状态 `identified`。

用户触发：节点页切到组 A，订阅设置曾选 B，点主菜单“更新当前订阅”。当前更新 B；订阅设置没有 selected 时则要求先去另一窗口选对象，即使节点页 A 已明确显示。

原版：`MainWindowViewModel.cs:186-192` 当前订阅更新直接传 `_config.SubIndexId`；`ProfilesViewModel.cs:334-338, 388-390` 维护/恢复这个当前组；对应 `ACT-MAIN-022/023` 台账 scope 为当前组。

当前：`apps/desktop/lib/features/subs/subs_actions.dart:442-450` 读取 `subsControllerProvider.selected`。节点页 `profiles_page.dart:105-116` 展示的组是 `profilesControllerProvider.groupSubId`，两者无自动绑定。

修复范围：subs_actions/main command dispatch + 共享当前组合同。命令开始捕获节点页当前组 ID，非空时读实际 SubItem，只更新该组。All组原版将空SubIndexId传下去，`SubscriptionHandler.cs:60-80` 只在subId非空时限定ID，因此All的当前更新等价更新所有合法HTTP(S)、非空URL订阅；迁移应保留此边界（若要改为禁用/确认，须明确行为改善决策）。“更新全部”保留独立入口。设置列表中的多选批量更新继续用设置列表的选择集合，不能全局强绑两个窗口 selection。

验收：A页/B设置选中、A页/设置无选中、All组、URL为空的普通组、当前组删除/禁用、开始下载后切组。请求里的 sub_ids 与命令开始时 A 一致；取消/失败保留数据；代理更新失败显示真实原因。

### UF-PROF-06 — P1：当前分组未持久化，重开与导入对象合同不完整

证据层级：源码确认，重开窗口待验；状态 `identified`。

原版：`ProfilesViewModel.cs:334-338` 写 `_config.SubIndexId`，`:388-390` RefreshSubscriptions 从它恢复组；导入/新增/更新当前组均消费该字段。

当前：`profiles_controller.dart:322-339` build 未读 SubIndexId；`:697-704` setGroupSubId 仅更改本地 state，未调用 settings/application 保存；节点页当前组与设置默认 `SubIndexId` 分离。build 也没有默认 primary/selected。重开重新 build 时回到 All/空选择，不能等价于原版。

修复范围：application 当前组设置 use case、bridge 读写、profiles 初始化/重开/删除组处理。SubIndexId 作为唯一持久化当前组；selected/primary 仍是 UI 临时对象，不能存成 active。组 ID 不存在时回退 All；普通 URL 空组仍可作为容器。读取组后获取对应列表并按 pending→可见 active→first 选择主行。

验收：选 A→重开仍 A；A/B 同名组以 ID 区分；删除 A→重开 All；新增/粘贴/扫码落到命令开始时的组；取消不改变存储；保存失败反馈且不显示未持久化的新组状态。迁移原版配置中 SubIndexId 也要同样生效。

## 2. 其余证实差异与待验风险

### UF-PROF-07 — P2：节点页“编辑订阅/新增订阅”都打开总列表

源码确认：`profiles_page.dart:118-128` 两按钮都 `openSubSettings`。原版 `ProfilesViewModel.cs:862-882` 的 EditSubAsync(false) 取当前组打开 SubEdit，true 构建新项直接打开编辑。修复两个明确命令：编辑当前组、创建新组，保存返回后刷新组项/当前组/列表，取消不改变当前组。All/no group 的编辑应按原版禁用/无操作，不能编辑设置列表旧选项。验收 A/B/All/空 URL 普通组、新建保存、取消重开。状态 `identified`，未真窗。

### UF-PROF-08 — P1（结构性延迟风险，尚未计时）：手工批导入重复提交，UI 同步逐条保存

源码确认：有组 `api/subs.rs:544-557` 已一次批量写入，再由 `subs_actions.dart:36-42, 116-122` 对每个 profile 执行 `persistImportedProfiles`；其 `import_persistence.dart:47-57` 每条同步 `profileRevision` + `saveImportedProfile`，`application/engine.rs:647-649` 每条 upsert、bump、persist_config。因此是“一次批提交 + N次再保存”，并非重复插入 N 个同 ID 节点，也不是覆盖旧组（remove_existing=false）。无组走 parse→UI逐条写，也缺少单个批事务。

该路径还可能显示“保存失败”，而组内部分节点在第一步已经写入；reported saved 不是实际数据库原子结果。不能仅优化 Dart 循环或减少 toast 来掩盖合同。

修复：统一 application 批导入用例，输入来源+组快照+去重策略，输出真实提交数/拒绝行/提交revision/选择建议；异步 FRB 一次调用，单事务后单次刷新。解析预览 API 应明确不持久化，提交 API明确持久化，避免同名 import 在有组/无组时副作用不同。冻结手工普通 URI `AddBatchServersCommon:1643-1645` 只在 isSub 时 Distinct；当前所有手工入口 deduplicate=true 也要单独恢复来源差异，不能擅自删重复手工条目。

验收 1/100/10000 条合成 URI，有组/无组，重复 URI、有效+坏行、异步切组、取消、revision conflict、一次存储失败。记录 FRB调用数、事务数、revision增量、真实持久化数、UI最大停顿与重开结果；指标测量后定门槛。本轮未测耗时，不能断言用户延迟全部由此造成。状态 `identified`。

### UF-PROF-09 — P2：首次打开/导入/刷新没有完整的原版默认选择与默认节点校正

原版 `ProfilesViewModel.cs:361-375` 每次构建列表后 pending→active→first；`:398-402` 调 SetDefaultServer；`ConfigHandler.cs:427-445` 默认 ID 无效/不存在时优先可见 Port>0，再全库 Port>0。

当前 `profiles_controller.dart:535-549` 已实现 UI pending/active/first，但只在新建保存 `516-522`、setGroupSubId `701-702` 使用；build `322-339` 初始化空选择，reload `496-507` 只保留/prune selection。导入、订阅替换、删除后的 reload 没有统一校正默认 active与可见主行。

修复：将“列表刷新后的 UI 主行建议”与“持久化默认有效性校正”分开，统一在列表数据提交/初始化后调用，既保留用户仍有效的多选交互，也遵循原版 pending选择和 active回退；不要每150ms测速轮询无条件重置用户多选。新建 port=0策略组/custom 的 UI主行可以选中，后台默认回退依冻结 Port>0 规则，不把 UI first直接当可启动。

验收初始化有/无active、空列表、导入成功、筛选清空、删除最后节点、活动对象删除、测速轮询期间多选、订阅替换。状态 `identified`，未真窗。

### UF-PROF-10 — P2：右键空白/表头的多选主行仍未完整冻结

已实现且不能算成新问题：菜单使用局部坐标；数据行右键成为 primary、保留该行所属多选；visible guard；菜单对象大部分显式传到编辑/分享/活动/完整导出；外点/失焦/生命周期关闭；根项/分隔与原版顺序已恢复。见 `profiles_table.dart:775-826, 1073-1105, 1122-1149`。

剩余源码差异：`:808-810` CommandContext.primaryId = rowId 或唯一selected；在表头/空白打开且多选时忽略已经存在的 `snapshot.primaryId`。单对象命令因传 null 再回落到 live primary（`profile_actions.dart:50-55`）；这不能保证打开时对象，刷新/程序化主行变化时可漂移。原版整个 DataGrid 的 contextmenu保留 current row，当前实现自己也声明该合同。

修复：快照明确保存 snapshot.primaryId（同时校验可见性），single-object command禁止由 captured-null静默回落 live对象；无目标按冻结入口启用规则处理。删/复制/测速/导出共享同一 command snapshot，不要通过先恢复全局选择再读取作为唯一保障。

验收 多选 A+B(primary=B)→表头/空白右键→程序化改变主行/过滤/删除→编辑/分享/活动/完整导出，只能B或明确拒绝；批量集合保持 A+B。真实鼠标中短暂刷新/失焦时验证，不仅直接调 controller。状态 `identified`，真窗未验证。

### UF-PROF-11 — P2：删除确认后的对象与确认内容可不一致

原版 `ProfilesViewModel.cs:504-517` 先 `lstSelected=GetProfileItems(true)`，确认后删除同一快照；当前 `profile_actions.dart:269-280` 仅用快照做数量/removedActive判断，确认后 `:291` 调 controller.deleteSelected，后者 `profiles_controller.dart:553-559` 再读 live selected。

普通模态对话框限制直接点击背景，不能据此断言用户能在弹窗中普通点击换组；但订阅刷新、节点删除、provider 更新、自动化或并发入口仍可让 live集合与确认集合不一致。删除目标和 activeRemoved判断不应来自不同时刻。去重 `_removeDuplicate:1215-1234` 同样在确认后读取 live group，修复卡已登记该缺口。

修复：命令开始捕获 targetIds/group/active reference/revision；确认展示该快照；提交时校验存在性/合法变化，明确部分失效怎么处理；后端返回实际删除IDs/活动受影响事实，不用旧UI推算。取消无写；失败详细错误可见。active回退+apply结果不能被“已删除”toast覆盖。

验收确认后合成触发列表更新/目标删除/换组、全部/部分失效、数据库拒绝、活动对象删除无替代、取消。状态 `identified`，真窗未验证。

### UF-PROF-12 — P2：测速期间全库同步重读造成 UI 停顿风险，停止结果缺乏运行代际合同

源码确认：`profiles_controller.dart:346-365` 150ms Timer 每次 reload；`:496-504` fetchSummaries后再次queryAllProfiles。真实 `bridge_port.dart:354-376` fetchSummaries 本身 queryAllProfiles + speedTestResults + statsSnapshot，`:380-390` 一次query至100000条且同步 FRB。一次poll因此至少两次全库读/DTO构造，表格虚拟化没有消除此成本。

已实现：选中测速与“当前列表全测”分开 `startSpeedTest:1257-1263`；错误/未完成总结 `:369-398`；UDP capability检查 `:1223-1242`；取消 `:1321-1333`；真实UDP会话在后续修复中已有实现，不能照搬旧“直接UDP探针”缺陷。

待验：停止只调用 cancelSpeedTest 并停止 poll，没有等待runner退出/最终结果补读；结果列表未按当前jobId过滤，成功/失败汇总可能混入上轮同ID缓存结果。必须先核对 runner/result_store 时间/清零合同并实测，不能仅凭此段宣布错误。并发启动/Stop→Restart、删除正在测速节点、订阅更新换ID、筛选/排序轮询、网络失败都需要代际验收。

修复：后台按operation/job提供增量结果与结束事件；UI一次获取静态节点页，按ID合并增量。统计与测试结果独立局部刷新，不重建全部ProfileDto。取消是请求→确认停止的状态过程，保留已完成数据但给本轮未完成标记；新job与旧job事件不能串台。性能用release真窗合成100/10000/100000节点测量后再固定目标。本轮未量化延迟，状态 `identified`（全库poll结构）；代际串台状态待核对，未宣布已复现。

### UF-PROF-13 — P1：单击选择等双击超时，Ctrl/Shift在延迟回调里读取导致正常操作失真

证据层级：当前源码 + 本地锁定Flutter SDK手势合同 + 当前真实ProfilesTable/完整MainShell的受控widget复现（SyntheticBridgePort，runtime/platform/monitor显式fake）；实际Windows交互时延未计时；状态 `identified`（缺陷已经controlled_test复现，产品功能未verified）。

用户触发：按Ctrl/Shift单击节点，在300ms内释放修饰键；或快速单击B后立刻按Enter/顶部应用。选择可能未及时更新，修饰键已释放时再执行onTap会按普通单选处理，后续命令可消费旧primary。该问题可解释一部分“别扭/延迟”，不能推断其解释所有后端启动失败。

定位：`profiles_table.dart:536-544` 同一个GestureDetector注册onTap和onDoubleTap；`onTap:541-542` 执行时读取 `HardwareKeyboard.instance.isControlPressed/isShiftPressed`。`_onPointerDown:650-676` 只准备拖选/focus，没有立即点击选择或保存修饰键。锁定本地SDK `C:/Users/Colby/toolchains/flutter/packages/flutter/lib/src/gestures/constants.dart:35` 的 `kDoubleTapTimeout=300ms`，`multitap.dart:329-358` 在第一次tap后hold gestureArena并Timer，超时才释放单击。原版 WPF DataGrid 自身处理点击选择，不能把Flutter的双击识别等待迁移成选择等待。

现有测试漏验：`test/support/profiles_harness.dart:96-106` 明确等待400ms，Ctrl测试还一直持键直到400ms结束；`profiles_select_basic_test.dart` 使用该helper，所以选择绿测不能证明“按Ctrl点击后快速松键”“点击立即Enter”的真实使用顺畅。

修复范围：profiles_table输入层 + selection合同/针对性测试。选择在可确定的指针阶段即时反馈；在指针按下时捕获Ctrl/Shift/anchor/rowId，后续回调用快照。双击只决定激活或编辑，不能延迟第一下选择。拖选、reorder handle、右键外点、doubletap第二下、Ctrl toggle必须约定单次提交，不能在pointer-down与onTap各toggle一次。键盘快捷键不得在TextField/菜单拥有焦点时穿透。

本轮实际复现：

- `profiles-ctrl-timing-repro.dart` → `profiles-ctrl-timing-repro.log`：先通过正常UI点击选A；Ctrl点击B后pump50ms仍selected=A/primary=A；释放Ctrl再pump350ms变为仅B，期望A+B失败，exit1。保留了默认表格onTap/双击逻辑，没有直接调用selectRow替代待核对路径。
- `profiles-enter-timing-repro.dart` → `profiles-enter-timing-repro.log`：通过正常UI选A，并仅将合成后台active设A；点击B后pump50ms仍primary=A；发送Enter后active仍A；再pump350ms主行变B但active仍A，期望Enter目标B失败，exit1。
- 两项独立顺序运行，均正常测试断言失败，没有引擎崩溃/`did not complete`。50ms/400ms是WidgetTester虚拟时间，不能当作真实Windows的定量延迟。测试只证明选中/命令对象时序，不证明真实内核启动或真实FRB性能。

安全验收：down(Ctrl)→clickB→100ms内release(Ctrl)，A+B多选仍保留；Shift相同；点击B后≤一帧/固定短预算检查主行与高亮，再立即Enter/应用只作用B；正常双击只开一次编辑/激活；Ctrl取消已选B只有一次toggle；拖选向上/下跨视口；右键菜单关闭后的同一点击正常选择。widget测试不得统一pump400ms后才断言，release Windows场景需记录pointerDown、selectionUpdated、commandTarget时间。与UF03启动接线一起守门。

### UF-PROF-14 — P2：节点表文字尺寸绕过当前字体设置

证据层级：源码确认，DPI/字体真窗未验证；状态 `identified`。

原版：`UP/v2rayN/Views/ProfilesView.xaml:29,187` 使用 `StdFontSize` 动态资源。当前 `UiItem.CurrentFontSize` 可在设置修改，`ui_shell_controller.dart:228-239` 和 `shared/theme/app_theme.dart:378-389` 会更新主题基准字号/表格行高，但 `profiles_table.dart:530-534` 给每个数据Text写 `const TextStyle(fontSize: AppTokens.fontSize)`，固定12.5，绕过主题基准字号。因此设置可能增大行高而节点文字仍小，表现不一致；具体拥挤程度须实际DPI与系统文字缩放核对，不能仅从padding断言。

修复范围：表格语义文字样式、当前字体设置与自动列宽测量同一来源。保持原版列/分组/命令布局，使用主题当前字号/字体；列宽TextPainter、行高、菜单/tooltip、长文本截断与焦点框同步。不要以全局增大所有间距掩盖字号/约束不一致。

验收CurrentFontSize 0/12/16/20、Windows DPI100/125/150/200%、系统textscale、窄/宽窗口、自动列宽、长协议/备注、数字右对齐；截图需原版同设置同DPI对照，不能用默认字样截图证明设置生效。

## 3. 完整用户流程矩阵

下表“implemented”仅表示可见源码链已经存在；每一行仍须补当前基线真实窗口/FRB/SQLite/重开/效果证据，不能由函数存在推测可用。

| 用户流程 | 冻结原版约束/对象 | 当前路径与判断 | 后续验收 |
|---|---|---|---|
| 首次启动有节点 | 原版Refresh选择active/first并校正默认 | `build:322-339` 空选择；UF09 | 首次打开可见current/active区分，Start目标明确 |
| 手工新建普通节点 | 当前组；独立AddServer；取消不落库 | `startAddProfile:166-178`→editor→saveDraft，已存在；默认来源/组与字段适用仍待验 | 11适用协议逐个新建/保存/重开/生成；坏字段定位 |
| 手工新建Custom/Outbound | AddServer2、文件来源/核心/日志/前置端口，保留raw | `startAddCustomProfile:631-649`、专用editor已存在 | JSON/YAML/text原文、文件选择取消、覆盖、损坏/缺失文件、重开 |
| PolicyGroup/ProxyChain | AddGroup、子节点顺序/循环/嵌套/订阅子组 | `startAddGroupProfile:574-594`、group_editor已存在 | picker多选/筛选/取消、端口0合法、引用失效、生成配置实际链顺序 |
| 粘贴普通URI/Base64/Inner | 当前组、manual来源；合法坏行可有定位 | `subs_actions:19-53`→Rust→UI逐条保存；UF01/08 | 上游各格式合成互通；无组/组；部分成功；取消/重开 |
| 完整配置/Outbound/Clash文件导入 | 冻结结构检测条件；RawConfig来源与专用编辑器 | 前轮修复已有；不重复旧非法夹具结论 | 使用至少protocol/settings/tag合法Outbound，原文/核心一致 |
| 图片/屏幕二维码 | 导入而非分享；屏幕隐藏恢复原窗口 | FIX05/05B路径已存在；本轮未实测 | 成功/无QR/坏QR/取消/异常均恢复窗口；组快照 |
| 新增/编辑订阅快捷键/工具按钮 | 直接新建或编辑当前组 | 当前两按钮均列表入口，UF07 | 保存/取消返回仍当前组，普通空URL组 |
| 当前组筛选、文字筛选 | SubIndexId持久化；Enter触发文字搜索 | 本地group + visible prune已存在；UF06/09 | 重开/导入所属/隐藏primary/空结果 |
| Ctrl/Shift/拖选/自动滚动 | selected集合独立current row，批量与单对象不同 | `selectRow:905`、`selectRange:930`、navigate:960、拖选代码已存在；UF13迟读修饰键 | 真鼠标向上/向下跨视口，快速松Ctrl/Shift，点击立即命令，Ctrl取消行仍current |
| 设置默认/Enter/双击 | 同active幂等；双击false编辑 | `setActiveSelected:546`、`_onDoubleTap:477`存在；UF04 | 检查设置读回、no-op与显式Start分离 |
| 顶部应用/停止/重启服务 | 显式动作对象/运行状态；F5使用默认 | 顶部直接applyActive，UF03/04；runtime另域 | selected→active→plan→Running全链；失败/重试/重复点击 |
| 右键打开/开子菜单/关闭 | Grid current row+选择集合，键鼠焦点不泄漏 | 大部分已修；UF10，子菜单Esc行为待真机对照 | 行/表头/空白，边缘翻转/多DPI，外点/Esc/右键另行/失焦 |
| 编辑各类节点 | ConfigType路由专用窗口；取消保留原值 | `resolveEditorKind:191-201`→`editSelectedProfile:209-259`已存在 | Custom/Group/普通所有入口同编辑器与对象；active编辑生效 |
| 删除/去重/移除无效 | 确认对象固定、整组范围、KeepOlder设置、默认回退 | 读KeepOlder/可见过滤范围已有修复；UF01/02/11仍影响更新/删除 | 删除active/no candidate、source/target失败、取消重开 |
| 复制节点/修改备注 | clone与Ctrl+C导出区别；单对象current | copy:356-360、rename:437-483、Ctrl+C:371-381已存在 | ID新、来源正确、所有字段保留、备注取消不写 |
| 分享/导出/文件取消 | share QR为current；普通分享与Inner批量；Custom原文 | shareProfileQr:390、exportSelectedClientConfig已有；header多选UF10 | 原版解析互通，100%适用字段，文件取消无落盘，错误可见 |
| 测速/停止/结果列/统计 | selected与全列表测试，旧统计/排序重开保持 | result/monitor overlay与真实UDP已有；UF12/订阅UF02 | 真core隔离测试，本轮job结果，停止最终态，失败沉底，重开 |
| 表头排序/结果排序/手动移动 | 整组排序；失败两方向沉底；持久化ProfileEx.Sort | `_groupScope:673`、`sortByResult:889-897`、`fetchSummaries:354`读回已修 | 真实SQLite保存重开；筛选隐藏项；poll不回退；写失败反馈 |
| 日常重开/恢复 | 当前组、默认、列宽、排序、结果、统计适用恢复 | UI布局持久化已有；group缺失UF06/primary UF09 | 分离暂态selection与持久active；全流程重开矩阵 |

## 4. 建议交给执行模型的顺序与边界

每卡遵循方案§19固定任务格式，一卡一个主用户流程；不要将全部问题塞进一个“优化节点页”的大卡。

| 建议卡 | 唯一流程 | 前置 | 允许修改范围 | 完成门槛 |
|---|---|---|---|---|
| FLOW-PROF-A | A组手工M经历订阅更新不丢失 | 冻结IsSub/删除来源合同 | application/subs、store_repo/persistence、bridge import来源 | UF01全失败矩阵+真SQLite重开 |
| FLOW-PROF-B | S活动节点更新后仍可应用且统计保留 | A | application identity/active map + monitor适用转移 | UF02匹配矩阵+配置目标+重开 |
| FLOW-PROF-C | 选B→顶部启动→实际运行B | runtime请求/失败retry合同先固定 | profiles输入/actions/controller + root负责main_shell接线，runtime由专责 | UF03/04/13，真FRB+合成内核/受管会话；快速选中与No-op保持 |
| FLOW-PROF-D | A页更新当前组，无需进入设置选A | A/C相关错误合同 | subs actions/当前组application读写 | UF05/06请求目标+重开 |
| FLOW-PROF-E | 直接新增/编辑当前订阅 | D | sub编辑窗口入口与profiles toolbar | UF07保存/取消/重开 |
| FLOW-PROF-F | 一次批导入10000条且正确落库 | A/D | application批用例、异步bridge、UI import | UF08真实提交数/revision/响应时间 |
| FLOW-PROF-G | 右键多选主行从开菜单到确认不漂移 | C/D | command_context/actions/table + 批用例目标参数 | UF10/11真键鼠+异步刷新 |
| FLOW-PROF-H | 测速中正常筛选/选择/停止/再测 | A/B/F | 结果事件/job合同、profiles局部刷新 | UF12 job隔离+release延迟 |
| FLOW-PROF-I | 一套合法节点全入口编辑/导出/重开 | A..H | 相应专用编辑器/codec/codegen，按协议再拆卡 | 原版字段适用矩阵，现有修复不得回归 |
| FLOW-PROF-J | 字体设置后节点表字与空间一起生效 | 输入/数据守门稳定 | 表格语义样式/测量，与设置域协调 | UF14同DPI/同字号原版截图对照 |

卡片输入/输出/错误/取消/权限/持久化/生效语义必须明确到ID与revision；禁止凭“UI点了”“bridge返回ok”“cargo测试绿”替代“数据库保存→重开读回→实际生成/运行效果”。P0数据卡先加回归守门，再推进启动入口；其余视觉/性能卡不得抢改共享runtime/生成文件。

## 5. 安全验证与证据要求

- 所有节点合成：`.example` 域名、RFC5737地址、合成UUID/密码；不读用户数据目录。每次隔离新的 `V2RAYN_R_DATA_DIR`，记录夹具哈希与基线SHA。
- 真内核/HTTP测试先检测≥11808测试端口可用，只停止本任务持有句柄的进程；不碰10808，不写宿主代理/TUN/自启。普通节点表/事务测试不需要启动代理。
- 分开记录 source_confirmed、controlled_test、real_ui、real_core_effect。状态只用 identified/implemented/verified/preserved_only/blocked/not_applicable；平台未跑写未验证，不能把历史证据复用成当前实测。
- 交互场景至少保留当前基线、夹具、录屏/截图或UI事件、请求target/subids/job、实际DB快照（无秘密）、重开结果、运行session/profile/revision、命令exitcode。截屏/日志先去掉节点凭据。
- Flutter tester偶发did not complete不能直接推断产品失败；与其他构建/测试顺序运行，独立重试并登记。真窗用户交互必须另测。

本轮实际执行：`git rev-parse HEAD`、`git status --short`、`git diff`/hash核对；`Get-Content`/`rg`读取AGENTS、相关任务卡、四台账定位与冻结/当前源码。部分 `rg` 的Windows通配路径失败后已改为目录+`-g`/明确文件继续定位。经根代理补充授权，在apps/desktop顺序执行：

```powershell
flutter test test/_audit_profiles_ctrl_timing_repro_test.dart --reporter expanded
flutter test test/_audit_profiles_enter_timing_repro_test.dart --reporter expanded
```

实际工具为锁定Flutter绝对路径，两个命令各exit1、各1个正常失败断言，日志归档本目录。临时测试来自本目录同名repro.dart的原样副本（运行时相对support导入解析到apps/desktop/test/support）；运行后两文件哈希与证据源码一致才删除。未执行Rust测试、release构建、Windows集成测试或真实订阅/代理流量/宿主设置动作；没有本轮通过用例数量可报告。
