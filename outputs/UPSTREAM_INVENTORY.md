**v2rayN 7.25.4 源码功能盘点（规划附录种子）**

研究日期：2026-10-01（Asia/Shanghai）。仅下载并阅读公开源码，未运行 v2rayN、未读取或改变用户代理/订阅配置。

本盘点用于 Flutter + Rust 重构规划，不采用 FlClash 底座。下列是**完整性种子和源代码定位索引，不是已经完成或验证的字段映射、全功能验收报告**。落地阶段仍需逐项追踪 UI → ViewModel → Handler/Service → 存储 → 生成配置 → 实际内核行为；UI 中已注释的控件、过时数据库字段及内核枚举成员不能直接当作现有可操作功能。

**1. 基线**

- GitHub API 当场读取 `repos/2dust/v2rayN/releases?per_page=8`：最新发布 **7.25.4**，`prerelease=true`，发布时间 `2026-09-30T06:39:05Z`（北京时间 2026-09-30 14:39）。固定 commit：`7d6a967c18c697f28dc6917122ed3a4993fcf336`。
- GitHub API `releases/latest` 返回最新稳定 **7.24.9**，`prerelease=false`，`2026-08-29T02:47:27Z`。其发布页短 commit 为 `521230c`。
- 搜索缓存仍可能显示 7.25.2；不可据搜索结果摘要判定最新。
- [7.25.4 发布页](https://github.com/2dust/v2rayN/releases/tag/7.25.4)，[7.24.9 发布页](https://github.com/2dust/v2rayN/releases/tag/7.24.9)。
- 以下源码路径均相对官方仓库根目录；链接均定位到冻结版本。
- 本版 release 说明新增/变化值得列回归：sing-box 1.14 支持及最高版本限制；通过代理检查更新；核心设置 Mux；订阅自定义 HTTP 请求头；Xray 配置格式；日志/临时文件 7 天清理；Clash API；阿塞拜疆语。稳定版还包含下载器安全修复、Happy Eyeballs、fake-IP 范围、TUN IPv4/IPv6、自定义 outbound、Xray allowInsecure 变更、HY2 ECH、阻止 AAAA、WireGuard 远程 DNS。

**2. 原版界面应当采用哪棵源码**

- Windows WPF 界面：`v2rayN/v2rayN/Views/*.xaml`；跨平台 Avalonia：`v2rayN/v2rayN.Desktop/Views/*.axaml`。二者共用大部分 `v2rayN/ServiceLib/ViewModels`，但控件、图标、平台可见性、焦点与原生行为并非像素相同。
- 主窗口 WPF 使用 MaterialDesign ToolBarTray + ToolBar，带图标菜单；Avalonia 顶部 DockPanel 中使用 Menu，主题控制停靠右侧。不能把跨平台版本截图和 Windows WPF 混成一个“原版”。建议锁定 Windows WPF 为 Windows 的视觉和操作基线；跨平台差异单独登记。
- 二者都实现 `EGirdOrientation.Horizontal / Vertical / Tab`：Horizontal 是左侧节点表、右侧信息/代理/连接标签；Vertical 是上方节点表、下方信息/代理/连接标签（默认）；Tab 将节点/信息/代理/连接放为平级标签。都有分隔条与尺寸持久化，绝不只做一种上下布局。
- `MainWindow.xaml.cs:359` / `MainWindow.axaml.cs:403` 的 `UpdateLayout` 和对应保存/恢复分隔比例逻辑是行为依据。`UIItem.MainGirdHeight1/2` 命名为 Height，但 Horizontal 下存的是宽度，迁移不要字面误判。
- 主窗下方始终是状态栏：代理/直连速率、本地/LAN 入站、TUN、系统代理四模式、路由选项、运行节点和运行信息。状态不应由 Flutter 开关乐观假定为成功，须来自 Rust 的实际状态。
- 主要源码：[WPF MainWindow](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/v2rayN/Views/MainWindow.xaml)、[Avalonia MainWindow](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/v2rayN.Desktop/Views/MainWindow.axaml)、[布局枚举](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Enums/EGirdOrientation.cs)。

**3. 功能家族检查清单**

**3.1 节点、导入、导出**

- 11 个基础节点类型：VMess、VLESS、Shadowsocks、SOCKS、HTTP、Trojan、Hysteria2、TUIC、WireGuard、AnyTLS、Naive；另有完整自定义配置 Custom、自定义出站 Outbound、策略组 PolicyGroup、代理链 ProxyChain，总计 15 个 `EConfigType`。
- 增删改复制、激活默认节点、去重（保留较旧项设置）、删除无效测试项、跨订阅组移动、置顶/上移/下移/置底、拖动排序、列排序/宽度/顺序恢复、过滤/搜索、多选/全选、按所有节点或地区生成组。
- 剪贴板导入、扫描屏幕二维码、从图像导入；分享二维码；导出客户端配置到文件/剪贴板；普通分享 URI、Base64 分享列表、内部 URI。
- Fmt 目录不能仅移植 VMess/VLESS：`VmessFmt / VLESSFmt / ShadowsocksFmt / SocksFmt / TrojanFmt / Hysteria2Fmt / TuicFmt / WireguardFmt / AnytlsFmt / NaiveFmt / InnerFmt / V2rayFmt / SingboxFmt / ClashFmt / HtmlPageFmt`。实际入口的 `FmtHandler`、`ConfigHandler` 必须一起读。支持类型不等于每种都有标准分享 URI，例如 HTTP 出现在节点类型中，但 `FmtHandler.GetShareUri` 不为其提供 case。
- URI 测试至少覆盖 IPv6/端口、URL 编码、中文备注、空值和大小写、重复参数、旧 VMess 与新格式、SS SIP008、HY2 的别名/Realm、Naive 的 https/quic 变体、无效项部分成功与错误定位；以基线源码为准，不凭协议印象补行为。
- `ProfileItem.ConfigVersion=4`，原版保留过时字段作迁移。协议专属字段放 `ProtoExtra` JSON，传输专属字段放 `TransportExtra` JSON。不得以“常用协议通用表单”掩盖高阶字段丢失。
- [节点类型](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Enums/EConfigType.cs)、[ProfilesViewModel](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/ViewModels/ProfilesViewModel.cs)、[Fmt 入口](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Handler/Fmt/FmtHandler.cs)。

**3.2 精确键鼠行为**

- WPF 表格当前快捷键：Ctrl+A 全选；Ctrl+C 导出分享 URI；Ctrl+D 编辑；Ctrl+F 显示分享；Ctrl+O TCPing；Ctrl+R 真连接延迟；Ctrl+T 下载测速；Ctrl+E 混合测试；Enter 激活；Delete/Back 删除；T/U/D/B 置顶/上移/下移/置底；Esc 停止测试。Ctrl+F 在原表格中不是搜索，不能套用通用习惯擅改。
- 双击行为由 `DoubleClick2Activate` 决定激活或编辑；拖排序受 `EnableDragDropSort` 控制。搜索输入框 Enter、表头排序、行选择、拖动阈值、焦点离开后按键作用域应逐项记录。
- 保持选中项和激活项是两个状态；测试/订阅更新/排序不能丢失焦点或错误激活新行；多选动作必须明确作用于所选集合。
- [WPF 表格与上下文菜单](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/v2rayN/Views/ProfilesView.xaml)、[WPF 键鼠事件](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/v2rayN/Views/ProfilesView.xaml.cs)。

**3.3 订阅和分组**

- 分组增删改、启用/禁用、组排序；全部更新/当前组更新；直连更新/经当前代理更新；自动更新间隔与上次更新时间；失败时现有节点及激活项保持策略必须复现。
- URL 与 MoreUrl、多 URL 合并、自定义 User-Agent、**RequestHeaders** 校验/传递、过滤表达式、订阅转换服务和目标、前置/后置代理节点、前置 SOCKS 端口、备注、指定自定义内核。不能只保留名字和 URL。
- `SubscriptionHandler` 具有下载回退路径；迁移时根据实际行为定义可解释状态，不能后台无界重试。自动更新、手动更新与测速/切节点并发需有版本与取消管理。
- [SubItem](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/SubItem.cs)、[SubscriptionHandler](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Handler/SubscriptionHandler.cs)。

**3.4 内核和自定义配置边界**

- `ECoreType` 14 个真实内核：v2fly、Xray、v2fly_v5、mihomo、hysteria、naiveproxy、tuic、sing_box、juicity、hysteria2、brook、overtls、shadowquic、mieru；另有 v2rayN=99 作为自身更新目标，不是代理内核。
- **结构化配置主链**：`CoreConfigHandler` 对 sing-box 调用 `CoreConfigSingboxService`，其它普通结构化节点走 `CoreConfigV2rayService`；应保留 Xray/sing-box 两套生成器，不将其无损性寄托于中间转换成 mihomo YAML。
- `Global.CoreTypes` 只有 Xray、sing_box。本版 `Global.XraySupportConfigType` 包括 VMess/VLESS/SS/Trojan/Hysteria2/WireGuard/SOCKS/HTTP；sing-box 另外含 TUIC/AnyTLS/Naive。表单能力、选择器与内核版本适配必须从这些集合及构建/校验代码反推。
- **完整自定义配置 Custom**：mihomo 使用 `CoreConfigClashService` 特殊处理；其它内核主要复制用户配置到启动文件，再用 `CoreInfoManager` 定义可执行文件、启动参数、版本参数和环境运行。不能把“可选该内核”误写成“所有通用表单均能为其生成配置”。
- mihomo Custom 路径修改 mixed-port/log-level/controller/allow-lan/IPv6，必要时 TUN；保留 mode；支持 Mixin.yaml 普通替换及 prepend-/append-/removed- 合并；YAML anchors 和特殊字符串、REALITY short-id 的字符串类型是回归点。不要直接照搬其 controller 移除 secret 的实现，重构时内部 API 应有明确访问边界。
- 自定义 outbound 与完整配置不是同一功能；有 `IsSingboxEndpoint`、完整配置模板 `Config/TunConfig/AddProxyOnly/ProxyDetour`，前置核心 `configPre.json`，自定义配置是否显示日志以及前置 SOCKS 端口。
- 组策略：LeastPing/Fallback/Random/RoundRobin/LeastLoad，节点与订阅动态子项、过滤、链顺序、环路校验、子节点删除后引用行为。
- 内核进程：启动/停止/重载、版本探测、路径与环境、崩溃退出、stdout/stderr、测试临时进程与主进程隔离、Windows Job、TUN 提权、不同平台进程树回收。
- 更新 UI 当前主要支持自身、Xray、mihomo、sing-box；其余内核枚举不表示内置自动更新支持。自身和 Xray 有 prerelease 选择；sing-box 有兼容范围约束。
- [CoreConfigHandler](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Handler/CoreConfigHandler.cs)、[CoreInfoManager](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Manager/CoreInfoManager.cs)、[Global](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Global.cs)、[Clash 自定义配置生成](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Services/CoreConfig/CoreConfigClashService.cs)。

**3.5 路由、DNS、TUN 与系统代理**

- 路由方案列表增删改、排序、启用/锁定/激活、订阅 URL 更新、规则数、图标、自定义 sing-box ruleset 路径；规则排序不能由容器遍历顺序决定。
- 规则包含 type/port/network/inboundTag/outboundTag/IP/domain/protocol/process/enabled/remarks/ruleType；同时保留 Xray 与 sing-box 的 DomainStrategy，含地区预设（默认/俄罗斯/伊朗）、远程规则模板与 Geo/SRS 来源。
- DNS 有 SimpleDNS 与按内核高级 DNSItem 两条路径；普通/TUN 配置分开，系统 Hosts、公共 Hosts、fake-IP 与全局 fake-IP、范围、阻止 binding/AAAA、直连/远程/bootstrap DNS、直连预期 IP、自由出站/代理/拨号解析策略、serve stale、并行查询、Happy Eyeballs 及高级参数。
- TUN：enable/auto_route/strict_route/stack/MTU/IPv6 地址/ICMP/legacy protect/排除网段/IPv4 和 IPv6 地址；生成配置及提权必须用同一个不可变启动快照，防止开关竞态导致配置和权限不一致。
- 系统代理四态：ForcedClear、ForcedChange、Unchanged、PAC；例外列表、不代理本地地址、自定义协议、PAC 路径、自定义系统代理脚本；Windows/Linux/macOS 实现不同。PAC 监听服务生命周期、退出清理/恢复行为、切换失败和重复启动是核心验收。
- [配置设置模型](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs)、[系统代理入口](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Handler/SysProxy/SysProxyHandler.cs)、[路由实体](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/RoutingItem.cs)。

**3.6 状态、日志、测速和 Clash API**

- TCPing、真实代理延迟、UDP 测试、下载测速、混合测试、快速真实延迟、停止、结果排序和无效节点删除；超时、目标 URL/IP API、并发数、分页批量及批间隔均有设置。
- 代理与直连速率、今日与累计上下行、按节点统计、清空统计、统计总开关、实时显示开关。内部 byte 计数用足够宽整数，刷新批量化，丢帧不丢累计流量。
- 日志级别/核心日志/应用日志分别处理；过滤、自动刷新、清理策略。UI 日志缓冲有上限，暂停滚动与暂停采集区分，重复错误合并不能隐藏第一条。
- Clash API 页面：代理组、当前项、高亮/切换、单节点和全组延迟、排序、自动刷新/间隔；连接列表、过滤、列设置、关闭选中/全部连接。并非启动任意内核都天然具备所有 Clash API 能力，UI 要以 adapter capability 显示可用性。
- 核心源码：`StatisticsManager`、`StatisticsXrayService`、`StatisticsSingboxService`、`SpeedtestService`、`ClashApiManager`、`ClashProxiesViewModel`、`ClashConnectionsViewModel`、`MsgViewModel`。

**3.7 桌面集成、设置与数据**

- 托盘四种系统代理模式、路由、节点数量限制、剪贴板/扫描导入、订阅更新直连/经代理、复制代理环境命令、退出；主窗启动隐藏、关闭到托盘、启动项、主题/强调色/语言/字体/字号/硬件加速、窗口尺寸、列表列状态、隐藏 IP 信息、Mac Dock 设置。
- 全局快捷键：显示窗口、清除代理、设置代理、不改变代理、PAC；和窗口内快捷键分开。Windows UWP 回环豁免、以管理员重启、打开配置所在目录不可漏（Avalonia 中根据 Windows 平台条件显示）。
- 备份与恢复：本地导出/恢复、WebDAV 连接检查/远程备份/远程恢复、远程路径/目录、加密秘密管理与失败回滚；不要只迁移 JSON 而忘 SQLite 和自定义配置/Mixin/证书等引用文件。
- 当前存储：`guiNConfig.json` + `guiNDB.db`；SQLite 初始化表：SubItem、ProfileItem、ServerStatItem、RoutingItem、ProfileExItem、DNSItem、FullConfigTemplateItem、过时 ProfileGroupItem。
- 当前 `AppManager.MigrateProfileExtra` 调用 group V2→V3、普通 profile V2→V3、transport V3→V4。数据导入必须识别版本，迁移后保持 IndexId/Subid/组引用/默认节点/路由引用，报告不支持项并保留原始数据；建议只读导入副本，禁止在旧客户端真实目录原地升级。
- 顶部菜单另外有帮助、检查更新、新更新提醒、重载、推广菜单、退出。若产品决定不保留推广入口，必须作为显式差异记录；不能仍宣称逐项完全一致。
- [存储初始化和迁移](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Manager/AppManager.cs)、[SQLite](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Helper/SqliteHelper.cs)、[状态栏和托盘](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/v2rayN/Views/StatusBarView.xaml)。

**4. 迁移台账应记录的列**

每个字段/命令独立一行：稳定 ID、原版页面与层级、显示文案资源键、原符号、类型/空值/默认值、取值域、验证、平台/核心/版本适用条件、原存储来源、Rust 域模型、Flutter 控件与交互、序列化位置、应用时机（立即/重启核心/重启应用）、导入转换、错误/取消行为、行为 fixture、UI golden、集成测试、完成状态、已知差异及批准理由。

完整性门禁不能只检查“模型字段存在”；至少追踪：菜单/上下文菜单、窗口/弹窗、ViewModel 命令、控件绑定、配置字段、SQLite 实体与迁移、Fmt、核心生成器、平台分支、更新/备份/自动任务、tray、快捷键、外部文件引用、资源本地化。台账分母锁定基线后固定，所有差异有状态，不能用删除行提高完成率。

**5. 模型与命令符号索引（自动提取，随后附加）**

下列仅提取公开源码类与属性/命令符号，不是实现代码；仍需人工识别注释、Obsolete 和运行路径。属性类型和显式 initializer 可作为追踪线索，但最终有效默认值还需核对 `ConfigHandler.LoadConfig`、ViewModel 初始化、平台默认值与核心版本。

**[v2rayN/ServiceLib/Models/Configs/Config.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Configs/Config.cs)**

- **Config**（25 个属性）：`IndexId: string`、`SubIndexId: string`、`CoreBasicItem: CoreBasicItem`、`TunModeItem: TunModeItem`、`KcpItem: KcpItem`、`GrpcItem: GrpcItem`、`RoutingBasicItem: RoutingBasicItem`、`GuiItem: GUIItem`、`MsgUIItem: MsgUIItem`、`UiItem: UIItem`、`ConstItem: ConstItem`、`SpeedTestItem: SpeedTestItem`、`Mux4RayItem: Mux4RayItem`、`Mux4SboxItem: Mux4SboxItem`、`HysteriaItem: HysteriaItem`、`ClashUIItem: ClashUIItem`、`SystemProxyItem: SystemProxyItem`、`WebDavItem: WebDavItem`、`CheckUpdateItem: CheckUpdateItem`、`Fragment4RayItem: Fragment4RayItem?`、`Inbound: List<InItem>`、`GlobalHotkeys: List<KeyEventItem>`、`CoreTypeItem: List<CoreTypeItem>`、`SimpleDNSItem: SimpleDNSItem`、`HappyEyeballs4RayItem: HappyEyeballs4RayItem`。

**[v2rayN/ServiceLib/Models/Configs/ConfigItems.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs)**

- **CoreBasicItem**（9 个属性）：`LogEnabled: bool`、`Loglevel: string`、`DefFingerprint: string`、`DefUserAgent: string`、`SendThrough: string?`、`BindInterface: string?`、`EnableFragment: bool`、`EnableFinalFragment: bool`、`EnableCacheFile4Sbox: bool = true`。
- **InItem**（11 个属性）：`LocalPort: int`、`Protocol: string`、`UdpEnabled: bool`、`SniffingEnabled: bool = true`、`DestOverride: List<string>? = ["http", "tls"]`、`RouteOnly: bool`、`AllowLANConn: bool`、`NewPort4LAN: bool`、`User: string`、`Pass: string`、`SecondLocalPortEnabled: bool`。
- **KcpItem**（6 个属性）：`Mtu: int`、`Tti: int`、`UplinkCapacity: int`、`DownlinkCapacity: int`、`CwndMultiplier: int`、`MaxSendingWindow: int`。
- **GrpcItem**（4 个属性）：`IdleTimeout: int?`、`HealthCheckTimeout: int?`、`PermitWithoutStream: bool?`、`InitialWindowsSize: int?`。
- **GUIItem**（9 个属性）：`AutoRun: bool`、`EnableStatistics: bool`、`DisplayRealTimeSpeed: bool`、`KeepOlderDedupl: bool`、`AutoUpdateInterval: int`、`TrayMenuServersLimit: int = 20`、`EnableHWA: bool = false`、`EnableLog: bool = true`、`RootCertProvider: string?`。
- **MsgUIItem**（2 个属性）：`MainMsgFilter: string?`、`AutoRefresh: bool?`。
- **UIItem**（17 个属性）：`EnableAutoAdjustMainLvColWidth: bool`、`MainGirdHeight1: int`、`MainGirdHeight2: int`、`MainGirdOrientation: EGirdOrientation = EGirdOrientation.Vertical`、`ColorPrimaryName: string?`、`CurrentTheme: string?`、`CurrentLanguage: string`、`CurrentFontFamily: string`、`CurrentFontSize: int`、`EnableDragDropSort: bool`、`DoubleClick2Activate: bool`、`AutoHideStartup: bool`、`Hide2TrayWhenClose: bool`、`MacOSShowInDock: bool`、`MainColumnItem: List<ColumnItem>`、`WindowSizeItem: List<WindowSizeItem>`、`HideColumnIpInfo: bool`。
- **ConstItem**（4 个属性）：`SubConvertUrl: string?`、`GeoSourceUrl: string?`、`SrsSourceUrl: string?`、`RouteRulesTemplateSourceUrl: string?`。
- **KeyEventItem**（5 个属性）：`EGlobalHotkey: EGlobalHotkey`、`Alt: bool`、`Control: bool`、`Shift: bool`、`KeyCode: int?`。
- **CoreTypeItem**（2 个属性）：`ConfigType: EConfigType`、`CoreType: ECoreType`。
- **TunModeItem**（11 个属性）：`EnableTun: bool`、`AutoRoute: bool = true`、`StrictRoute: bool = true`、`Stack: string`、`Mtu: int`、`EnableIPv6Address: bool`、`IcmpRouting: string`、`EnableLegacyProtect: bool = true`、`RouteExcludeAddress: List<string>?`、`IPv4Address: string`、`IPv6Address: string`。
- **SpeedTestItem**（8 个属性）：`SpeedTestTimeout: int`、`SpeedTestUrl: string`、`SpeedPingTestUrl: string`、`MixedConcurrencyCount: int`、`IPAPIUrl: string`、`UdpTestTarget: string`、`SpeedTestPageSize: int?`、`SpeedTestDelayInterval: int?`。
- **RoutingBasicItem**（3 个属性）：`DomainStrategy: string`、`DomainStrategy4Singbox: string`、`RoutingIndexId: string`。
- **ColumnItem**（3 个属性）：`Name: string`、`Width: int`、`Index: int`。
- **Mux4RayItem**（3 个属性）：`Concurrency: int?`、`XudpConcurrency: int?`、`XudpProxyUDP443: string?`。
- **Mux4SboxItem**（3 个属性）：`Protocol: string`、`MaxConnections: int`、`Padding: bool?`。
- **HysteriaItem**（3 个属性）：`UpMbps: int`、`DownMbps: int`、`HopInterval: int = Global.Hysteria2DefaultHopInt`。
- **ClashUIItem**（8 个属性）：`EnableIPv6: bool`、`EnableMixinContent: bool`、`ProxiesSorting: int`、`ProxiesAutoRefresh: bool`、`ProxiesRefreshInterval: int = 2`、`ConnectionsAutoRefresh: bool`、`ConnectionsRefreshInterval: int = 2`、`ConnectionsColumnItem: List<ColumnItem>`。
- **SystemProxyItem**（6 个属性）：`SysProxyType: ESysProxyType`、`SystemProxyExceptions: string`、`NotProxyLocalAddress: bool = true`、`SystemProxyAdvancedProtocol: string`、`CustomSystemProxyPacPath: string?`、`CustomSystemProxyScriptPath: string?`。
- **WebDavItem**（4 个属性）：`Url: string?`、`UserName: string?`、`Password: string?`、`DirName: string?`。
- **CheckUpdateItem**（3 个属性）：`CheckPreReleaseUpdate: bool`、`UpdateViaProxy: bool = true`、`SelectedCoreTypes: List<string>?`。
- **Fragment4RayItem**（6 个属性）：`Packets: string?`、`Lengths: List<string>?`、`Delays: List<string>?`、`MaxSplit: string?`、`Length: string?`、`Interval: string?`。
- **WindowSizeItem**（3 个属性）：`TypeName: string`、`Width: int`、`Height: int`。
- **SimpleDNSItem**（18 个属性）：`UseSystemHosts: bool?`、`AddCommonHosts: bool?`、`FakeIP: bool?`、`GlobalFakeIp: bool?`、`FakeIPRange: string?`、`BlockBindingQuery: bool?`、`BlockAAAAQuery: bool?`、`DirectDNS: string?`、`RemoteDNS: string?`、`BootstrapDNS: string?`、`Strategy4Freedom: string?`、`Strategy4Proxy: string?`、`Strategy4ProxyDial: string?`、`ServeStale: bool?`、`ParallelQuery: bool?`、`Hosts: string?`、`DirectExpectedIPs: string?`、`EnableHappyEyeballs: bool?`。
- **HappyEyeballs4RayItem**（4 个属性）：`TryDelayMs: int?`、`PrioritizeIPv6: bool?`、`Interleave: int?`、`MaxConcurrentTry: int?`。

**[v2rayN/ServiceLib/Models/Entities/ProfileItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/ProfileItem.cs)**

- **ProfileItem**（40 个属性）：`IndexId: string`、`ConfigType: EConfigType`、`CoreType: ECoreType?`、`ConfigVersion: int`、`Subid: string`、`IsSub: bool = true`、`PreSocksPort: int?`、`DisplayLog: bool = true`、`Remarks: string`、`Address: string`、`Port: int`、`Password: string`、`Username: string`、`Network: string`、`HeaderType: string`、`RequestHost: string`、`Path: string`、`StreamSecurity: string`、`AllowInsecure: string`、`Sni: string`、`Alpn: string = string.Empty`、`Fingerprint: string`、`PublicKey: string`、`ShortId: string`、`SpiderX: string`、`Mldsa65Verify: string`、`Extra: string`、`MuxEnabled: bool?`、`Cert: string`、`CertSha: string`、`EchConfigList: string`、`VerifyPeerCertByName: string`、`Finalmask: string`、`ProtoExtra: string`、`TransportExtra: string`、`Ports: string`、`AlterId: int`、`Flow: string`、`Id: string`、`Security: string`。

**[v2rayN/ServiceLib/Models/Entities/ProtocolExtraItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/ProtocolExtraItem.cs)**

- **ProtocolExtraItem**（30 个属性）：`Uot: bool?`、`CongestionControl: string?`、`HttpHeaders: string?`、`AlterId: string?`、`VmessSecurity: string?`、`Flow: string?`、`VlessEncryption: string?`、`SsMethod: string?`、`WgPublicKey: string?`、`WgPresharedKey: string?`、`WgInterfaceAddress: string?`、`WgReserved: string?`、`WgMtu: int?`、`WgDns: string?`、`SalamanderPass: string?`、`UpMbps: int?`、`DownMbps: int?`、`Ports: string?`、`HopInterval: string?`、`Hy2RealmUrl: string?`、`GeckoMinPacketSize: string?`、`GeckoMaxPacketSize: string?`、`InsecureConcurrency: int?`、`NaiveQuic: bool?`、`GroupType: string?`、`ChildItems: string?`、`SubChildItems: string?`、`Filter: string?`、`MultipleLoad: EMultipleLoad?`、`IsSingboxEndpoint: bool?`。

**[v2rayN/ServiceLib/Models/Entities/TransportExtraItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/TransportExtraItem.cs)**

- **TransportExtraItem**（11 个属性）：`RawHeaderType: string?`、`Host: string?`、`Path: string?`、`XhttpMode: string?`、`XhttpExtra: string?`、`GrpcAuthority: string?`、`GrpcServiceName: string?`、`GrpcMode: string?`、`KcpHeaderType: string?`、`KcpSeed: string?`、`KcpMtu: int?`。

**[v2rayN/ServiceLib/Models/Entities/SubItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/SubItem.cs)**

- **SubItem**（17 个属性）：`Id: string`、`Remarks: string`、`Url: string`、`MoreUrl: string`、`Enabled: bool = true`、`UserAgent: string = string.Empty`、`RequestHeaders: string?`、`Sort: int`、`Filter: string?`、`AutoUpdateInterval: int`、`UpdateTime: long`、`ConvertTarget: string?`、`PrevProfile: string?`、`NextProfile: string?`、`PreSocksPort: int?`、`Memo: string?`、`CustomCoreType: ECoreType?`。

**[v2rayN/ServiceLib/Models/Entities/ProfileGroupItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/ProfileGroupItem.cs)**

- **ProfileGroupItem**（5 个属性）：`IndexId: string`、`ChildItems: string`、`SubChildItems: string?`、`Filter: string?`、`MultipleLoad: EMultipleLoad = EMultipleLoad.LeastPing`。

**[v2rayN/ServiceLib/Models/Entities/ProfileExItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/ProfileExItem.cs)**

- **ProfileExItem**（6 个属性）：`IndexId: string`、`Delay: int`、`Speed: decimal`、`Sort: int`、`Message: string?`、`IpInfo: string?`。

**[v2rayN/ServiceLib/Models/Entities/ServerStatItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/ServerStatItem.cs)**

- **ServerStatItem**（6 个属性）：`IndexId: string`、`TotalUp: long`、`TotalDown: long`、`TodayUp: long`、`TodayDown: long`、`DateNow: long`。

**[v2rayN/ServiceLib/Models/Entities/DNSItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/DNSItem.cs)**

- **DNSItem**（9 个属性）：`Id: string`、`Remarks: string`、`Enabled: bool = false`、`CoreType: ECoreType`、`UseSystemHosts: bool`、`NormalDNS: string?`、`TunDNS: string?`、`DomainStrategy4Freedom: string?`、`DomainDNSAddress: string?`。

**[v2rayN/ServiceLib/Models/Entities/RoutingItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/RoutingItem.cs)**

- **RoutingItem**（13 个属性）：`Id: string`、`Remarks: string`、`Url: string`、`RuleSet: string`、`RuleNum: int`、`Enabled: bool = true`、`Locked: bool`、`CustomIcon: string`、`CustomRulesetPath4Singbox: string`、`DomainStrategy: string`、`DomainStrategy4Singbox: string`、`Sort: int`、`IsActive: bool`。

**[v2rayN/ServiceLib/Models/Entities/RulesItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/RulesItem.cs)**

- **RulesItem**（13 个属性）：`Id: string`、`Type: string?`、`Port: string?`、`Network: string?`、`InboundTag: List<string>?`、`OutboundTag: string?`、`Ip: List<string>?`、`Domain: List<string>?`、`Protocol: List<string>?`、`Process: List<string>?`、`Enabled: bool = true`、`Remarks: string?`、`RuleType: ERuleType?`。

**[v2rayN/ServiceLib/Models/Entities/FullConfigTemplateItem.cs](https://github.com/2dust/v2rayN/blob/7d6a967c18c697f28dc6917122ed3a4993fcf336/v2rayN/ServiceLib/Models/Entities/FullConfigTemplateItem.cs)**

- **FullConfigTemplateItem**（8 个属性）：`Id: string`、`Remarks: string`、`Enabled: bool = false`、`CoreType: ECoreType`、`Config: string?`、`TunConfig: string?`、`AddProxyOnly: bool? = false`、`ProxyDetour: string?`。

**6. ViewModel 命令索引（声明层扫描）**

以下不包括 view code-behind 事件、定时任务和所有 public 方法，不能作为全部功能的唯一分母。

- **AddGroupServerViewModel.cs**（7 个声明）：`AddCmd`、`RemoveCmd`、`MoveTopCmd`、`MoveUpCmd`、`MoveDownCmd`、`MoveBottomCmd`、`SaveCmd`。

- **AddServer2ViewModel.cs**（3 个声明）：`BrowseServerCmd`、`EditServerCmd`、`SaveServerCmd`。

- **AddServerViewModel.cs**（3 个声明）：`FetchCertCmd`、`FetchCertChainCmd`、`SaveCmd`。

- **BackupAndRestoreViewModel.cs**（3 个声明）：`RemoteBackupCmd`、`RemoteRestoreCmd`、`WebDavCheckCmd`。

- **CheckUpdateViewModel.cs**（2 个声明）：`CheckUpdateCmd`、`CheckOnlyCmd`。

- **ClashConnectionsViewModel.cs**（2 个声明）：`ConnectionCloseCmd`、`ConnectionCloseAllCmd`。

- **ClashProxiesViewModel.cs**（4 个声明）：`ProxiesReloadCmd`、`ProxyDelayTestCmd`、`GroupProxiesDelayTestCmd`、`ProxiesSelectActivityCmd`。

- **DNSSettingViewModel.cs**（3 个声明）：`SaveCmd`、`ImportDefConfig4V2rayCompatibleCmd`、`ImportDefConfig4SingboxCompatibleCmd`。

- **FullConfigTemplateViewModel.cs**（1 个声明）：`SaveCmd`。

- **GlobalHotkeySettingViewModel.cs**（1 个声明）：`SaveCmd`。

- **MainWindowViewModel.cs**（35 个声明）：`AddVmessServerCmd`、`AddVlessServerCmd`、`AddShadowsocksServerCmd`、`AddSocksServerCmd`、`AddHttpServerCmd`、`AddTrojanServerCmd`、`AddHysteria2ServerCmd`、`AddTuicServerCmd`、`AddWireguardServerCmd`、`AddAnytlsServerCmd`、`AddNaiveServerCmd`、`AddCustomServerCmd`、`AddCustomOutboundServerCmd`、`AddPolicyGroupServerCmd`、`AddProxyChainServerCmd`、`AddServerViaClipboardCmd`、`AddServerViaScanCmd`、`AddServerViaImageCmd`、`SubSettingCmd`、`SubUpdateCmd`、`SubUpdateViaProxyCmd`、`SubGroupUpdateCmd`、`SubGroupUpdateViaProxyCmd`、`OptionSettingCmd`、`RoutingSettingCmd`、`DNSSettingCmd`、`FullConfigTemplateCmd`、`GlobalHotkeySettingCmd`、`RebootAsAdminCmd`、`ClearServerStatisticsCmd`、`OpenTheFileLocationCmd`、`RegionalPresetDefaultCmd`、`RegionalPresetRussiaCmd`、`RegionalPresetIranCmd`、`ReloadCmd`。

- **OptionSettingViewModel.cs**（1 个声明）：`SaveCmd`。

- **ProfilesSelectViewModel.cs**（1 个声明）：`SaveCmd`。

- **ProfilesViewModel.cs**（29 个声明）：`EditServerCmd`、`RemoveServerCmd`、`RemoveDuplicateServerCmd`、`CopyServerCmd`、`SetDefaultServerCmd`、`ShareServerCmd`、`GenGroupAllServerCmd`、`GenGroupRegionServerCmd`、`MoveTopCmd`、`MoveUpCmd`、`MoveDownCmd`、`MoveBottomCmd`、`MoveToGroupCmd`、`MixedTestServerCmd`、`TcpingServerCmd`、`RealPingServerCmd`、`UdpTestServerCmd`、`SpeedServerCmd`、`SortServerResultCmd`、`RemoveInvalidServerResultCmd`、`FastRealPingCmd`、`Export2ClientConfigCmd`、`Export2ClientConfigClipboardCmd`、`Export2ShareUrlCmd`、`Export2ShareUrlBase64Cmd`、`Export2InnerUriCmd`、`AddSubCmd`、`EditSubCmd`、`DeleteSubCmd`。

- **RoutingRuleDetailsViewModel.cs**（2 个声明）：`SelectProfileCmd`、`SaveCmd`。

- **RoutingRuleSettingViewModel.cs**（11 个声明）：`RuleAddCmd`、`ImportRulesFromFileCmd`、`ImportRulesFromClipboardCmd`、`ImportRulesFromUrlCmd`、`RuleRemoveCmd`、`RuleExportSelectedCmd`、`MoveTopCmd`、`MoveUpCmd`、`MoveDownCmd`、`MoveBottomCmd`、`SaveCmd`。

- **RoutingSettingViewModel.cs**（4 个声明）：`RoutingAdvancedAddCmd`、`RoutingAdvancedRemoveCmd`、`RoutingAdvancedSetDefaultCmd`、`RoutingAdvancedImportRulesCmd`。

- **StatusBarViewModel.cs**（12 个声明）：`AddServerViaClipboardCmd`、`AddServerViaScanCmd`、`SubUpdateCmd`、`SubUpdateViaProxyCmd`、`CopyProxyCmdToClipboardCmd`、`NotifyLeftClickCmd`、`ShowWindowCmd`、`HideWindowCmd`、`SystemProxyClearCmd`、`SystemProxySetCmd`、`SystemProxyNothingCmd`、`SystemProxyPacCmd`。

- **SubEditViewModel.cs**（3 个声明）：`SelectPrevProfileCmd`、`SelectNextProfileCmd`、`SaveCmd`。

- **SubSettingViewModel.cs**（4 个声明）：`SubAddCmd`、`SubDeleteCmd`、`SubEditCmd`、`SubShareCmd`。


