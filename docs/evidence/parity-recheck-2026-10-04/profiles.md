# 节点、导入、测速、右键与编辑流程第二轮复核

状态：`identified`。2026-10-04；冻结原版 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`，Windows 使用 WPF 合同，Avalonia 只用于适用的平台补充。

开始 HEAD 为 `efd6b2fdd8bfb22fcf191c8dc5a3ea1ce6e200ca`，当时 `profiles_controller.dart`、`profiles_table.dart`、`table_actions.dart`、`profiles_selection_test.dart` 有用户未提交修改。本代理全程只读它们，没有覆盖、格式化、暂存或提交。期间它们被其他工作提交为 `5471e4d`，发布包刷新提交为 **`a2b6905c595cb69a61e53c27a6fb34acaa210a81`**；本报告按收尾当前源码核对，旧 `1251cbc` 报告只用于寻找复核点。

没有读取个人配置、真实节点/订阅/日志，没有启动内核、监听端口、改宿主代理/自启/TUN或杀进程。本代理只运行合成 codec 和隔离临时 SQLite 测试；Flutter 选择测试由根代理运行，结果单独注明来源。本报告的安全复现是待实施的场景，不冒称已运行。

**收尾后窗口补证**：根代理随后用新增 [Windows 集成测试](windows-ui-01/README.md) 跑真实 Flutter 窗口→FRB→Rust→临时 SQLite，RE-PROF-01 的“选A新增 TUIC 保存成无组”和 RE-PROF-05 的“从B切A仍保留B隐藏选择”均实际观察为失败，结构化事实见 [observations.json](windows-ui-01/observations.json)。该测试没有重开、删除/测速或原版双窗口操作；下文这两项原先“本轮未运行”的表述指**本代理写报告时**的状态，已由此补证更新。

阅读依据包括 AGENTS.md、upstream-lock、相关 compat 功能/动作/实体/布局条目，FIX-01/02/03/04/04B/10/10B 任务与证据，以及 FIX-03B/03C、05/05B、16D 等新证据。没有用旧台账 `implemented` 标签推断用户流程完成，也不把下列问题数换算成完成百分比。

下文原版 `ServiceLib/...` 与 `v2rayN/Views/...` 路径相对于 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`；当前文件名分别位于明确写出的 `apps/desktop/lib/features/profiles/`、`features/subs/`、`bridge/` 或Rust crate路径。行号取本轮收尾源码。

## 当前最优先的真实流程缺口

下面编号 `RE-PROF-*` 属本轮复核，括号保留上轮关联。每项状态为 `identified`；除明确实际测试外，证据层级为当前源码和冻结源对照。

### RE-PROF-01 / P1：选组后手工新增、粘贴、扫码仍落入无分组

用户流程：在顶部选择合成分组 A，再添加普通/特殊节点或粘贴合成链接；用户期待对象属于眼前 A。

- 原版 `ServiceLib/ViewModels/MainWindowViewModel.cs:449-455` 创建 `ProfileItem{Subid=_config.SubIndexId}`；`:497` 剪贴板和 `:547` 扫码调用 `AddBatchServers(...,_config.SubIndexId,false)`。`ProfilesViewModel.cs:331-338` 换组更新 `_config.SubIndexId`。
- 当前 `profiles_controller.dart:350-358` 的 `newDraft` 没有写 `subid`；`setGroupSubId:505-510` 只改 UI 状态。`subs_actions.dart:27`、`:96` 两个 `importFromText` 调用没有传可用的 `subid` 参数；`scan_image_qr.dart:112` 复用这一入口。`import_persistence.dart` 只保存返回 DTO，不补当前组。
- 因此当前导入可以成功落库却从所选组视图消失；手工新增同样没有继承所选组。FIX-01 生成组使用正确当前组，不等于其它新增入口也承接了它。
- 安全复现：临时数据目录创建 A/B，无需订阅下载；选 A，导入 `vless://11111111-2222-3333-4444-555555555555@node.example.invalid:11980?encryption=none#synthetic`，查看保存 DTO 的 Subid、A视图与全部视图，再重开。只保存，不 apply。普通添加与两个 QR 入口各验证一次。
- 证据边界：根代理后续真实 Windows 窗口已复现新增 TUIC 的无组保存与当前组视图消失；粘贴/扫码、重开和异步期间换组仍未实测。需要统一命令的当前组快照，避免异步解析期间换组导致目标漂移。

### RE-PROF-02 / P1：设活动已修，编辑/删除活动节点的 Reload 仍缺

用户流程：正在使用节点 A，编辑 A 的地址、端口或安全字段保存；或删除 A。预期运行状态按原版刷新。

- 原版 `ProfilesViewModel.cs:464-507`：编辑 `ret==true` 后刷新；`item.IndexId==_config.IndexId` 时 `Reload()`。`:511-541` 删除集合包含活动 ID 后刷新并 Reload；`:569-603` 重复设同 ID 不清空，不同 ID 保存后刷新/Reload。
- 当前 `profile_actions.dart:181-222` 已改成幂等 `activateProfileById/setActiveSelected` 并调用 `runtimeController.applyActive`，这是已完成的修复，不再报旧“按第二次清空”活动缺陷。
- 但 `editSelectedProfile:65-107` 各分支只传 `controller.saveDraft`；`profiles_controller.dart:389-400` 保存成功只 reload。`deleteSelected:403-414` 也只删除/reload；`profile_actions.dart:110-135` 确认后未补 apply。没有原版成功提交后对活动节点的运行刷新。
- 原版刷新中的 `ConfigHandler.SetDefaultServer:427-449` 在默认ID已不存在时从当前列表或全库选择Port>0的回退节点。当前删除路径没有这条回退；需要连同删除活动节点一起验收，不能留下已不存在的活动ID。
- `activateProfileById` 调 `applyActive` 后直接返回 true，外层“已设为活动节点”不检查运行是否实际应用成功；应把持久化成功与应用失败分开反馈。
- 安全复现分两层：先用合成 Bridge/Runtime 测试确认活动节点保存/删除后产生一次共享 reload 用例、非活动不产生；运行效果由根代理在批准的隔离环境用探测空闲且≥11808端口验证，不能用用户代理。当前保存/删除实际效果未验证。

### RE-PROF-03 / P1：Ctrl+C 仍克隆，Ctrl+F 仍没有分享窗口

- 原版 `v2rayN/Views/ProfilesView.xaml.cs:221-246`：Ctrl+C→`Export2ShareUrlAsync(false)`；Ctrl+F→`ShareServerAsync`。
- 当前 `table_actions.dart:47-50` 仍 Ctrl+C→`ProfileAction.copy`、Ctrl+F→share；`profiles_table.dart:399-403` 对 copy 调 `copySelectedProfiles`，是数据库克隆。share 没有被此 switch 接住，落入 `profiles_controller.dart:855-906` 的 default log/echo，不打开 QR。
- 右键导出分享可以工作，与键盘是否同一行为是两个验收对象。FIX-01 的命令上下文修复没有修复这个入口差异。
- 安全复现：合成两节点，记录行数，选一项 Ctrl+C；断言行数不变、剪贴板是分享 URI。再 Ctrl+F 断言分享窗口显示对应节点。只用临时目录/合成剪贴板。
- 证据层级：源码确认，本轮未运行该窗口/键盘测试。

### RE-PROF-04 / P1：Sort 已写入，但表格刷新/重开仍按 IndexId

- 原版 `ProfilesViewModel.cs:400-437` 将 `ProfileExItem.Sort` 连接节点并 `OrderBy(Sort)`；移动/拖动/排序完成后重取这一顺序。
- 当前 FIX-10B 的写链真实存在：`profiles_controller.dart:638-657`、`:791-810`、`:1203`，经 `BridgePort.applyProfileOrder`→`speedtest_apply_profile_order`→ProfileExItem.Sort。Rust 底层落库/重开测试本轮通过，不能再说“排序只有内存、完全未写”。
- 读链仍缺：真实 `bridge_port.dart:342-360` `fetchSummaries` 取 `queryAllProfiles`，该查询固定 `ProfileSortDto.indexId`；`crates/bridge_api/src/api/speedtest.rs:315-323` 的 `from_ex`/`SpeedTestResultDto` 不输出 Sort。`_recompute:478-491` 仅应用内存 SortSpec，没有按持久化 Sort恢复。
- `reload:374-384` 会覆盖手动拖动后的 all；测速150ms轮询持续 reload。因此已写数据库的顺序仍不能保证眼前刷新、更不能保证重开。FIX-10B README本身也登记这个 DTO读回缺口，不能把该卡写端通过当整条流程完成。
- 附带差异：`profiles_models.dart:242-258` 是升序→降序→none三态；原版 `ProfilesViewModel.SortServer` 在两向间切换。`sortByResult:662-673` 固定升序，没有原版同列方向切换。`_persistOrder` 忽略桥接错误返回，写失败也无用户反馈。
- 安全复现：三个合成节点，初始 ID 顺序 A/B/C，拖成 C/A/B；刷新、开始合成结果轮询、关闭并重开。验证 UI顺序与SQLite Sort一致；不能只读 Rust Sort数组通过测试。

### RE-PROF-05 / P1：换组/过滤仍保留隐藏选择，批量入口可操作旧对象

- 原版 `ProfilesViewModel.RefreshServersBiz:361-379` 重建可见节点，选择生成项/默认项/首项；WPF `SelectionChanged` 同步 SelectedProfiles。
- 当前 `setGroupSubId:505-510`、`setFilter:629-631`、`_recompute:478-491` 都不与 visible求交 selected；`reload` 也未清理已删除/不可见的选择。菜单目标校验已改良，但键盘和工具栏读取 state.selected；`startSpeedTest:972-976` 非Mixed/Fast直接用它，`deleteSelected` 同样直接用它。
- 典型流程：A选节点→切B→按Delete/Ctrl+D/Ctrl+O，可能命中A的隐藏节点。修复“刷新漏分组”这一部分不能代表选择对象问题也已修。
- 安全复现：临时A/B各一节点；选A节点，切B或过滤隐藏A，记录选择、按快捷键后的目标。删除前确认窗必须显示可见正确对象；不运行真实测速/内核。
- 根代理后续真实 Windows 窗口已确认从B切A仍保留B的隐藏选中ID；未按删除/测速等有副作用按钮，动作目标后果仍需单独验证。普通多选测试本身不覆盖此项。

### RE-PROF-06 / P1：去重/移除无效的范围被文字过滤缩小，还清其他组失败记录

- 原版 `ConfigHandler.DedupServerList:1156-1193` 取 `ProfileItems(subId)`，不带文本过滤。`RemoveInvalidServerResult:1595-1611` 取 `ProfileModels(subid, "")`，按Delay=-1删除同组非复杂节点。
- 当前 `profiles_controller.dart:1093-1098` `_profileInCurrentGroup` 除组ID还套 `state.filter`；`removeInvalidResults:1060-1088` 和 `removeDuplicateProfiles:1110-1128` 都复用它。因此同组只是被文本过滤隐藏的失败/重复节点会遗漏。
- 另 `removeInvalidResults:1079` 无论删除是否成功都调用无分组参数的 `speedtest_remove_invalid`；后者 `speedtest.rs:541-548` 清整个 hub 的 Delay=-1记录并flush。B组失败节点本体保留，但失败证据被A组动作清掉；若delete失败，同组失败记录也丢失。
- 去重入口与真实DB删除已实现，底层测试本轮4/4通过；以上是UI所选范围与错误分支的新问题，不能再概括“去重禁用/只清结果、完全未做”。
- 安全复现：A两条同传输不同备注（一个被filter隐藏），B一条失败；临时SQLite注入合成ProfileEx，A筛选其中一条后去重/移除无效；核对A整组、B结果及delete故障。不发网络请求。

### RE-PROF-07 / P1：TUIC双字段已修，协议归一/适用控件仍缺

- FIX-02 已恢复 TUIC username/password、Reality即时重建、Fingerprint可编辑与未知候选、VLESS encryption文本、SS按核候选、UUID生成及Cert→SHA。这些源码改变和既有FIX证据不能当旧PR-03/04/26未修重报。
- 但 `Engine.save_profile:367-399` 对普通类型仍只 `Profile.validate`；`save_imported_profile:410-445` 没有逐协议归一。冻结 `ConfigHandler.AddTuicServer:847-881` 强制sing-box、Trim、清Network/Fingerprint、拥塞控制默认、空TLS→tls、空ALPN→h3和密码校验；Trojan/Hysteria2/AnyTLS/Naive等各自也有合同。当前FIX-02任务本身把后端归一列为PR-27待补。UI默认不能覆盖导入/订阅等入口。
- `profile_fields.dart` 没有MuxEnabled/Finalmask的FieldSpec；只有TUIC部分有UOT。`profile_draft.dart:23-24/42/111-126/187-205` 保留字段，但用户仍不能设原版VMess/VLESS/SS/Trojan适用Mux、Finalmask或SS/Naive UOT。冻结 `AddServerWindow.xaml:226/300/454/514/1386` 及对应绑定可核对。
- 安全复现：合成TUIC导入/桥接保存空TLS/ALPN/拥塞控制、另核与Network，再重开/生成sing-box配置，核对原版归一；编辑各适用协议检查控件、取消、保存/重开。不进行握手。
- 证据层级：源码确认；本轮codec TUIC自发自收通过不覆盖这些默认/编辑合同。

### RE-PROF-08 / P1：完整客户端配置两导出和UDP仍未移植

- `profiles_table.dart:949-951` 两个完整客户端配置导出继续 `notImplemented`，不是 share/base64/inner导出。原版 `ProfilesViewModel.cs:749-811` 构造CoreConfigContext并 GenerateClientConfig，文件入口有保存对话框，剪贴板入口写完整生成文本。
- `crates/bridge_api/src/api/speedtest.rs:70/360` 及菜单模型仍明确UDP unsupported/disabled。FIX-10文档也保留未完成，不能因ACT-PROF-018仍标implemented而销项。
- 安全复现：合成节点，两个导出分别核对完整配置/取消/指定路径与剪贴板；UDP按原版规定的协议目标先做合成会话测试，真正网络效果另外隔离验收。本轮未运行。

### RE-PROF-09 / P1：节点表统计仍恒0，后台统计实现不等于该表可见

- 原版 `ProfilesViewModel.cs:311-331/400-437` 把ServerStat按IndexId连接节点表，今日/总上传下载随真实统计更新。
- 当前 `bridge_port.dart:342-345` 只叠测速；`dtoToSummary:815-833` 把四流量值设置BigInt.zero，没有消费monitor `statsSnapshot.nodes`。FIX-11的后台统计、落库与状态栏修复不能让这四节点列自动有数据。
- 安全复现：合成统计节点ID A/B和计数，用真实桥/API受控夹具写统计并刷新节点表，确认映射/单位/清空/重开，不能使用SyntheticBridge自造流量显示来验生产数据链。
- 本轮源码确认，未启动真实内核或流量。本项交叉运行/监控领域。

### RE-PROF-10 / P1：策略组“刷新预览”查看存量，不查看当前草稿

- 原版 `AddGroupServerViewModel.cs:192-222` 的 `GetUpdatedProtocolExtra` 取当前ChildItems、模式、SubChildItems、Filter；`UpdatePreviewList` 将这个草稿交给GroupProfileManager解析，新建亦能预览。
- 当前 `group_editor_dialog.dart:96-108` 调 `preview(_draft.indexId)`，`:372-419` 明写“已落库解析，保存后确认”；`profiles_controller.groupChildPreview:526-535` 从bridge只取持久化节点。修改列表/订阅/Filter后点刷新仍看旧组合；保存成功即关窗口，不存在建议文字暗示的留在原窗口确认顺序。
- FIX-03已修专用编辑器分流、允许嵌套组、多选添加、五模式、排序和桥接存量解析。这里是仍未等价的草稿预览，不重报“完全没有组编辑器”。
- 安全复现：已有组含A，编辑改为B并换Filter，保存前点刷新，应展示B；取消后库仍A；新建组添加A后保存前也应能预览。纯合成数据，不启动核。

## 需要恢复的交互细节与台账查漏

### RE-PROF-11 / P2：选择节点窗口缩成复选框列表

当前 `group_editor_dialog.dart:527-637` 的 `showNodePicker` 仅全选、复选框列表、确定/取消，没有当前分组切换、地址/端口/传输/延迟/速度列、搜索、表头排序/自动列宽、单选模式的同一选择器合同。冻结 `ProfilesSelectWindow.xaml:49-199` 和 `.xaml.cs` 的相关绑定/键鼠，以及 `ProfilesSelectViewModel.cs:23-50/137-220/257-325` 有这些行为。

组中已加入子项仍是逐行按钮移除（当前 `group_editor_dialog.dart:312-355`）；冻结 `AddGroupServerViewModel.ChildRemoveAsync:125-140` 遍历SelectedChildren，可批量移除。新增多选“添加”不能替代已有子项多选/批量移除的合同。

**旧800台账遗漏建议**：`FilterConfigTypes/FilterExclude/SetConfigTypeFilter` 的 include/exclude类型筛选和回传范围没有独立动作/状态条目。它是原版VM的真实调用合同（AddGroup调用排除Custom；其它调用可设置类型），并非凭空要求新增WPF“类型筛选按钮”——冻结WPF上部只显示组、搜索、自动列宽。建议追加选择器“分组/文本/列排序/类型include-exclude/单选多选返回/取消”合同，把控件和调用方约束分开。

安全复现：两组合成节点、多种ConfigType、重复备注不同地址；从组窗口选子项，按当前组/搜索/类型约束找指定ID，再取消确认未改草稿。源码确认，未运行原版实际窗口。

### RE-PROF-12 / P2：草稿证书辅助动作还有明显边界问题

- `profile_editor_dialog.dart:406-434` 算出serverName，`profile_fields.dart:804-831` 的 `fetchPeerCertPem` 却未将该参数用于 SecureSocket 握手；连接IP但SNI为合成域名时实际证书可能不是原版期望。原版 `AddServerViewModel.cs:493-544` 将SNI/transportHost/address依次作为serverName传给CertPemManager。
- “获取证书链”依然只取叶子；当前消息显示“已获取证书链”，与FIX-02自己登记的leaf-only边界不一致。不要把同一叶子返回值报成完整链成功。
- `certSha256Thumbprint` 直接 `base64.decode`，没有错误保护；Cert输入的 `_syncCertSha:474-479` 同步调用它。粘贴带PEM头但非法base64文本可能产生FormatException；原版ParsePemChain/GetCertSha处理无有效证书时不更新SHA。此为源码风险，未运行故障注入，不宣称真窗口已崩溃。
- 安全复现只需本地合成合法/非法PEM单元/窗口输入，取消无落盘；SNI/全链另用受控本地TLS测试端口≥11808、预先探测，禁止真实节点和证书秘密。

### RE-PROF-13 / P2：活动节点与“眼前选中行”没有独立视觉标记

冻结 `ProfilesView.xaml:273` 按 `IsActive` 设置整行单元格背景；当前 `profiles_table.dart:505-536/547-575` 只根据 `state.selected.contains(row.id)`上色，没有按 `activeId`作活动标记。用户设A活动后点B查看，表格只剩B选中色，难以分辨当前实际活动节点。

建议新增“活动节点标记独立于选择/多选”的可视语义验收，不把它塞进“列能画出”的完成条件。安全场景用合成活动ID，不apply核；选A→设活动→再选B→A保留活动标记、B保留选择标记；深浅主题都检查。本轮源码确认，未新跑画面测试。

### RE-PROF-14 / P2：自动列宽、拖动开关和搜索时机未按原版消费

`profiles_page.dart:138-142` 自动列宽仍只 `emitAction('autofit-columns')`，`emitAction` 没有该case，最终log/echo。原版 `ProfilesView.xaml.cs:298-319` 改每列Auto。`_handleCell:547-589` 无条件Draggable，未读UiItem.EnableDragDropSort；冻结`:27-34` 只有开关true才注册拖拽重排。不能把新增拖选等价为尊重拖动排序开关。

当前 `setFilter:629-631` 每次输入立即筛选，且 `profiles_models.rowMatchesQuery:261-273` 搜索组名/协议/TLS/IP/端口等多字段。冻结 `ServerFilterChanged` 只在清空时刷新，Enter提交非空搜索；`AppManager.ProfileModels:202-235` 搜索备注/地址。若保持增强搜索，应有明确兼容决策与操作范围验收，否则“和原版保持一致”仍有行为差距。属于已有F-PROFILE-014细化，不新增同名假功能。

## 上轮29个问题的当前裁定

这是复核裁定，不重新统计800项完成度。`implemented`仅表示列出的修复代码/证据存在，完整功能未运行的边界仍保留。

| 上轮编号 | 本轮状态 | 当前结论 |
|---|---|---|
| PR-01 | identified | 设活动幂等/apply已修；编辑/删除活动生效仍见RE-02。 |
| PR-02 | implemented | Custom/Outbound/PolicyGroup/ProxyChain均分派专用编辑器；不再报普通窗口路由缺陷。 |
| PR-03 | implemented | TUIC UUID/password独立，FIX-02有真实保存/重开证据；本轮未重复GUI。 |
| PR-04 | implemented | dropdown onChanged有setState；不再报Reality不刷新旧缺陷。 |
| PR-05 | identified | UUID/CertSHA/取叶子已做；Mux/Finalmask/适用UOT、全链见RE-07/12。 |
| PR-06 | identified | 合法完整配置bridge导入物化文件、普通保存/生成已修；订阅完整配置文件物化仍是FIX-04B已登记缺口，需另域复核。 |
| PR-07 | identified | Custom/Outbound浏览拷贝和外部编辑已接；mac/linux opener返回false，适用核/原文/mixin效果归运行领域。 |
| PR-08 | verified | 本轮原版形状InnerFmt回归9/9通过；仅codec合成合同验证，未运行原版C#进程。 |
| PR-09 | implemented | saveImportedProfile与宽容导入已接；本轮空备注/地址应用层落库/重开测试通过。 |
| PR-10 | identified | 键盘入口仍不同，RE-03。 |
| PR-11 | identified | 真实删除已修；filter/跨组清结果错误仍见RE-06。 |
| PR-12 | identified | 去重入口/比较/真实删除已修；范围遗漏RE-06。 |
| PR-13 | identified | 两完整配置导出仍占位，RE-08。 |
| PR-14 | implemented | 图片与屏幕扫描已分派各自实现；本轮未做真实截图/文件选择或平台验证。 |
| PR-15 | identified | Sort写端已修、底层重开通过；UI读端与排序方向仍缺，RE-04。 |
| PR-16 | implemented | Mixed/Fast取visible、其它取selected、空集不扩全库；隐藏选择仍交叉RE-05。 |
| PR-17 | verified | ProfileEx SQLite读/flush路径存在；本轮底层结果落库/重开测试通过，GUI完整矩阵未重复。 |
| PR-18 | identified | reload组筛选已修；选择/当前组持久化与新增对象归属仍见RE-01/05。 |
| PR-19 | implemented | CommandContext闭包贯穿、targetSubId稳定ID取代label猜测；不再报旧session/null或同名移动旧缺陷。 |
| PR-20 | identified | 多选添加/嵌套/五模式/重排已补；选择器、批量移除、草稿预览仍缺RE-10/11。 |
| PR-21 | implemented | 生成按captured groupSubId，无需选节点，selectGenerated存在；FIX-01有合成真窗/重开证据。 |
| PR-22 | identified | 生产节点表四统计值仍零，RE-09。 |
| PR-23 | identified | 新TypeName/ColumnLayout状态源及迁移已做；FIX-16D登记窗口/桥接消费者未接，不能整体销项。 |
| PR-24 | identified | 本轮未重新核查分享QR尺寸/文本完整窗口合同，不援用旧画面当现状证明。 |
| PR-25 | identified | 本轮未重新跑Avalonia JSON编辑器对照；无跨平台验证，不能据Windows表单测试销项。 |
| PR-26 | implemented | 可编辑候选/未知初值/SS完整候选修复已存在；其它协议逐项输入约束另验。 |
| PR-27 | identified | 普通协议后端归一仍缺，RE-07。 |
| PR-28 | identified | UDP明确未完成，RE-08。 |
| PR-29 | identified | FIX-11B已补Clash模式/选择/关闭/延迟API；本轮未跑该API，自动列宽/错误反馈与平台完整合同由监控域复核。 |

## 本轮实际运行的测试与向上拖选边界

本代理实际命令：

1. `C:/Users/Colby/.cargo/bin/cargo.exe test -p subscriptions --locked --test parity_original_inner --test parse_pipeline --test fmt_roundtrip`：退出0；InnerFmt 9/9、pipeline13/13、格式23/23，**45通过、0失败、0ignored**。纯解析，没有网络或文件写入。
2. `C:/Users/Colby/.cargo/bin/cargo.exe test -p application --locked --test fix10_speedtest_result --test fix04_inner_import`：退出0；ProfileEx/Sort/真实删除/去重4/4、导入SQLite重开2/2，**6通过、0失败**。测试用tempdir、NullRuntimeClient；没有监听或系统副作用。

这些通过证明修复后的底层特定合同；不证明UI读取Sort、跨组选择、活动节点修改生效、所有协议及所有导入入口已验收。

根代理告知当前 `profiles_selection_test.dart` 的实际结果：普通多选单独通过，向下拖选单独通过；**`drag upward selects the reversed range` 单独跑两次均 `did not complete [E]`；整文件运行三个场景均未完成**。本代理未复跑Flutter，不将其写成通过，不据此断言已证明向上选择集合错误。

静态分析：`table_actions.dart:104-115` 的rangeSelection以min/max索引构成闭区间，正反向数学路径对称；`profiles_controller.dart:689-696` setEquals可避免同集合反复写入。没有看到足以解释原生退出的确定Dart异常。`profiles_table.dart:635-659` 根据本地Y、滚动偏移与行高命中，只改变选择；同时TableView的Scrollable/单元格GestureDetector参与同一主按钮手势，可能竞争滚动，必须实际确认。越出视口时currentId=null直接return，未实现拖选边界自动滚动；Ctrl/Shift拖选也未保留原选集合。它们是待验证的交互边界，**不是已定位的引擎崩溃根因**。

测试harness `test/support/profiles_harness.dart:18-21` 本身记有Flutter原生资源/segfault历史，但这也不能自动替当前两次失败归因。建议根代理保留exit code/原生堆栈、隔离只泵ProfilesTable的最小窗口，再验证上/下同路径；不要删除向上断言或简单重试标绿。

## 按用户流程排序的下一步

1. 先修当前组快照与选中对象：RE-01/05，所有新增/导入/键盘/菜单共享稳定对象；回归主组A/B、文本隐藏、异步期间换组。
2. 补成功保存/删除活动节点的共享生效用例与失败反馈：RE-02；继续保留已修幂等活动行为。
3. 修Ctrl+C/F真实dispatch、Sort读回和错误反馈：RE-03/04；再处理自动列宽与开关。
4. 校正整组去重/无效删除范围和结果清理事务，不能删其它组失败证据：RE-06。
5. 逐协议后端归一/缺控件、完整配置导出/UDP、节点统计：RE-07/08/09，每条独立小任务，不以禁用入口销项。
6. 修草稿预览、恢复选择器、活动视觉标记与证书辅助错误边界；新增台账条目先登记源合同，避免把接口存在当完成。

本轮只新增本报告，没有改生产代码、compat、原件、冻结源、生成桥接、dist或提交Git。尚未验证原版实机双窗口、当前全部窗口流程、macOS/Linux/ARM64、真实UDP/扫码权限、完整TLS证书链、真实节点流量及各协议×核完整矩阵。审查收尾不等于上述产品缺口完成。
