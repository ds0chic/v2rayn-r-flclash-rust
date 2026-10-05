# R4-30 未提取字符串清单（不静默遗漏）

本清单登记本轮**未迁入** `lib/shared/l10n/strings.g.dart` 的用户可见文字，供后续按上游 ResUI 键分批提取。卡允许“分批提取”，但要求登记，故此处逐文件列出。已迁入的最低集（菜单/主窗口/工具栏/状态栏/错误反馈/设置窗口外壳 tab 与按钮/路由与 DNS 顶层与主操作/区域预设/校验与状态消息）不在本清单内。

提取时请从冻结 `ResUI*.resx` 取键，勿自造；下表“建议 ResUI 键”为对照上游 `OptionSettingWindow.xaml` / `DNSSettingWindow.xaml` / `RoutingSettingWindow.xaml` 的推断，实施前需核对。

## apps/desktop/lib/features/settings/option_setting_window.dart（正文逐字段标签，约 119 条）

| 当前中文（冻结上游文案） | 建议 ResUI 键 |
|---|---|
| 本地监听 / 本地混合监听端口 | TbSettingsSocksPort |
| 开启第二个本地监听端口 | TbSettingsSecondLocalPort |
| 开启 UDP | TbSettingsUdpEnabled |
| 开启流量探测 | TbSettingsSniffingEnabled |
| 仅限路由 (routeOnly) | TbSettingsRouteOnly |
| 允许来自局域网的连接 | TbSettingsAllowLAN |
| 为局域网开启新的端口 | TbSettingsNewPort4LAN |
| 认证用户名 / 认证密码 | TbSettingsUser / TbSettingsPass |
| 日志与指纹 / 启用日志存到文件 / 日志等级 | TbSettingsLogEnabled / TbSettingsLogLevel |
| 默认 TLS 指纹 (fingerprint) | TbSettingsDefFingerprint |
| 用户代理 (User-Agent) | TbSettingsDefUserAgent |
| 本地出站地址 (SendThrough) / 绑定网口 | TbSettingsBoundInterface |
| 多路复用 (Mux) / Xray Mux concurrency 等 | TbSettingsMux4RayConcurrency 等 |
| sing-box Mux 多路复用协议 | TbSettingsMux4SboxProtocol |
| 启用 sing-box (规则集文件) 的缓存文件 | TbSettingsEnableCacheFile4Sbox |
| Hysteria2 / 上行 Mbps / 下行 Mbps / Hop 间隔 | TbSettingsHysteria2* |
| 分片 (Fragment) 各字段 | TbEnableFinalFragment / TbSettingsFragment* |
| 历史保留（KCP 页）各字段 | 原版注释，无 ResUI 键 |
| 显示 / 启用流量统计 (需重启) | TbSettingsStatistics |
| 显示实时速度 (需重启) | TbSettingsDisplayRealTimeSpeed |
| 去重时保留序号较小的项 | TbSettingsKeepOlderDedupl |
| 自动调整配置列宽在更新订阅后 | TbSettingsEnableAutoAdjustMainLvColWidth |
| 隐藏 IP 信息 | TbSettingsHide2Info |
| 主界面双击设为活动 | TbSettingsDoubleClick2Activate |
| 主界面布局方向 (需重启) | TbSettingsMainGirdOrientation |
| 窗口与托盘 / 关闭窗口时隐藏至托盘 | TbSettingsCloseToTray |
| 启动后隐藏窗口 | TbSettingsAutoHideStartup |
| macOS 在 Dock 栏中显示 (需重启) | TbSettingsShowInTaskbar |
| 启用配置拖放排序 (需重启) | TbSettingsDragDropSort |
| 托盘右键菜单配置展示数量限制 | TbSettingsTrayMenuServersLimit |
| 自动更新 Geo 文件的间隔 (小时) | TbSettingsAutoUpdateInterval |
| 开机启动 (可能会不成功) | TbSettingsStartBoot |
| 字体与语言 / 当前字体 (需重启) | TbSettingsFontFamily |
| 字体大小 | TbSettingsFontSize |
| 语言 (需重启) | TbSettingsLanguage |
| 测速各字段 | TbSpeedtestSingleTimeout / TbSpeedtestPageUrl / TbRealPingUrl / TbUdpTestUrl / TbConnectionInfoUrl / TbSpeedtestThreads |
| 资源与证书各字段 | TbSettingsSubConvert / TbGeoSiteUrl4* / TbRootCertificateProvider / TbSettingsHardwareAcceleration |
| 历史保留（FakeIP/HappyEyeballs）各字段 | TbEnableHappyEyeballs 等 |
| 历史保留（Clash UI）各字段 | TbSettingsClashUi* |
| 系统代理设置页各字段 | TbSettingsSystemproxy* / TbSettingsNotProxyLocalAddress / TbSettingsException |
| Tun 模式页各字段 | TbEnableTunAs / TbRouting* / TbSettingsTun* |

注：这些标签已能显示（默认中文来自冻结 UI 文案，切换语言时未随之变化），属“产品文字未全部资源化”。本卡最低集已含设置窗口的 Tab 与 确定/取消，逐字段标签留待后续卡。

## apps/desktop/lib/features/routing/routing_windows.dart（列表列头/编辑器/弹窗，约 90 条）

已迁入：窗口标题、工具栏（添加规则集/一键导入规则集）、预定义规则集列表块标题、空列表、上下文菜单（添加/移除/设为活动/全选/一键导入）、确定/取消/关闭/保存。
未迁入（示例）：列头（序号/备注/排序/名称/自定义图标等）、规则编辑器字段（出站/域名/IP/进程/协议/端口/入站/本地端口/反选/域名策略…）、删除确认文案、导入 URL 对话框文案、`RoutingRulesetWindow` 与 `RoutingRuleDetailsWindow` 内部标题。建议键见 `menuRule*` / `TbRouting*` / `TbRule*`。

## apps/desktop/lib/features/routing/dns_window.dart（正文标签，约 40 条）

已迁入：窗口标题、四个 Tab、区域预设（名称/应用预设）、取消/应用/保存、状态消息、直连/远端/引导/FakeIP 段标签、禁用了提示。
未迁入（示例）：基础/高级/自定义页其余开关与字段（使用系统 hosts/附加公共 hosts/FakeIP/屏蔽绑定查询/屏蔽 AAAA/并行查询/ServeStale/HappyEyeballs/域名策略/期望 IP/hosts/TunDNS/导入默认等）及其提示。

## 其它

- `main_shell.dart`：证据钩子与注释中的中文（非用户可见工具提示除外）未资源化；运行时工具提示（启动/停止）已资源化。
- `status_bar_view.dart`：`系统代理` 冲突字段名与 `tunActualLabel`/`SysProxyMode.label`/`runtime.state` 等运行枚举值仍为中文/原始英文标识，属运行域读模型（R4-11/R4-24 ownership），本卡未改。

## 统计

- 已迁入资源键：约 120 个（见 `strings.g.dart`，覆盖菜单、状态栏、设置外壳、路由/DNS 顶层、错误与状态反馈）。
- 未迁入但登记：上表逐文件条目；不静默遗漏。
