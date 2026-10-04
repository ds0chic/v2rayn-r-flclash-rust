# R3-WPF-COMPARE — 冻结原版 WPF 三窗口 vs RC 三窗口可见结构对照

日期：2026-10-04。基线：repo HEAD `baf2299`；冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；RC 包 `dist/v2rayN-R-1.0.0+1-windows-x64`（`build-info.json`：`git_commit=3fc49c487308f76ccbb0ce351d45d0d7b6b3abbd`、`git_dirty=false`；注意该构建提交与本轮 repo HEAD `baf2299` 不同，属既有产物口径问题，不在本轮处理）。

本轮只做**三窗口可见结构对照**（主窗口 + 参数设置窗口 + 路由设置窗口），不做逐事件行为对照。所有截图均为隔离数据目录下的合成/空配置，无真实节点、订阅、证书。

## 1. 构建冻结原版（work/ 只读）

- 冻结源 `work/.../2dust-v2rayN-7d6a967` 复制到 `$env:TEMP\v2rayn-wpf-build\src`（robocopy，未写回 work/）。
- 目标框架：`v2rayN/v2rayN/v2rayN.csproj` → `net10.0-windows10.0.19041.0`（WPF）。
- 命令与结果（exit 0）：

```
& "C:\Program Files\dotnet\dotnet.exe" build "$env:TEMP\v2rayn-wpf-build\src\v2rayN\v2rayN\v2rayN.csproj" -c Debug -v minimal
# 还原：ServiceLib.UdpTest / v2rayN / ServiceLib 均成功（无 NuGet 网络阻塞）
# 生成：ServiceLib.dll、v2rayN.dll
# 已成功生成。0 个警告 0 个错误。已用时间 00:00:20.86
```

- 产物 `...\bin\Debug\net10.0-windows10.0.19041.0\v2rayN.exe`；原生 `e_sqlite3.dll` 在 `runtimes\win-x64\native`（随 deps.json 解析）。

## 2. 启动隔离与安全边界

- 原版数据目录 = 临时副本 exe 所在目录（`Utils.StartupPath()` = `AppDomain.BaseDirectory`，未设 `V2RAYN_LOCAL_APPLICATION_DATA_V2`）；未触碰用户真实配置。
- **关键**：启动前在临时 `guiConfigs/guiNConfig.json` 预置 `SystemProxyItem.SysProxyType=2`（Unchanged）。否则 `StatusBarViewModel.Init()` 会以默认 `ForcedClear` 调 `ProxySettingWindows.UnsetProxy()` 清空宿主系统代理；且退出时 `AppExitAsync` 也会清代理。原版以 `Stop-Process -Force` 结束，`AppExitAsync` 不执行。
- RC 用 `V2RAYN_R_DATA_DIR` 指向临时 data dir，并在其中预置 `Inbound.LocalPort=11808`。
- 未对 `127.0.0.1:10808` 做任何监听/占用/修改；未改系统代理/注册表/路由/TUN；未读用户凭据。
- 只停止本脚本启动的 PID 及后代（`original-probe.json`/`original-windows-probe.json`/`rc-windows-probe.json` 记录 `stopped`/`alive_after=true`）。已排除并保留非本脚本进程：用户真实 `v2rayN.exe`（`C:\Users\Colby\Desktop\v2rayN-windows-64`，PID 9584）与另一处从 `dist` 启动的 RC（PID 6948 + `net_host` 35992）。

## 3. 截图与探针文件

| 端 | 窗口 | 文件 | 备注 |
| --- | --- | --- | --- |
| 原版 | 主窗口 | `original-main.png` | 1200×800，dpi 96，标题 `v2rayN - V7.25.4 - X64 - 以管理员身份运行` |
| 原版 | 参数设置 | `original-option.png` | 独立窗口 1000×700，标题「设置」 |
| 原版 | 路由设置 | `original-routing.png` | 独立窗口 1000×700，标题「路由设置」 |
| RC | 主窗口 | `rc-main.png` | 1200×800，dpi 96，标题 `v2rayN` |
| RC | 参数设置 | `rc-option.png` | 内嵌 `showDialog`，标题「参数设置」 |
| RC | 路由设置 | `rc-routing.png` | 内嵌 `showDialog`，标题「路由设置」 |

辅助：`original-main-uia.txt`（原版主窗口 UIA 控件树，含菜单/表头/状态栏），`original-probe.json`、`original-windows-probe.json`、`rc-windows-probe.json`，脚本 `capture_original.ps1`、`capture_original_windows.ps1`、`capture_rc.ps1`。

原版菜单/子项通过 UIA + 真实鼠标点击驱动；RC 用公开的 `V2RAYN_R_OPEN_SETTINGS` / `V2RAYN_R_OPEN_ROUTING` 导航钩子。

## 4. 对照结论

### 4.1 主窗口

**一致**
- 顶级菜单组与顺序：`配置项 / 订阅分组 / 设置 / 帮助 / [重载|重启服务] / 推广 / 关闭`（7 组）。
- 节点表中文列头与集合：`类型 / 别名 / 地址 / 端口 / 传输协议 / TLS / 订阅分组 / 延迟 (ms) / 速度 (MB/s) / 今日上传 / IP 信息 / 今日下载 / 总上传 / 总下载`。
- 状态栏都有系统代理与路由下拉，路由均显示 `V4-绕过大陆(Whitelist)`；Tun 开关位置相近。

**差异**
| # | 项 | 原版 | RC | 证据 |
| --- | --- | --- | --- | --- |
| M1 | 主题 | 固定 Light | 跟随持久化 `system`（本机系统深色 → 深色）；`V2RAYN_R_THEME=light` 未生效 | `original-main.png` vs `rc-main.png` |
| M2 | 重载命名 | 菜单「重启服务」 | 菜单「重载」(F5) | 两主窗口截图 |
| M3 | 节点表索引列 | 最左为无标题窄列 | 显式 `#` 列头 | 两主窗口截图 / `original-main-uia.txt` |
| M4 | 工具栏按钮 | 无「应用/停止/布局/主题」按钮 | 右侧多 `运行时:Stopped(未应用) / 应用 / 停止 / 垂直布局 / 主题` | `rc-main.png` |
| M5 | 状态栏文案 | `本地:[mixed:10808] / 局域网:none / 启用 Tun / 系统代理 [不改变系统代理] / 路由 [V4-…]` | `入站 -- / LAN -- / TUN / 实际:未启用 / 系统代理:清除系统代理(未启用) / 路由模式:Rule / 路由:V4-… / 节点:未运行 / 运行时:Stopped host=alive … rev/epoch/seq` | 两主窗口截图 |
| M6 | 空态 | 空表格无提示 | 居中「暂无节点 / 可从剪贴板导入或添加节点」 | `rc-main.png` |
| M7 | 底部信息区 | 过滤器/复制所有/清除所有/自动刷新/自动滚动 | 左导航栏 `信息/当前代理/当前连接` + `过滤/Trace+/自动刷新/自动滚动/暂停采集/复制全部/清空` | 两主窗口截图 |
| M8 | 菜单分隔线 | 各子菜单有分隔线（配置项 3 段、设置 2 段、帮助 1 段） | 菜单模型无分隔字段，配置项/设置/帮助未见分隔线 | `main_menu.dart` vs `MainWindow.xaml` |

**无法判定**：1200px 宽度下原版可见到 `总上传/总下载` 而 RC 未显示到（可能列宽差异，需定宽对照）；高 DPI；菜单折叠态下的完整子项与分隔线（本轮未逐菜单展开 RC）。

### 4.2 参数设置窗口

**一致**
- 概念上同为 5 个上游分组页（Core 基础 / 显示(v2rayN) / 系统代理 / Tun 模式 / 内核类型）。
- 都含 `本地端口/10808`、UDP、嗅探、http/tls/quic 探测类型、路由等级 `warning` 等核心字段。

**差异**
| # | 项 | 原版 | RC | 证据 |
| --- | --- | --- | --- | --- |
| O1 | 窗口形态 | 独立窗口，标题「设置」 | 内嵌对话框，标题「参数设置」 | 两截图 |
| O2 | 分组名 | `Core: 基础设置 / v2rayN 设置 / 系统代理设置 / Tun 模式设置 / Core 类型设置` | `核心基础 / 显示 / 系统代理 / Tun 模式 / 内核类型` | 两截图 |
| O3 | 字段名 | `本地混合监听端口 / 开启第二个本地监听端口 / 开启 UDP / 开启流量探测 / 仅限路由 (routeOnly)` | `本地端口 (LocalPort) / 第二本地端口 / UDP 转发 / 嗅探 (SniffingEnabled) / 仅路由 (RouteOnly)`（带原始字段名） | 两截图 |
| O4 | 提示文案 | 右上 `Pac 端口 = +3; Xray API 端口 = +4; mihomo API 端口 = +5;` | 无 | 两截图 |
| O5 | 字段增减 | Core 页含 `认证用户名/认证密码`、`默认 TLS 指纹 (fingerprint)` | 认证项仅在 AllowLANConn+NewPort4LAN 时显示；`默认指纹/默认 UA` 取代指纹选择 | `original-option.png` vs `rc-option.png` |
| O6 | 额外分区 | Core 页无 Mux/Hysteria2/Fragment/KCP | Core 页追加 `多路复用(Mux)/Hysteria2/分片(Fragment)/历史保留(KCP)` | `rc-option.png` |
| O7 | 按钮 | `确定 / 取消` | `取消 / 应用 / 保存` | 两截图 |
| O8 | 未翻译 key | — | 顶部显示裸 key `settings.saved`（`settings_controller.dart` 写入，未见本地化映射） | `rc-option.png` |

**无法判定**：各页完整字段矩阵（本轮只截 Core 首页）；显示/系统代理/Tun/内核类型四页未逐一截图对照。

### 4.3 路由设置窗口

**一致**
- 内置规则集数据一致：`V4-绕过大陆(Whitelist)=8/序1`、`V4-黑名单(Blacklist)=11/序2`、`V4-全局(Global)=4/序3`。
- 都有域名策略下拉（原版默认 `AsIs`）与 sing-box 域名策略下拉。

**差异**
| # | 项 | 原版 | RC | 证据 |
| --- | --- | --- | --- | --- |
| R1 | 标签 | `域名解析策略` / `sing-box 域名解析策略` | `域名策略` / `域名策略 (sing-box)`（空值显 `(空)`） | 两截图 |
| R2 | 列表标题 | 有蓝色链接「预定义规则集列表」 | 无标题，直接渲染表格 | 两截图 |
| R3 | 列头 | `别名 / 数量 / 排序 / 可选地址 (Url) / 自定义图标` | `备注 / 规则数 / 排序 / URL / 状态`（多「状态」列显示「默认」，少「自定义图标」） | 两截图 |
| R4 | 顶部操作 | `添加规则集 / 一键导入规则集` | 无（改为底部 `添加/删除/设为默认/应用/关闭`） | 两截图 |

**无法判定**：列宽与滚动；规则集编辑子流程。

## 5. 分类汇总

- **一致**：顶级菜单组与顺序；节点表中文列头集合；路由规则集内置数据与顺序；设置 5 分组概念。
- **差异**：见 M1–M8 / O1–O8 / R1–R4（其中 O8 `settings.saved` 裸 key、R3 列头、M2/M3/M5 文案属高优先级对齐项）。
- **无法判定**：完整子菜单项/分隔线、逐页字段矩阵、列宽与高 DPI、行为/事件级等价。

## 6. Blocked / 边界

- 未做逐事件行为对照、真实平台效果（系统代理/TUN/内核）验证、高 DPI/托盘视觉、菜单逐项展开对照。
- RC 设置/路由为内嵌对话框，无独立窗口，无法做窗口级几何对照。
- `V2RAYN_R_THEME` 证据钩子与异步加载竞态：本次无法在不改生产代码的前提下让 RC 稳定呈现 Light，主题对照以现状截图为准。
- 未运行仓库门禁（非本轮范围）；未改生产代码。

## 7. 下一步前置

1. 立卡 `R3-WPF-Option-Labels`（设置窗口标题/分组/字段/按钮/裸 key/O4/O5/O6）。
2. 立卡 `R3-WPF-Routing-Structure`（R1–R4）。
3. 立卡 `R3-WPF-Main-Chrome`（M2/M3/M5/M8，确认 M1 主题与 M4 额外按钮取舍）。
4. 后续如需逐事件对照，需稳定的 Light 证据运行与菜单逐项展开脚本；TUN/系统代理/内核效果按项目约束在隔离环境单独验收。

## 8. Wave K 后重拍（最终包 ff45519）

三张 RC 截图（`rc-main.png` / `rc-option.png` / `rc-routing.png`）已用最终包（build-info `git_commit=ff45519`、`git_dirty=false`、zip sha256 `d3b4a07a…`）经同一 `capture_rc.ps1` 隔离重拍；上表 O/R/M 差异中以下项已在 Wave K 修复（见 `R3-WPF-OPTION/`、`R3-WPF-ROUTING/`、`R3-WPF-MAIN-CHROME/`）：

- 设置窗口：标题「设置」、五页 `Core: 基础设置 / v2rayN 设置 / 系统代理设置 / Tun 模式设置 / Core 类型设置`、`确定/取消`、无裸 key（`settings.saved` 已本地化）、补 `认证用户名/认证密码` 与 `默认 TLS 指纹`；KCP 标历史保留。
- 路由窗口：`预定义规则集列表` 标题、顶部 `添加规则集/一键导入规则集`、列头 `别名/数量/排序/可选地址 (Url)/自定义图标`、去掉非上游 `状态` 列。
- 主窗口：行头 40px 无 `#`、状态栏 `本地/局域网/启用 Tun`、根菜单分隔线（`main_shell.dart:614-619` 已渲染）、`重启服务` 命名。
- 仍为有意增强（不回退）：M1 主题跟随系统、M4 `运行时 应用/停止`、M6 空态、M7 底部信息区；窗口形态（内嵌 dialog）登记 `R3-WPF-WINDOW-FORM`。
