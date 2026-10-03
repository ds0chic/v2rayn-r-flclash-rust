# FIX-16 字段消费矩阵 (OptionSettingWindow)

范围：`apps/desktop/lib/features/settings/option_setting_window.dart` 当前可见字段。
上游基准：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN` commit `7d6a967`
（WPF `v2rayN/Views/OptionSettingWindow.xaml`、Avalonia `v2rayN.Desktop/Views/OptionSettingWindow.axaml`、
`ServiceLib/ViewModels/OptionSettingViewModel.cs`）。字段集以 `settings-items.json` 的
SET-16/17/19/20 与 `domain::settings_timing::FIELD_TIMING`（180 项）为准，不凭记忆。

## 分组对照（TabItem → TabItem）

| 上游 TabItem | 本窗口 tab | 结论 |
|---|---|---|
| `TbSettingsCore` | 核心基础 | 对齐；入站/日志/指纹/绑定/Mux/Hysteria/Fragment 全部归入本页 |
| `TbSettingsCoreKcp`（第 619 行起整段注释） | 核心基础「历史保留」 | 上游 WPF/Avalonia 均未暴露；6 个 KcpItem 字段保留可编辑，不计缺口 |
| `TbSettingsN` | 显示 | 对齐；UI/托盘/字体语言/测速/资源证书全部归入本页 |
| `TbSettingsSystemproxy` | 系统代理 | 对齐 |
| `TbSettingsTunMode` | Tun 模式 | 对齐 |
| `TbSettingsCoreType` | 内核类型 | 对齐（ConfigType 1/2/3/4/5/6/7/9 共 8 个下拉，同上游） |

上游同一窗口不暴露、被误拆成独立页而后合并回「历史保留」段的字段：
`SimpleDNSItem` FakeIP/HappyEyeballs（上游属 `DNSSettingViewModel`）、`ClashUIItem`（上游窗口无控件）。
`GuiItem.EnableLog` 上游窗口无控件，未造控件，仅登记。

## 状态口径

- `verified`：本卡真实跑了 UI→保存→重开→实际消费者（locale/主题）。
- `consumed`：控件存在，写入经 `saveDocument`→Rust 设置引擎，`settings_timing` 消费者已知（内核/应用重启时读取）；本环境未执行内核/平台动作。
- `registered`：持久化与 apply_timing 已在台账，但 Flutter 侧真实消费者尚未接线 → FIX-16B/C。
- `blocked`：平台效果需 T13/网络/权限，本环境不执行。
- `preserved_only`：上游本窗口不暴露的历史保留字段，保留可编辑，不作为缺口。

## 核心基础

| 字段 (group.key) | UI | 类型 | 默认 | 校验 | 存储 | apply_timing (消费者) | 状态 |
|---|---|---|---|---|---|---|---|
| Inbound.LocalPort | 本地端口 | number | 10808 | 1..65535 | guiNConfig.Inbound[0] | restart_core | consumed |
| Inbound.SecondLocalPortEnabled | 第二本地端口 | checkbox | false | - | 同上 | restart_core | consumed |
| Inbound.UdpEnabled | UDP 转发 | checkbox | true | - | 同上 | restart_core | consumed |
| Inbound.SniffingEnabled | 嗅探 | checkbox | true | - | 同上 | restart_core | consumed |
| Inbound.DestOverride | http/tls/quic chips | list | [http,tls] | - | 同上 | restart_core | consumed |
| Inbound.RouteOnly | 仅路由 | checkbox | false | - | 同上 | restart_core | consumed |
| Inbound.AllowLANConn | 允许局域网 | checkbox | false | - | 同上 | restart_core | consumed |
| Inbound.NewPort4LAN | 局域网新端口 | checkbox | false | 依赖 AllowLANConn | 同上 | restart_core | consumed |
| Inbound.User / Pass | 用户名/密码 | text | '' | 依赖上两项 | 同上 | restart_core | consumed |
| CoreBasicItem.LogEnabled | 启用日志 | checkbox | false | - | CoreBasicItem | restart_core | consumed |
| CoreBasicItem.Loglevel | 日志等级 | dropdown | warning | - | 同上 | restart_core | consumed |
| CoreBasicItem.DefFingerprint | 默认指纹 | text | null | - | 同上 | restart_core | consumed |
| CoreBasicItem.DefUserAgent | 默认 UA | text | null | - | 同上 | restart_core | consumed |
| CoreBasicItem.SendThrough | SendThrough | text | null | - | 同上 | restart_core | consumed |
| CoreBasicItem.BindInterface | BindInterface | text | null | - | 同上 | restart_core | consumed |
| CoreBasicItem.EnableCacheFile4Sbox | sing-box 缓存 | checkbox | true | - | 同上 | restart_core | consumed |
| CoreBasicItem.EnableFragment | 启用分片 | checkbox | false | - | 同上 | restart_core | consumed |
| CoreBasicItem.EnableFinalFragment | 最终分片 | checkbox | false | - | 同上 | restart_core | consumed |
| Mux4RayItem.Concurrency | Ray 并发 | number | 8 | - | Mux4RayItem | restart_core | consumed |
| Mux4RayItem.XudpConcurrency | Ray XUDP 并发 | number | 16 | - | 同上 | restart_core | consumed |
| Mux4RayItem.XudpProxyUDP443 | Ray XUDP 443 | dropdown | reject | - | 同上 | restart_core | consumed |
| Mux4SboxItem.Protocol | sing-box 协议 | dropdown | h2mux | - | Mux4SboxItem | restart_core | consumed |
| HysteriaItem.UpMbps/DownMbps | 上下行 Mbps | number | 100 | - | HysteriaItem | restart_core | consumed |
| HysteriaItem.HopInterval | Hop 间隔 | number | 30 | - | 同上 | restart_core | consumed（上游窗口未暴露，保留） |
| Fragment4RayItem.Packets | 分片包 | dropdown | tlshello | - | Fragment4RayItem | restart_core | consumed |
| Fragment4RayItem.Lengths | 长度 | text(逗号) | [50-100] | 正整数或 a-b | 同上 | restart_core | consumed |
| Fragment4RayItem.Delays | 延迟 | text(逗号) | [10-20] | 正整数或 a-b | 同上 | restart_core | consumed |
| Fragment4RayItem.MaxSplit | MaxSplit | text | 0 | 0..10000 | 同上 | restart_core | consumed |
| KcpItem.{Mtu,Tti,UplinkCapacity,DownlinkCapacity,CwndMultiplier,MaxSendingWindow} | KCP 6 项 | number | 1350/50/12/100/1/2097152 | - | KcpItem | restart_core | consumed（UI 属上游注释页，preserved_only 入口） |

## 显示

| 字段 | UI | 类型 | 默认 | 存储 | apply_timing (消费者) | 状态 |
|---|---|---|---|---|---|---|
| GuiItem.EnableStatistics | 启用统计 | checkbox | false | GuiItem | restart_app → uiShell.showStatistics | consumed |
| GuiItem.DisplayRealTimeSpeed | 实时速率 | checkbox | false | 同上 | restart_app | consumed |
| GuiItem.KeepOlderDedupl | 保留旧重组 | checkbox | false | 同上 | save（subscriptions merge） | consumed |
| GuiItem.TrayMenuServersLimit | 托盘节点上限 | number | 20 | 同上 | save → uiShell | consumed |
| GuiItem.AutoUpdateInterval | 自动更新间隔 | number | 0 | 同上 | immediate → updater | consumed |
| GuiItem.AutoRun | 开机自启 | checkbox | false | 同上 | immediate → AutoStartupHandler（FIX-08 已实测） | consumed |
| GuiItem.EnableHWA | 硬件加速 | checkbox | false | 同上 | restart_app | consumed（持久化）；平台渲染效果 not_applicable（Flutter 无 WPF 渲染器） |
| GuiItem.RootCertProvider | 根证书来源 | dropdown(system/chrome/mozilla) | system | 同上 | immediate → CertPemManager（T13） | consumed（持久化）；证书安装 blocked |
| UiItem.CurrentFontFamily | 字体族 | text | null | UiItem | restart_app → app.dart buildAppTheme | **verified** |
| UiItem.CurrentFontSize | 字号 | number | 0 | 同上 | immediate → app.dart buildAppTheme | **verified** |
| UiItem.CurrentLanguage | 语言 | dropdown(9) | zh-Hans | 同上 | restart_app → app.dart MaterialApp.locale | **verified** |
| UiItem.EnableAutoAdjustMainLvColWidth | 自动列宽 | checkbox | false | 同上 | immediate → uiShell | consumed |
| UiItem.HideColumnIpInfo | 隐藏 IP | checkbox | false | 同上 | immediate → uiShell | consumed |
| UiItem.DoubleClick2Activate | 双击激活 | checkbox | false | 同上 | immediate → profilesController | consumed |
| UiItem.MainGirdOrientation | 主界面布局 | dropdown | 1 | 同上 | restart_app → uiShell.layout | consumed |
| UiItem.Hide2TrayWhenClose | 关闭到托盘 | checkbox | false | 同上 | immediate → desktop integration | consumed |
| UiItem.AutoHideStartup | 启动隐藏 | checkbox | false | 同上 | immediate | consumed |
| UiItem.EnableDragDropSort | 拖放排序 | checkbox | false | 同上 | restart_app | consumed |
| UiItem.MacOSShowInDock | Dock 显示 | checkbox | false | 同上 | restart_app | preserved_only（仅 macOS 显示，Windows 隐藏） |
| SpeedTestItem.SpeedTestTimeout/MixedConcurrencyCount/SpeedTestUrl/SpeedPingTestUrl/UdpTestTarget/IPAPIUrl | 测速 6 项 | number/text | 10/10/url/url/ntp/ null | SpeedTestItem | immediate → configureSpeedTest | consumed |
| ConstItem.SubConvertUrl | 订阅转换 | text | null | ConstItem | save → `dns::effective_sub_convert_url`（空回退 `Global.SubConvertUrls[0]`） | consumed（解析器已接线；订阅下载链在 `crates/subscriptions`，登记为 FIX-16B 缺口） |
| ConstItem.GeoSourceUrl | Geo 来源 | text | null | 同上 | save → `dns::effective_geo_source`（空回退 `Global.GeoUrl`） | consumed（解析器已接线；`.dat` 下载在 update 模块，登记为 FIX-16B 缺口） |
| ConstItem.SrsSourceUrl | SRS 来源 | text | null | 同上 | save → `dns::effective_srs_source` → sing-box `route.rule_set[].url` | **verified**（生成输入断言实际使用） |
| ConstItem.RouteRulesTemplateSourceUrl | 路由规则来源 | text | null | 同上 | save → `dns::effective_routing_template_source`（空=内置路由） | consumed（解析器已接线；外部模板抓取消费者登记为 FIX-16B 缺口） |
| SimpleDNSItem.FakeIP / GlobalFakeIp / EnableHappyEyeballs | DNS 历史段 | checkbox | false/true/false | SimpleDNSItem | restart_core | preserved_only（上游 DNS 窗口） |
| HappyEyeballs4RayItem.{TryDelayMs,PrioritizeIPv6,Interleave,MaxConcurrentTry} | DNS 历史段 | number/checkbox | 250/false/1/4 | HappyEyeballs4RayItem | restart_core | preserved_only（上游 DNS 窗口） |
| ClashUIItem.EnableIPv6/EnableMixinContent | Clash 历史段 | checkbox | false | ClashUIItem | restart_core | preserved_only（上游窗口无控件；Rust 侧 `grep` 无消费者，config_codegen 未接，登记后续） |
| ClashUIItem.ProxiesSorting/ProxiesAutoRefresh/ProxiesRefreshInterval | Clash 历史段 | dropdown/checkbox/number | 0/false/2 | 同上 | immediate | **verified**（FIX-16C：`proxies_view` 读 `clashUiConfigProvider`；排序/自动刷新 widget 测试） |
| ClashUIItem.ConnectionsAutoRefresh/ConnectionsRefreshInterval | Clash 历史段 | checkbox/number | false/2 | 同上 | immediate | **verified**（FIX-16C：`connections_view` 读同一 provider；自动刷新 widget 测试） |
| ClashUIItem.ConnectionsColumnItem | Clash 历史段（无控件） | list | [] | 同上 | save | registered（列状态源已统一：`ColumnLayout`→`ui_state.json` 的 `clash_connections_column_layout`；`connections_view` 消费者未接线，FIX-16D→FIX-16C-2） |

## 系统代理

| 字段 | UI | 类型 | 默认 | apply_timing | 状态 |
|---|---|---|---|---|---|
| SystemProxyItem.SysProxyType | 系统代理类型 | dropdown | 0 | immediate | consumed（持久化）；写系统代理 blocked（T13） |
| SystemProxyItem.NotProxyLocalAddress | 忽略本地地址 | checkbox | true | immediate | blocked（T13 WinINET） |
| SystemProxyItem.SystemProxyExceptions | 例外列表 | text | 本地网段 | immediate | blocked（T13） |
| SystemProxyItem.SystemProxyAdvancedProtocol | 高级协议 | text | null | immediate | blocked（T13） |
| SystemProxyItem.CustomSystemProxyPacPath | PAC 路径 | text | null | immediate | blocked（T13） |
| SystemProxyItem.CustomSystemProxyScriptPath | PAC 脚本路径 | text | null | immediate | blocked（T13，上游 Avalonia） |

## Tun 模式

| 字段 | UI | 类型 | 默认 | apply_timing | 状态 |
|---|---|---|---|---|---|
| TunModeItem.EnableTun | 启用 Tun | checkbox | false | restart_core | consumed（持久化）；TUN 启动 blocked（T13） |
| TunModeItem.AutoRoute | 自动路由 | checkbox | true | restart_core | blocked（T13） |
| TunModeItem.StrictRoute | 严格路由 | checkbox | true | restart_core | blocked（T13） |
| TunModeItem.Stack | 协议栈 | dropdown | null | restart_core | blocked（T13） |
| TunModeItem.Mtu | MTU | number | 9000 | restart_core | blocked（T13） |
| TunModeItem.IcmpRouting | ICMP 路由 | dropdown | rule | restart_core | blocked（T13） |
| TunModeItem.EnableIPv6Address | IPv6 地址 | checkbox | false | restart_core | blocked（T13） |
| TunModeItem.EnableLegacyProtect | 旧版保护 | checkbox | true | restart_core | blocked（T13） |
| TunModeItem.RouteExcludeAddress | 路由排除 | text(逗号) | null | restart_core | blocked（T13） |
| TunModeItem.IPv4Address / IPv6Address | 隧道地址 | text | null | restart_core | blocked（T13） |

## 内核类型

| 字段 | UI | 类型 | 默认 | apply_timing | 状态 |
|---|---|---|---|---|---|
| CoreTypeItem.ConfigType 1/2/3/4/5/6/7/9 → CoreType | 8 个下拉 | dropdown | Xray(2) | restart_core | consumed |

## 窗口尺寸 / 列宽状态源（FIX-16D，非本窗口控件）

本窗口不暴露以下字段的控件；FIX-16D 将它们统一到一个按窗口 `TypeName`、按表名键入的可迁移状态源（`ui_state.json`），并由 Rust `application::settings` 提供 `guiNConfig` 树权威 upsert。

| 字段 | 层 | 状态源（本仓） | apply_timing | 状态 |
|---|---|---|---|---|
| UiItem.WindowSizeItem（FLD-CFG-082） | 窗口几何列表 | `window_geometry`（按 `TypeName`） | immediate | implemented（状态源）；窗口绑定 registered（FIX-16D-2） |
| WindowSizeItem.TypeName/Width/Height（FLD-CFG-156/157/158） | 窗口几何行 | `WindowGeometry{typeName,width,height}` + `application::settings::{get,save}_window_size` | immediate | verified（单元/widget 保存重开） |
| UiItem.MainGirdHeight1/2（FLD-CFG-068/069） | 主布局星值 | `WindowGeometry.mainGridHeight1/2` + `save_main_grid_height` | immediate | verified（单元/widget） |
| UiItem.MainColumnItem（FLD-CFG-080，LAY-PROFILES-003） | 节点表列 | `ColumnLayout`→`column_layout`（稳定键，不用本地化标题） | immediate | verified（列宽编辑器保存重开） |
| ClashUIItem.ConnectionsColumnItem（FLD-CFG-136，LAY-CLASHCN-002） | 连接表列 | `ColumnLayout`→`clash_connections_column_layout` | immediate | implemented（状态源）；消费者 registered（FIX-16C-2） |
| 旧扁平键 `window` / `column_widths` | 迁移源 | `migrateLegacyUiState()` 幂等折叠，键保留不丢 | — | verified（迁移测试） |

迁移/备份：`loadDocument()` 即备份文档，`saveDocument()` 可恢复；`meta.schema_version=2` 记录口径。

窗口「位置」（Left/Top）：上游 `WindowBase` 不持久化位置，恢复时按工作区居中（`WindowBase.cs:23-24`），故 `not_applicable`；本状态源同样只存/还 `Width/Height`。

## 统计

- 可见控件：103（核心 36 / 显示 42 / 系统代理 6 / Tun 11 / 内核类型 8；FIX-16C 在显示页 Clash 历史段新增 Connections 自动刷新/间隔 2 项）。
- `verified`（实测消费者）：9（FIX-16 的 CurrentLanguage / CurrentFontFamily / CurrentFontSize / SrsSourceUrl 4 项 + FIX-16C 的 ProxiesSorting / ProxiesAutoRefresh / ProxiesRefreshInterval / ConnectionsAutoRefresh / ConnectionsRefreshInterval 5 项）。
- `consumed`（含 verified，持久化 + 领域消费者已知，本环境未跑内核）：76（FIX-16B 3 个源解析消费者 + FIX-16C 5 个 Clash UI 消费者）。
- `registered`（待接线，已登记后续卡）：1（ClashUIItem.ConnectionsColumnItem；Clash Proxies*/Connections* 已转 verified）。
- `blocked`（平台/网络，本环境不执行）：17（系统代理 6 + Tun 11）。
- `preserved_only`（上游本窗口不暴露）：9（DNS 见上；EnableIPv6/EnableMixinContent 保留但 Rust `grep` 无消费者，登记后续；KCP 6 项入口保留但主状态计 consumed）。
- 未伪造缺口：KCP 页（上游注释）、FakeIP/HappyEyeballs（上游 DNS 窗口）、ClashUIItem（上游无控件）、WPF 四个额外主题（`ThemeSettingView` 未暴露）均如实登记，不造控件。
- FIX-16D（不计入上述可见控件分母，为设置窗口外的窗口/表格状态源）：`WindowSizeItem`/`MainGirdHeight1/2`（4 行）与列状态（`MainColumnItem`/`ConnectionsColumnItem`，2 行）统一到 `ui_state.json`，新增 `verified` 4 项（TypeName 行、星值、节点列宽、旧键迁移）、`implemented` 2 项（WindowSizeItem 列表状态源、连接表列状态源）；消费者绑定登记 FIX-16D-2 / FIX-16C-2。
