# 设置、路由/DNS、底部、窗口、托盘、热键：正常用户流程审查

日期：2026-10-05。审查基线：`77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。冻结原版：v2rayN 7.25.4，`7d6a967c18c697f28dc6917122ed3a4993fcf336`。本轮没有改生产源码、测试源码、任务卡或台账，没有提交。

本报告证明的是具体调用链断点，不是“功能全未实现”。原生参数设置和路由顶层窗口已经实现，问题集中在提交时机、错误回传、数据保护、主题传递和使用反馈。四份台账的登记状态不作为通过证据；旧审查中已修复的启动接线、恢复热键、订阅逐组报告等不重复判缺失。

定位约定：当前路径相对仓库根；下文 `UP/` 指 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。行号按本轮基线。状态均为 `identified`；“源码确认”不等于真实 Windows 用户流程 `verified`。本轮未操作真实应用窗口，未启动内核、监听端口、写宿主代理/自启/TUN；数据只使用合成公共 DNS 地址及内存桥。

## 覆盖和边界

| 入口/流程 | 本轮检查的链路 | 未闭环部分 |
|---|---|---|
| 参数设置五页：基础、v2rayN、系统代理、TUN、内核类型 | 打开→快照→编辑→确定/取消→主 engine 保存→平台同步→运行应用→重开来源 | 未逐个完成 180 字段真机效果；内核/平台字段消费者由运行域报告补齐 |
| 主题设置、语言、字体、子窗口外观 | 主 app 与独立 engine 的 theme/locale 配置、原版资源绑定 | 未真机 DPI/字体矩阵 |
| 路由方案、规则详情、策略、默认路由、内置导入 | 原版各提交点/Closing→当前草稿/保存→Rust/存储→Reload；键鼠入口 | URL 远端/实际配置生成与流量效果未验证 |
| DNS 四页及区域预设 | 草稿/默认导入/校验/保存/应用/取消；内存刷新 | 有 1 个新增合成 widget 失败复现；真实 Rust DNS/网络效果未验证 |
| 底部端口、TUN、系统代理、路由、节点、速率、错误 | 布局结构、控件辨识、消息优先级、实际/期望状态模型 | 未真实截图/点击/DPI，未 TUN/系统代理实测 |
| 托盘四模式、路由/节点、导入/扫描/订阅、复制命令、退出 | 菜单模型→dispatcher→共享用例；模式保存结果 | 主业务由其他领域报告核对；未宿主写入 |
| 热键五动作、录制/重置/保存/冲突/取消 | 暂停、WPF Key→VK/Flutter 桥、分组派发、重新注册、失败分支 | 未真实 OS 占用/按键/关闭窗口/权限验证 |
| 订阅、备份、导入迁移、帮助 | 本轮只复查相关修复卡与已有恢复/调度接线，未重跑完整流程 | 不以本报告宣称这些领域全通过；纳入主报告其余专项和最终场景矩阵 |

必读任务卡已对照：`R3-WPF-OPTION-WINDOW.md`、`R3-WPF-ROUTING-WINDOW.md`、`R3-WPF-MAIN-CHROME.md`、`R3-VISUAL-DPI-TRAY.md`、`RR-02.md`、`RR-03.md`、`R3-SET-01..06.md`。涉及 `ACT-MAIN-024..026`、`ACT-OPT-001/002`、`ACT-ROUTE-*`、`ACT-DNS-*`、`ACT-TRAY-012`、`F-DESKTOP-002/003`、`F-SYSPROXY-*`、`LAY-OPTSET-001`、`LAY-ROUTINGSET-001`、`LAY-STATUSBAR-001`。

## 必须修复的具体问题

### UFS-01 / P1：路由顶层窗口改变了原版提交与关闭语义

- 原版：`UP/ServiceLib/ViewModels/RoutingSettingViewModel.cs:58` 策略变动自动保存；`:134` 子规则集编辑确定后已落库；`:153` 删除所有选中方案、`:175` 设默认直接提交并置 `IsModified`。`UP/v2rayN/Views/RoutingSettingWindow.xaml.cs:43` 关闭时有修改返回 true，`UP/ServiceLib/ViewModels/MainWindowViewModel.cs:598` 刷新菜单并 Reload。
- 当前：`apps/desktop/lib/features/routing/routing_windows.dart:1828`、`:1893`、`:1914`、`:1928` 全部只改 `_schemes`/局部策略；`:1947` 只有新增的全窗“确定”才调用 host.save；`:1968` 取消/关闭丢弃。当前任务卡明确禁止改变原版持久化语义，实施代码却改变了它。
- 用户触发：编辑某规则集→在子编辑器点保存→关闭外层路由窗口→重开。用户已经执行原版提交动作，当前修改仍会丢失；切默认或策略后直接关闭也不生效。
- 原因：独立窗口迁移把所有业务换成一个全窗草稿提交合同，未保留原版各提交点/Modified 关闭结果。
- 修复：保留已有原生窗口，把原版每个提交动作传给主 engine 独立用例；每个用例自身事务化。关闭仅按真实 `modified` 事实刷新/重载；子编辑器取消仍不提交。不能以“整窗事务+取消丢弃”替代原版语义。
- 验收：逐项录制策略变动、子编辑保存/取消、删除、设默认、修改后 Esc/标题关闭、未修改关闭；比较数据库、重开值、Reload 次数和菜单。当前仅源码确认；现有 fake 测试反而固定了整窗确定/取消合同，不是原版等价证明。

### UFS-02 / P1：一键导入规则集仍是无操作的成功提示

- 原版：`UP/ServiceLib/ViewModels/RoutingSettingViewModel.cs:182` 调用 `ConfigHandler.InitRouting(config,true)`，成功后刷新列表并置修改。
- 当前：`apps/desktop/lib/features/routing/routing_windows.dart:1939` 原生入口与 `:225` 旧入口都只提示“内置规则集已刷新（导入后端用例待接入）”；没有 bridge 调用。`crates/application/src/dns.rs:253` 只有模板源选择函数，不能算导入消费者。
- 用户触发：点“一键导入规则集”或右键同名项→看到已刷新→列表/规则无变化。
- 修复：实现原版 InitRouting 的模板获取/解析/替换边界和结果，并连接两个入口；源 URL 使用 `ConstItem.RouteRulesTemplateSourceUrl`。错误、取消、解析失败须保留原数据，完成后报告实际导入数量。
- 验收：空库、已有自定义方案、重复导入、自定义合成模板源、非法 JSON、下载失败、取消、重开；真实保存和配置规则顺序均需证据。源码确认，未真实导入。

### UFS-03 / P1：原生路由保存忽略删除/策略/应用失败，可能部分保存却关闭

- 当前：`apps/desktop/lib/features/routing/routing_actions.dart:103` 逐方案保存并即时 reload；`:117` 忽略 delete 返回值；`:121` 忽略 settings.saveGroup 返回值；`:135`/`:137` 返回 `Future<void>` 的重载结束后，`:139` 一律 `ok:true`。`routing_controller.dart:312` 删除实际有结果；`:340` 会检测运行失败并写 status，但原生回传未消费该状态。
- 用户触发：删除一个方案并改策略→确定，删除或策略写入失败；或路由配置保存后 codegen/check/reload 失败。窗口会关闭，部分修改已经落库，用户难以知道失败在哪步。
- 原版锚点：UFS-01 的每个提交用例和 `MainWindowViewModel.cs:598`；不能把整窗失败时的部分写入包装成原版成功。
- 修复：先完成 UFS-01 的提交合同；各提交用例有独立 revision/事务/结构化结果。区分“保存成功但运行未应用”和“保存失败”，明确回传错误阶段/保持编辑草稿；不能仅将所有写操作加到全窗事务改变原版时机。
- 验收：对保存、删除、策略持久化、生成校验、运行 apply 分别注入失败，核对本用例回滚、此前已提交用例保留、窗口反馈和重开。源码确认；故障注入/真实运行未验证。

### UFS-04 / P1：DNS 普通“保存”不会按原版生效

- 原版：`UP/ServiceLib/ViewModels/DNSSettingViewModel.cs:195` 保存后关闭；`UP/ServiceLib/ViewModels/MainWindowViewModel.cs:613` 收到成功立即 Reload。
- 当前：`apps/desktop/lib/features/routing/dns_window.dart:403` 保存走 `_save` 默认 `applyAfter=false`（`:699`）；`:814` 关闭，只有 true 才 `applyActive`（`:815`）。外层 `routing_actions.dart:153` 没有补重载。`dns_controller.dart:76`/`:100` 的“未应用，点应用”提示留在已关闭窗口内。
- 用户触发：运行合成节点→修改 DNS→点“保存”→服务继续使用旧 DNS；重开能看到新值，使用户误以为已经生效。
- 修复：恢复原版确认保存后主窗重载时机；若保留额外“应用”操作，须是明确新增行为，不能让默认原版保存偷偷只保存。反馈包含保存和应用结果，应用失败不得显示全面成功。
- 验收：分别点普通保存/取消/默认导入后保存，比较持久化 revision、desired/applied revision 和真实内核配置，失败时旧运行应可解释。源码确认；现有 fake 测试只覆盖“应用”路径，不覆盖普通保存实际生效。

### UFS-05 / P1：DNS 草稿基线按文本作键，跨字段相同值会丢编辑（已合成复现）

- 当前：`apps/desktop/lib/features/routing/dns_window.dart:149` `_textBaseline[controller.text]=value`，以所有字段共享的文本内容作键；`:167` 使用当前文本反查该表判断是否未编辑。`:623` 应用预设成功后触发刷新。
- 原版：`UP/ServiceLib/ViewModels/DNSSettingViewModel.cs:69` 的加载、`:106` 的保存以独立属性保存各字段；原版没有这种共享文本键脏状态算法。区域预设是新增入口，不能覆盖用户正在编辑的另一页字段。
- 实际合成操作：种入 direct=`1.1.1.1`、remote=`8.8.8.8`→打开 DNS→把 direct 改成 `8.8.8.8`→点击预设。断言期望 direct 草稿仍为 `8.8.8.8`，实际变回 `1.1.1.1`。即使合成预设本身没有修改存储，刷新也会丢草稿。
- 证据：`dns-baseline-collision-repro.dart:75`、`dns-baseline-collision-repro.log`；harness SHA256 `B149527F59FFC88DB597F5DF17B43B349E83D2DD59D488EFE8F9B637E7C987CB`。只用 SyntheticBridgePort、SyntheticRuntimeBridge、FakePlatformBridge、MemoryUiStateStore。
- 修复：基线使用稳定字段 ID 或 controller 身份作键，比较该字段自己的原值；无脏编辑的字段同步后更新自身基线；布尔字段也需同步基线。预设事务结果与局部编辑冲突明确提示，不用字符串相等猜编辑状态。
- 验收：保持上述失败断言转绿；再覆盖两个相同初值、改为空、改成另一个字段旧值、重复切预设、多页文本/布尔编辑、取消。已实际运行 widget 复现；未真实窗口操作。

### UFS-06 / P1：参数设置确认的跨 engine 结果不包含平台/运行效果

- 原版：`UP/ServiceLib/ViewModels/OptionSettingViewModel.cs:400` 保存成功后 await AutoStartup 更新、发送成功/需重启提示；主窗 `MainWindowViewModel.cs:583` 接着 await Reload。原版同样先关闭设置再由主窗重载，且 `UP/ServiceLib/Handler/AutoStartupHandler.cs:9` 也未保证自启失败回滚；本项不能解释为“原版一定应用完才关窗”，而是当前跨 engine 合同没有带回平台/运行工作结果。
- 当前：`apps/desktop/lib/features/settings/settings_actions.dart:69` 调用 `_writeAutostart` 的 bool 被丢弃，`:72` 不 await applyActive，`:73` 即刻 `ok:true`；子窗 `option_setting_window.dart:381` 收到结果就关闭。注释“应用后窗口关闭”与实际行为不符。
- 用户触发：变更开机启动、入站或内核选项→确定；自启权限/写入失败或新配置应用失败，子窗口仍报告成功关闭。运行错误可能后续进入主窗，但当前没有完整、可见的本次操作结果。
- 修复：主 engine 返回分阶段结果：保存事实、平台同步事实、运行应用事实、需要重启字段。await 该动作所属的实际工作，按原版时机提交；若保存已成功、运行失败，明确“已保存/未应用”并可重试，不能谎称未保存或全面成功。
- 验收：平台拒绝、codegen/check/apply 失败、无活动节点、合法保存，比较已保存值与实际效果；自启只允许隔离专项验证。本轮源码确认，未写宿主自启/代理。

### UFS-07 / P2：独立设置窗口没有接回“需要重启”等生效提示

- 原版：`UP/ServiceLib/ViewModels/OptionSettingViewModel.cs:308` 计算需重启条件，`:405` 告知用户。
- 当前：`settings_controller.dart:120` 和 `:191` 计算 restart/nextLaunch status；主回调 `settings_actions.dart:73` 只回 ok；独立 engine 不读主 settings state，编辑窗 `option_setting_window.dart:202` 的 status 区也收不到此结果，成功后直接关闭。
- 用户触发：保存 HWA、统计或其它 restartApp/nextLaunch 字段→未收到生效时间说明，重复点击/重开也无法判断。
- 修复：回传字段分类并在主窗口操作结果/通知中展示；哪些当前尚无消费者须在字段验收里明确阻断，不能仅提示“重启即可”。
- 验收：每类 Immediate/RestartCore/RestartApp/NextLaunch 至少一个真实消费者场景，对比重开与效果；当前仅源码确认。

### UFS-08 / P1：快照失败被当成正常空配置，可能覆盖配置或删除路由

- 当前参数：`settings_actions.dart:26` load 异常被吞；`settings_window_host.dart:99` 快照初值 `{}`，`:104` 50 次 ready 重试耗尽不抛错，`:123` 失败返回空 Map；`option_setting_window.dart:80` 仍设 draftInit=true 并允许确定。`:132` 缺 Inbound 还会创建默认监听项（LocalPort=10808），所以本地表单校验不等于已经拒绝空快照；本轮只读取这一分支，未调用/监听该端口。
- 当前路由：`routing_actions.dart:58` 规则读取失败转换为 `[]`；`routing_windows.dart:1659`/`:1661` 同样默认空快照，`:1535` 解码失败也变空；主保存 `routing_actions.dart:115` 会删除不在草稿的原方案。
- 条件触发：真实快照通道持续失败，或某方案读取 rules 失败→编辑窗呈现空/少数据→用户按确定。代码无法区分“合法空库”和“读取失败”，故存在覆写默认值/清空规则/删除方案的条件链。
- 原版锚点：`UP/ServiceLib/ViewModels/RoutingSettingViewModel.cs:84` 从实际项载入、`:119` 按选中项编辑；原版没有把“全量读取失败”转成用户删除全部方案的动作。本项是故障边界问题，未声称已在用户真机发生。
- 修复：严格版本化 snapshot envelope（成功、内容、revision、错误），未 ready/载入失败禁提交并给重试；规则读取失败不构造可提交空规则；删光必须是明确用户操作。非法 `{}` 草稿拒绝，不能隐式作删除全部。
- 验收：丢 ready、延迟 ready、无效 JSON、断 rules 读取、真实空库分别测试，所有失败提交路径数据库 no-diff。源码确认条件链；原生故障注入未运行。

### UFS-09 / P2：跨 engine 保存没有超时/异常结果收尾，参数设置可重复提交

- 当前：`settings_window_host.dart:67` await 回调没有异常转换；`:146` pending completer 无 timeout。路由同型 `routing_windows.dart:1632`、`:1699`。原生 `windows/runner/option_window_host.cpp:71` / `routing_window_host.cpp:72` 即使主 channel 不存在仍回 Success，后续没有 saveOutcome 就永久 pending。参数按钮 `option_setting_window.dart:233` 没有 busy guard；路由有 busy，但 `:2092` 取消仍可用。
- 用户触发：主回调抛错/结果丢失→确定后一直等；连续按确定或保存途中关闭，结果和窗口生命周期可能交叉。
- 修复：每个提交 request ID 有生命周期/幂等保证；主回调 try/finally 转结果；channel 缺失明确失败；超时不自动重放有副作用命令，应查询该 request 的真实提交状态；保存中控制可取消边界并等待 owned work 收尾。
- 验收：延迟响应、主回调异常、关闭重开后旧回复、双击确定、主退出，均无永久 busy/重复写/旧回复污染。源码确认；真实 engine 通道故障未验证。

### UFS-10 / P1（可用性）：底部状态栏把操作控件、诊断和状态压成横向长串

- 原版：`UP/v2rayN/Views/StatusBarView.xaml:17` DockPanel；`:23` 右侧两行速率；`:38` 左侧两行端口；`:48` TUN；`:73` 系统代理 160 宽 ComboBox；`:87` 路由 160 宽 ComboBox；`:98` 中部两行服务/信息。`compat/layouts.yaml:1205` 有对应结构。
- 当前：`apps/desktop/lib/app/shell/status_bar_view.dart:36` 固定 11.5 字，`:40` 固定 30 高，`:42` 横向 ScrollView，`:45` 单行 Row；`:96`、`:120`、`:144` 选择器为无边框/无箭头普通 Text；`:154` host/PID/ports、`:163` revision、`:171` epoch/seq、`:185` total/visible/selected、速率/错误/提示全混在一起。
- 用户触发：正常窗口/800宽/150% DPI→找系统代理、路由与速率；文本像状态而非可点控件，主要信息需横向滚动才见。没有 overflow exception 不能证明用户可用。
- 修复：恢复冻结左右/中部结构与顺序，两行端口/速率，显式下拉轮廓/箭头/焦点；技术诊断进 tooltip/详情，不占主信息流。扩展 rule-mode 若保留须有明确产品位置与标签。窄宽只合理压缩服务摘要/二级信息，不缩字、不隐藏主控件、不以滚动作为正常使用前提。
- 验收：800/1200/1280 宽、100/125/150/200% DPI、长合成节点/错误/中文、亮暗主题；截图加真实鼠标/Tab/方向键选择，全部主控件和两类速率同时可见。仅源码诊断；本轮没有真实截图。

### UFS-11 / P2：底部“节点”显示 PID/端口，没有服务摘要

- 原版：`UP/ServiceLib/ViewModels/StatusBarViewModel.cs:267` 显示 running.GetSummary，`:354` 独立运行信息。
- 当前：`status_bar_view.dart:148` 使用 `RuntimeView.statusLabel`；`features/runtime/runtime_bridge.dart:95` 运行时文本只有 PID/端口。RuntimeView 也没有 applied node identity，因此不能直接把 profiles.activeId 当真实正在运行节点（应用失败时会错）。
- 用户触发：从 A 切到 B/保存后应用失败→底部不能确认实际节点，且重复出现 PID/端口。
- 修复：运行读模型带实际 applied profile identity 和允许显示的服务摘要；UI 将期望选择/实际服务/未应用分开，故障时显示真实仍运行对象。摘要不展示凭据。
- 验收：A运行、B应用成功、B失败保留A、无节点、恢复、重开，主表/托盘/底部事实一致。源码确认；真实切换未验证。

### UFS-12 / P2：旧平台提示会永久遮住后来的普通操作结果

- 当前：`status_bar_view.dart:234` 只要 platform.message 非空就不显示 shell.message；`platform_controller.dart:67` 每次成功写平台提示，`:342` clearMessage 无生产消费者（本轮 rg 调用搜索）；普通动作写 `ui_shell_controller.dart:339` 的 message 不会清掉平台提示。
- 用户触发：切过一次系统代理模式→之后执行导入/订阅等写 shell.message 的操作，底部仍优先展示旧“系统代理”反馈。无需宿主写入才能证明 UI 优先级断点。
- 原版锚点：`UP/ServiceLib/ViewModels/StatusBarViewModel.cs:354` 当前测试结果更新独立运行信息；原版 NoticeManager 发布独立操作通知，没有这种永久跨域遮盖合同。
- 修复：操作结果有来源、时间、严重性/可关闭语义；显示最近相关结果，持久故障独立标识，不能无期限用某域 message 覆盖所有新操作。
- 验收：用假平台产生平台消息→不同业务成功/失败→用户看到最新结果且仍可追查旧平台故障。源码确认，未新增该 widget 测试。

### UFS-13 / P2：原生子窗口固定亮色，丢失当前主题/字体/语言

- 原版：`UP/v2rayN/Views/RoutingSettingWindow.xaml.cs:40`、`GlobalHotkeySettingWindow.xaml.cs:24` 使用当前主题边框，各视图使用共享资源。
- 当前：`features/settings/option_setting_window_entry.dart:21`、`features/routing/routing_windows.dart:1756` 均只 `buildAppTheme(Brightness.light)`；没有 parent accent/font/locale；主 app `app/app.dart:29` 已有完整 theme/font/locale 参数。
- 用户触发：主窗口暗色/指定字体/英文→打开参数或路由→亮色默认字体/中文，界面一致性断裂。
- 修复：snapshot envelope 带只读 presentation 配置，子 engine 应用同样 theme/字体/locale；不要读取个人配置或初始化第二 Rust engine。明确主题变动时是否要同步打开窗口。
- 验收：亮暗/自定义字体/字号/语言各打开重开/跨 DPI，与原版和主窗一致。源码确认；未真实 DPI 对照。

### UFS-14 / P2：路由列表失去多选、全选、Enter/Delete/Back 键语义

- 原版：`UP/v2rayN/Views/RoutingSettingWindow.xaml.cs:52` Ctrl+A/Enter/Delete/Back，`:80` 全选菜单，`:85` SelectedSources；VM `:153` 遍历选中项删除。
- 当前：`routing_windows.dart:2052` 只有 `_selectedId`；`:2151` 右键重置单选；`:2159` 菜单只有添加、删除单行、默认、导入。顶层快捷键只为 Esc（`:1977`）。
- 用户触发：想批量删方案→Ctrl+A/Delete无效，原版全选菜单消失；Enter设默认也无效。
- 修复：选择模型区分焦点与 selectedIDs，支持 Ctrl/Shift/框定原版键鼠语义，右键保留已有多选并据命中更新，完整恢复全选菜单与命令门控。
- 验收：单选、多选、右键已选/未选/空白、Ctrl+A、Enter、Delete/Back、确认取消、重开，原版逐事件对照。源码确认，未真机键鼠。

### UFS-15 / P2：托盘复制代理命令没有派发入口

- 原版：`UP/v2rayN/Views/StatusBarView.xaml:224` 菜单；`UP/ServiceLib/ViewModels/StatusBarViewModel.cs:227` 生成 Windows set/其它 export 的大小写六行代理环境变量并交给剪贴板。
- 当前：`tray_menu_model.dart:184` 有 ACT-TRAY-012，但 `:101` 无共享映射；`desktop_integration.dart:274` 没专门 case，`:292` 输出“尚未接入后端”。
- 用户触发：托盘→复制代理命令→没有复制。
- 修复：接共享有类型用例，按当前实际入站协议/端口生成原版命令并写剪贴板；无有效入站给明确结果，不能复制错误期望端口。
- 验收：合成 mixed/HTTP/SOCKS 入站、端口改变尚未应用、无运行状态，对照原版六行输出；只校验剪贴板内容，不执行命令。本轮源码确认，未写真实剪贴板。

### UFS-16 / P1（错误边界）：托盘/底部/热键切代理忽略模式持久化失败

- 原版：`UP/ServiceLib/ViewModels/StatusBarViewModel.cs:362` 切模式→平台更新→保存配置；成功和重开选择构成同一用户合同。原版本段也没有完整写盘错误回滚保证，本项是当前明确丢弃已有 Result 的可靠性缺口，不声称原版具备全程原子事务。
- 当前：`features/settings/platform_controller.dart:263` _persistAndReturn 调 `_persistMode` 后仍原样返回平台结果；`:289` 丢 saveGroup 结果；`:276` 无运行会话也丢保存结果。托盘、状态栏、热键共享此路径。
- 用户触发：模式切换时 settings revision 冲突/存储失败→显示平台操作成功；重开还原旧模式。也可能真实外部效果已变而选择/持久化不同步。
- 修复：结果分别携带 desired mode 保存/外部 apply 事实，失败不得报全面成功；确认当前 ownership 后才采取补偿，不回退项目已有安全 ownership 合同。
- 验收：FakePlatform+失败存储/revision冲突，各模式/无运行场景检查反馈、设置重开值和 actual/desired；宿主真实验证仅隔离专项。源码确认，未宿主写入。

### UFS-17 / P2：切语言只是 Material locale，产品文字仍硬编码中文

- 原版：`UP/v2rayN/ViewModels/ThemeSettingViewModel.cs:101` 切 CurrentUICulture；参数窗口 `UP/v2rayN/Views/OptionSettingWindow.xaml:49` 等通过 ResUI 资源取文字。
- 当前：`app/app.dart:46` 已接 locale 和 Material delegates，应承认这部分实现；但参数五页 `option_setting_window.dart:183`、底部/路由/DNS/热键上列文件均硬编码中文，没有对应产品资源。两个独立 engine 更没有 locale（UFS-13）。
- 用户触发：主题设置选 English→重开应用/设置，控件系统文字可能变英文，产品菜单/字段仍中文。
- 修复：冻结 ResUI key 对应可追踪的产品本地化资源，绑定主/子 engine 统一 locale；语言保留原版生效时机，不仅改 Material locale。
- 验收：冻结各适用语言的关键主流程文字、长文本布局、重开，资源 key 无遗漏/原始 key。不以 JSON 保留和 locale 单测作为完整移植证明。源码确认；本轮未切真实 UI 语言。

## 待验证候选和不能扩大成既成事实的部分

1. **子窗口 stale draft 覆盖更新 / P1 候选**：`settings_window_host.dart:48` 只传文档，不传 opening revision；`settings_controller.dart:114` 在保存时用主 controller 最新 revision。若子窗口打开期间托盘/热键/其它设置消费者更新主文档，旧全量草稿可能用新 revision 覆盖并发值。Windows owner 被禁用不等于托盘/全局热键被禁用。静态 revision 合同缺口确认，但此并发真实用户触发未验证。应以 opening revision/分组变更补丁+冲突比较修复，先做假 host 并发模型，再 Windows 可控非平台场景。
2. **热键暂停并非派发门 / P2 候选**：`global_hotkey_window.dart:56` unawaited beginEdit；`hotkeys.dart:517` 设 _paused 后 async unregister，异常被吞；注册 handler `:441` 直接调用 onTriggered，未读 _paused。原版 `UP/v2rayN/Manager/HotkeyManager.cs:145` 在派发处检查 IsPause。因此存在已排队事件/注销失败/冲突保存短暂重注册期间触发的条件。当前正常注销和保存冲突后注销已实现，不能再说“完全没暂停”。建议增加同步派发门并 await editor ready；未真实按键/注销失败实测。
3. **DNS 多行写入故障后部分落库 / P1 可靠性候选**：`dns_window.dart:742/768/795` 顺序独立提交 Simple/Xray/sbox，后段 IO 失败时前段已保存。所有文本先校验已修复，不能重复旧“非法第二文本导致第一文本保存”结论。冻结 `DNSSettingViewModel.cs:185/193/195` 也顺序保存，未证明原版全程事务。本项目应把一次原版 DNS 确认提交作为 Rust 单事务+一个结构化结果；故障后取消不应让用户误以为全部未写入。未 IO 注入。
4. **托盘图标 desired/actual**：`tray_menu_model.dart:277` 依据 desiredMode/coreRunning/pacRunning；`desktop_integration.dart:245` 没带代理外部 enabled/error。平台失败时可能亮代理图标。冻结 `StatusBarView.xaml.cs:82` 本身也按配置取图标，因此先定清图标代表“模式选择”还是“真实已启用”，不武断判原版差异；需与运行域一起真实验证失败视觉反馈。
5. **TrayMenuServersLimit=0**：当前 `tray_menu_model.dart:200` 解释为无限制，冻结 `StatusBarViewModel.cs:289` 是 count>limit 则隐藏。原版可设置范围/0语义需联合字段校验确认，避免无条件改变边界值。
6. HWA、日志开关、证书 provider、内核更新间隔、Mixin/IPv6、各种 URL 源、TUN/系统代理读取后的真实消费：本轮 `rg` 重新查了当前代码，部分仍主要见于序列化/选择逻辑；以运行域完整链路为准，单独记录每字段消费者/时机/平台效果。不能把未找到 consumer 的粗搜结果直接当所有字段未实现，也不能把设置能重开当已生效。

## 供低一级模型执行的修复顺序和合同

1. **先冻结原版提交点与选择语义**：完成 UFS-01/14。每个动作写清输入、原版提交/取消时机、选中集合、错误、持久化 revision、是否置 Modified、关闭是否 Reload。不得继续把原版即时操作改成整窗最终保存。以现有原生 HWND/engine/host 为基础补接线，避免重建架构。
2. **补真实动作与保护数据**：UFS-02/03/05/08/16。Rust 实现或复用每个原版提交用例，事务范围只包该用例；UI 不自行串多个返回值不检查的写操作。读取失败不产生可提交空草稿。各故障场景先保留/明确已提交事实，再继续。
3. **打通保存→应用→反馈**：UFS-04/06/07/09。使用 `requestId + openingRevision + persistedRevision + apply result + timing fields + human message` 合同；有副作用工作可取消点明确，过期回复不影响新窗口；适用动作 await 实际效果。保存成功而 apply 失败保留正确已保存状态/旧实际运行，并提示重试。
4. **恢复可使用的外观**：UFS-10/11/12/13/17。冻结底部布局顺序、显式下拉控件、服务摘要/状态两层、公共主题/字体/locale，移出 raw runtime 调试字段。不要靠缩字/横向滚动躲布局验证。主窗口、参数/路由独立窗口、托盘使用一致的读模型。
5. **完成入口和生命周期**：UFS-15、热键候选、并发 snapshot 候选。主菜单/托盘/快捷键调用同一业务用例；热键暂停在派发处生效；恢复/重开/退出事件使用同一版本化状态来源。
6. **最终字段与流程移植验收**：180 设置字段和所有实体按“可见/可编辑→校验→取消→提交→重开→运行/平台效果→错误”补 evidence，不单凭静态分析或 mock。覆盖订阅自动/手动/代理/取消、原版导入资源/恢复、内核/更新、帮助链接，引用其他专项报告形成完整移植版本门禁。原版不适用、明确有意的安全差异另登记，不减分母。

每张新修复任务卡只包含一个原版用户流程，指向上面具体问题和 source 行；状态只用 identified/implemented/verified/preserved_only/blocked/not_applicable。以真实冻结对照的逐事件证据晋升 verified。避免把“1000 多测试绿”或“原生窗口已打开”当全部正常用户流程已完成。

## 本轮实际命令、结果、测试不足

以下在 `apps/desktop` 顺序执行，使用锁定 `C:/Users/Colby/toolchains/flutter/bin/flutter.bat`；未并行运行 Flutter。现有测试均为 synthetic/fake，未接宿主。

| 命令 | 实际结果 | 能证明 / 不能证明 |
|---|---|---|
| `flutter test test/fix08_dns_draft_test.dart --reporter expanded` | exit 0，3/3 | 默认导入草稿/取消、GlobalFakeIp保留、先校验、显式“应用”假路径；不覆盖普通保存真实生效 |
| `flutter test test/fix08b_dns_apply_test.dart --reporter expanded` | exit 0，4/4 | 唯一文本草稿保留/两个 custom gate；不覆盖相同跨字段文本 |
| `flutter test test/r3_wpf_option_window_test.dart --reporter expanded` | exit 1，3 通过，2 `did not complete` | 批跑失败保留；不能宣布5/5整套通过，输出未给足以归因引擎崩溃的证据 |
| 上一文件 `--plain-name 'Esc closes without writing the draft'` | exit 0，1/1 | 单独假host Esc通过 |
| 上一文件 `--plain-name 'a failed save keeps the window open and shows the error'` | exit 0，1/1 | 单独假host错误通过，不证明主engine真实失败回传 |
| `flutter test test/r3_wpf_routing_window_test.dart --reporter expanded` | exit 1，5 通过，1 `did not complete` | 批跑失败保留；测试确认当前新增整窗提交合同，不能证明原版时机正确 |
| 上一文件 `--plain-name 'a failed save keeps the window open and shows the error'` | exit 0，1/1 | 单独假host错误通过 |
| `flutter test ../../docs/evidence/user-flow-audit-2026-10-05/dns-baseline-collision-repro.dart --plain-name 'audit dirty DNS equal to other baseline survives preset' --reporter expanded` | exit 1，明确断言 Expected `8.8.8.8` / Actual `1.1.1.1` | 实际 widget 草稿丢失失败复现；不是 did not complete，也不是真实 Windows 窗口 |

最后一项的精确 harness 和运行输出已保存本目录；它从现有 `fix08b_dns_apply_test.dart` 的公共合成搭架复制，新增一个碰撞用例。附带原有用例没有在复现命令中执行。harness 的两个 support 文件 import 为本工作区绝对路径，换 checkout 时替换这两条 import 即可；不引用用户目录中的配置文件。

本轮另运行 `git rev-parse HEAD`、`git status --short`、`rg` 源搜索/行号读取、`Get-FileHash`。未运行 cargo 全量、flutter analyze、release build、真实应用窗口操作、远端下载/TLS/内核/宿主代理、自启/TUN 或真实全局热键。测试批跑未完成仍需单独定位，不用“其他用例单独通过”覆盖原失败记录。

仓库已有的 `apps/desktop/lib/features/profiles/profile_actions.dart` 用户/根代理未提交修改未触碰。本报告和合成复现证据之外无本代理仓库写入。
