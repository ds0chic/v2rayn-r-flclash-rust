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
| ClashUIItem.EnableIPv6/EnableMixinContent | Clash 历史段 | checkbox | false | ClashUIItem | restart_core | preserved_only（上游窗口无控件） |
| ClashUIItem.ProxiesSorting/ProxiesAutoRefresh/ProxiesRefreshInterval | Clash 历史段 | number/checkbox | 0/false/2 | 同上 | immediate | registered → FIX-16C（Clash UI 消费者未接） |

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

## 统计

- 可见控件：101（核心 36 / 显示 40 / 系统代理 6 / Tun 11 / 内核类型 8）。
- `verified`（本卡实测消费者）：4（CurrentLanguage / CurrentFontFamily / CurrentFontSize / SrsSourceUrl→sing-box rule_set URL）。
- `consumed`（含 verified，持久化 + 领域消费者已知，本环境未跑内核）：71（FIX-16B 新增 SubConvertUrl/GeoSourceUrl/RouteRulesTemplateSourceUrl 解析消费者）。
- `registered`（待接线，已登记后续卡）：3（仅 Clash Proxies* 3 项；FIX-16B 的 4 个源字段已接线到 `dns::effective_*` 解析器）。
- `blocked`（平台/网络，本环境不执行）：17（系统代理 6 + Tun 11）。
- `preserved_only`（上游本窗口不暴露）：9（DNS 4+1+1+... 见上；另 KCP 6 项入口保留但主状态计 consumed）。
- 未伪造缺口：KCP 页（上游注释）、FakeIP/HappyEyeballs（上游 DNS 窗口）、ClashUIItem（上游无控件）、WPF 四个额外主题（`ThemeSettingView` 未暴露）均如实登记，不造控件。
