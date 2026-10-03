# 设置、订阅、路由、DNS、热键与备份原版差异审查

状态：`identified`。审查基线：当前 HEAD `1251cbc6821276e35d082f7739b8b9c15b6f93dd`；冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。Windows 对照 WPF；Avalonia 的窗口行另外记录，不能把 Flutter 单份页面认作其他平台已验收。

本领域分配 373 行：180 设置、52 实体字段、77 动作、33 功能、11 布局、20 窗口。逐项结果在 `settings-items.json`。本轮未改业务代码、台账、冻结源码、方案原件或发布包，也没有读取个人节点、订阅或旧真实运行证据。以下结果分为源代码确认的差异、后端合成测试通过、仍未验证的真实窗口/平台效果；不能由本报告给整体完成百分比。

## 最先修复的实际用户流程

| Finding | 优先级 | 正常操作与当前结果 | 原版依据与当前定位 |
|---|---|---|---|
| SET-01 | P1 | 新增仅有备注的普通分组，当前要求 URL 非空，保存不了。 | 原版 `SubEditViewModel.cs:63-76` 只在 URL 非空时校验；当前 `sub_edit_window.dart:132-143` 和 `application/subs.rs:164-175` 强制必填。 |
| SET-02 | P1 | 主窗口选 B 组后“更新当前组”，动作读取订阅设置窗口里的单独选择。也可能更新 A、或提示必须先到设置窗口选择。 | 原版动作取 `_config.SubIndexId`；当前 `subs_actions.dart:333-355` 取 `SubsState.selected`，`profiles_controller.setGroupSubId` 不同步。 |
| SET-03 | P1 | 在首次下载期间取消，UI 还没有当前任务 ID；再次更新时可能拿的是上次已结束任务 ID。 | `SubsController.update` 等 `await updateSubscriptions` 完成后才写 `lastJobId`；`cancel()` 读此值。Rust `api/subs.rs:309-333` 先完成下载/finish，再返回 job ID。当前按钮不能证明真实取消。 |
| SET-04 | P1 | 填好自动更新间隔后正常启动，后台定时器没有启动调用；即使手工接上，定时请求强制 via_proxy，未运行内核时返回 proxy unavailable。 | 原版 `TaskManager.cs::UpdateSubscriptionTask` 与全局任务；当前 `SubsController.startScheduler` 只有定义，应用 bootstrap 无调用，`application/subs.rs:590-608` 的 tick 强制代理。 |
| SET-05 | P1 | 订阅转换目标、前置/后置节点看起来可填写，实际仅保存或影响其他分支，未完成原版效果。 | 原版 `SubscriptionHandler.DownloadMainSubscription` 构造转换 URL；当前直接下载 item.url，只用 ConvertTarget 决定是否跳过 MoreUrl。PrevProfile/NextProfile 没有生成订阅链，且原版排除 Custom 的节点选择器缺失。 |
| SET-06 | P1 | 参数“保存→再改端口→应用”应用的是上次保存内容，最新输入被弹窗关闭丢弃；“开机自启→取消”已经写宿主自启。 | `option_setting_window.dart:209-212` 不先保存 `_draft`；:493-508 切换直接 `setAutostart`。原版 AutoRun 在 `OptionSettingViewModel.SaveSettingAsync:351/402` 保存后更新任务。未写宿主自启实测。 |
| SET-07 | P1 | 路由新建方案连续加两条规则，保存前两条 ID 都是空串；选/编辑一条可能命中全部新规则。 | 原版 `RoutingRuleDetailsViewModel.cs:56-60` 进入新规则即生成 GUID。当前 `newRuleDraft()` id:''，详情 `_save` 返回 widget.rule.id，父 `_editRule` 用 ID 等值替换。 |
| SET-08 | P1 | 现有路由编辑后“移动”立即写数据库并从数据库重新加载，丢弃本地未保存修改；“导入→取消”也留下导入。 | 原版移动/导入改私有 `_rules`，最终 SaveRouting 统一写。当前 `_move`、文件/剪贴板/URL 导入调用 Rust 持久化并 `_loadRules`。 |
| SET-09 | P1 | 删除方案全部规则后保存，旧规则仍在；后半段规则保存失败仍关闭弹窗。 | `routing_windows.dart:718-753` 分别保存方案和规则，只有 `_rules.isNotEmpty` 时 saveRules，忽略后者返回。原版序列化完整 `_rules`，包括空列表。 |
| SET-10 | P1 | 路由设置顶部域名策略改变的是选中方案，未改变原版全局 RoutingBasicItem；新方案/别的方案及 sing-box 分支会不同。 | 原版 `RoutingSettingViewModel.SaveSettingsAsync` 写全局字段；当前 `_strategyValue/_saveStrategy` 读写 selectedRouting。codegen 会由非空方案覆盖全局，但 sing-box 的 IPIfNonMatch/IPOnDemand 判断仍读全局。 |
| SET-11 | P1 | DNS 页“导入默认→取消”已经持久化模板并覆盖整个弹窗草稿；“改输入→应用”没有保存输入；只改普通 DNS 时隐式擦掉 GlobalFakeIp。 | 原版导入命令仅改窗口属性；当前 `_importDefault`→engine.import_default_dns 保存后 `_fillFromState`。`dns-apply` 直接 pop+applyActive；`_save` 新构造 SimpleDnsDto 不传 globalFakeIp，转换时 extra 也清空。 |
| SET-12 | P1 | 原版 ZIP 导入显示成功与行数，但 180 设置/默认节点/当前分组未进入活动配置。 | `persistence/candidate.rs:300-303` 仅写 meta.upstream_config；BackupService.import_upstream 仅替换 DB；AppEngine 初始化仅读 data_dir/guiNConfig.json，无读 meta 应用流程。 |
| SET-13 | P1 | 运行中恢复配置没有关闭业务 DB 句柄、重建 AppEngine/settings/active 或刷新 UI；后续保存可能继续写旧内存配置。 | 原版 LocalRestore 先 AppExit/DisposeDbConnection、解压再重启；当前 t16_restore/import/webdav_restore 文件交换后只返回 success，BackupController 只更新提示。Windows 文件交换是否直接失败、后续覆盖路径未实机运行。 |
| SET-14 | P1 | 备份资源被验证却没有恢复；顶层资源收集不遍历配置子目录。 | `BackupService.collect_resources` 只顶层普通文件；restore 只 DB+config。原版整体 guiConfigs 目录 ZIP。现有测试仅检查 DB/config，未断言资源回拷。 |
| SET-15 | P1 | 保存/导入热键、重开或触发，录制编码、注册编码和原版编码不一致；注册也没有动作回调，保存后没有重载注册。 | 原版 WPF 存 System.Windows.Input.Key 枚举，HotkeyManager 使用 KeyInterop 转 VK；当前录制 Flutter keyId，registrar 按 Windows VK 解释。`manager.register` 未传 keyDownHandler；窗口只 saveGroup，未 HotkeyController.save/re-register。 |
| SET-16 | P2 | 原版设置分组和候选控件丢失，且大量 UI 文案直接展示内部字段/任务编号。 | 原版有效 5 页（KCP 页在注释内）；当前 12 页，多个原版核心页控件散到新页。缺缓存开关、保留旧节点、HWA、证书来源、转换/Geo/SRS/路由源 URL、自定义脚本浏览等入口。 |
| SET-17 | P1 | 改语言/字号/布局/监控刷新参数，保存成功不代表对应界面或后台真正改变。 | MaterialApp 没有 locale/delegates、页面硬编码中文；表格和弹窗大量固定字号。监控刷新/排序参数、HWA/RootCertProvider/EnableLog/全局自动更新缺真实消费链。详见 runtime/profiles 报告。 |
| SET-18 | P2 | 消息页输入原版正则变成普通不区分大小写 contains；过滤/自动刷新不回写原版 MsgUIItem，文本复制全部/选择操作缺入口。 | 原版 MsgViewModel 的 IsRegexMatch/MainMsgFilter/AutoRefresh；当前 MonitorState.visibleLogs / setKeyword / setScrollPaused 内存字段与 Text 行列表。清空入口原版 handler 已注释，当前禁用这一点不作为“漏实现”。 |
| SET-19 | P1 | 区域预设主菜单禁用；DNS 页额外入口返回“已应用（离线）”，远程模板永远 pending，原版 URL 下载/Geo 更新未完成。 | main_menu ACT-MAIN-032..034 preservedOnly；engine.apply_regional_preset 与 pending_remote_templates 无后续下载链。不能以 offline 测试通过认作原版区域预设。 |
| SET-20 | P2 | 检查更新窗口预览版/代理/勾选项仅当前 Riverpod 内存，重开不从 CheckUpdateItem 恢复；后台每日检查无任务启动。帮助的核心网站、推广、管理员重启/UWP 入口仍禁用/占位。 | update_controller build 默认 false/全选，不 load/save CheckUpdateItem；main_menu preservedOnly、main_shell notImplemented。运行领域另外复核下载/安装本体。 |

以上优先级指迁移使用差异。SET-01 的普通空 URL 分组已由根代理在真实 Windows Flutter 窗口复现：`ui-run-02/observations.json` 的 `create-group-without-subscription-url` 返回 `subscriptionCount=0`、`editorStillOpen=true`、`urlRequiredError=true`，完整录制标记 `recordingComplete=true`。其他尚未实际运行的窗口/平台行为继续写未验证，不能从这个单一场景推广为全部设置验收。

## 订阅字段与正常操作边界

SubItem 的 17 字段 DTO/SQLite 映射真实存在，合成 CRUD 重开测试已通过。URL 和备注可操作，UA 与 Header 已有本地 HTTP 发送测试；MoreUrl 主/附加 base64 合并顺序测试通过；Filter 对备注做正则过滤。不能因为这条储存链存在就认定全部订阅体验一致。

- 订阅设置的顶部工具栏迁到弹窗底部，原版 DataGrid 多选变单个 selectedId，删除没有原版确认；Sort 只显示而编辑器不可编辑，没有上下拖排序入口。当前右键位置固定在 overlay 的三分之一，与点击点无关；菜单不保留原版条目结构。
- 主窗口“新增订阅”和“编辑当前订阅”两个图标都打开订阅列表，未直接创建/编辑目标对象。删除主窗口当前组入口未迁完整。保存订阅只 reload SubsController，节点页从 ProfilesController.subItems() 读取桥接结果，没有订阅 state 的监听依赖，主分组新建/改名后刷新仍需真实场景验收。
- Header 当前校验只检查 JSON 字符串/控制字符；原版以大小写不敏感字典拒绝重复 Header，并用 HttpRequestMessage 的 Header 解析验证名称。`{"X-Test":"a","x-test":"b"}` 当前不拒绝、原版拒绝，是可重复的纯校验差异。不得用真实 auth Header 做测试。
- 当前 customCoreType 文本填写数字，原版枚举 ComboBox；PrevProfile/NextProfile 没有排除 Custom 的节点选择器。请求 Header/filter/转换等的空串统一转 null，没有整体验证原版 null/空串合同。
- 转换目标真实原版有后端行为，台账名称后附“保留-only”不是用户授权的删减；本审查把未迁链路继续列为缺口。经代理无本地端点立即报错是项目已有防止意外直连的有意策略差异，修复应明确策略并保留安全约束，不能偷偷取消保护。
- 取消能力是新增任务合同，同时影响正常交互。直接 token 的 Rust 取消测试通过，UI 必须在任务运行前拿到同一个 ID 并正确取消；当前回包时才得到 ID 不满足此条件。

## 路由与规则编辑的提交合同

当前后端具备真实方案/规则存储、顺序、默认路由、导入导出、dangling warning、三模式与重开；本轮 t11 的 11 测试通过。前端问题发生在这些有效后端 API 的调用对象和调用时机上。

必须让一个方案编辑会话始终持有同一份草稿。新建规则在加入草稿时获得唯一 ID；新增位置原版为第一条，当前为追加尾部，应对齐。新增/编辑/移位/导入/删除均操作草稿。导出要导出用户正在看见的草稿 JSON，空选中应提示而非隐式导出数据库全部。

现有方案当前导出读取 DB 的老版本；新建方案导出只有 outboundTag 换行（`_exportSelected:616-625`），与原版 camelCase RulesItem JSON（去 Id）不一致。当前导入新建方案按钮可点但 item.id 空时直接 return；原版新建方案能先导入再保存。

详情选择节点应保留原版 ProfilesSelectWindow 的排除 Custom、列表过滤、确认/取消和 Remarks 写回。当前 SimpleDialog 展开所有节点备注、混入 Custom，重复备注没有识别信息。只选 inboundTag 当前当作有效规则，原版要求 Network/Port/Protocol/Domain/IP/Process 之一。Type 字段 DTO 虽保留，详情返回新 DTO 不保留 widget.rule.ruleKind，编辑已存在带 Type 的规则会丢 Type，须用纯合成 fixture 验收。

列表当前用点击切换集合取代原版 DataGrid Ctrl/Shift 选择，方案只单选，没有 Ctrl+A/移动快捷键/右键同结构；详情三个 Domain/IP/Process 可调整栏改为垂直文本字段。布局差异与业务差异分开，不能只抬高行高来规避。

## DNS 的保存、应用与取消

四页结构存在，基于公开冻结模板的 Xray/sing-box 校验/存储、hosts 合并测试通过，实际解析/TLS/路由效果仍属于内核验收，未在本领域真机验证。

- 导入默认应先预览草稿，取消不留持久化结果，也不覆盖其他页未保存编辑。当前导入立即 engine.save_dns 后刷新所有控件。
- 当前 SimpleDNS 先保存，然后逐个保存 Xray/sing-box DNS。第二/三阶段校验失败时前阶段已经落盘，错误后点取消仍留部分修改。原版也会先改内存 SimpleDNS，不能宣称原版是完全原子事务；但原版先验证兼容文本再 SaveDNSItems/SaveConfig，当前磁盘部分提交是新增差异。
- 保存构造 SimpleDnsDto 丢 globalFakeIp（内部默认 true），bridge dto_to_simple 清 extra。保存不相关字段必须保留原有未编辑字段、null/空串和未知键。GlobalFakeIp 本来没有原版窗口控件，缺控件本身不是漏项；隐式擦掉原值才是缺陷。
- 原版 Direct/Remote/Bootstrap、FakeIPRange、策略、ExpectedIPs 是可编辑 ComboBox/候选；当前全是 TextField，丢候选、约束和选择路径。HappyEyeballs 参数迁到参数设置另页，原版 DNS 高级页的位置未保持。
- Xray/sing-box 自定义同时启用时原版 IsSimpleDNSEnabled 禁止普通设置，当前没有联动禁用，用户修改普通区仍看到保存但实际被自定义覆盖。
- 区域预设只能离线保留 URL/内置模板；远程模板、外部路由和 SRS 资源必须有实际下载/验证/事务效果，不能用 pendingUrls 永久悬空。

## 全部 180 设置的真实消费分类

逐字段 JSON 保留原版路径、类型、默认、classification、apply_timing、控件及源定位；同组共用保存链是可复用实现，但不是每个叶子都已生效。每行把保存/重开/效果分列，逐项真机验证没有完成。

1. `CoreBasicItem`、`Inbound`、KCP、gRPC、Mux、Hysteria、Fragment、HappyEyeballs、TUN 基本字段已有类型和 codegen 投影。以 codegen 定位作为实现证据，不能写真实连接通过。缓存开关 `EnableCacheFile4Sbox` 当前 codegen→sing-box experimental.cache_file 有消费，但 UI 缺原版开关。gRPC 原版就没有独立窗口控件，缺 UI 不单独判漏；Xray 四字段有消费者，sing-box initial_window_size 是否原版支持另由运行领域确定。
2. `GuiItem.AutoUpdateInterval/EnableHWA/EnableLog/RootCertProvider` 没有生产消费链；相关存储与默认处理只足以认保留。统计/实时速率虽有显示与后端能力，普通 startup/apply 不启动监控链，具体见运行报告。`KeepOlderDedupl` 原版有开关，当前没有，订阅 merge_options 又固定 keep_older:true。
3. `ConstItem` 四种源 URL 保存/区域预设会写值，但普通更新没有对应消费，SubConvertUrl 未构建转换 URL。
4. `UiItem` theme/accent/family/fontsize/layout 由 shell 读取，语言只存 shell.language，没有 MaterialApp 本地化消费；表格/弹窗固定字号未做到统一缩放。MainGirdHeight/WindowSizeItem/MainColumnItem 与本地 ui_state.json 独立偏好未迁映射，原版列设置导入/重开不能按当前新 store 来宣称一致。macOSShowInDock 本轮平台未验证，Windows不适用不能据此删掉macOS需求。
5. `MsgUIItem` 没有从原版字段读写 UI 状态。`ClashUIItem` 排序/自动刷新/interval 与 Proxies/Connections 的本地固定 Timer 脱节，ConnectionsColumnItem 没原版迁移链；EnableMixinContent/IPv6 的纯函数存在，生产调用未连。
6. `SystemProxyItem` 参数页保存后真实平台写入未在本轮执行；状态栏/托盘更新模式不会 save_settings，重开回原 JSON。PAC 服务 FRB 存在，但 UI 无 pac_start 调用，两个入口缺普通启用路径。不能通过自动测试写宿主代理。
7. `CheckUpdateItem` 三字段有存储，但检查更新窗口独立默认 false/全选且没同步；WebDavItem 四字段具备专用 config DTO/保存/HTTP usecase，窗口布局、默认远端配置再加载、检查连接保存边界、实际 TLS/远端恢复未整体验收。
8. `GlobalHotkeys` 行存储正常，WPF Key 枚举/VK/Flutter keyId 三套编码有实质错误，注册动作回调和保存后的注册重载缺失。旧 `Fragment.Length/Interval` 是迁移字段，UI 无控件符合其类别，必须保存旧数据并补旧配置迁移夹具；不是新 UI 缺漏。
9. 顶层复合对象、内部 IndexId/SubIndexId/Inbound.Protocol/CoreType.ConfigType/列和窗口模型不能虚构“必须有一个输入框”；要检查它们在对应操作中读写与生效。当前 active_index_id 与 IndexId 两套状态、主组仅内存，都需要原版导入与正常选中操作的映射证明。

## 布局和剩余动作

本报告逐项核对了 11 布局和 20 WPF/Avalonia 窗口库存，不只看有没有类名。参数原版 1000×700 当前 content720×520且增加到12页；订阅、路由顶部工具栏改弹窗底部，原版 DataGrid 列/多选/排序/右键能力简化；备份原版两卡片+原生文件选择变为多个目录/ZIP路径文本框和列表文字，普通用户操作成本明显上升；主题原版主窗口 PopupBox 内候选改独立对话框和英文内部字符串；DNS/路由/模板文档链接没有迁移。

全局热键/管理员重启/UWP/核心网站/推广/区域预设入口仍占位或禁用，不能作为完成。关闭到托盘原生实现存在，但真实 tray/native close 生命周期、退出恢复、重开此次未测，交由运行领域。新增帮助/更新入口不得拿 mock 消息替代外部浏览器/工具/安装的实际效果。

## 本轮实际命令和证据边界

所有源码对照通过 `rg`/`Get-Content`/Python 解析分配清单进行，仅公开冻结源码与当前源码。第一条裸 `cargo` 命令因本 shell 的 PATH 找不到而失败，随后使用明确安装路径运行。

```powershell
& 'C:/Users/Colby/.cargo/bin/cargo.exe' test -p application --test t11_routing_dns --test t16_backup --test subs_pipeline --locked
# subs_pipeline 9/9、t11_routing_dns 11/11、t16_backup 4/4，exit 0
& 'C:/Users/Colby/.cargo/bin/cargo.exe' test -p application --test t18_settings_chain --locked
# 2/2，exit 0
```

共 26 个后端合成用例通过。只使用临时数据与合成 fixture；本地服务器从 ≥11808 逐口 bind 探测，settings_chain 从 ≥11811 探测且仅生成配置，无内核/代理/TUN/自启写入。它们证明后端局部能力及数据链，未覆盖前端草稿对象、原生热键、实时恢复、真实远端 TLS 或用户实际逐功能操作。

特别注意：`inbound_protocol_is_persisted_but_generator_always_emits_mixed` 的名称/注释/打印仍声称 Protocol 未映射；当前 codegen 已有 `inbound_protocol_token`，SOCKS 族 mixed 输出可以是原版行为。此旧 characterization 的打印**不能**作为当前漏移植结论。

未运行：本代理独立新增 Flutter/Windows 集成场景、冻结原版所有窗口人工逐事件操作、真实 WebDAV/TLS/代理下载、宿主自启/代理写入、TUN 与 macOS/Linux。根代理串行真实场景归档在 `ui-run-02/`，本领域只引用其普通空 URL 分组结果，不与其他源追踪结论混算。

逐项清单已完成 373/373 分配 key：33 功能、180 设置、52 实体字段、77 动作、11 布局、20 窗口。通过 Python 集合/唯一性/必填字段/状态/引用文件及行号检查：无缺项、无多项、无重复、无空必填、无非法状态；2088 个唯一源码/证据引用均存在且行号不超过文件长度。审查标签为 identified 158、implemented 159、preserved_only 56、verified 0。implemented 仅表示源码有该路径，并保留每项未验证边界，不是迁移完成比例。

台账源合同纠正建议：F-HELP-003 写“关闭退出并恢复代理”，但冻结 WPF `MainWindow.xaml.cs:240-244` 的 `MenuClose_Click` 实际执行 `StorageUI(); ShowHideWindow(false);`，即隐藏到托盘。当前 `main_shell.dart:381-389` 的 `hideToTray` 符合此源行为；不应制造“关闭未退出”的缺陷。保留该 key 和分母，在后续台账中追加纠正依据，本轮未改 compat。

## 后续修复的固定验收队列

1. 两个普通空 URL 分组、两个下载订阅；主窗口 B → 当前组更新必须只请求 B；订阅添加/编辑图标直接打开对应对象。保存/改名/删除之后主组和选中/运行目标同步，取消不修改。
2. 延迟本地订阅 → 运行中拿当前任务 ID → 取消 → HTTP/job/原节点都符合取消合同；重复点击、并发更新、远端/解析失败保留旧数据；重新启动后自动定时真实运行。
3. 新建路由 → 连续加两条 → 编辑其中一条 → 只变一条；导入草稿/移动/取消不落盘；导出当前草稿 JSON 可重新导入；删光保存重开为零；失败仍保留会话且不部分提交。
4. DNS 普通和自定义草稿 → 导入模板 → 取消 → 字节级/字段级没有意外变更；应用保存当前稿；改 DirectDNS 不擦掉 GlobalFakeIp/未知键；非法第二核心文本失败不提交第一阶段。
5. 合成原版 ZIP（180 设置取可区别值、当前默认节点/分组、资源）→ 导入 → 重开 → UI/DB/config/生成配置一致；恢复先停止本项目活动会话并完成 DB 句柄生命周期，失败保留完整旧配置/资源，不能只数 DB 行。
6. 冻结合成 WPF KeyCode 热键导入和新录制 → 编码正确 → 保存后重载 → 非网络 showWindow 动作实际触发；冲突/重复绑定/修饰键录制/取消均符合源合同，禁止为此触发宿主代理动作。
7. 恢复原版五设置分组和候选/浏览入口，再逐字段走更改→保存→重开→配置/平台效果，未有消费者的字段继续挂牌而不是留永久“需重启”提示。语言/字号/DPI和三布局在真实窗口验收。
8. 最后补区域模板/SRS资源、每日更新、帮助/核心网站/推广、备份/恢复原生文件选择和列表多选快捷键；全部原版入口保持可达，不能通过去掉功能降低分母。

发现接口缺口：登记阻塞与建议，不自行削减需求。逐项状态是审查状态，不等同于发布批准或迁移完成。
