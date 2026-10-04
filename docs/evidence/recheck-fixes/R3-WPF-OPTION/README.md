# R3-WPF-Option-Labels — 参数设置窗口可见结构/文案对齐证据

日期：2026-10-04。开始 HEAD `f9ebe29`（工作树干净）。冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
控制文件：`v2rayN/Views/OptionSettingWindow.xaml`(+`.cs`)、`ServiceLib/Resx/ResUI.zh-Hans.resx`（缺失键回退 `ResUI.resx`）。
对照输入截图：`../R3-WPF-COMPARE/original-option.png`、`../R3-WPF-COMPARE/rc-option.png`。

本轮只对齐**可见结构/文案**；窗口形态（内嵌 showDialog vs 独立窗口）未改，登记 `R3-WPF-WINDOW-FORM`。未做逐事件行为对照、未重跑真实独立窗口。

## 1. Tab 标题（原版 XAML TabItem Header / resx）

| 上游 key | resx 值（zh-Hans） | RC 修改前 | RC 修改后 |
| --- | --- | --- | --- |
| TbSettingsCore | `Core: 基础设置` | 核心基础 | `Core: 基础设置` |
| TbSettingsN | `v2rayN 设置` | 显示 | `v2rayN 设置` |
| TbSettingsSystemproxy | `系统代理设置` | 系统代理 | `系统代理设置` |
| TbSettingsTunMode | `Tun 模式设置` | Tun 模式 | `Tun 模式设置` |
| TbSettingsCoreType | `Core 类型设置` | 内核类型 | `Core 类型设置` |

顺序保持 5 页。窗口 Title 由 `参数设置` 改为上游 `ResUI.menuSetting`=`设置`（O1 文案部分；形态仍内嵌）。

## 2. 按钮集（原版 XAML `btnSave`/`btnCancel`）

| 原版 | RC 修改前 | RC 修改后 | 语义 |
| --- | --- | --- | --- |
| `TbConfirm`=`确定`、`TbCancel`=`取消` | 取消 / 应用 / 保存 | `确定` / `取消` | `确定`(`settings-save`)=保存眼前草稿+`applyActive`+关闭；`取消`(`settings-cancel`)=丢弃草稿不落盘 |

FIX-08「Apply 用眼前草稿、取消不落盘」语义保留；原版无独立 Apply，故 `确定` 承担「应用并关闭」。

## 3. Core: 基础设置 字段（XAML 行序）

| 行 | 上游 key / 文案 | RC 修改前 | RC 修改后 |
| --- | --- | --- | --- |
| 0 | TbSettingsSocksPort `本地混合监听端口` | 本地端口 (LocalPort) | `本地混合监听端口` |
| 0c2 | TbSettingsSocksPortTip `Pac 端口 = +3；Xray API 端口 = +4；mihomo API 端口 = +5；` | 无 | 新增提示文本 |
| 2 | TbSettingsSecondLocalPortEnabled `开启第二个本地监听端口` | 第二本地端口 | `开启第二个本地监听端口` |
| 3 | TbSettingsUdpEnabled `开启 UDP` | UDP 转发 | `开启 UDP` |
| 4 | TbSettingsSniffingEnabled `开启流量探测` | 嗅探 (SniffingEnabled) | `开启流量探测` |
| 5 | TbSettingsDestOverride `流量探测类型` + chips | 无标签、且随嗅探门控 | 新增 `流量探测类型` 标签，chips 始终渲染 |
| 6 | TbSettingsRouteOnly `仅限路由 (routeOnly)` | 仅路由 (RouteOnly)，且随嗅探门控 | `仅限路由 (routeOnly)`，始终渲染 |
| 7 | TbSettingsAllowLAN `允许来自局域网的连接` | 同名 | 同名 |
| 8 | TbSettingsNewPort4LAN `为局域网开启新的端口` | 为局域网使用新端口（门控） | `为局域网开启新的端口`，始终渲染 |
| 9 | TbSettingsUser `认证用户名` | 用户名 (User)（门控） | `认证用户名`，始终渲染（O5） |
| 10 | TbSettingsPass `认证密码` | 密码 (Pass)（门控） | `认证密码`，始终渲染（O5） |
| 12 | TbSettingsLogEnabledToFile `启用日志存到文件` | 启用日志 (LogEnabled) | `启用日志存到文件` |
| 13 | TbSettingsLogLevel `日志等级` | 日志等级 (Loglevel) | `日志等级` |
| 15 | TbSettingsDefFingerprint `默认 TLS 指纹 (fingerprint)` | 默认指纹 (DefFingerprint) | `默认 TLS 指纹 (fingerprint)`（O5） |
| 16 | TbSettingsDefUserAgent `用户代理 (User-Agent)` + tip `仅对 raw/http、ws、gRPC、xhttp 生效` | 默认 UA (DefUserAgent) | `用户代理 (User-Agent)` + 提示 |
| 17/18 | TbSettingsMux4SboxProtocol / TbSettingsEnableCacheFile4Sbox | sing-box 协议 / 缓存文件（带 key） | `sing-box Mux 多路复用协议` / `启用 sing-box (规则集文件) 的缓存文件` |
| 19 | TbSettingsHysteriaBandwidth（上游单标签，RC 拆两字段） | 上行/下行 Mbps | 保留（无裸 key） |
| 20/21 | TbSettingsEnableFragment / TbEnableFinalFragment | 启用分片 (EnableFragment) / 最终分片 (EnableFinalFragment) | `启用分片 (Fragment)` / `启用末端分片` |
| 22/23 | TbSettingsSendThrough / TbSettingsBindInterface | SendThrough / BindInterface | `本地出站地址 (SendThrough)` / `绑定网口` |
| 24 | TbSettingsMux4Ray + 并发子项 | Ray 并发/… （带 key） | `Xray Mux concurrency` 等（zh-Hans 缺失键，按上游英文回退值） |

Fragment 子项：`分片包类型` / `分片长度` / `分片间隔` / `最大分片数`（上游 key TbSettingsFragment*；zh-Hans 有值）。

## 4. 其它页字段（消除裸 key，取上游 resx）

- v2rayN 设置：`启用流量统计 (需重启)`、`显示实时速度 (需重启)`、`去重时保留序号较小的项`、`自动调整配置列宽在更新订阅后`、`主界面双击设为活动`、`关闭窗口时隐藏至托盘`、`启动后隐藏窗口`、`启用配置拖放排序 (需重启)`、`托盘右键菜单配置展示数量限制`、`自动更新 Geo 文件的间隔 (小时)`、`开机启动 (可能会不成功)`、`当前字体 (需重启)`、`字体大小`、`语言 (需重启)`、`测速单个超时值`、`多线程测试时的并发数量`、`测速文件地址`、`真连接测试地址`、`UDP 测试地址`、`当前连接信息测试地址`、`订阅转换网址 (可选)`、`Geo 文件来源 (可选)`、`sing-box ruleset 文件来源 (可选)`、`路由规则集来源 (可选)`、`启用硬件加速 (需重启)`、`根证书提供者`、`主界面布局方向 (需重启)`。
- 系统代理设置：`系统代理类型`、`请勿将代理服务器用于本地 (Intranet) 地址`、`例外`、`高级代理设置，协议选择 (可选)`、`自定义 PAC 文件路径`、`自定义系统代理脚本文件路径`。
- Tun 模式设置：`启用 Tun`、`自动路由`、`严格路由`、`协议栈`、`MTU`、`ICMP 路由策略`、`启用 IPv6`、`旧版 TUN 保护`、`路由排除地址`、`Ipv4 地址`、`Ipv6 地址`。
- Core 类型设置：去掉 `(${configType})` 裸枚举；行标签取上游 XAML：`VMess / Custom Pre / Shadowsocks / Socks / VLESS / Trojan / Hysteria2 / Wireguard`。
- 历史保留区（KCP / DNS FakeIP / ClashUIItem）标签同样去除裸 PascalCase key，仅保留中文描述。

## 5. 状态裸 key（O8）

`SettingsController` 仍以稳定 key（`settings.saved` 等）回报状态；窗口新增 `_statusText()` 映射，不再显示裸 key：`settings.saved`→`操作成功`、`saved_need_app_restart`→`操作成功。请点击设置菜单重启应用。`、`error.*`→失败文案（对齐 resx OperationSuccess/NeedRebootTips）。

## 6. 对 R3-WPF-COMPARE O6 的修正

冻结 `OptionSettingWindow.xaml` 的 Core TabItem **确实包含** `TbSettingsMux4SboxProtocol`(行17)、`TbSettingsEnableCacheFile4Sbox`(18)、`TbSettingsHysteriaBandwidth`(19)、`TbSettingsEnableFragment`(20)、`TbEnableFinalFragment`(21)、`TbSettingsBindInterface`(22)、`TbSettingsSendThrough`(23)、`TbSettingsMux4Ray`(24)；仅 KCP 页(行619–737)与旧 TabItem 被注释。因此 RC 的 Mux/Hysteria2/Fragment 分区属上游该窗口，保留正确；KCP 作为显式「历史保留」登记。

## 7. 实际命令与结果

```
dart format lib/features/settings/option_setting_window.dart test/... (8 files)
# Formatted 8 files (0 changed)

flutter analyze <8 files>
# No issues found! (ran in 2.1s)

flutter test t12a_option_window fix08_option_apply fix08_option_cancel fix08_option_error \
  fix15c_autostart_timing fix16b_settings_source fix16_settings_field fix16d_layout_reachability
# +12: All tests passed!

flutter test t21e_dialogs_responsive fix16c_clash_ui_config fix16e_cert_provider fix16d_ui_state_store
# first run: t21e one 'did not complete' flake; standalone + retry: +20 All tests passed!
```

## 8. 未完成 / 边界

- 未重跑真实 Windows 独立窗口对照（RC 仍内嵌对话框），未做窗口几何对照 → 后续卡 `R3-WPF-WINDOW-FORM`。
- 未逐页截图对照（v2rayN/系统代理/Tun/Core 类型四页），仅以冻结 XAML+resx 与 widget 断言核对。
- 未跑全量 workspace 门禁、未 `flutter build windows`（按项目约束）。
- 未触 10808、未写系统代理/注册表/路由/TUN、未读用户凭据。
- 登记：`integration_test/ux_parity_fix08_test.dart:191`、`integration_test/user_journey_review_test.dart:484` 仍点 `settings-apply`（已移除）；因允许修改范围仅 `test/**`，未改 integration_test，待真实 Windows 集成跑前更新为 `settings-save`。
