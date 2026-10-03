# 节点、协议字段、导入导出、测速与相关窗口原版差异审查

审查日期：2026-10-03。审查基线：当前应用 `1251cbc6821276e35d082f7739b8b9c15b6f93dd`；冻结上游 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。Windows 主合同使用冻结 WPF 源，Avalonia 只用于相应跨平台合同。

本报告覆盖 `profiles-assigned.json` 全部 **258 行**：35 功能、106 实体字段、59 动作、11 布局、15 窗口、15 配置类型、17 格式。逐项记录在 `profiles-items.json`；key 集合一致、无重复、无遗漏。共归纳 29 个关联问题。这是审查覆盖数，**不能换算迁移完成度**。

本代理没有操作原版 GUI，没有读取用户节点/订阅/个人日志，没有启动内核、监听端口、写宿主代理、自启或 TUN。实际执行的是纯 Rust 合成 codec 测试；当前 Flutter 真窗口场景由根代理串行运行，结果应以根汇总及实际观察文件为准。JSON 中 `implemented` 表示查到代码链路，`identified` 表示找到具体差异，`preserved_only` 表示旧字段保留/迁移路径，均不冒充完整用户验收。

本轮先重新检查当前实现。旧审查中菜单偏移、根条目顺序、4 根分隔、本地化列标题、顶部分组和大段文字工具栏等问题已有修改，不能作为当前缺陷复制。当前菜单有 `globalToLocal`、会话目标、外点/Esc 关闭，原版 32 px 项高也已设置；原版按下/松开、子菜单 Esc 退层、窗口失活、多 DPI 等逐事件对照仍未验证。

## 正常使用流程中的主要差异

### 1. 从分享链接或旧版内部链接导入节点

**PR-08 / P1：原版 `v2rayn://` 内部 URI 双向不兼容，已用真实 codec 确认失败。**

冻结 `ServiceLib/Handler/Fmt/InnerFmt.cs::ToUriSingle/ResolveSingle` 使用 `ProfileItem` 的 PascalCase 属性：`ConfigType`、`ConfigVersion`、`Remarks`、`Address` 等，扩展为 `ProtoExtraObj/TransportExtraObj`。`JsonUtils` 没有 snake_case 命名策略。当前 `subscriptions::inner` 却直接把 `domain::Profile` 序列化为 snake_case；导入只转换两个扩展容器，余属性不作转换，`Profile` 也没有原版 alias。

新增审查回归 `crates/subscriptions/tests/parity_original_inner.rs` 使用合成域名、UUID、端口 11980，无网络/文件效果。实际结果：

- 原版形状的 VLESS URI 导入，实际 `ConfigType=Vmess`，预期 `Vless`；2 个协议扩展中的 PascalCase 属性也缺对应转换。
- 当前导出解码 JSON 的 `ConfigType` 是 `Null`，预期 `5`。
- 两条测试失败，退出码 1，日志 `profiles-inner-regression.log`。这不是实际执行上游 C# API，但夹具形状来自冻结源，可证明当前 codec 不满足其公开格式。

当前也没有处理原版 `CustomOutboundObj`。原版会将对象写为自定义出站内容并置 `Address`；当前只接受额外的 `RawConfig` 或路径，无法承接上游这一 wire 合同。

修复应建立独立 wire DTO，与内部 domain 命名分开，覆盖普通节点、所有扩展、组引用改写、CustomOutboundObj 和未知字段。测试需包含“原版形状→当前解析”“当前导出→冻结消费者属性”，不能只用当前发当前收。

**PR-09 / P1：合法无备注分享链接能解析但不能保存。**

`vless::parse` 等在 URI 无 fragment 时 `remarks=""`。UI `persistImportedProfiles` 逐个调用 `save_profile`，`Profile.validate` 要求备注非空。冻结 `ConfigHandler.AddBatchServersCommon→AddServer→AddServerCommon` 的批量路径没有编辑窗口 `Remarks` 必填限制。因此，编辑草稿和导入对象使用了错误的共同校验合同。相关 codec 测试只检查解析，没覆盖这一步落库失败。

**PR-06 / P1：完整配置解析成功，但保存和实际生成断开。**

`fmt/batch.rs` 将完整 Xray/sing-box/Clash 配置生成 Custom 或 Outbound，原文放 `Profile.extra["RawConfig"]`，`Address` 为空。普通剪贴板导入随后进入 `Engine.save_profile→validate_custom`，后者强制 Address 非空，不能完成持久化。即使通过订阅替换绕过这项校验保存，`build_codegen_input` 只认 `proto_extra.extra["customConfigText"]`，未消费 RawConfig，也未读取 Address 文件，配置没有进入运行。

根代理已用满足冻结识别规则的完整合成 Xray 配置真窗口重跑：outbound 含 protocol、settings:{}、tag，达到三项门槛。`ui-run-02/observations.json` 实际节点数仍为0，反馈“已从剪贴板导入 0 个节点，1 个保存失败（E_FIELD_REQUIRED），1 行未识别”。这确认当前入口已进入保存阶段后失败；Address 原因由上述源码定位解释，错误消息本身未点名字段。RawConfig 后续生成/运行缺口仍仅源码确认，未启动内核。

需统一原版完整配置/出站的内容与文件模型，验收“解析→保存→重开→生成→指定内核”。解析器测试全绿并不能代表完整配置导入可用。

**PR-14 / P1：屏幕扫码、图片扫码入口实现成了别的动作。**

`main_shell.dart` 的 ACT-MAIN-017 调 `shareProfilesQr`，显示所选节点二维码；ACT-MAIN-018 调文本粘贴窗口。原版这两项是屏幕扫码和图片扫码导入。二维码显示、粘贴文本是不同流程，不能替代扫码能力或算完成其功能 ID。

### 2. 手工添加节点、修改已有节点、取消和再打开

**PR-03 / P1：手填 TUIC 的 UUID 和密码绑定错误。**

当前 TUIC 表单只有 `password` 字段，标签写 UUID；没有 `username` 控件。冻结 `AddServerWindow.xaml.cs:114..118` 分别绑定 Username→UUID 与 Password→认证密码。实际 sing-box 生成器 `outbound.rs:487..489` 也明确 `uuid=node.username`、`password=node.password`。现有 TUIC codec 自发自收测试正确保留了两字段，但不能验证手填 UI；当前用户无法按正常流程正确添加这两个认证值。

**PR-04 / P1：选择 Reality 后相关字段不会立即出现。**

`securityFields` 根据 `streamSecurity` 决定是否创建 PublicKey/ShortId/SpiderX/ML-DSA 字段。其 setter 只改 draft；通用 dropdown onChanged 没有 `setState`。因此这一事件没有重建父级字段树，用户需碰到其它刷新或点保存才改变字段，操作顺序不自然。保存/校验中的刷新不能代替正确联动。适用 TLS/Reality 能力函数虽然定义，表单没有据其限制内容。

**PR-26 / P1：可编辑/可为空字段被缩减为固定选项，已有值编辑有风险。**

- 冻结 VLESS encryption 是普通 TextBox，当前只有 `none/mlkem768x25519plus` 两个固定选项；真实加密参数不能输入。
- 冻结 Fingerprint ComboBox 可编辑，并含 `randomized` 与空值；当前列表缺这两项，也没有自定义输入或未知初值兜底。
- Shadowsocks 按内核支持的完整方法集合缩减为 7 项，例如原版 Xray 的 `plain/xchacha20-*`，sing-box 的 `aes-192-gcm/ctr/cfb` 等缺失。

`DropdownButtonFormField.initialValue` 直接使用存储值。打开一个正常导入、但值不在列表中的已有节点，可能在 Debug 触发选值断言或无法完成编辑。不能为了改善布局删协议可选值；仅改备注也应无损保留旧值。

**PR-05 / P1：字段已建模但没有用户入口，辅助动作未移植。**

普通编辑器没有 MuxEnabled、Finalmask 控件。Shadowsocks 和 Naive 原版 UOT 开关未加入各自表单，当前 UOT 反而出现在 TUIC 部分。没有生成 UUID、获取证书/证书链、证书文本变化自动算 CertSha 的行为。Draft/DTO/Rust/配置生成字段存在，不代表普通用户能设置它；需要逐一补适用协议入口与本地草稿的取消语义。

**PR-27 / P1：后端普通协议保存缺原版归一和校验。**

`Engine.save_profile` 对基础协议只调用 `Profile.validate` 的 ID/备注/地址/端口检查。没有逐协议等价于 `ConfigHandler.Add*Server` 的归一：默认 TLS、TUIC/AnyTLS/Naive 的强制 core、密码要求、HTTP headers/Realm 解析、WgMtu/Gecko/并发数等处理。因此即使 UI 校验通过，导入/订阅/桥接另一路也能写入不符合原版合同的值。普通编辑器新草稿的 streamSecurity 是空，不能依赖内核自动补齐必须的 TLS。

**PR-02 / P1：特殊节点的所有“编辑”入口走错窗口。**

`editSelectedProfile` 无条件 `showProfileEditor`；`editSelectedCustom/Group` 只有定义，实际节点右键、Ctrl+D、双击没有分流。冻结 `ProfilesViewModel.EditServerAsync` 明确 Custom/Outbound→AddServer2、PolicyGroup/ProxyChain→AddGroup、其它→AddServer。当前已存在两种专用“添加”窗口，但保存后从节点列表再编辑无法回到它们，还会进入普通编辑器的 core/port/network clamp。

### 3. 选择节点、右键、设为活动和节点管理

**PR-01 / P1：设为活动不是原版语义，保存当前节点不触发生效。**

当前 `toggleActiveSelected` 把重复选择活动节点写成 null；`setActive` 仅保存 ID，没有应用内核。原版重复设相同节点直接 return；切换新节点调用 Reload，编辑/删除当前节点也触发相应 Reload。此差异关系“我刚选了节点，为什么连接没改变”，不能只修文字。运行 revision/IPC 具体缺口交叉 `runtime-report.md`。

**PR-18 / P1：刷新失去分组约束，选择对象仍可隐藏。**

`ProfilesController.reload` 仅 text filter + sort，没有 `_recompute` 的 group 筛选；任何保存、克隆、测速 150 ms 轮询都会走 reload。`setGroupSubId` 不清理不可见 selected ID。冻结 `RefreshServers` 用当前 SubIndexId/filter 重新取对象并选择默认或首项。当前在分组 A 选节点、切 B 后，编辑/删除/测速可能仍命令隐藏的 A 节点。

分组只放内存 `groupSubId`，没有原版 SubIndexId 的持久化联动。编辑/新增订阅图标都只打开列表、当前分组更新取另一套 SubsState.selected 的跨域问题见 `settings-report.md`。

**PR-19 / P1：刚恢复的移动/生成菜单可能因关闭会话失效。**

当前 `_onContextAction` 接收捕获 session，但 `_moveToGroup/_genGroup` 不使用它，重新读取 `_menuSession`。MenuItemButton 关闭在 onPressed 前；`onClose::_onMenuClosed` 清 `_menuSession`；动作自身也先 `_menuController.close`。于是菜单项存在、目标最初合法，进入动作时会话却空，报“请先选择”而没有操作。

`_moveToGroup` 的下一层还用 `label.startsWith(sub.remarks)` 解析业务 ID。两组同名、A 与 AB、备注以“无分组”开头都可能移动到错误真实 subid。修复须传捕获目标与明确 subid，不从显示标签猜对象。

**PR-21 / P2：策略组生成对象取选中节点，而非当前订阅分组。**

冻结生成动作使用 SelectedSub；当前需选一个 primary 节点，从其 subid 取组。因此当前组无选中节点不能生成，若选择残留还可能针对另一组生成。生成后的 `_pendingSelectIndexId`/新组选择反馈也未移植。

**PR-10 / P1：快捷键与菜单定义不一致。**

Ctrl+C 仍映射 copy→克隆所选，原版是分享链接复制到剪贴板；当前上下文菜单自己却已显示导出 Ctrl+C。Ctrl+F 映射 share，但 `_onKey` 没有对应窗口 dispatch，`emitAction` default 只 log/echo，不能打开二维码。需统一所有入口的行为，不能只把菜单标签修正确。

**PR-12 / P1：移除重复仍禁用。**

当前右键去重 enabled=false，动作 notImplemented；已有 `subscriptions.deduplicate` 只是纯函数。原版当前组、KeepOlderDedupl、确认、DB 删除与默认/引用刷新没有真实用户入口。

**PR-15 / P1：移动、拖动、表头排序、按测试结果排序均不持久化顺序。**

- `moveSelected` 明写 UI-only，不更新后端 Sort。
- `handleDrop` 仅记 log，用户拖到目标行没有真正重排。
- `sortBy/sortByResult` 只改内存。SortSpec 三段循环含 none，原版表头两向切换；测试结果排序固定升序，而原版沿当前列方向切换。
- 当前没有接 `EnableDragDropSort` 适用开关；真实结果模型没有 Sort。

冻结 MoveServer/MoveServerTo/SortServers 调 `ProfileExManager.SetSort`。刷新或重开后应保持顺序；当前测速轮询即可覆盖 UI 顺序。`ACT-PROF-036` 的自动列宽按钮也只 emit 未处理动作、log/echo，没有实际测宽/Auto 列效果（独立 P2）。

### 4. 测速、失败节点处理和结果显示

**PR-16 / P1：实际测试集合与用户可见/选中集合不同。**

混合/快速传 ids=[]，Rust 将空集合解释为全库；其他测速无选择也默认全库。冻结 Mixed/Fast 仅测 ProfileItems 当前分组和过滤后的列表，其它需 SelectedProfiles。错误对象范围会更改隐藏组节点结果、启动大量不预期临时任务。HTTP(S) 传输的近期修复已在当前源存在，此报告不重复宣称“HTTPS仍未实现”；没有复用历史真实用户节点结果。

**PR-28 / P1：UDP测试明确尚未实现。**

`speedtest_supported.udp=false`，菜单禁用。该提示比伪实现诚实，但原版适用功能分母仍然保留，不能算已移植。

**PR-11 / P1：按测试结果移除无效实际上只清测试记录。**

`removeInvalidResults→speedtest_remove_invalid→ProfileExStore.remove_invalid` 只删进程内 delay=-1 的测量条目。冻结 `ConfigHandler.RemoveInvalidServerResult` 按当前组找到 Delay=-1 的 ProfileItem 并删除节点，复杂节点排除。当前动作名称与实际效果不符，重开节点仍在。

**PR-17 / P1：测速结果没有接 SQLite 生命周期。**

SpeedTestHub 的 results 初始化为纯内存 ProfileExStore，没有读取/flush SQLite。ProfileExItem schema/mapping 存在于迁移模块，不意味着当前新测结果被保存。当前启动 `dtoToSummary` delay=-1、speed-，只叠内存 hub。需保证测试、取消、重开、删除节点后的关联一致。

**PR-22 / P1：节点表统计列恒为零。**

真实 `fetchSummaries` 只合并测速 overlay；`dtoToSummary` 把今日/累计上传下载全部设为零。monitor 后端有统计、SQLite store 和 statsSnapshot.nodes，Flutter表格没有按 IndexId 接它们。状态栏统计能力存在不能证明节点表完整；业务字段“存了”与“用户看得到”要分开验收。

Clash代理刷新、组选中、节点/组延迟和关闭连接已接真实 monitor bridge；**PR-29 / P2** 是代理页缺原版 RuleMode 入口、连接页缺自动列宽按钮，以及 close/delay 失败没有将结果解释到可见反馈。实际 Clash API/内核效果本轮未启动验证，交叉 runtime 领域。

### 5. 策略组、自定义配置、导出和相关窗口

**PR-20 / P1：策略组编辑结构和组合能力被缩减。**

原版 AddGroup 的子节点多选表格、节点选择窗口、全选、批量移除、预出站列表 Tab/刷新，都被单个 dropdown 逐项加入、单行操作替代。ProfilesSelectWindow 本身没有移植。当前候选排除所有组，原版手工选择仅排 Custom，可嵌套组并通过 GroupProfileManager 环检测；订阅派生子项排复杂组是另一条规则，不能套到手工 ChildItems。

后端组引用/环检测、订阅子项/Filter、五种 MultipleLoad 及两核 codegen 确实已有实现；当前问题是入口和对象范围，以及缺预览，不能概括“组功能全部没做”。

**PR-07 / P1：Custom文件编辑与原文执行链缺口。**

原版浏览文件、编辑/导入文件路径动作缺入口。当前地址可输入但实际生成不读；YAML 即便允许保存，生成/计划会包装成 JSON 字符串，适用 mihomo 等核还缺 runtime adapter。实际核矩阵由 runtime 报告汇总，不能用两个核的生成器代表全部自定义配置。

**PR-13 / P1：客户端完整配置文件/剪贴板导出禁用。**

上下文两项均 disabled，没有 CoreConfigContext→GenerateClientConfig→文件对话框/剪贴板路径。现有 exportProfilesToFile 只生成 share URI 并写系统 temp txt，不等于原版用户选择路径导出完整配置；此 helper 实际可达性也没有界面证据。不能降级为“URI支持”后关闭该功能 ID。

**PR-23 / P2：节点列状态和设置迁移两套来源。**

当前存 executable 旁 ui_state.json 的 order/visible/width，未读原版 UIItem.MainColumnItem，统计/IP显示设置也没驱动同一列；写失败吞异常。应接统一后端设置迁移/保存合同，保留内部 ExName 与本地化显示文本，测试只读目录失败、重开、原配置迁入后恢复。

**PR-24 / P2：QR视图没有原版分享文本。**

原版为400x400图和3行只读 txtContent（聚焦全选可复制），当前只256x256图。生成 QR 能运行不等于用户能复制、检查、选择原始分享文本。

**PR-25 / P2：Avalonia JSON编辑器未等价移植。**

XHTTP Extra用普通 multiline TextFormField，无行号、格式化菜单和对应编辑行为。此项按原平台记录，不能把 Avalonia 独立组件直接当 Windows WPF 必有的独立窗口。

FullConfigTemplate 的两个 core tab、8 字段、草稿取消、真实保存与 codegen 投影均有实现。布局中的 Config/TUN两个文本框当前上下排列，原版是并排；存储由 FullConfigTemplateItem SQLite改为JSON，需要验证原配置迁入后的连续性。冻结原版本身也是先保存Xray再保存sing-box，不能臆造其双行保存必然原子性，再把当前相同顺序当差异。

## 根代理真实 Windows 窗口补充

根代理运行当前 HEAD 的 `apps/desktop/integration_test/parity_review_smoke_test.dart`，并在纠正完整配置夹具后重跑，第二轮 `ui-run-02` 用合法完整配置并完成了前五项；策略组场景未明确选中有效组，不能单凭那一轮判断与原版差异。第三轮 `ui-run-03` 在首场景后 Flutter Windows 引擎 0xc0000005 崩溃，部分记录不计为流程验收。第四轮 `ui-run-04/observations.json` 在真实持久化中将合成节点归入有效组，UI也选中该组后完整结束，`recordingComplete=true`；六项差异均复现。第一轮 `ui-run-01/observations.json` 仅保留过程和夹具纠正记录。本代理未重复运行 Flutter，也未把原版源码对照称为原版实机操作。

- TUIC 表单实际 `usernameFieldCount=0`、`passwordFieldCount=1`，唯一密码字段标签 UUID，确认 PR-03 的真实入口问题。尚未做两字段保存、重开和内核握手。
- 屏幕扫码实际打开分享二维码窗口，图片扫码实际打开文本导入窗口，确认 PR-14 的两个入口分流错误。
- 第四轮右键“一键生成策略组”在有效组前提下实际节点数 1→1，提示“请先选择节点以确定订阅分组”，确认 PR-19 的菜单目标丢失；原版此动作直接生成，不以是否打开组编辑器作为缺陷判断；不把菜单定位、根条目和关闭的已有修复推翻。
- 普通分组留空订阅 URL 实际不能保存，编辑器仍打开、显示 URL 必填；该跨域问题由 `settings-report.md` 主责。普通节点分组不应被强制变成带 URL 的订阅。

第一轮完整 Xray 配置导入场景显示 `nodeCount=0`，错误是“unsupported format: not a share URI（E_INVALID_ARGUMENT）”。但其合成 outbound 只有 protocol 和 tag 两项，冻结 `V2rayFmt.IsValidV2rayOutbound` 要求至少三项（protocol、settings、streamSettings、tag、mux），因此原版也会拒绝这份夹具。**第一轮此观察不能当作 PR-06 的对照验证，也不能将错误说成 Address 校验失败。** 第二轮加入 settings:{} 后通过结构识别，并实际显示1个保存失败（E_FIELD_REQUIRED），没有新增节点；PR-06 的保存阶段差异已获当前真窗口证据，后续生成/运行差异仍保留源码层级。

## 实际命令与验证边界

1. `cargo` 短命令在本代理 `login:false` 的 PowerShell 无PATH，退出1；随后使用锁定路径正常执行。
2. `C:/Users/Colby/.cargo/bin/cargo.exe test -p subscriptions --locked --test fmt_roundtrip --test parse_pipeline`：退出0，23+13个合成测试通过。没有绑定端口或读取个人资料。它们证实基础样本的 codec/识别可用，不覆盖UI保存、原版内部URI、所有变体或实际内核。
3. 最初直接执行 `C:/Users/Colby/.cargo/bin/cargo.exe test -p subscriptions --locked --test parity_original_inner`：退出1，2/2失败，原始日志保留。为了不让默认 workspace 门禁被已知待修差异持续打红，随后将这2个审查断言标记 `#[ignore]`；再次执行 `cargo test -p subscriptions --locked` 退出0，97通过、2 ignored。显式执行 `cargo test -p subscriptions --locked --test parity_original_inner -- --ignored` 退出101，仍2/2失败，见 `profiles-inner-regression-ignored.log`。修复后须转绿并移除 ignore，不能把默认门禁通过当作原版互通通过。
4. `C:/Users/Colby/.cargo/bin/rustfmt.exe --check --edition 2021 crates/subscriptions/tests/parity_original_inner.rs`：退出0。
5. JSON key集合检查：258/258，无重复和漏行；所有行有上游及当前源定位。部分旧台账文本原来有乱码，因此原版语义以实际冻结文件为准，没有将乱码当产品需求。

尚未验证：原版原生窗口实机及两版逐事件对照、当前窗口其余流程；macOS/Linux实际应用；每个节点字段所有协议×内核组合；扫码权限/图像处理；批量保存失败的事务效果；分组名冲突的真实菜单移动；测量新结果重开/Sort落库；真实UDP；Custom文件执行；所有格式的上游历史/边界链接；所有取消、错误和重开路径。上述真窗口补充只覆盖其明确记录的事件，不代表对应功能的完整保存/重开/内核验收。

## 按正常使用顺序的修复前置

1. 固定当前组/可见/选中目标和刷新链，恢复正确编辑器路由、幂等默认节点，以及键盘与菜单统一dispatch。
2. 修TUIC、Reality联动、原版输入类型/候选、Mux/Finalmask/UOT与辅助动作，建立后端逐协议归一和错误路径。
3. 解决内部URI互通、完整配置RawConfig/文件模型，贯通保存、重开和指定核生成；合法无备注导入单独验收。
4. 修右键会话传递、稳定subid与批量命令；补真实去重、Sort/拖动/自动列宽、删除无效节点及ProfileEx持久化。
5. 校正测试作用集合，节点表接真实统计；实现UDP和原版Clash剩余动作。
6. 恢复ProfilesSelect/策略组多选与预览、自定义文件动作、完整配置导出、扫码、QR文本与平台适用编辑组件。

每一步用固定小任务验收一条用户流程，入口、对象、错误、取消、保存、返回、重开与配置效果都需证据。不要用删除菜单、禁用字段或“存在后端函数”来削減需求。
