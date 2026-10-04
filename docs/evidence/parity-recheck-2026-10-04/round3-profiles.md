# 第三轮节点、导入、选择与测速复核

日期：2026-10-04。提交基线 `7edf1ee4e64590093e39444188229a69beb59908`；修复源码提交 `a4ceab5`，包含 waves A–G。冻结原版 v2rayN 7.25.4：`7d6a967c18c697f28dc6917122ed3a4993fcf336`。

对照源码根目录记为 `UP = work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。以下 `UP/ServiceLib/...` 与 `UP/v2rayN/...` 均是该冻结目录的真实源码，不是远端最新版。应用定位均为当前工作树的一基行号。

本轮只读生产代码，没有修改应用、台账、冻结源或方案，没有 commit。实际修改仅本报告。运行时外部并发变更包括 `profiles_table.dart` 增加 69 行拖选边缘自动滚动、旧 `profiles_selection_test.dart` 被拆分/删除，以及五个新的选择/拖选测试。审查时 `profiles_table.dart` SHA256 为 `4B6A4351730E8E7886E9ADA8C43BCF1032EDA10DDC63F40E5036336E2C0EA2DF`；该差异只改变拖选滚动段与 dispose/pointer-up 清理，不改变本报告指出的菜单业务分派。报告收尾时外部已提交为 `f328d0f91fe95ce5ff64d1b56d382c9e25c1c68d`，文件哈希未变；本文因此覆盖起始 HEAD 的已提交业务逻辑与该最终拖选快照。发布包仍来自 `a4ceab5`，不能用新工作树测试替代正式包证据。

## 证据边界

- `verified` 只用于本轮实际跑到的具体断言；其余接线已具备但用户整链未跑，记 `implemented`。仍有明确缺口记 `identified`，现有卡明确未提供完整证书链能力记 `blocked`。同一旧编号可同时有已修子流程和新发现，不能因新缺口否认已经完成的工作。
- 根代理本轮 Windows 原生 integration test 已运行：`round3-windows-ui-01/README.md`、`observations.json`。隔离 SQLite，普通空 URL 分组 + 合成 TUIC，两个断言均绿：新增节点归属当前组；节点从 B 切回 A 后隐藏选择清空。该证据是 Windows Debug 窗口→FRB→Rust 的真实自动集成测试，未包含冻结 WPF 同屏对照、重开、扫码/粘贴、活动内核效果或发布 exe。
- 本代理本轮运行 Rust 83 个测试全部通过，见末尾命令表；包含当前 Inner URI 互通、保存重开、逐协议归一、排序/移除结果、组验证与真实内核配置校验。配置校验不启动代理会话，不监听入站。
- 修复卡此前 Flutter widget/controller 测试作为已有证据引用，本代理没有本轮重跑 Flutter；没有把这些 synthetic bridge 结果写成真实内核或系统行为通过。
- 旧菜单坐标、关闭/再次打开、17 根菜单条目/4 分隔与顶部分组恢复已经完成；本轮不把它们重列为新问题。当前菜单仍有业务对象缺口，见 R3-PROF-02。
- 本轮未读取真实用户节点/订阅/证书秘密，未碰 10808，未改宿主代理、注册表、自启或 TUN，未杀外部进程。

## RE-PROF-01..14 当前结论

### RE-PROF-01：新增/粘贴/扫码分组归属

**状态：`verified`（Windows 手动新增 TUIC 归组断言）；`implemented`（粘贴/扫码继承）。原先缺组问题已修。**

冻结 `UP/ServiceLib/ViewModels/MainWindowViewModel.cs:449-455` 新建对象带当前 `SubIndexId`，`:497`/`:547` 导入与扫码传同一组。当前 `profiles_controller.dart:410-418` 的 `newDraft` 赋 `state.groupSubId`；`subs_actions.dart:19-40` 在读取剪贴板前快照，`:100-120` 共享文本导入在解析前快照；`import_persistence.dart:38-57` 防御补空后经 `saveImportedProfile` 落库。Native 实测 TUIC 的保存 `Subid` 等于 A，仍在 A 可见列表。

未在本轮真实窗口验证剪贴板/图片/屏幕扫描及异步换组。共享文本入口是在文本准备完之后取组，图片选择/扫描期间切组的命令时机需单独定合同，不由共享函数存在推断所有入口一致。当前合法完整 Xray JSON 的 parser 测试通过；本轮没有再次走真实 UI 导入，不能照搬前轮失败为当前结果。

### RE-PROF-02：活动节点编辑、删除、设置与应用

**状态：`implemented`（普通编辑/删除后的重载和回退）；`identified`（多选单节点动作）。**

冻结 `ProfilesViewModel.cs:494-500` 编辑活动节点成功后 Reload，`:515-526` 删除含活动后 Reload。当前 `profile_actions.dart:130-175` 保存前捕获是否活动，成功后 `applyAfterEditIfActive`；`:184-216` 确认删除后调用 `reconcileActiveAfterRemoval`；`:223-255` 按当前列表/存量 `Port>0` 回退；`:367-431` 区分持久化与应用失败，重复设活动不清空。这些已接真实 bridge 和共享 runtime apply，不能再说“只保存不应用”。

未在本轮启动真实内核验证这些效果；根此前 synthetic runtime 计数证据见 `recheck-fixes/RE-PROF-01-05-02/`。普通编辑和设置活动仍要求 `selected.length == 1`，多选无主行，见 R3-PROF-01。无候选时清 active 而未 stop 的行为需结合运行合同评估；冻结 Reload 在无默认对象时也提前返回（`MainWindowViewModel.cs:690-695`），本报告不把“最后一个节点删除后未 stop”单独称为新迁移差异。

### RE-PROF-03：Ctrl+C / Ctrl+F

**状态：`implemented`（快捷键接线已修）；`identified`（多选 Ctrl+F 主对象缺失）。**

冻结 `UP/v2rayN/Views/ProfilesView.xaml.cs:221-246` 对应分享链接和分享窗口。当前 `table_actions.dart` 映射、`profiles_table.dart:411-425` 实际分派，`profile_actions.dart:288-299` 导出真实分享 URI 写剪贴板；空集合静默返回，不再克隆。Ctrl+F 实际打开 QR 窗口，不再只是日志。已有两个 shortcut widget 证据可查 `recheck-fixes/RE-PROF-03/`，本轮 codec 互通 9 个断言通过。

`subs_actions.dart:295-303` 多选时直接拒绝分享，原版 `ShareServerAsync:593-607` 取 `SelectedProfile`。见 R3-PROF-01，不能以“分享单节点”推导“必须只有一个选中项”。

### RE-PROF-04：排序写入、读回和显示稳定性

**状态：`verified`（Rust 持久化 Sort/方向读链）；`implemented`（Dart 读回和失败提示）；`identified`（排序边界）。**

当前 `FrbBridgePort.fetchSummaries:354-375` 先按 speedtest result list（后端 Sort 顺序）重排，再叠加测速/统计；后端 `ProfileExStore::all` 与 `speedtest.rs:807-831` 的两向读回测试本轮通过；SQLite 排序重开测试通过。`SortSpec.next:254-261` 已两向切换；`profiles_controller.dart:729-746` 真实写失败显示 `orderMessage`。旧“按 IndexId 覆盖所有持久化排序”已修。

三个边界仍差异：①表头延迟/速度排序 `profiles_models.dart:350-374` 直接数值比较，未把失败/未测 `<=0` 沉底；冻结 `ConfigHandler.cs:1087-1105` 两方向均补沉底。②`sortByResult:753-769` 只替换 `visible`，保留旧 `state.sort`，下一次 `reload/_recompute:539-551` 会重按旧列排序；已有列排序后“按测试结果排序”不稳定。③冻结 `SortServers:1004-1006` 整组且不叠文字过滤；当前 `sortBy/_persistOrder:715-730` 仅当前筛选的可见行，隐藏行 Sort 不更新。安全复现见 R3-PROF-03。

### RE-PROF-05：可见集与选择/刷新行为

**状态：`verified`（切组清隐藏选择）；`implemented`（过滤/刷新交集）；`identified`（主行、默认选择与 Esc）。**

`_recompute:552-559` 已对所有常用刷新路径做 `selected ∩ visible`；真实 Windows 切组断言通过。旧“切组后仍批量作用另一组的隐藏节点”已修。

冻结 `RefreshServersBiz:366-375` 在非空列表默认选 pending → 活动 → 第一行；当前只求交，切入非空组/新增保存后经常无选择，须额外点击才能编辑/Enter。表格多选只有 Set，没有独立主行和 anchor，`extendSelection`/方向键从 Set/首个可见选中项推导，难与 WPF 当前行语义一致。`emitAction(Escape):965-972` 停测速后清选择，而冻结 `ProfilesView.xaml.cs:289-291` 只有 `ServerSpeedtestStop()`。菜单打开时 Esc 先关闭菜单的逻辑已正确接线，不与这里混同。

外部本轮已修拖选 auto-scroll 并在收尾时提交 f328d0f，不再以旧“没有边缘自动滚动”断言当前树未实现。根本轮独立原三个选择断言已绿；整文件 `did not complete` 属 tester 未完成，不能认定产品交互失败。五个新拆分/自动滚动测试由根继续复验，本文不虚构通过。

### RE-PROF-06：去重 / 移除无效

**状态：`verified`（Rust 范围和持久化删除断言）；`implemented`（整组/失败安全清理）；`identified`（去重设置、生效与错误提示）。**

`profiles_controller.dart:1188-1241` 已按整组非复杂 `delay==-1` 删除，只有所有目标真实删除才清孤儿；`:1252-1268` 去重不叠文字过滤、删除失败不假装数量。后端 `remove_invalid_orphans` 保留他组仍存在的失败节点。本轮 `fix10_speedtest_result` 四断言与 bridge 结果清理通过，旧过滤范围/误清他组证据问题已修。

剩余：UI调用 `removeDuplicateProfiles()`，默认 `keepOlder=true` 从未读取 `GuiItem.KeepOlderDedupl`；冻结 `ConfigHandler.cs:1166-1168` false 时 reverse。原版去重有删除即 Refresh + Reload（`ProfilesViewModel.cs:537-541`），当前函数只 reload 表格，若删掉活动重复节点，active ID 仍可指已删除对象（`engine.delete_profiles:540-559` 不处理 active），实际内核运行节点与表格持久化不一致。`_removeDuplicate` 将删除失败返回的 0 显示“没有重复节点”（`profiles_table.dart:1189-1190`），不能算错误已完整反馈。见 R3-PROF-04。

### RE-PROF-07：逐协议归一 / Mux / Finalmask / UOT

**状态：`verified`（Rust 当前归一/存储测试）；`implemented`（UI 控件与两路径）；`identified`（Reality 配置默认值持久化）。**

普通手动保存 `engine.rs:471-475` 调 `custom::normalize_server + validate_server`，导入 `:520-526` 调同一归一并保留宽容导入。TUIC 强制 sing-box、trim username/password、清网络/指纹、默认 TLS/ALPN/h3/拥塞等在 `custom.rs:206-226`，本轮 10 例通过；11 协议归一保存重开 5 例通过。Mux 四适用协议、SS/Naive UOT、按上游隐藏范围显示 Finalmask 已在 `profile_fields.dart:116-134`、`:226`/`:257`/`:283`/`:336`/`:499`/`:664`，不是还未做。

冻结 `AddServerCommon:1214-1216` 将空 Reality Fingerprint 持久化为 `CoreBasicItem.DefFingerprint`。当前归一未读设置；codegen（`application/codegen.rs:307`，`config_codegen/xray/outbound.rs:696-701`）有运行时 fallback，因此不能声称实际配置必然缺指纹，但保存/重开/导出值及以后修改默认设置的行为仍不同。修复卡已诚实登记该缺口。本轮没有真实 UI 操作新增 Mux/Finalmask/UOT 后启动内核，不能将形状测试升为全字段运行验收。

### RE-PROF-08：完整配置两导出 / UDP 测速

**状态：`implemented`（FRB 两导出接线、文件取消/失败路径）；`identified`（UDP 实际链路、Custom 格式与菜单目标）。**

`bridge_port.dart:519-526` 已真实调用生成 FRB `speedtest.exportClientConfig`；旧 `E_FRB_REGENERATION_REQUIRED` 已消失。`profile_actions.dart:43-96` 导出到剪贴板/真实保存选择器/`writeExportFile`，取消不写，有写失败提示。不能照抄修复卡初稿“FRB 尚未生成”为现状。本轮 bridge missing-profile 导出错误通过，真实 OS 保存窗口和文件字节尚未复验。

UDP现在可点击且支持标记 true，但生产探针 `bridge_api/speedtest.rs:199-211` 忽略 `_session`，直接调用宿主 UDP；冻结 `SpeedtestService.cs:371-401/538-542` 先加载节点测试核，`ServiceLib.UdpTest/UdpTestService.cs:103-123` 经 SOCKS5 UDP association。当前无法衡量节点 UDP 能力，成功延迟具有错误归因，P1。另 Custom 非 JSON 原始文本经 `Value::String` 再 `serde_json::to_string_pretty` 包成带引号/转义的 JSON，损坏 YAML/原始配置文件格式；原版文件导出 `CoreConfigHandler.cs:44-72` 是 File.Copy。原版 Custom 剪贴板 `fileName=null` 的失败分支也必须明确，不能按普通节点推断全类型支持。详见 R3-PROF-05/06。

### RE-PROF-09：节点表四流量列

**状态：`implemented`。原先真实 bridge 恒零的读链已修。**

`bridge_port.dart:370-375` 从生产 `monitor.statsSnapshot()` 在 enabled 时 join 节点，`:862-918` 转换字节与四项计数，`profiles_models.dart:169-224` 显示今日/累计上传下载。现有 `reprof09_node_stats_test` 覆盖 overlay/reload/disabled，证据在修复卡；本轮源码重新确认，不用旧恒零结论。

未在本轮受管内核流量→监控→节点表同屏证明非零；“今日”的时间范围与 source 调度由 runtime/monitor 代理复核。选择器仍未消费此类 overlay，见 RE-PROF-11，不能因为主表修了而推断 picker 也修了。

### RE-PROF-10：策略组草稿预览

**状态：`implemented`（新建/当前草稿预览、取消不落库）；`identified`（订阅子项有效性）。**

`group_editor_dialog.dart:165-180` 读取 `_childIds/_selectedSubId/_draft.filter/subid` 纯解析，三编辑入口不再传旧落库 ID；新建可预览。冻结 `AddGroupServerViewModel.cs:199-222` 的当前草稿合同已恢复。组循环/丢失子项、child order 保存重开本轮 10 例通过；此前预览 8 例见修复卡，未本轮真窗复跑。

冻结 `GroupProfileManager.cs:118-122` 订阅子项必须 `p.IsValid()`。当前 Dart `resolveGroupPreview:59-64` 和 Rust `groups.rs:27-28/260-266` 仅类型/备注，不要求有效。宽容导入的空 UUID、空密码、缺地址叶子会进入自动组预览与真实解析。域实体已有 `Profile::is_valid`（`domain/profile.rs:305-355`），不用通过新增前端猜测代替后端规则。另预览去重与原版简单 AddRange 的细微差异已在卡中注明，不能宣称完全字节/顺序一致。见 R3-PROF-07。

### RE-PROF-11：G 选择器与子项批量移除

**状态：`implemented`（主体流程）；`identified`（测速/顺序/名称数据与键鼠细节）。**

新 picker 的分组、Enter搜索/清空刷新、九列、表头两向、自动宽度、单/多选返回、取消、调用方类型 include/exclude 约束已在 `group_editor_dialog.dart:656-1105`；组入口 `:366-375` 确实调用并只排除 Custom；已有子项批量移除 `:386-403` 已实现。七个 synthetic widget 证据见 `recheck-fixes/RR-08-REPROF11/`，本轮只读确认。原版没有独立可见“类型过滤按钮”，本报告不凭想象要求此按钮。

剩余可直接源码确认：`:693-697` 延迟/速度恒 `-`，`:691` 订阅列直接 subid；候选来自 `state.profiles/queryAllProfiles` 的 IndexId 顺序，`:809` 空 sort 原样返回，而冻结 `ProfilesSelectViewModel.cs:182-208` join ProfileEx 后按 Sort，显示 SubRemarks/DelayVal/SpeedVal。`_groups` 默认全部而原版 `RefreshSubscriptions:177-179` 默认主窗口当前组；单选点击再次清空（`:838`），主选中无默认值，且没有冻结选择器 Ctrl+A / Enter 确定 / 双击确定键鼠合同（`UP/v2rayN/Views/ProfilesSelectWindow.xaml.cs:79/104-119`）。当前 `_searchHit:817-820` 额外搜端口，与原版 remarks/address 搜索不一致。`_picked` 换组/过滤保留隐藏选择，确认数量可能包括不可见节点；此项需要真窗口定期望，不以复用主表测试代替。见 R3-PROF-08。

### RE-PROF-12：证书 SNI / 全链 / SHA

**状态：`implemented`（叶子如实提示、非法 base64 保护）；`identified`（连接地址错误）；`blocked`（现实现未提供完整链）。**

`profile_fields.dart:824-829` 捕获 FormatException；`:897-925` 明示 leafOnly，编辑器 `:443` 不再虚称拿到完整链。原先崩溃/假全链问题已修。

当前 `profile_fields.dart:877-887` 连接选定 SNI 的 DNS 地址而非节点 Address。冻结 `CertPemManager.cs:40/48`、`:89/97` 连接 Address:Port，单独设置 TLS TargetHost。IP+虚拟域名场景会连接错服务或解析失败，P1。修复卡“Dart 无法 IP 另发 SNI”的理由不成立：本机 SDK `C:/Users/Colby/toolchains/flutter/bin/cache/dart-sdk/lib/io/secure_socket.dart:115-118/145-166` 明确 `SecureSocket.secure(socket, host: serverName)` 不做 DNS lookup。可先 Socket.connect(Address,Port) 后升级 TLS；尚未实测，不以方法存在声称 TLS 通过。完整链仍需额外 provider，叶子诚信提示不是全功能已完成。见 R3-PROF-09。

### RE-PROF-13：活动节点独立标记

**状态：`implemented`。旧缺标记问题已修。**

`profiles_table.dart:527-549` 活动数据单元格填色，`:606-616` handle 实心前导标记，独立于 selected；即使活动行同时选中，标记仍在。已有浅/深主题 widget 证据 `recheck-fixes/RE-PROF-13-14/`。冻结 `UP/v2rayN/Views/ProfilesView.xaml:271-279` 独立 IsActive 样式满足；尚未本轮真窗像素/缩放/密集长文本对照，不能从颜色函数测试断言最佳颜值。

### RE-PROF-14：自动宽度 / 拖动开关 / 搜索时机

**状态：`implemented`。旧占位/未消费开关/即时全字段搜索问题已修。**

`profiles_controller.dart:1283-1325` TextPainter 量宽并持久化，最多500行、40..600钳制；非旧 log-only。`profilesEnableDragDropSortProvider` 读设置，`profiles_table.dart:581-603/623-643` false 不构建拖动/目标。搜索 `profiles_controller.dart:693-712` 非空暂存、Enter提交、清空即刷新，匹配 remarks/address，已对齐冻结 `ProfilesViewModel.cs:343-350` 和 `ProfilesView.xaml.cs:316-322`。

已有测试证据见 `RE-PROF-13-14/`；本轮没有重新做真实窗口自动宽度/拖动排序/高 DPI 视觉。外部新增拖选自动滚动已在收尾时提交 f328d0f，仍需根单独稳定测试；不覆盖 EnableDragDropSort 语义，也尚未进入 a4ceab5 发布包。

## 优先修复的当前用户流程缺口

下列为当前源码确认，除明确注明外没有本轮真实 UI/网络复现。建议安全夹具只用 RFC 5737/文档域名、合成 UUID/空 URL 分组；用 SyntheticBridge/NullRuntime 拦截 apply，严禁真实用户配置。

| ID / 优先级 | 用户触发与实际差异 | 精确当前定位 / 安全复现 |
|---|---|---|
| R3-PROF-01 / P1 | Ctrl选 A+B，右键其中一行，编辑/分享/设活动/导出完整配置均提示必须单个选中，原版使用独立 SelectedProfile | `profile_actions.dart:130-139/43-62/407-413`、`subs_actions.dart:295-300`。菜单捕获 `primaryId`（`profiles_table.dart:809-811`）却未消费。Synthetic 两节点，多选后逐命令断言目标应等于主行；全局 primary/anchor 与 batch selection 应分离。 |
| R3-PROF-02 / P1 | 菜单在 A 打开后刷新/选择漂移成 B，完整配置导出读取 live B；同组过滤隐藏 A 后菜单仍可恢复 A 并执行 | `profiles_table.dart:943-953/1065-1108`；`restoreContextTargets:849-855` 只查 all。用合成状态在菜单开/执行之间改选择或过滤，记录 bridge 请求 ID；完整导出需同样使用 immutable command target，隐藏对象合同需明确定义。去重确认期间换组同样读取 live group，需快照。 |
| R3-PROF-03 / P1 | 延迟升序把失败放前；已按备注排序后“按测试结果排序”，下一次 refresh 回到备注；搜索后表头排序不改变整组顺序 | `profiles_models.dart:350-374`、`profiles_controller.dart:539-551/715-730/753-769`。合成 A/B/C，delay=-1/10/100；先 sort Remarks，再 sortByResult，再 reload，断言稳定顺序及失败沉底；另隐藏 B 后验证整组写序。 |
| R3-PROF-04 / P1 | 关“保留较旧项”仍删新项；去重删活动重复对象后不 Reload；DB删除失败提示“没有重复节点” | `profiles_controller.dart:1252-1269`、`profiles_table.dart:1189-1190`、`engine.rs:540-559`。Synthetic 两条相同节点+false设置，统计选中 ID 和 apply调用；failDeleteProfiles 必須给真实失败信息而非无重复。 |
| R3-PROF-05 / P1 | 节点 UDP 测速返回宿主直连目标的延迟，无法判定所选节点 UDP 通否 | `bridge_api/src/api/speedtest.rs:199-211/393-400`。用两种合成 session 的可控 SOCKS5 UDP server/录制 probe 证明目标走对应 session.port；纯 udp echo 回环通过仅证明宿主 UDP，不证明节点链路。 |
| R3-PROF-06 / P1 | 自定义 YAML/原始配置导出文件变带引号与转义的 JSON 文本 | `bridge_api/src/api/speedtest.rs:698-704`、`config_codegen/src/xray/mod.rs:119`（singbox 同规则）。隔离临时 Custom 原始文本经真实导出 API，断言字节不变；当前只有源码依据，未新增实测。文件选择器只提供 json 扩展也与原始格式不合。 |
| R3-PROF-07 / P1 | 宽容导入的无效叶子被自动订阅策略组包含，预览/真实解析与原版跳过无效不同 | `group_editor_dialog.dart:59-64`、`application/src/groups.rs:27-28/260-266`。合成同组有效/缺UUID/缺地址三个叶子，resolveGroupPreview 与 Rust resolve_sub_children 应同时只留有效项；不需要启动内核。 |
| R3-PROF-08 / P2 | 主表已有排序与延迟/速度，选节点窗口全部 '-'、顺序不同/订阅列为 ID，当前组与主行键盘合同未延续 | `group_editor_dialog.dart:685-697/761-814/833-860/1062-1105`；生产查询 `bridge_port.dart:380-391`。合成固定 Sort/测速值并打开真实调用方 picker，断言 Overlay/组名/默认组/键盘确认；现有 picker 单元只证明主体结构。 |
| R3-PROF-09 / P1 | 节点 Address=IP，SNI为另一域名，“获取证书”访问域名解析地址而非节点IP | `profile_fields.dart:877-887`。应在≥11808空闲端口起受管本地TLS证书夹具，Address=127.0.0.1，SNI=文档域名，验证不依赖域名DNS。未运行TLS夹具；SDK已证明存在可实现 API，不能继续以平台不支持阻断。 |
| R3-PROF-10 / P2 | 切到非空组/新增后没有默认选中，Esc意外清多选；Shift/方向键目标按集合首项而非主行 | `profiles_controller.dart:539-559/777-827/965-972`。合成非空组切换应选 pending→active→first；Esc测速停止但保留选中。外部drag scroll修复与这项不同，不覆盖结论。 |
| R3-PROF-11 / P2 | 空 Reality Fingerprint 不保存当时配置默认值，改默认后旧节点随之改变；原版在保存时固化 | `custom.rs::normalize_server`、`engine.rs:471-475/520-526`，生成fallback `application/codegen.rs:307`。临时SQLite设默认A→保存空指纹Reality→改默认B→重开/分享值，对照原版保存值。代码生成有fallback，本报告不声称实际配置必缺指纹。 |

## 本轮实际命令与结果

工作目录为仓库根；cargo 使用 `C:/Users/Colby/.cargo/bin/cargo.exe`。测试内容为合成输入、临时SQLite/内存库；没有个人数据。

| 命令 | 实际结果与边界 |
|---|---|
| `cargo test -p application --locked --test re_prof_07_normalize --test profiles_persist --test fix10_speedtest_result --test fix04_inner_import --test t10_groups_templates --test t11_codegen_matrix` | exit 0，2+4+5+10+10+1 = **32 个测试通过**。t11 是一个测试内多案例真实 Xray `run -test` / sing-box `check`，只校验生成配置；已读源码确认端口先探测且≥11808，未开代理监听。产物仅 `target/t11/matrix/`。不能当用户实际代理/UDP/TUN效果。 |
| `cargo test -p subscriptions --locked --test parity_original_inner --test fmt_roundtrip --test parse_pipeline` | exit 0，9+23+13 = **45 个测试通过**。冻结 PascalCase / ProtoExtraObj Inner 互通、legacy兼容、标准各协议roundtrip、合法完整Xray JSON parser、部分成功/取消/错误均有现有断言。纯codec未启动网络。 |
| `cargo test -p bridge_api --lib api::speedtest::tests --locked` | exit 0，**6 个测试通过**。Sort两向读回、结果清理、missing-profile导出、配置钳制/动作映射。`udp_is_supported_by_the_direct_probe` 只是支持位断言，与 R3-PROF-05 不冲突，不能由它证明节点UDP。cfg(test)默认engine为内存库。 |
| 根代理 `flutter test integration_test/parity_recheck_group_selection_test.dart -d windows -r expanded` | exit 0，1个集成测试、2个合同观察通过；参见 `round3-windows-ui-01/`。本代理未重复启动Flutter构建。 |

未运行：本轮本代理Flutter analyze/widget/整仓门禁、冻结原版WPF双窗口逐事件对照、真实TLS/UDP经节点链路、真实活动编辑删除内核效果、完整客户端配置OS导出文件字节、QR全部入口、重开当前组/选择、正式发布exe视觉与高DPI。此前修复卡的测试不冒充本轮再次运行。

下一步前置：先补主行/命令目标与排序/去重的纯状态回归，再用隔离Windows窗口验证同一UI入口；UDP和证书需合成受管loopback场景，不能复用历史真实节点证据。上述仍有 identified/blocked 子项，因此不宣称节点域全部完成。
