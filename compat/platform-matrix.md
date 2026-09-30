# T00 平台差异矩阵（v2rayN 7.25.4 @ 7d6a967）

上游冻结源码根：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967`。
约定：`unverified` = 本 T00 阶段无法运行 GUI / 无法构建发行物，需在 T01 用引擎移植验证后回填。所有结论均来自冻结源码读到的事实，未读到的写 `unresolved`，不做“默认应该如此”。

## 1. 六交付单元表

| 交付单元 | 上游界面基准 | 上游发行物（workflow 证据） | 上游 RID | Flutter 目标状态 (T01 前) |
|---|---|---|---|---|
| Windows x64 | WPF (`v2rayN/v2rayN/v2rayN.csproj`, `net10.0-windows10.0.19041.0`) 与 Avalonia (`v2rayN.Desktop`) 双实现 | `v2rayN-windows-64.zip`（WPF）；`v2rayN-windows-64-desktop.zip`（Avalonia） | `win-x64` | unverified |
| Windows ARM64 | 同上双实现 | `v2rayN-windows-arm64.zip`；`v2rayN-windows-arm64-desktop.zip` | `win-arm64` | unverified |
| macOS x64 | 仅 Avalonia（build.yml 默认 project = `v2rayN.Desktop.csproj`） | `v2rayN-macos-64.zip` / `v2rayN-macos-64.dmg` | `osx-x64` | unverified |
| macOS ARM64 | 仅 Avalonia | `v2rayN-macos-arm64.zip` / `v2rayN-macos-arm64.dmg` | `osx-arm64` | unverified |
| Linux x64 | 仅 Avalonia | `v2rayN-linux-64.deb`、`v2rayN-linux-rhel-64.rpm` | `linux-x64` | unverified |
| Linux ARM64 | 仅 Avalonia | `v2rayN-linux-arm64.deb`、`v2rayN-linux-rhel-arm64.rpm` | `linux-arm64` | unverified |

证据：
- `.github/workflows/build.yml:17-19`（matrix `[x64, arm64]`）、`:31-38`（RID 规则 `win-*`/`osx-*`/`linux-*`）、`:12`（默认 project `./v2rayN.Desktop/v2rayN.Desktop.csproj`）。
- `.github/workflows/build-windows.yml:22`（WPF `./v2rayN/v2rayN.csproj`）、`build-windows-desktop.yml:21,28`（Avalonia，`windows-desktop`）。
- `.github/workflows/package-zip.yml:50-60`（zip 命名与 `-desktop` 重命名）。
- `.github/workflows/build-osx.yml:38`（dmg matrix `[x64, arm64]`）、`:65`（`v2rayN-macos-$Arch`）。
- `.github/workflows/build-linux.yml:57-58`（`v2rayN-linux-64.deb`、`v2rayN-linux-arm64.deb`）、`:116-117`（rpm 命名）。
- `v2rayN/v2rayN/v2rayN.csproj:5`（WPF TargetFramework 为 windows-only）。

> Windows x64/ARM64 同时存在 WPF 与 Avalonia 两套界面；macOS/Linux 只有 Avalonia。Flutter 若统一一套跨平台界面，需要明确 Windows 侧以哪套为像素基准（当前台账同时登记两者）。

## 2. 上游额外架构与标准 Flutter 引擎的差异（不可宣称等价）

| 上游架构 | 上游发行物 | 上游 RID/构建方式 | 与标准 Flutter 引擎的差异 | T00 状态 |
|---|---|---|---|---|
| Windows x86 (32-bit) | `v2rayN-windows-86.zip`、`v2rayN-windows-86-desktop.zip` | `win-x86`（`.github/workflows/build-windows-x86.yml:26,29,59`） | 标准 Flutter Windows 桌面引擎仅提供 x64（ARM64 支持受限/实验），无官方 win32 目标 | unverified，需引擎移植验证 |
| Linux riscv64 | `v2rayN-linux-riscv64.deb`、`v2rayN-linux-rhel-riscv64.rpm` | `ubuntu-24.04-riscv` runner（`build-linux.yml:124-162`） | Flutter Linux 引擎无官方 riscv64 target | unverified，需引擎移植验证 |
| Linux loong64 | `v2rayN-linux-loong64.deb`、`v2rayN-linux-rhel-loong64.rpm` | QEMU loongarch VM（`build-linux.yml:205-353` / `355-503`） | Flutter Linux 引擎无官方 loongarch64 target | unverified，需引擎移植验证 |

结论：**不能宣称 Flutter 目标与上游全部架构等价。** 上述三类的存在只能视为“上游覆盖广度”，Flutter 侧是否支持必须通过独立引擎移植验证，当前一律 unverified。

## 3. WPF vs Avalonia 差异清单（仅源码可证明）

### 3.1 主窗口
| 维度 | WPF | Avalonia | 证据 |
|---|---|---|---|
| 顶部菜单容器 | `ToolBarTray > ToolBar > Menu`（每组一个 Menu + PackIcon 图标 + Separator） | `DockPanel > Menu`（无 ToolBarTray、无图标） | `MainWindow.xaml:41-329`；`MainWindow.axaml:25-104` |
| 主题入口位置 | 工具栏右侧 `PopupBox pbTheme`（`HorizontalAlignment=Right`） | 顶部右侧 `ContentControl conTheme`（`DockPanel.Dock=Right`） | `MainWindow.xaml:314-320`；`MainWindow.axaml:24` |
| 退出菜单键 | `menuClose` Header=`ResUI.menuClose`（Minimize 图标） | `menuClose` Header=`ResUI.menuExit` | `MainWindow.xaml:298-312`；`MainWindow.axaml:103` |
| 平台条件可见项 | 无 XAML 条件绑定（Windows-only 工程） | `menuGlobalHotkeySetting` / `menuRebootAsAdmin` / `menuSettingsSetUWP` 均 `IsVisible="{Binding BlIsWindows}"` | `MainWindow.axaml:73,78,82` |
| 竖排布局 Tab 头 | NavigationRail TabControl + PackIcon（MessageTextOutline/ArrowDecisionOutline/LanConnect） | 纯文本 TabItem，仅 `TabStripPlacement=Left` | `MainWindow.xaml:359-398`；`MainWindow.axaml:138-145` |
| `menuAddServerViaScan` | 恒在 | 非 Windows 构造中 `IsVisible=false` | `MainWindow.axaml.cs:149` |
| 自动隐藏启动 | 任意平台读取 `AutoHideStartup` | 仅 `IsWindows()` 时最小化 | `MainWindow.xaml.cs:146-149`；`MainWindow.axaml.cs:152-155` |
| 关闭行为 | 关闭即 `Hide()`；`menuClose` 直接 `ShowHideWindow(false)`（无退出确认） | 关闭按 `CloseReason` 取消并隐藏；`menuClose` 弹 `UI.ShowYesNo(menuExitTips)` 确认后退出 | `MainWindow.xaml.cs:193-197,240-244`；`MainWindow.axaml.cs:192-214,290-308` |
| 最小宽度 | 800 | 600 | `MainWindow.xaml:16`；`MainWindow.axaml:14` |
| SoftwareOnly 渲染 | `!EnableHWA` 时 `RenderOptions.ProcessRenderMode = SoftwareOnly` | 无此逻辑 | `MainWindow.xaml.cs:151-154` |

### 3.2 参数设置（OptionSettingWindow）
| 项 | 平台/实现 | 证据 |
|---|---|---|
| `togEnableHWA`（硬件加速） | 仅 WPF | `OptionSettingWindow.xaml:906`（Avalonia 无同名项） |
| `togAutoRun` 自动启动 | 两者都有；Avalonia 额外包在 `tbAutoRun` 区块并 `IsVisible=BlIsWindows` | `OptionSettingWindow.axaml:555,562,572` |
| `togHide2TrayWhenClose` | 仅 Avalonia，`IsVisible=BlIsLinux` | `OptionSettingWindow.axaml:646,649,654,660` |
| `togMacOSShowInDock` | 仅 Avalonia，`IsVisible=BlIsIsMacOS` | `OptionSettingWindow.axaml:668,671,676` |
| 自定义系统代理脚本路径 | 仅 Avalonia，`IsVisible=BlIsNonWindows` | `OptionSettingWindow.axaml:938,946,954,961,1009,1015` |
| 平台标志来源 | `BlIsWindows/BlIsLinux/BlIsIsMacOS/BlIsNonWindows` | `OptionSettingViewModel.cs:74-77,124-127` |

### 3.3 状态栏 / 托盘
| 维度 | WPF | Avalonia | 证据 |
|---|---|---|---|
| 状态栏外框 | `materialDesign:ColorZone` | 纯 `DockPanel` | `StatusBarView.xaml:18`；`StatusBarView.axaml:14` |
| TUN 开关控件 | `ToggleButton togEnableTun` | `ToggleSwitch togEnableTun`（Semi Theme） | `StatusBarView.xaml:60-65`；`StatusBarView.axaml:43-47` |
| 托盘 | `H.NotifyIcon` `TaskbarIcon tbNotify` + ContextMenu（含路由/服务器 ComboBox、PAC 等） | `App.axaml` 的 `TrayIcon` + `NativeMenu` | `StatusBarView.xaml:106-234`；`App.axaml:35-84` |
| 托盘非 Windows 项 | 无（Windows Only） | “显示 GUI / 显示或隐藏主窗口” `IsVisible=BlIsNonWindows`；PAC 项 `IsVisible=BlSystemProxyPacVisible` | `App.axaml:46,50,72`；`StatusBarViewModel.cs:112-113` |

### 3.4 主题入口
| 维度 | WPF | Avalonia | 证据 |
|---|---|---|---|
| 形态 | Grid 表单（主题/配色/字号/语言） | IconButton + Flyout（主题/字号/语言） | `ThemeSettingView.xaml:15-88`；`ThemeSettingView.axaml:22-66` |
| 配色项 `cmbSwatches` | 有 | 无 | `ThemeSettingView.xaml:49`（Avalonia 文件内未出现 cmbSwatches） |
| 默认主题资源 | `materialDesign:BundledTheme BaseTheme=Light PrimaryColor=Blue SecondaryColor=Lime` + MaterialDesign2.Defaults | `semi:SemiTheme` + `AvaloniaEditSemiTheme` + Semi DataGrid，`RequestedThemeVariant=Default` | `App.xaml:11-15`；`App.axaml:11,19-24` |

### 3.5 各窗口默认尺寸差异（同一逻辑窗口 WPF vs Avalonia）
| 窗口 | WPF 尺寸 | Avalonia 尺寸 | 证据 |
|---|---|---|---|
| AddServerWindow | 900x700 | 900x600 | `AddServerWindow.xaml:13-14`；`AddServerWindow.axaml:12-13` |
| DNSSettingWindow | 1000x700 | 900x600 | `DNSSettingWindow.xaml:13-14`；`DNSSettingWindow.axaml:12-13` |
| FullConfigTemplateWindow | 1000x700 | 900x600 | `FullConfigTemplateWindow.xaml:13-14`；`FullConfigTemplateWindow.axaml:12-13` |
| OptionSettingWindow | 1000x700 | 1000x600 | `OptionSettingWindow.xaml:13-14`；`OptionSettingWindow.axaml:12-13` |
| RoutingSettingWindow | 1000x700 | 900x600 | `RoutingSettingWindow.xaml:13-14`；`RoutingSettingWindow.axaml:12-13` |
| RoutingRuleSettingWindow | 1000x700 | 900x600 | `RoutingRuleSettingWindow.xaml:13-14`；`RoutingRuleSettingWindow.axaml:12-13` |
| RoutingRuleDetailsWindow | 900x700 | 900x600 | `RoutingRuleDetailsWindow.xaml:13-14`；`RoutingRuleDetailsWindow.axaml:12-13` |
| SubSettingWindow | 1000x700 | 900x600 | `SubSettingWindow.xaml:13-14`；`SubSettingWindow.axaml:12-13` |
| SubEditWindow | 700x650 | 700x600 | `SubEditWindow.xaml:13-14`；`SubEditWindow.axaml:12-13` |

### 3.6 仅某一实现存在的窗口/控件
- 仅 Avalonia：`JsonEditor.axaml`（AvaloniaEdit）、`MessageBoxDialog.axaml`（自定义消息框）、`SudoPasswordInputView.axaml`（Linux sudo）、`GlobalResources.axaml`、`GlobalStyles.axaml`。
- 仅 WPF：无独立“消息框窗口”（用系统 MessageBox，见 `v2rayN/Common/UI.cs:14`）；QrcodeView 为 UserControl 经 DialogHost 弹出。
- WPF QrcodeView 与 Avalonia QrcodeView 均为 UserControl（非 Window）。

## 4. macOS / Linux 平台特有行为（源码位置）

| 行为 | 实现位置 | 说明 |
|---|---|---|
| macOS Dock 显示策略 | `v2rayN.Desktop/App.axaml.cs:34-109`（`OperatingSystem.IsMacOS()`、`MacAppUtils.SetActivationPolicyAccessory()`、`MacOSShowInDock`）；`Program.cs:70-72` | `MacOSShowInDock=false` 时设为 Accessory 策略；Dock 激活/还原有 300ms 定时对齐动画 |
| macOS 自启 | `ServiceLib/Handler/AutoStartupHandler.cs:29-37,178-248` | 写 `~/Library/LaunchAgents/v2rayN-LaunchAgent.plist`，`launchctl load/unload -w`，`pgrep` 防重复 |
| Linux 自启 | `AutoStartupHandler.cs:20-28,131-172` | 写 `~/.config/autostart/v2rayN.desktop`（freedesktop autostart），非 systemd unit |
| Windows 自启 | `AutoStartupHandler.cs:11-19,44-125` | 注册表 `Global.AutoRunRegPath`；管理员时用 Task Scheduler（LogonTrigger，RunLevel=Highest） |
| 系统代理 | `ServiceLib/Handler/SysProxy/SysProxyHandler.cs:26-58` | `ForcedChange/ForcedClear/Pac` 按 `IsWindows/IsLinux/IsMacOS` 分支；默认例外串按平台不同：`ConfigHandler.cs:193` |
| Linux 关闭隐藏 | `MainWindow.axaml.cs:331-350` | `Hide2TrayWhenClose=false` 时改为最小化；ShowHideWindow 的显示判定对 Linux/macOS 用 `ShowInTaskbar ^ Minimized` |
| PAC / 系统代理脚本 | `OptionSettingWindow.axaml:938`（CustomSystemProxyScriptPath 非 Windows）；`StatusBarViewModel.cs:112`（PAC 可见性=Windows） | Avalonia 下 PAC 自定义脚本仅非 Windows 显示 |
| TUN 权限 / sudo | `CoreAdminManager.cs:81`（macOS/Linux kill-sudo 脚本名）；`SudoPasswordInputView.axaml`（Linux 密码输入）；`CoreManager.cs:88,367`（Windows TUN 相关分支） | 提权/密码输入仅在 Avalonia/Linux 侧有独立 UI |
| 复制命令前缀 | `StatusBarViewModel.cs:229`（Windows `set` vs 其它 `export`） | 复制环境变量命令按平台区分 |
| 二维码截屏 | WPF `QRCodeWindowsUtils.CaptureScreen`；Avalonia `QRCodeAvaloniaUtils.cs:10`（`!IsWindows()` 返回 null） | Avalonia 非 Windows 不支持截屏取码 |
| 全局热键 | `v2rayN.Desktop/Manager/HotkeyManager.cs:32`（非 Windows 直接 return）；`MainWindow.axaml:73` | 热键设置项仅 Windows |
| macOS Dock/窗口缩略 | `App.axaml.cs`、`Program.cs` | 见上 |

## 5. T00 无法运行验证、一律 unverified 的清单

- 所有平台的 Flutter 实际渲染结果、DPI 缩放行为、字体回退（T01 前无构建物）。
- 主窗口三种布局在真实运行时的分栏比例恢复是否与 `MainGirdHeight1/2` 语义一致（源码已读，运行未验）。
- macOS Dock Accessory 策略的可见行为、`MacOSShowInDock` 切换效果（需 macOS 运行）。
- Linux sudo 提权、TUN 权限、autostart `.desktop` 生效（需 Linux 运行）。
- 托盘图标/NativeMenu 在 macOS/Linux 的呈现与菜单可见性。
- Windows x86、Linux riscv64/loong64 的 Flutter 引擎可行性。
- Avalonia `JsonEditor`（AvaloniaEdit）在 macOS/Linux 的字体/输入法表现。
- 上游发行包（zip/deb/rpm/dmg）的最终内容与我方 Flutter 打包等价性。
- 各窗口尺寸持久化的实际写入/读取（含多屏 DPI；Avalonia WindowBase 已按 `screen.Scaling` 换算，见 `WindowBase.cs:32-48`）。
- WPF 与 Avalonia 在同一 Windows 机器上默认尺寸不一致（表 3.5）是否影响“像素对齐”验收口径。
