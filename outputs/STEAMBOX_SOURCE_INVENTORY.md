**SteamBox 本地源码、设置和功能索引**

研究日期：2026-10-01。配合 `STEAMBOX_FLUTTER_RUST_PLAN.md` 和 `STEAMBOX_EXECUTOR_PROMPT.md` 使用。以下研究以本地dirty源码为基准；源码事实与实施建议分别标注。没有运行原应用/Steam/ASF/加速器，没有读取用户配置、数据库、日志、cookies或密钥，没有修改目标项目。

最新范围：用户只保留截图中的网络加速（Accelerator）、账号切换（GameAccount）、库存游戏（GameList）、挂卡（SteamIdleCard）。Authenticator、ArchiSteamFarmPlus/ASF、GameTools彻底删除。以下索引已去除这三个模块的实施章节和ASFSettings 14字段；保留初查101顶层字段及四模块共享能力。原115只是删减前历史总数。

本文件整合A界面/101保留字段、B Steam/账号/库/挂卡、C网络/系统接入/SDK。最终以主方案统一contracts/数据库/owner实施。S25–S29已撤销，S00–S24和S30–S34共30个有效任务；不执行被撤销任务。

**授权删除清单与共享边界**

| 删除模块 | 不实施内容 | 必须证明 |
|---|---|---|
| Authenticator | OTP库/导入导出、手机令牌管理、交易确认、令牌云同步、专属加密库迁移 | 无页面/命令/初始化/云请求/数据导入/专属发布依赖 |
| ArchiSteamFarmPlus / ASF | ASF进程/IPC/控制台/Bot/插件、ASFSettings全部14字段 | 不启动/托管/下载/打包ASF，不注册或迁移其设置 |
| GameTools | 无边框/窗口工具/VAC修复/CPU工具入口和其专属权限动作 | 无路由/资源/后台注册/权限命令 |

四模块必需Steam Web登录、用户输入2FA/邮箱验证码、Cookie/AccessToken/RefreshToken、secret-store、Steam原生worker、Cloud/成就/AFK、网络CA/私钥、helper/电源适配/WinDivert/迅游SDK继续保留。WinAuth/SteamClient按反向引用保留最小共享部分，不按目录名整体删掉。旧删除模块个人数据不读取、解密、迁移或自动删除。新版模块白名单严格四项，不兼容任意旧.NET程序集加载。

**基线元数据与阅读规则**

源码根：`C:\Users\Colby\.gemini\antigravity\scratch\SteamTools`。下文所有 `src/`、`ref/`、`packaging/` 路径相对该根。HEAD：`a7a5ce7f0830b9b759090a75b59d82679084b478`，develop，本地领先两个提交；必须另含未提交改动和递归子模块工作树。本文件的hash只是采样元数据，不是可恢复源码快照，也没有替代S00最终冻结。

当前源码出现SteamBox/Steam++及历史ApplicationId混用；按实际源与运行基线核实，不凭旧shortcut或README推断当前产品。README是上游通用说明，当前导航/初始化/可达性必须分开。原程序集或Debug exe存在不证明它与工作树一致。

**主仓库已修改源码文件SHA-256（只记录路径与摘要）**

| 当前修改路径 | SHA-256 |
|---|---|
| `src/AssemblyInfo.Constants.cs` | `5d875961345334dc238e63da9e051553f0b3a46bd4285e14660e8e7fc09b41fc` |
| `src/Avalonia.Skia.Internals/ClassicDesktopStyleApplicationLifetime.cs` | `b11927d601855eaac09a4e7181071de88d89e74858abb34e875b7acb77691a60` |
| `src/BD.WTTS.Client.Avalonia.App/BD.WTTS.Client.Avalonia.App.csproj` | `db65e71c701929f5241ab6cbcbebb3e293d87c9dae887db6f76381618dd21cfc` |
| `src/BD.WTTS.Client.Avalonia.App/Program.cs` | `a86d7dbb49af7c7f22af613e6af9ff1cb6225eb61ce87a1e73e7a69d2f37b666` |
| `src/BD.WTTS.Client.Avalonia/Services.Implementation/UI/Widgets/NavigationService.cs` | `abeb719a82d4c81625702fbd5e34bec469645fd41429227b664ec7746997d5a2` |
| `src/BD.WTTS.Client.Avalonia/UI/App.Interface.cs` | `55fc8480686c846ce391229961dbdebfe65fea96741ebc2b135969eb7f752296` |
| `src/BD.WTTS.Client.Avalonia/UI/App.axaml.cs` | `3e46ea9230983a2670a4edc5e921e860618a5e94aa94733301fe83ec8848f1ad` |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/MainView.axaml.cs` | `e5ab374f835643df0932a15d331bb6f7af16fc9edee22a1b2347ae0473ec1d59` |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Windows/MainWindow.axaml.cs` | `b49c3e89c4856153d7a49ecae70265486a94cfbdc03342a62fe423dd8200136a` |
| `src/BD.WTTS.Client.Plugins.Accelerator/Services/Mvvm/ProxyService.Operate.cs` | `8b5b8af90e8cb6190744d2f3f1d466be7883b76bb9470c80b3f23ded9d518761` |
| `src/BD.WTTS.Client.Plugins.Accelerator/Services/Mvvm/ProxyService.cs` | `14d6eda7647a6b780351317382dc8b754368b3ca7fd70a0d9386c7a5ed443bdd` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/MainFramePage.axaml.cs` | `581dc8705861e6ac2054deda964abc74745f16cafefe1a802366de3c52242b6f` |
| `src/BD.WTTS.Client.Plugins.GameAccount/Services.Implementation/SteamPlatformSwitcher.cs` | `c11d43be022096521556170a8d3bda9528364a988eb734da10b7ecc7658fd7b8` |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Controls/AccountItems.axaml` | `c49cdce16871afc9fa58c2c080ec43278276cbf1edc3e0903817957f5666011c` |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Controls/AccountItems.axaml.cs` | `7cba1a2af737aee6720f14b1b552089c805b4a4360849cf467eb339391333c3a` |
| `src/BD.WTTS.Client/FileSystem/WindowsFileSystem.cs` | `d39964d2767205a5bebac1aa73decef13acce7741c125baae5d6d286c2e90ffb` |
| `src/BD.WTTS.Client/Services.Implementation/Platform/Windows/App/WindowsPlatformServiceImpl.App.cs` | `7e3c4720d5f17c5ce45cf0a0a5d9403d2f1c98a1aacb5079ed84c451c3bc80ee` |
| `src/BD.WTTS.Client/Services.Implementation/Steam/SteamServiceImpl2.cs` | `278f3251cb1b3036e8b6362776a305f38bb6eedb320e9635d7383b2cf4bdbb1b` |
| `src/BD.WTTS.Client/Startup/Startup.GlobalExceptionHandler.cs` | `ec53e378093902d005144744c63c1bc6bcd7273efa5d14c11a5c0ecc6ba42283` |
| `src/BD.WTTS.Client/Startup/Startup.Host.cs` | `2d8206435343ff6bc97080b2bd814d4dcbe176b522c74eed24e52aa4286ac8fc` |
| `src/BD.WTTS.Client/Startup/Startup.Properties.cs` | `719136edb21b1cac42c695d6e883866e8871cdb231d384aa1d1c09541e191215` |
| `src/BD.WTTS.Client/Startup/Startup.SingleInstancePipeline.cs` | `fd2302ae6c650bb7c4d82acb06591b206fa435a471852267e0caf59cb4b09fde` |
| `src/BD.WTTS.Client/Startup/Startup.cs` | `433e7d3403d1ad1d61ac7e01f719462d50547e07db7bfa9480a515f36cc6c7b8` |
| `src/BD.WTTS.Client/UI/ViewModels/Windows/Main/MainWindowViewModel.cs` | `6c17a08dae097efdf5714eb79e141817860e130b24fb208b35070cc47c2c0c13` |
| `src/TFM_NETX_WITH_ALL.props` | `8ba2dd6de8e8b1e969b89375f112008ff8fc639255ca2918c8f11d960a5430eb` |
| `src/TFM_NETX_WITH_DESKTOP.props` | `37ac879925139947c0d62ef43d538991e1d96688ac9cb31b3e5bb33b37864721` |
| `src/TFM_NETX_WITH_WINDOWS.props` | `e885cca56b04d6d677759e312149ffdfb8648640fd5cc92382b68ea5c794365e` |

子模块初查12个dirty：Avalonia.Image2、Avalonia8、Common、DirectoryPackages、Facepunch.Steamworks、Gameloop.Vdf、Steam4NET、SteamClient、WTTS.MicroServices.ClientSDK、WinAuth、appcenter-sdk-dotnet、dotnetCampus.Ipc。嵌套SAM/SteamClient依赖必须递归冻结；子模块HEAD不代表子模块工作树内容。

**A. 四模块界面、操作和101个保留顶层设置字段**

**SteamBox / SteamTools 本地 UI、设置与交互研究索引**

研究日期：2026-10-01（Asia/Shanghai）。只读研究本地源码；未启动程序，未读取真实账号、配置、日志、数据库或密钥。基线是 `C:/Users/Colby/.gemini/antigravity/scratch/SteamTools` 的**当前 dirty 工作树**，不是 README 截图或干净 HEAD。以下路径均相对该 Git 根目录。

此索引供 Flutter 前端 + Rust 后端方案引用。它记录源码证据，不声称静态分析已验证运行时视觉或全部功能。执行期需要用原程序、隔离测试资料、截图与命令结果完成最终普查；101 个保留顶层设置字段只是已核对的种子清单，不能代替最终全面盘点。

**1. 外壳、导航、主页与窗口行为**

| 证据路径 / 符号 | 当前行为 | 移植要求 |
|---|---|---|
| `src/BD.WTTS.Client.Avalonia.App/Program.cs`：`Main`、`BuildAvaloniaApp`、`StartUIApplication` | 桌面入口；Windows UseWin32，macOS UseAvaloniaNative，Linux UseX11；UseSkia + ReactiveUI | Flutter 启动宿主保留平台能力，Rust 单实例 / 服务生命期与 UI 显隐分开 |
| `src/BD.WTTS.Client.Avalonia/UI/App.axaml.cs`：`OnFrameworkInitializationCompleted` | desktop MainWindow；single-view MainView；系统会话结束触发 Shutdown | 保留桌面关机退出清理，避免只隐藏 UI 后丢失退出路径 |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Windows/MainWindow.axaml` | 默认 960×680，最小 528×508，CenterScreen，高质量图片插值，内部 MainView | 同密度 / DPI 下复刻尺寸、占位、间距和控件位置 |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/MainView.axaml` | 左栏 72 DIP；项宽 72、最小高 75；图标 28；文字 10；选中项文字被样式隐藏，图标强调色；顶部内容 margin 50；右侧 Frame | 不自动换成现代侧栏、卡片仪表盘或不同信息架构 |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Controls/TitleBar.axaml`；`src/BD.WTTS.Client.Avalonia/UI/Styling/Controls.axaml` | TitleBarHeight=50，应用图标 26×26；Windows 右侧预留 140；其他平台 22；标题、可选搜索和 ActionContent | Flutter 原生窗口按钮、拖动 / 命中测试、平台预留必须匹配 |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Abstractions/Windows/ReactiveAppWindow.cs` | AppWindow，内容延伸标题栏；Complex 标题栏命中测试；背景材质 fallback；窗口位置大小恢复 / 保存 | 无边框外观仍需正确缩放、拖拽、最大化、任务栏、系统菜单、DPI 和多屏恢复 |
| `src/BD.WTTS.Client/UI/ViewModels/Windows/Main/MainWindowViewModel.cs`：构造器 | 可进入主导航的插件白名单：Accelerator、GameAccount、GameList、SteamIdleCard | 当前 profile 默认可见集合必须一致；ASF / Authenticator / GameTools 不可擅自加回主导航 |
| 同上：`PluginTabItems`、`SaveTabItemsSort` | PluginTabItems 排除 HomePage；排序从 UISettings.SortMenuTabs；停用保存菜单顺序 | 保留配置稳定 ID，勿用显示中文或数组位置当 ID；旧 HashSet 的顺序风险要记录 |
| `MainWindow.axaml.cs`：`AppSplashScreen.RunInit`、`SetStartDefaultPageName` | 传入 Welcome/Home 和 Settings footer；StartDefaultPageName 按菜单 ID 查找，否则 HomePage | 有插件时 MainWindowViewModel **覆盖传入 tabItems** 为插件集合，因此 Home 可能不在左栏，但启动 fallback 仍是 HomePage；必须实际截图确认 |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/HomePage.axaml` | ScrollViewer padding 20，MaxWidth 1080；Steam 用户头像 36、昵称 / IPCountry 欢迎；“已安装功能”120 宽磁贴，图标 96；底部标语 | 保留当前简化主页，不按 README 恢复公告 / 商城 / 广告 |
| `src/BD.WTTS.Client/UI/ViewModels/Pages/HomePageViewModel.cs`：`GetServerContent`、`NavgationToMenuPage` | Articles / NavigationBanners / Shops 模型还在，但 GetServerContent 返回 CompletedTask；磁贴导航 None | 模型存在不代表功能可见；不得增加服务器内容请求 |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/MainView.axaml.cs`：`NavView_ItemInvoked` | 已在目标页面时不跳转；切侧栏 useCache:false；无页面跳 ErrorPage | 同页重复点击幂等；根路由切换清子栈；保留各功能领域状态而非保留所有 Widget 树 |
| `src/BD.WTTS.Client.Avalonia/Services.Implementation/UI/Widgets/NavigationService.cs`：`Navigate`、`GoBack` | useCache 控制导航堆栈；false 后 BackStack.Clear；子页面可堆栈；GoBack 不同类型检查；2500ms 后 TrimMemoryDebounced | 顶层 / 子层导航语义明确；返回状态恢复与标题栏 back 显隐匹配；不能让根页面无限叠栈 |
| `MainView.axaml.cs`：`OnFrameViewNavigated`、`AnimateContentForBackButton` | 按页面 / VM 更新选中项；按 BackStackDepth 显示后退；标题图标 margin 18→48 的 250ms 动画 | 后退、菜单选择、标题栏应同步；减少动画只能在保持位置和交互含义时处理 |

**键盘 / 输入证据**

在以上外壳、四个可见插件 UI 范围搜索 `KeyBinding / HotKey / KeyDown / KeyGesture / AccessKey`，未发现应用级自定义快捷键声明；不能凭习惯声称原版已有 Ctrl+F、F5、Alt+Left 等。`UI/Styling/Controls/Button.axaml` 的 RecognizesAccessKey=true、`TabStrip.axaml` TabNavigation=Once、`TaskDialogEx.axaml` 按钮区域 TabNavigation=Continue 表明需要保留正常 Tab / Shift+Tab 焦点、按钮键盘激活和弹窗焦点边界。Flutter 执行期须实测上游框架隐含行为，才决定对等键位；新增快捷键需作为明确增强项记录。

**托盘、关闭、恢复、手动启动**

- `src/BD.WTTS.Client.Avalonia/UI/App.TrayIcon.cs`：`InitTrayIcon` 仅主进程创建托盘；点击 RestoreMainWindow。默认菜单：启动 / 关闭 Steam（根据 IsRunningSteamProcess 动态标题）、打开主窗口、退出。`UpdateMenuItems` 先插入插件菜单，再默认菜单；子菜单支持 IsVisible / IsEnabled / CommandParameter。
- 托盘开启时 shutdown mode=OnExplicitShutdown；关闭时 OnMainWindowClose（UI_DEMO 另分支）。`MainWindow.OnClosing` 在 TrayIcon=true 时取消关闭、Hide 并 trim；调用基类仍可保存窗口尺寸；也清理 Steam rich presence。
- `src/BD.WTTS.Client.Avalonia/UI/App.Interface.cs`：`RestoreMainWindow` 优先复用 desktop.MainWindow 或 MainWindow；补 DataContext；失效后重建；最小化恢复 Normal、临时 Topmost、BringIntoView、ActivateWorkaround、Focus。保存状态应属于 Rust / 状态仓库，而非某一个旧窗口对象。
- 当前 dirty `App.axaml.cs` 不再直接用 `GeneralSettings.MinimizeOnStartup` 把所有启动都最小化；注释明确手动桌面启动应显示，只有显式 silence 场景最小化。设置字段和 UI 开关仍存在，须在启动来源语义中解释并测试。
- `src/BD.WTTS.Client.Avalonia/UI/Views/Abstractions/Windows/ReactiveAppWindow.cs`：WindowSizePositions 按窗口类型名；只在 WindowState.Normal 保存 ClientSize / Position。恢复时检查屏幕 bounds，现实现有 scalingOffset /正坐标限制，Flutter 需保持用户结果并修正负坐标多屏不可恢复等已知缺陷，不能照抄像素偏移 hack。

**2. 四模块注册与原有隐藏插件排除**

`MainWindowViewModel.allowedPluginNames`原来只允许Accelerator、GameAccount、GameList、SteamIdleCard导航，但`Initialize`仍遍历全部已加载激活插件。这是旧版行为证据，新版要纠正为严格四模块加载和初始化，不保留其他隐藏插件。

| 保留模块 | 源入口及当前页面 | 移植边界 |
|---|---|---|
| Accelerator | `src/BD.WTTS.Client.Plugins.Accelerator/Plugins/Plugin.cs` → MainFramePage | 社区加速AcceleratorPage2、脚本配置及实际SDK游戏加速能力 |
| GameAccount | `src/BD.WTTS.Client.Plugins.GameAccount/Plugins/Plugin.cs` → GameAccountPage | Steam及Windows其他平台账号；家庭共享按钮当前注释，不新增入口 |
| GameList | `src/BD.WTTS.Client.Plugins.GameList/Plugins/Plugin.cs` → MainFramePage | 库存、下载、挂机、编辑及云档/成就窗口 |
| SteamIdleCard | `src/BD.WTTS.Client.Plugins.SteamIdleCard/Plugins/Plugin.cs` → IdleCardPage | 挂卡、登录挑战、规则/顺序/黑名单、后台会话 |

旧`PluginsCore.LoadAssemblies/GetExports/InitPlugins`、`IPlugin`、`Startup.Host`仅作注册和删除定位：新版不从modules目录加载任意.NET DLL；三删除模块的配置/初始化/退出/路由/托盘/进程入口全部移除。PluginCount、设置页PluginResults、首页磁贴都只计当前有效四模块。DisablePlugins及safe mode保留四模块语义，残留ID规范化忽略。

`Accelerator.Plugin.HasValue`检查反代子进程存在，缺失返回CommunityFix_SubProcessFileNotExist；这一真实错误状态保留。源码/csproj存在不证明当前Release装有可用模块，执行期核实产物匹配。
**3. 四个当前可见模块的 UI / 命令种子**

| 页面 / 路径 | 活跃布局 / 操作契约与精确符号 |
|---|---|
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/MainFramePage.axaml(.cs)` | 右上 Tabs，社区加速→AcceleratorPage2、脚本配置→ScriptPage；InnerNavFrame.IsNavigationStackEnabled=false；切 Tab 70 水平 slide（首次 None） |
| 同目录 `AcceleratorPage2.axaml`；对应 `UI/ViewModels/AcceleratorPageViewModel.cs` | StartProxyCommand（启动 / 停止根据 ProxyStatus 切按钮）、RefreshCommand、ProxySettingsCommand；SetupCertificateCommand / DeleteCertificateCommand / ShowCertificateCommand / OpenCertificateDirCommand；EditHostsFileCommand / ResetHostsFileCommand / OpenHostsDirCommand / OpenLogFileCommand；代理模式选项。页内 tabs：未启动平台加速、启动后服务、已有脚本时加速脚本、启动后代理日志 / 流量、始终网络检测、隐藏 dummy tab |
| 同目录 `AcceleratorPage2.axaml` 游戏加速区域 | XunYouSDK.IsSupported 条件显示；GameAcceleratorService 的 ShowXunYouWindow、InstallAcceleratorCommand、UninstallAcceleratorCommand；AutoShowWattAcceleratorWindow 复选框。第三方 SDK 依赖能力应准确显示 |
| 同目录 `ScriptPage.axaml` / `ScriptStorePage.axaml` | AddNewScriptCommand、RefreshALLScriptCommand、DownloadScriptItemCommand、EditScriptItemCommand、RefreshScriptItemCommand、DeleteScriptItemCommand；脚本开关 / IsLoading / IsUpdate；自动更新与仅 Steam 浏览器设置；脚本商店下载和来源链接。源码中另有注释旧按钮不可重复算功能 |
| 同目录 `ProxySettingsPage.axaml` | ResetSettings；监听 IP /端口 /DNS、UseDoh→CustomDohAddres2 条件 enable、启动时代理、HTTP→HTTPS、仅脚本、二级代理类型 /地址 /端口 /用户名 /密码；修改后的生效范围应逐项追溯服务 |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Pages/GameAccountPage.axaml(.cs)` | 平台 TabView；LoginNewCommand、SaveCurrentUserCommand（非 Steam）、RefreshCommand；PlatformSettingButton（非 Steam）；显示账号用户名（Steam）。首次选平台且 ExePath 不存在会打开设置对话框；移除平台时确认，Steam 不可移除 |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Controls/AccountItems.axaml(.cs)`；`Models/PlatformAccount.cs` 等 | 当前 dirty 增加非 MostRecent 账号显式“切换”按钮；双击卡片也调用 SwapToAccountCommand；右键包含复制昵称 /账号、链接、EditRemarkCommand、SetAccountAvatarCommand、CreateShortcutCommand、DeleteAccountCommand；需要防止 XAML DoubleTapped + codebehind 双击处理导致重复发命令 |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Controls/PlatformSettingsPage.axaml` | SelectProgramPath；PlatformSettingsPageViewModel 绑定平台路径，GameAccountPage 对话框取消 /关闭验证路径。迁移应保留正常取消语义与错误提示，不制造无法关闭的无效路径对话框 |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Pages/SteamFamilyShareManagePage.axaml` | RemoveButton_Click；VM源 `UI/ViewModels/SteamFamilyShareManagePageViewModel.cs`；入口当前注释，旧家庭共享接口行为还需版本能力核对 |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/MainFramePage.axaml(.cs)` | 库存 / 下载 / 挂机 / 编辑 tabs；InnerNavFrame 禁用堆栈；切 Tab 水平 slide |
| 同目录 `GameListPage.axaml`；`UI/ViewModels/Pages/GameListPageViewModel.cs(.props.cs)` | RefreshAppCommand、ShowHideAppCommand；搜索 /分类 /已安装 /云档筛选；布局 tabs；游戏卡片右键 InstallOrStartAppCommand、EditAppInfoClickCommand、ManageCloudArchive_ClickCommand、UnlockAchievement_ClickCommand、AddAFKAppListCommand、NavAppToSteamViewCommand、NavAppScreenshotToSteamViewCommand、OpenFolderCommand、AddHideAppListCommand |
| 同目录 `DownloadPage.axaml`；`UI/ViewModels/Pages/DownloadPageViewModel.cs` | SteamConnectService.IsWatchSteamDownloading toggle、选所有 IsAllCheck、每项 IsWatchDownloading、下载 /安装字节、下载完成任务 Sleep/Hibernate/Shutdown（Windows 条件）；空状态文案。下载完成应依据选中任务状态，不凭列表暂无元素关机 |
| 同目录 `IdleAppsPage.axaml`；`UI/ViewModels/Pages/IdleAppsPageViewModel.cs` | RunOrStopAllButton_Click、Refresh_Click、DeleteAllButton_Click、RunStopBtnCommand、DeleteButtonCommand、IsAutoAFKApps；process 状态图标 / hover 操作，不允许 widget dispose 停任务 |
| 同目录 `EditAppsPage.axaml`；`UI/ViewModels/Pages/EditAppsPageViewModel.cs` | SaveSteamEditedApps、LoadSteamEditedApps、ExportSteamEditedAppsBackup、ImportSteamEditedAppsBackup、EditAppInfoClickCommand |
| 同目录 `EditAppInfoPage.axaml`；`UI/ViewModels/Pages/EditAppInfoPageViewModel.cs` | 名称 /封面 /启动项编辑；SteamGridDB 图片 /游戏 /作者链接，OpenSteamGridDBImageUrlCommand、OpenSteamGridDBAppUrlCommand、OpenSteamGridDBAuthorUrlCommand；UpLaunchItemCommand /DownLaunchItemCommand /DeleteLaunchItemCommand /AddLaunchItem；OpenFolder、ManageCloudArchive_Click；需继续按所有非注释绑定穷举字段 |
| 同目录 `HideAppsPage.axaml`；`UI/ViewModels/Pages/HideAppsPageViewModel.cs` | RemoveHideAppCommand |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Windows/CloudArchiveWindow.axaml`；`UI/ViewModels/Windows/CloudArchiveAppPageViewModel.cs` | RefreshList、UploadFile、ClearAllFiles、DownloadFile、DeleteFile，CloudGrid 选中项；上传 /删除需真实结果反馈和可恢复策略 |
| 同目录 `AchievementWindow.axaml`；`UI/ViewModels/Windows/AchievementAppPageViewModel.cs` | RefreshStats_Click、ResetAllStats_Click、SaveChange_Click；成就与统计编辑 |
| `src/BD.WTTS.Client.Plugins.SteamIdleCard/UI/Views/Pages/IdleCardPage.axaml`；`UI/ViewModels/IdleCardPageViewModel.cs(.props.cs)` | IdleRunStartOrStop、PriorityRunIdle、ToggleBlacklistIdle、NavAppToSteamViewCommand、LoginSteamCommand；规则 /順序 /并行数 /切换间隔 /最少时长 /徽章刷新；loading、IsLogin、RunState、Steam 未运行、私密游戏排除等状态文案。IdleManualRunNext 命令定义存在，但当前活跃 XML Command 没绑定它，不能仅凭 VM 算作可见按钮 |
| 同目录 `IdleSteamLoginPage.axaml`；`UI/ViewModels/IdleSteamLoginPageViewModel.cs(.props.cs)` | Login、CookieLogin；IdleCardPageViewModel.LoginSteam 优先加载已存会话，缺失 ViewState=0 登录，成功 ViewState=1 加载徽章；需保留账号不匹配错误。仅源码研究未读取会话实际值 |

**4. 设置 UI：不是五个独立页面**

`src/BD.WTTS.Client.Avalonia/UI/Views/Pages/Settings/SettingsPage.axaml` 在单一 StackPanel 顺序组合：Settings_General、Settings_UI、Settings_Plugin、Settings_Steam、AboutPage。右上 TabStrip Tag 指向同名区域。`SettingsPage.axaml.cs.SettingsScrollTab_Tapped` 调 BringIntoView；`SettingsScrollViewer_ScrollChanged` 按可见区域更新 selected tab。因此 Flutter 应采用单页带锚点滚动 /滚动联动标签，保留滚轮和键盘体验，不能直接改成五页路由。

| 区段源码 | 当前有 UI 控件的设置与动作 |
|---|---|
| `Settings_General.axaml(.cs)` | AutoRunOnStartup、MinimizeOnStartup、TrayIcon、GPU（标注重启生效）；App /AppData /Cache /Logs 打开目录，缓存 /日志大小异步计算 |
| `Settings_UI.axaml(.cs)` | SelectDefaultPage→StartDefaultPageName；Theme、UseSystemThemeAccent、ThemeAccent（颜色列表 /选择器）、SelectLanguage→Language、SelectFont→FontName；WindowBackgroundMaterial（None、Transparent、Blur、AcrylicBlur、Mica）、WindowBackgroundOpacity；自定义图开关 /选择 /预览 /透明度 /Stretch。Win11 runtime 默认和其它平台 fallback 都要分开 |
| `Settings_Plugin.axaml(.cs)` | PluginResults 列表，SwitchEnablePlugin_Click、OpenPluginDirectory_Click、OpenPluginCacheDirectory_Click（用于 AppDataDirectory 与 CacheDirectory）；禁用 /启用后提示重启；DeletePlugin_Click VM 存在但当前活跃 XML 未绑定，不能说有删除按钮 |
| `Settings_Steam.axaml(.cs)` | SteamProgramPath 选择、IsAutoRunSteam、IsRunSteamAdministrator（Windows）、IsRunSteamMinimized、IsRunSteamNoCheckUpdate、IsRunSteamChina、IsEnableSteamLaunchNotification、SteamStratParameter 编辑。SteamSkin /VGUI 等 schema 存在但未展示 |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/About/AboutPage.axaml` | About 区域；详细动态项、链接、法律通知还需执行时穷举，保持该区域位置 |

设置命令源：`src/BD.WTTS.Client/UI/ViewModels/Pages/SettingsPageViewModel.cs`、`.props.cs`、`.Plugin.cs`。OpenFolder/PluginOpenFolder 按目标路径 7.5秒防连点。背景图片检查存在并识别真实图片格式；错误 Toast。CheckUpdate 当前仅 Toast “不存在更新”，不是真实更新请求。SwitchEnablePlugin 修改 DisablePlugins 后提示重启，TODO 明确运行中菜单增删 /assembly unload 尚未完成。

**5. 101 个保留顶层字段与序列化编号完整种子清单**

下面每组的 Key 已核对 `_` 模型上 MP2Key 与 JsonPropertyOrder；除 GameAccelerator 外生成模型还声明 MPKey 同编号。这是**源码的 schema 与默认值**，未读取用户配置。JSON 默认 enum 使用字符串（ISettings.GetDefaultOptions→JsonStringEnumConverter），因此编号不能当 JSON 属性名；MessagePack 编号不能因 Rust 命名调整而重排。字段拼写中的 `SteamStratParameter`、`GameTypeFiltres`、`IdleSequentital`、`CustomDohAddres2` 是历史契约，要通过兼容别名保存。

来源缩写只在表里使用：

- G：模型 `src/BD.WTTS.Client/Settings/GeneralSettings.cs` /接口默认 `src/BD.WTTS.Client/Settings/Abstractions/IGeneralSettings.cs`。
- U：模型 `src/BD.WTTS.Client/Settings/UISettings.cs` /接口默认 `src/BD.WTTS.Client/Settings/Abstractions/IUISettings.cs`。
- S：模型 `src/BD.WTTS.Client/Settings/Steam/SteamSettings.cs` /接口默认 `src/BD.WTTS.Client/Settings/Abstractions/ISteamSettings.cs`。
- P：模型 `src/BD.WTTS.Client.Plugins.Accelerator/Settings/ProxySettings.cs` /接口默认 `src/BD.WTTS.Client.Plugins.Accelerator/Settings/Abstractions/IProxySettings.cs`。
- A：模型 `src/BD.WTTS.Client.Plugins.GameAccount/Settings/GameAccountSettings.cs` /接口默认 `src/BD.WTTS.Client.Plugins.GameAccount/Settings/Abstractions/IGameAccountSettings.cs`。
- L：模型 `src/BD.WTTS.Client.Plugins.GameList/Settings/GameLibrarySettings.cs` /接口默认 `src/BD.WTTS.Client.Plugins.GameList/Settings/Abstractions/IGameLibrarySettings.cs`。
- I：模型 `src/BD.WTTS.Client.Plugins.SteamIdleCard/Settings/SteamIdleSettings.cs` /接口默认 `src/BD.WTTS.Client.Plugins.SteamIdleCard/Settings/Abstractions/ISteamIdleSettings.cs`。
- X：模型 /默认 `src/BD.WTTS.Client.Plugins.Accelerator/Settings/GameAcceleratorSettings.cs`。

| 来源-Key | 字段（原名） | 原类型 | 源码默认 |
|---|---|---|---|
| G-0 | AutoCheckAppUpdate | bool | true |
| G-1 | UpdateChannel | UpdateChannelType | Auto |
| G-2 | AutoRunOnStartup | bool | false |
| G-3 | MinimizeOnStartup | bool | false |
| G-4 | TrayIcon | bool | true |
| G-5 | MessagePopupNotification | bool | true |
| G-6 | GameListUseLocalCache | bool | false |
| G-7 | TextReaderProvider | Dictionary<Platform,string>? | null |
| G-8 | HostsFileEncodingType | EncodingType | Auto |
| G-9 | GPU | bool | true |
| G-10 | NativeOpenGL | bool | false |
| G-11 | ScreenCapture | bool | false |
| G-12 | DisablePlugins | HashSet<string>? | null |
| G-13 | PluginSafeMode | bool | true |
| G-14 | LastLookNoticeDateTime | DateTimeOffset | default |
| G-15 | WebProxyMode | AppWebProxyMode | default=FollowSystem(0) |
| G-16 | CustomWebProxyModeHost | string? | null |
| G-17 | CustomWebProxyModePort | int | 0 |
| G-18 | CustomWebProxyModeAddress | string? | null |
| G-19 | CustomWebProxyModeBypassOnLocal | bool | false |
| G-20 | CustomWebProxyModeCredentialUserName | string? | null（敏感字段，不读取实际值） |
| G-21 | CustomWebProxyModeCredentialPassword | string? | null（敏感字段，不读取实际值） |
| G-22 | CustomWebProxyModeCredentialDomain | string? | null |
| U-0 | Theme | AppTheme | FollowingSystem |
| U-1 | ThemeAccent | string? | #FF0078D7（实际字符串含 alpha，不仅 RGB） |
| U-2 | UseSystemThemeAccent | bool | true |
| U-3 | Language | string? | null（系统默认语义） |
| U-4 | MessageBoxDontPrompts | HashSet<MessageBox.DontPromptType>? | null |
| U-5 | IsShowAdvertisement | bool | true（当前简化 Home 未展示该源功能） |
| U-6 | WindowSizePositions | ConcurrentDictionary<string,SizePosition>? | null |
| U-7 | FontName | string? | null（实际 default family 为 HarmonyOS Sans SC） |
| U-8 | GameListGridSize | int | 150 |
| U-9 | Fillet | bool | false |
| U-10 | WindowBackgroundOpacity | double | Win11AtLeast ? 0.0 : 0.8 |
| U-11 | WindowBackgroundMaterial | WindowBackgroundMaterial | Win11AtLeast ? Mica : AcrylicBlur |
| U-12 | WindowBackgroundDynamic | bool | false |
| U-13 | WindowBackgroundCustomImage | bool | false |
| U-14 | WindowBackgroundCustomImagePath | string? | avares://BD.WTTS.Client.Avalonia/UI/Assets/back.png |
| U-15 | WindowBackgroundCustomImageOpacity | double | 0.8 |
| U-16 | WindowBackgroundCustomImageStretch | XamlMediaStretch | UniformToFill |
| U-17 | SortMenuTabs | HashSet<string>? | null |
| U-18 | StartDefaultPageName | string? | null（Home fallback） |
| S-0 | SteamStratParameter | string? | null |
| S-1 | SteamSkin | string? | null |
| S-2 | SteamProgramPath | string? | null |
| S-3 | IsAutoRunSteam | bool | false |
| S-4 | IsRunSteamMinimized | bool | false |
| S-5 | IsRunSteamNoCheckUpdate | bool | false |
| S-6 | IsRunSteamChina | bool | false |
| S-7 | IsRunSteamVGUI | bool | false |
| S-8 | IsEnableSteamLaunchNotification | bool | true |
| S-9 | DownloadCompleteSystemEndMode | OSExitMode | Sleep |
| S-10 | IsRunSteamAdministrator | bool | false |
| P-0 | IsAutoCheckScriptUpdate | bool | true |
| P-1 | IsEnableScript | bool | false |
| P-2 | SupportProxyServicesStatus | IReadOnlyCollection<string>? | empty string array |
| P-3 | ScriptsStatus | IReadOnlyCollection<int>? | empty int array |
| P-4 | ProgramStartupRunProxy | bool | false |
| P-5 | SystemProxyPortId | ushort | 26561 |
| P-6 | SystemProxyIp | string? | IPAddress.Any.ToString() = 0.0.0.0 |
| P-7 | OnlyEnableProxyScript | bool | false |
| P-8 | ProxyMasterDns | string? | 223.5.5.5 |
| P-9 | EnableHttpProxyToHttps | bool | true |
| P-10 | Socks5ProxyEnable | bool | false |
| P-11 | Socks5ProxyPortId | ushort | 8868 |
| P-12 | TwoLevelAgentEnable | bool | false |
| P-13 | TwoLevelAgentProxyType | ExternalProxyType | IReverseProxyService.Constants.DefaultTwoLevelAgentProxyType=Socks5 |
| P-14 | TwoLevelAgentIp | string? | IPAddress.Loopback.ToString() = 127.0.0.1 |
| P-15 | TwoLevelAgentPortId | ushort | 7890 |
| P-16 | TwoLevelAgentUserName | string? | null（敏感字段，不读取实际值） |
| P-17 | TwoLevelAgentPassword | string? | null（敏感字段，不读取实际值） |
| P-18 | ProxyMode | ProxyMode | Hosts（最终平台合法性另做 ProxyModeValue fallback） |
| P-19 | IsProxyGOG | bool | false |
| P-20 | IsOnlyWorkSteamBrowser | bool | false |
| P-21 | UseDoh | bool | true |
| P-22 | CustomDohAddres2 | string? | null |
| P-23 | AcceleratorTabsSelectedIndex | int | 0 |
| P-24 | AutoShowWattAcceleratorWindow | bool | true |
| P-25 | ProxyBeforeDNSCheck | bool | true |
| A-0 | AccountRemarks | ConcurrentDictionary<string,string?>? | empty map |
| A-1 | DisableAuthorizedDevice | IReadOnlyCollection<DisableAuthorizedDevice>? | empty array |
| A-2 | EnablePlatforms | HashSet<string>? | empty set |
| A-3 | PlatformSettings | ConcurrentDictionary<string,PlatformSettings>? | empty map |
| A-4 | IsShowAccountName | bool | false |
| L-0 | GameInstalledFilter | bool | false |
| L-1 | GameLibraryLayoutType | GridLayoutType | Grid |
| L-2 | GameCloudArchiveFilter | bool | false |
| L-3 | GameTypeFiltres | List<SteamAppType>? | [Game,Application,Demo,Beta] |
| L-4 | HideGameList | Dictionary<uint,string?>? | empty map |
| L-5 | AFKAppList | Dictionary<uint,string?>? | empty map |
| L-6 | IsAutoAFKApps | bool | true |
| I-0 | IdleTime | TimeSpan | FromMinutes(6) |
| I-1 | IdleRule | IdleRule | FastMode |
| I-2 | IdleSequentital | IdleSequentital | Default |
| I-3 | MaxIdleCount | int | 30 |
| I-4 | MinRunTime | double | 2（小时） |
| I-5 | SwitchTime | double | 5000（毫秒） |
| I-6 | RefreshBadgesTime | double | 6（分钟） |
| I-7 | BlacklistAppList | Dictionary<uint,string?>? | empty map |
| X-0 | MyGames | Dictionary<int,XunYouGameViewModel> | empty map |
| X-1 | WattAcceleratorDirPath | string | Windows: ProgramFilesX86/WattAccelerator；其它平台 empty string |

保留总计：G23 + U19 + S11 + P26 + A5 + L7 + I8 + X2 = **101**。ASFSettings的14字段已明确排除，不注册、不迁移。P/X/A/L/I从对应保留模块GetConfiguration加载；目录与核心设置分开。

嵌套类型补充（不算上述 101 保留顶层）：`src/BD.WTTS.Client.Plugins.GameAccount/Models/PlatformSettings.cs` 的 PlatformPath:string?，MessagePack.Key(0)/MP2Key(0)；`src/BD.WTTS.Client/Models/SizePosition.cs` 的 X:int-Key0、Y:int-Key1、Height:double-Key2、Width:double-Key3。DisableAuthorizedDevice 和 XunYouGameViewModel 的更多嵌套字段仍需最终schema普查，严禁为求方便丢弃复杂值。

`src/BD.WTTS.Client/Settings/Abstractions/IPartialGameAccountSettings.cs`、`IPartialGameLibrarySettings.cs` 是以上属性的部分接口访问投影，不能重复计数。删除模块专属数据库/领域数据不进入本次迁移；四模块实体和嵌套字段继续普查。

`src/BD.WTTS.Client/Models/AppSettings.cs` 是微服务资源设置（ApiBaseUrl、AesSecret、RSASecret、已弃用 AppVersion 等）；它不是普通用户设置。只记录结构 /依赖，不读取或移植秘钥资源实际值，不把官方渠道资源假装新产品已经拥有。

**迁移与生效语义**

`src/BD.WTTS.Client/Settings/Infrastructure/Abstractions/ISettings.cs`：文件位置由 appDataDirectory / Settings / <Name>.json 组合；DirectoryExists 可能创建目录，本研究只读取该方法源码，未调用。GetDefaultOptions：DefaultIgnoreCondition.Never、IgnoreReadOnlyProperties=true、IncludeFields=false、WriteIndented=true、JsonStringEnumConverter。`SettingsPropertyBase.cs` 有 AutoSave、Reset、Subscribe、SaveNameStatus 避免重复配置监听；不能把每次 Slider onChanged 同步 fsync 到 UI线程。

`App.axaml.cs.InitSettingSubscribe`：TrayIcon 即时更新；UseSystemThemeAccent /ThemeAccent 即时；AutoRunOnStartup 调 SetBootAutoStart；WindowBackgroundMaterial 更新所有窗口；语言 /字体通过 VM和服务变更；GPU 为重启生效提示。执行模型必须为每个字段标记“存储 /仅 UI /即时 /重启 /下次启动 /下次代理启动 /后台任务使用 /隐藏兼容”。没有界面入口的字段也要 round-trip 与保留未知字段。

`src/BD.WTTS.Client.Plugins.Accelerator/Settings/ProxySettings.ProxyMode.cs`：Windows Hosts、DNSIntercept（除 REMOVE_DNS_INTERCEPT）、PAC、System；Android VPN、ProxyOnly；Linux/macOS Hosts、System；ProxyModeValue 对不合法平台值回退 ProxyModes[0]。构建宏、运行平台和schema值三个层面要分开。

**6. 当前外观、字体、渲染及 dirty 优化**

| 源码 /本地状态 | 事实 | Flutter/Rust 保留目的和验证 |
|---|---|---|
| `src/BD.WTTS.Client.Avalonia/UI/App.axaml.cs`：DefaultFontFamilyName | 默认内嵌 `UI/Assets/Fonts/HarmonyOS_Sans_SC_Regular.ttf#HarmonyOS Sans SC`；不是这次 dirty 新增 | 字体资源 /许可可复用后配置 Flutter family、fallback；中英混排、中文标点、不同weight、DPI基线逐项截图比对 |
| `src/BD.WTTS.Client.Avalonia.App/Program.cs`：BuildAvaloniaApp | FontFallbacks 顺序 HarmonyOS Sans SC→FontFamily.Default；GPU与NativeOpenGL；Windows AngleEgl→Vulkan→可选Wgl→Software，Linux Glx/Egl→Software，macOS OpenGl→Software | Flutter 渲染器不是 Avalonia，不能承诺相同枚举可透传；保留“GPU开关 /可用渲染fallback /老设备稳定”用户语义并核实支持 |
| 同上：SkiaOptions dirty | GPU资源预算从 1,024,000,000 改 64×1024×1024=64MiB | 实际缓存与压力目标来自测量；Flutter不套用同一参数但图片 /文本 /shader资源必须有界 |
| `src/BD.WTTS.Client.Avalonia/UI/Styling/Themes.axaml`、Theme/Light.axaml、Dark.axaml、HighContrastTheme.axaml；CustomTheme.axaml；Controls/各控件 | FluentAvalonia + 自定义theme /材质 /圆角 /强调色 /资源大小；Fonts.axaml 默认12、Medium14、description10、min8 | 抽取token映射而非直接套 Material defaults；视觉改善限定字体、边缘、阴影、渲染清晰度、hover/pressed/focus，不重排 |
| `src/BD.WTTS.Client.Avalonia/UI/Styling/Window.axaml` | 无边框template，背景材质 /自定义背景图片 /透明度 | 平台材质与绘制fallback分开；透明度变更不导致整个重绘树长期60fps |
| `src/BD.WTTS.Client.Avalonia.App/BD.WTTS.Client.Avalonia.App.csproj` dirty | AssemblyName SteamBox→Steam++，ApplicationId net.steambox.app→net.steampp.app | SteamBox是用户称呼；实际当前本地输出身份已改，最终产品名 /迁移查找路径不能只凭文件夹或旧shortcut |
| 同上 dirty | GC DynamicAdaptationMode=1、ConserveMemory=5、HighMemoryPercent 75→60；ServerGC=false、RetainVM=false、Concurrent=true原已有 | Rust无需.NET GC；不照搬调参，把工作集 /private bytes /commit /frame latency都测量，优先真实对象生命周期 |
| `MainWindow.axaml.cs` dirty：TrimMemory /TrimMemoryDebounced | Aggressive full GC×2、WaitForPendingFinalizers、Windows EmptyWorkingSet /SetProcessWorkingSetSize(-1)；最小化 /关闭、失焦1.5s、每45s、Opened后4s、导航2.5s触发 | 这是本地为了内存优化的实现，不能作为“最佳性能”的证据。定时trim可让工作集看起来更小而增加分页 /卡顿；Flutter/Rust应用目标是释放真实资源、暂停隐藏任务和有界缓存，回归看用户结果 |
| `NavigationService.cs` + `MainView.axaml.cs` dirty | 顶层 useCache:false，清BackStack，触发debounced trim；NavigateFromContext改页面类型映射并catch | 保留稳定导航 /释放离开页面资源；领域状态独立，不把旧类型映射 /GC hack逐字照搬 |
| `App.Interface.cs` dirty | 窗口复用与重建路径修复 | 托盘反复显示、关闭、最小化恢复、二次启动唤起、无空白页须回归 |
| `App.axaml.cs` dirty | 手动启动显示，仅显式silence最小化 | 保留桌面启动使用体验 |
| `MainWindowViewModel.cs` dirty | 白名单移除 ArchiSteamFarmPlus | 当前导航一致，不恢复隐藏 |
| `AccountItems.axaml(.cs)` dirty | “切换”按钮恢复，双击修复（含额外AddHandler） | 操作单发 /busy feedback /切换完成前不乐观显示新当前账号 |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/MainFramePage.axaml.cs` dirty | diff仅新增using并移除两行旧Tab注释；切Tab仍走原InnerNavFrame.Navigate | 切Tab行为没有新的本地语义，不应推断此diff实现了新的缓存 /任务生命期 |

额外 dirty 源码列表（UI结果相关，细节由其它功能研究补全）：`src/AssemblyInfo.Constants.cs`、`src/Avalonia.Skia.Internals/ClassicDesktopStyleApplicationLifetime.cs`、`src/BD.WTTS.Client/Startup/Startup.*.cs`、`src/BD.WTTS.Client.Plugins.Accelerator/Services/Mvvm/ProxyService*.cs`、`src/BD.WTTS.Client.Plugins.GameAccount/Services.Implementation/SteamPlatformSwitcher.cs`、`src/BD.WTTS.Client/Services.Implementation/Steam/SteamServiceImpl2.cs` 等。必须保存完整dirty基线而非新clone覆盖；研究未修改这些文件。

**7. 精确验收种子（最终测试用合成资料）**

1. 960×680、528×508和实际默认恢复尺寸，在100/125/150/200%缩放匹配布局；拖动、resize、最大化、负坐标显示器、材质fallback、CJK /英文 /emoji /高对比。
2. 启动Home、默认页、被禁用默认页fallback、菜单缺少Home的本地情况、进入子弹窗 /返回、同导航连点、切四模块 /内tab、设置锚点滚动联动、保留筛选与滚动位置。
3. 托盘开启 /关闭，主窗X、托盘退出、隐藏后再唤起、二次启动、手动启动vs自动silence启动；代理 /挂机任务独立于Widget生命期，退出清理符合现有语义。
4. 所有101保留顶层字段、上述已知嵌套字段和最终普查补项，在旧JSON→新schema→导出中保留值、null/empty差异、字符串enum、MessagePack编号、未知字段；损坏资料可诊断不覆盖，复杂值无损。
5. 设置即时 /重启生效逐项验收；背景图不可读 /不是图片 /修改路径 /取消，字体缺失fallback，禁用插件重启、反代子进程缺失状态、删除模块不能加载或复活。
6. 账号卡按钮 /双击 /右键三种路径指向同Rust操作，single-flight防双发，当前账号标记只随后端完成更新；平台首设路径对话框可取消，Steam tab不可删。
7. 未启动 /运行中 /启动失败 /停止中代理状态，tab可见条件、脚本增删更新与loading，日志 /流量列表有界，XunYou capability降级准确。
8. 库存搜索筛选 /布局 /右键 /云档 /成就 /编辑 /备份 /挂机；下载完成操作仅在明确选中任务真正完成触发；隐藏UI、排序、tab切换不误触发系统退出。
9. 挂卡登录 /失效 /账号不匹配 /缺Steam /私密游戏 /黑名单 /任务切换 /停止；任务计时单位准确，加载与empty state不混用。
10. 包含端到端时延 /帧耗时 /private bytes /CPU /文件句柄 /GPU预算 /UI隐藏负载的性能对照；不能用定时EmptyWorkingSet后一个瞬间的内存数字通过验收。

最终执行清单还必须继续追踪所有非注释AXAML绑定、code-behind事件、VM命令、托盘贡献、领域持久化和平台构建条件；本文件明确保持“证据已知 /源码候选 /运行未验证”的界线。

**B. Steam账号、游戏库、挂卡与共享登录**

**SteamBox 领域功能、原生依赖与数据迁移研究**

研究日期：2026-10-01。只读研究本地 `C:/Users/Colby/.gemini/antigravity/scratch/SteamTools`，以主仓库 HEAD + 主仓库未提交改动 + 各层子模块 HEAD/未提交改动作为实际基线。没有启动应用、Steam，没有访问真实账号配置、数据库、Cookie、令牌、日志、签名密钥，没有修改项目文件。以下源路径均相对该仓库。后面的 Rust 分层和验收规则是建议，源码事实单独标明。

**1. 范围：仅四模块及共享支撑**

网络模块见C部分；本部分覆盖GameAccount、GameList、SteamIdleCard和所需Steam原生/登录服务。三个隐藏插件按用户指令彻底删除，不移植其业务/数据/设置/后台初始化。

| 保留领域 | 主要入口 | 必须覆盖 |
|---|---|---|
| GameAccount | `src/BD.WTTS.Client.Plugins.GameAccount/UI/ViewModels/GameAccountPageViewModel.cs` | Steam/其他平台账号、备注/头像/快捷方式/托盘、缓存恢复及本地实际可达行为 |
| GameList | `src/BD.WTTS.Client.Plugins.GameList/UI/ViewModels/Pages/GameListPageViewModel.cs` | 库、过滤/编辑/封面/备份、下载、AFK、成就、云文件 |
| SteamIdleCard | `src/BD.WTTS.Client.Plugins.SteamIdleCard/UI/ViewModels/IdleCardPageViewModel.cs` | 登录/徽章、四算法/四排序、单并行、黑名单、token刷新 |
| 共享Steam服务 | SteamClient和本体的实际调用链 | 原生worker、账号epoch、Web会话/验证码挑战、秘密保护，不包含OTP令牌库 |
**2. GameAccount：账号切换与多平台缓存**

**2.1 Steam 源功能**

`Services.Implementation/SteamPlatformSwitcher.cs` 实现 `IPlatformSwitcher`：

- `GetUsers` 从 `SteamConnectService.Current` 刷新 `SteamUsers`，后台刷新昵称/头像，并生成账号托盘子菜单；账号按 `LastLoginTime` 排序。
- `SwapToAccount` 停止 Steam 相关进程，修改 Windows 自动登录用户名/其他平台注册表 VDF 等平台设置，更新本地 `loginusers.vdf` 的当前账号标记，按账号设置离线模式和 PersonaState，重新启动 Steam。
- `ClearCurrentLoginUser`、`NewUserLogin` 清空当前自动登录用户名以进入新账号登录。
- `DeleteAccountInfo` 先确认删除账号信息，再询问是否删除该 Steam 用户的本地 userdata；两种行为不能合并。
- `SteamAccount` 保留 SteamId64、AccountName、DisplayName、AliasName、LastLoginTime、MostRecent、WantsOfflineMode、SkipOfflineModeWarning、RememberPassword、PersonaState、AvatarMedium、动画头像/头像框等。
- `PlatformAccount.cs` 的命令包括切换、删除、头像替换、备注编辑、复制、打开链接、创建桌面账号快捷方式。
- `CreateShortcut` 生成头像 ICO 和 Windows `.lnk`，调用自定义 URL scheme 传递 `-clt steam -account ...`，快捷方式命令契约要向后兼容，且路径/参数不能作为拼接 shell 文本。

**2.2 本地未提交行为必须保留**

以下是本地工作区变化，不可仅拉取上游重新生成：

1. `src/BD.WTTS.Client.Plugins.GameAccount/Services.Implementation/SteamPlatformSwitcher.cs` 在停进程后新增 300ms、写数据后新增 200ms；选择目标账号改为优先 `SteamUser.AccountName`，用 SteamId64 或忽略大小写用户名匹配，而非仅精确比较用户名。
2. `UI/Views/Controls/AccountItems.axaml` 恢复可见“切换”按钮，对当前最近登录账号隐藏；双击事件连接调整。`AccountItems.axaml.cs` 新增从实际视觉祖先获取 `AppItem.DataContext` 的双击处理。
3. `src/BD.WTTS.Client/Services.Implementation/Steam/SteamServiceImpl2.cs::KillSteamProcess` 先尝试普通权限停止，失败才经 `IPCRoot` 提权服务回退。
4. `ref/SteamClient/src/BD.SteamClient/Services.Implementation/SteamServiceImpl.cs` 停止列表移除了 `steamservice`；等待退出改为 `WaitForExit(3000)`；写 `loginusers.vdf` 时只有目标账号 `AutoLogin=AllowAutoLogin=MostRecent=RememberPassword=1`，更新 `Timestamp`；其他账号 AutoLogin/AllowAutoLogin/MostRecent 归零。

Rust 移植应保留这些最终行为，使用可取消的退出等待、文件稳定条件和原子写入替代不可解释的固定延迟。不要复制“无论前面是否失败都 return true”的旧实现缺陷。`MostRecent` 仅是本地标记，不是 Steam 登录成功证明。完成账号切换需核对重新连接后 Steam 原生返回的实际 SteamId64。

**2.3 推荐切换事务**

`Idle → Validating(target SteamId) → Stopping workers → Stopping Steam → Snapshotting → Updating local login data → Starting Steam → Waiting native identity → Completed/Needs login/Failed`。

- 全应用只允许一个 Steam 账号切换事务。停止同账号 AFK/挂卡/成就/云工作器，废止旧 account_epoch，保证 UI 的旧操作结果无法写回新账号。
- 区分 Steam 未启动、游戏/下载正在运行、退出超时、账号失效、文件不可写、需要用户重新登录。外部 Steam 登录过程不应卡住 Flutter UI。
- `loginusers.vdf`、`localconfig.vdf` 和注册表写前复制快照，保存未知字段，检测 Steam 文件并发变化，写临时文件并原子替换；注册表和文件跨资源事务用日志记录+补偿恢复。
- 仅停止本应用已识别的 Steam 进程或本应用工作器，不按模糊名称误杀同名程序；普通权限优先，明确失败才用有界提权接口。
- 用户取消在可恢复阶段执行补偿；Steam 已启动后返回真实状态，不把“取消等待”包装成“恢复旧账号成功”。

**2.4 其他平台**

配置：`src/BD.WTTS.Client.Plugins.GameAccount/UI/Assets/Platforms.json`；内置 Epic Games、Ubisoft、EA Desktop、Riot Games、Rockstar、BattleNet、GOG Galaxy、Discord。`GameAccountPageViewModel.LoadPlatforms` 将非 Steam 平台限制为 Windows。

`BasicPlatformSwitcher.cs`：保存当前账号、停止平台进程、备份/清理当前登录文件、将目标账号缓存复制回去、写注册表或 JSON 指定字段、重新启动。`UniqueIdType` 包含 `CREATE_ID_FILE`、`REGKEY`、`REGEX`、`JSON_SELECT`、`JSON_SELECT_FIRST`、`JSON_SELECT_LAST`、`FILE_MD5`。平台描述还含 `LoginFiles`、`ClearPaths`、`CachePaths`、`BackupPaths`、include/ignore 扩展名、`ExesToEnd`、启动参数、`AllFilesRequired`。

迁移需保留插件 AppData 下平台账号目录、`ids.json`、`reg.json`、头像 ICO、平台路径覆盖；这些目录可能含登录凭据，执行模型不能递归读取后写进报告或测试 fixture。实际迁移由用户启动应用在本机完成，对受保护缓存封装加密，使用合法平台适配器边界。外部客户端更新导致格式失效时返回“适配器不兼容/需重新登录”，不能凭空保证所有历史缓存永久有效。

**2.5 旧家庭共享授权**

`UI/ViewModels/SteamFamilyShareManagePageViewModel.cs` 从 `ISteamService.GetAuthorizedDeviceList` 读取 `config.vdf` 的 `AuthorizedDevice`，结合账号缓存与 `DisableAuthorizedDevice` 集合展示；支持删除授权条目、临时禁用、保存。源码中上下移动/置顶相关命令有注释。

这是旧共享授权数据模型。方案不能把它称为新 Steam Families 管理，更不能保证编辑本地 VDF 会改变 Valve 当前的服务器端家庭状态。迁移旧数据并标明能力来源；新版服务器能力要另做协议和真实账号测试。

**2.6 GameAccount 设置字段**

`Settings/Abstractions/IGameAccountSettings.cs`：`AccountRemarks`、`DisableAuthorizedDevice`、`EnablePlatforms`、`PlatformSettings`、`IsShowAccountName`。`Models/PlatformSettings.cs`：`PlatformPath`。Steam 全局路径、启动参数、管理员启动等设置另外来源于 `SteamSettings`，不能漏掉。

**3. GameList：游戏库、编辑、下载、AFK、成就、云存档**

**3.1 页面行为清单**

- `GameListPageViewModel`：游戏名和 AppId 搜索、按类型统计、已安装/支持云存档/类型过滤、布局设置、排序、刷新，游戏安装/运行 URL，跳 Steam 对应游戏及截图页，打开安装目录。
- `EditAppInfoPageViewModel`：编辑游戏展示数据、启动项新增/删除/上下移、选择本地图片或 SteamGridDB 的 Grid/Logo/Hero/Header、保存/取消/重置。封面会写 Steam `userdata/<SteamId32>/config/grid`，账号改变时绝不能落在另一个账号目录。
- 保存当前编辑后标 `IsEdited`；`EditAppsPageViewModel.SaveSteamEditedApps` 再批量落盘 Steam appinfo，随后可重启 Steam。需要保留此两步操作含义，UI 显示“保存到本应用”和“应用到 Steam”的明确状态，不要让用户误以为第一次保存已应用所有元数据。
- `EditAppsPageViewModel.ExportSteamEditedAppsBackup`/`ImportSteamEditedAppsBackup` 导出/导入 `.stmbak`，是 MessagePack 序列化 `List<ModifiedApp>`。`ModifiedApp` 索引键为 0 AppId、1 OriginalData bytes、2 ChangesData bytes，变更数据是 Steam 二进制属性表。不能以扩展名为依据直接当 JSON。
- `HideAppsPageViewModel`：隐藏列表和恢复显示；`IdleAppsPageViewModel`：AFK 游戏列表、开始/停止、移除及自动 AFK。
- `SteamConnectService.SteamAFKMaxCount=32`；当前通常为每个 AppId 拉起本应用的 `-clt app -silence -id ...` 子进程。并行 hang/card 与成就/Cloud 一起需要工作器共享登记和所有权。
- `DownloadPageViewModel` 监听 manifest 下载状态、选择等待的游戏和全选三态，全部选中项结束后执行系统睡眠、休眠或关机等设置。
- `SteamConnectService.WatchDownloadingComplete` 当前通知文案写 30 秒，但其源路径直接调用电源操作，没有在该方法看到可取消 30 秒等待；不能把文案当已实现倒计时。重构必须实现可见可取消倒计时和执行前重新检查下载状态，避免游戏状态瞬时抖动触发系统操作。

**3.2 Steam 文件格式，必须做独立兼容层**

`ref/SteamClient/src/BD.SteamClient/Services.Implementation/SteamServiceImpl.cs` 实际处理：

| 文件 | 格式/用途 | 新实现要求 |
|---|---|---|
| `Steam/config/loginusers.vdf` | 文本 Valve KeyValues，账号及本地自动登录标记 | 保留未知键和 Unicode；写入原子性和冲突检测 |
| `Steam/config/config.vdf` | 文本 VDF，旧授权设备等 | 不把旧设备条目等同新家庭服务状态 |
| `Steam/userdata/<id32>/config/localconfig.vdf` | 文本 VDF，PersonaState 等 | 绑定 SteamId64→Id32 的明确转换 |
| 非 Windows `registry.vdf` | Steam 本地注册表替代文件 | 平台路径探测和未知字段保留 |
| `Steam/steamapps/libraryfolders.vdf` | Steam 库目录，存在历史/新格式 | 多库路径、空目录、Unicode、文件失效检测 |
| 库目录中的 `appmanifest_*.acf` | 文本 VDF，安装/下载状态 | 读取稳定快照，写入期间不误判下载完成 |
| `Steam/appcache/appinfo.vdf` | 二进制 AppInfo，不是文本 VDF | 3 代 magic 与 string pool 兼容，未知字段原样往返 |
| `Steam/appcache/stats/UserGameStatsSchema_<AppId>.bin` | 成就/统计 schema，源代码识别新旧格式 | 独立受限解析器和 fuzz/property 测试 |
| 应用 AppData `modifications.vdf` | MessagePack `List<ModifiedApp>`，虽扩展名 vdf 但不是普通VDF | 兼容旧字段索引和 binary property table |
| `.stmbak` | 同上备份格式 | 导入预检、AppId 匹配报告、未知字段留存 |
| `userdata/<id32>/config/grid` | 用户自定义封面和 logo/hero 图 | 当前账号校验，图片尺寸/字节限制 |

`GetAppInfos` 读取 magic 常量 123094055、123094056、123094057，V3 含字符串表偏移和字符串池；`SteamApp.FromReader`/`Write` 与 `SteamAppPropertyTable` 处理具体二进制记录。当前 `SaveAppInfosToSteam` 一律重写 V3，大文件全量内存拼接且源代码备份被注释。Rust 版本应有“读格式→保留原记录/未知字段→仅变更目标字段→临时写→复读验证→备份原文件→替换”的受控流程；不支持格式绝不降级为空库后覆盖原文件。

离线 `appinfo.vdf` 是本地缓存信息，不能直接把缓存出现过的所有游戏当成当前账号已购库存。原生连接可用时，用 `OwnsApps` 等订阅查询过滤并标记来源；离线页面显示离线缓存状态，账号识别缺失时不伪造拥有状态。

**3.3 成就与统计**

`UI/ViewModels/Windows/AchievementAppPageViewModel.cs` 初始化指定 AppId 的 Steam 原生连接，加载 schema，注册 `UserStatsReceived` 回调、请求/刷新统计，读成就解锁和解锁时间，编辑 `IsAchieved`，读写 int/float stat，存储 `StoreStats`，支持 `ResetAllStats` 可选一起重置成就。定义里有 protected stats 和 int/float限制，必须保留受保护字段不可编辑规则。重置源流程有多次确认，重构保留按影响理解的确认而不是删去安全语义。

主页面 `UnlockAchievement_Click` 仅对应用/游戏类型开放，要求 Steam 已启动，保留风险说明和记住选择设置，再启动成就工作进程。Steam Cloud 也以独立工作进程启动。

**3.4 Cloud 文件操作**

`CloudArchiveAppPageViewModel`：列表、时间排序、数量、配额、单文件下载、单文件删除、多文件上传、全部删除、刷新。背后是 `ISteamworksLocalApiService.GetCloudArchiveFiles/GetCloudArchiveQuota` 与 `SteamRemoteFile.ReadAllBytes/WriteAllBytes/Delete`，不是简单下载 Steam 远端 HTTP 页。

迁移要修正旧 `int` 配额截断，Rust 内部用 u64 和 streaming/bounded buffers。必须区分 Steam 原生“成功写本地远程存储接口”和“云已同步”；检查账号/AppId/文件名、重名覆盖、同步冲突、配额不足、API 返回失败。删除全部和覆盖上传需明确影响并提供已下载备份路径；对大文件后台执行并可取消。

**3.5 GameLibrary 设置字段**

`GameInstalledFilter`、`GameLibraryLayoutType`、`GameCloudArchiveFilter`、`GameTypeFiltres`（旧拼写要兼容）、`HideGameList`、`AFKAppList`、`IsAutoAFKApps`。`IsAutoAFKApps` 源默认 true，具体新产品默认是否变更需在迁移计划标出，不能静默改变已有用户设置。下载完成电源模式来自 `SteamSettings`。

**4. Steam 原生接口：Rust 不能直接替换所有 .NET 依赖**

实际绑定并不只是 top-level `ref/Facepunch.Steamworks`：`ref/SteamClient/src/BD.SteamClient/Services.Implementation/SteamworksLocalApiServiceImpl.cs` 直接使用 `SAM.API.Client`，委托 Steam 安装 DLL 路径，再用 SteamClient vtable 接口。对应实现位于嵌套 `ref/SteamClient/ref/SteamAchievementManager/SAM.API/Client.cs`。

这个 SAM Client 当前初始化 `SteamClient018`，CreateSteamPipe、ConnectToGlobalUser，再获取 SteamUser、UserStats、Apps001/008、RemoteStorage 等接口。它设置进程级 `SteamAppId`，用 ABI、vtable、回调队列。`IsSupported` 源限制 x86/x64。单凭使用 Rust `steamworks` crate 不能断言覆盖这些 Steam 客户端管理行为；官方游戏 SDK授权/初始化方式与连接本地全局 Steam Client 的旧接口绑定必须逐功能验证。

推荐边界：

- Flutter 只处理 UI；Rust 领域层负责状态、数据和任务，Rust 主服务不直接加载未验证 Steam vtable。
- 独立 `steambox-steam-worker`，按固定 `account_epoch + AppId + mode` 启动，初始化失败、回调超时、Steam 退出都转成明确错误，不拖跨主进程。Cloud/成就不可通过修改共享进程环境变量来来回回切 AppId。
- 工作器和主服务之间采用版本化私有 IPC、请求Id/操作Id、当前账号、能力表、超时、进程句柄、退出码；可读写性分离，写操作每次复核实际 SteamId。
- 阶段一允许有界的 .NET bridge 保留已验证 SAM/SteamKit2 行为，以免全栈重写先丢功能。过渡 bridge 必须明确定位为兼容适配，不作为“Rust 全部已完成”的完成标记。
- 每个依赖分别制定退场标准：Rust适配通过源行为差分、真实 Steam x64、错误与退出处理、应用Id隔离，才移除对应 .NET 实现。无法证明等价的适配标 `Blocked/Unsupported`，不能靠隐藏按钮宣称100%完成。
- 需要新架构 ARM64 支持时可用匹配架构工作器或平台允许的 x64 helper；“Flutter 可启动 ARM64”与“SteamClient ABI 支持 ARM64”为两个验收维度。
- 多游戏 AFK 可评估合并进程，但不能预先保证一个连接可等价模拟原有每AppId进程。先原样用小型Rust worker替代多份完整UI进程，测得节约后再优化。

**5. SteamIdleCard：调度器与 Web 登录**

**5.1 真实功能**

源 `IdleCardPageViewModel.cs`、`IdleSteamLoginPageViewModel.cs`、`Enums/IdleRule.cs`、`Enums/IdleSequentital.cs`：

- Steam 用户名密码登录、2FA/邮箱挑战；Cookie `sessionid + steamLoginSecure` 登录；记住登录、退出登录。
- 连接 Steam 客户端的账号必须与 Web session SteamId 相同，否则拒绝挂卡。
- 从社区徽章页读取可掉卡游戏、剩余卡、已玩时间，查询平均卡价用于价值排序，Token 过期尝试刷新。
- FastMode、OnlyOneGame、OneThenMany、ManyThenOne；Default字母顺序、LeastCards、Mostcards、Mostvalue。保留 enum 数字和旧拼写。
- 手动下一游戏、指定游戏优先运行、黑名单 toggling；`PrivateGameAppIds` 把 Steam 私有游戏排除；开始/停止、暂停自动下一游戏、掉卡检测与完成通知。
- 最小运行时间、并发上限、下一游戏时间、刷新徽章频率、状态更新时间；挂时长达到阈值再单独掉卡等规则。

`ref/SteamClient/src/BD.SteamClient/Services.Implementation/SteamIdleCardServiceImpl.cs` 使用 AngleSharp 解析 Steam 社区 HTML，分页遍历 `pagelink`，进一步获取单游戏卡片页/价格。HTML scraping 是外部变化依赖，应封装 parser 版本和响应合法性，不把登录页或空解析当“挂卡已全部完成”。不承诺 Rust 可增加 Valve 的掉卡速率；可以优化本机资源、响应与排队。

**5.2 推荐任务状态机**

`Disconnected → NeedsWebLogin → LoadingBadges → Ready → RunningSolo/RunningParallel → Refreshing → Completed`，含 `Paused`、`Stopping`、`RateLimited`、`Failed`。调度为一个有界 async actor，不能让多个 Timer/Task 并发重写 CurrentIdle。规则更改、排序变化、手动优先、黑名单、Steam账号切换形成串行事件，所有异步结果检查 epoch。

- 价格是可降级信息：价格 API失效仍可挂卡，Mostvalue 排序明确“价格数据不足”而不是自动掉空列表。
- 登录失败/令牌刷新失败暂停任务并保留队列，不无限循环自动登录。
- 限流退避、分页并发上限、取消令牌、缓存有效期；Windows休眠/唤醒重新核对身份和单调时钟。
- 停止只终止本挂卡任务拥有的 worker，不影响用户自己正在运行的游戏；GameList AFK 和 IdleCard 若共享同AppId worker需要引用计数/所有权，不重复杀进程。
- 完成判定必须基于有效完整徽章快照，不能基于网络错误返回空集合。

**5.3 设置字段**

`IdleTime` 默认6分钟，`IdleRule=FastMode`、`IdleSequentital=Default`、`MaxIdleCount=30`、`MinRunTime=2`小时、`SwitchTime=5000`毫秒、`RefreshBadgesTime=6`分钟、`BlacklistAppList`。单位在新 DTO/API 中明确命名并迁移校验，不能把毫秒当分钟。32个GameList AFK上限与挂卡30并发是不同设置，不要合并。

**7. SteamSession 登录迁移与已发现的代码逻辑风险**

`ref/SteamClient/src/BD.SteamClient/Services.Implementation/SteamSessionServiceImpl.cs`：按 SteamId维护 `SteamSession`，每个session独立CookieContainer/HttpClient；`SaveSession` 用JSON存 `ISecureStorage` 的 `CurrentSteamUserKey`；`LoadSession` 反序列化、生成Cookie、调用RefreshAccessToken后再存储；另有Family View PIN锁/解锁接口。

来源代码外层 `if (session != null && accountService.IsAccessTokenValid(session.AccessToken))` 内又检查 `!IsAccessTokenValid` 才刷新，过期Token通常进不了刷新分支。这是只读审阅发现的逻辑风险，不能在方案中把当前方法称为“可靠的自动刷新”。Rust新实现用显式 `Valid/ExpiredRefreshable/RequiresLogin` 状态，synthetic fixture验证过期与刷新失败情况。

`ref/SteamClient/src/BD.SteamClient/Models/SteamLoginState.cs` 的本地脏改把 `ToBytes/Parse` 从 `SMP2/DMP2`（MemoryPack）改为 `SMP/DMP`（MessagePack），删除MP2注解；包含Cookies定制formatter。执行模型必须定位各调用者、IPC双方版本，读兼容两种基线格式或只在有可验证版本标志时处理，不能全局替换后宣称跨版本可兼容。

迁移确保SteamId是u64/string，不用Flutter int/double截断；Cookie保留domain/path/expiry/secure/httpOnly/sameSite信息；账号session按SteamId分别保护，不用当前一个secure-key覆盖所有账号。密码和PIN仅短期内存，持久化的是经过平台保护的合法session数据。外部token失效是合法状态，要求UI重新登录，不伪造迁移完成。

**10. 领域 crate 与协议建议**

推荐按单个领域职责分离（非同时创建全部空壳）：

- `steam-discovery`：安装路径/库目录、Steam进程、账号Id转换、uri/快捷方式契约。
- `steam-formats`：文本VDF、二进制appinfo/属性表、stats schema、old modifications/stmbak；对每种格式单独能力判定和corpus。
- `steam-accounts`：账号切换事务和多平台适配器，不直接拿UI全局对象。
- `steam-worker-protocol` + executable：Steam ABI绑定、callbacks、AppId隔离、身份验证、Cloud、achievements、AFK。
- `game-library`：库存来源融合、搜索筛选、编辑overlay、封面缓存、下载监听、电源操作计划。
- `steam-session`：Web登录挑战、Cookie/Token生命周期、每账号HTTP session。
- `idle-card`：徽章parser、价格降级、调度规则、worker资源所有权。
- `platform`：账号、下载电源、网络接入所需typed权限动作，不包含窗口工具/VAC修复。

Flutter操作先提交TypedCommand，得到operationId；不可阻塞主UI等外部进程。事件携带seq/account_epoch/status/progress/error_code，不广播完整包含secret的领域对象。业务错误分类包含PermissionDenied、SteamNotRunning、IdentityMismatch、AuthRequired、UnsupportedFormat/Abi、RateLimited、OperationCancelled、StorageConflict、ExternalProcessExited。

**11. 真实验收用例（给实施模型）**

1. 账号A/B均有缓存；B用户名大小写与昵称不同；切换目标仍按SteamId正确选定；保留离线/Persona设置；重连后的SteamId匹配才成功；Steam需重新登录走NeedsLogin。
2. Steam无法退出/文件不可写/权限拒绝，UI返回失败并恢复可恢复配置，没有伪造success；保留用户自行运行且本操作未授权管理的游戏和未参与任务的进程。
3. 未启动Steam可看缓存，当前账号所有权未知明确标出；大库过滤/排序与旧列表差分，隐藏/AFK设置被迁移。
4. VDF Unicode、未知字段、libraryfolders旧/新格式、三种appinfo magic、string pool、缺损/过长输入；fixture来自脱敏或合成数据；读→写→读保持未修改字段。写时Steam外部更新触发冲突，原文件可恢复。
5. `.stmbak/modifications.vdf`旧格式导入含AppId正确匹配和部分不匹配；Steam自定义四种封面放在正确账号目录；取消编辑不误应用元数据。
6. 成就/Cloud工作器按AppId隔离，实际SteamId错误时拒绝写；callback超时关闭窗口不拖住app；int/float受保护限制一致；Cloud大文件、配额、重名、删除失败可理解。
7. 下载状态短暂空/manifest写一半不触发关机；选择全部三态一致；倒计时可取消且完成前重新确认下载快照。
8. 挂卡4算法×4排序、solo/parallel切换、30并发限制、手动优先/下一、黑名单/私有游戏、休眠唤醒、Web与native身份不一致、徽章HTML改变/登录页、价格失败、Token过期刷新；失败不标Completed。

**12. 四模块关键边界与移除验收**

保留难点是SteamClient ABI、每AppId worker、Steam本地文本/二进制文件、账号/挂卡Web会话双序列化与保护、脚本及厂商SDK。OTP保护库、令牌云服务、ASF/.NET插件生态、GameTools/VAC均退出实施范围。

挂卡独立注册AddSteamAccountService，IdleSteamLoginPageViewModel调用DoLoginV2Async；用户输入2FA/邮箱验证码是保留登录挑战，不是本地OTP生成。Cookie/AccessToken/RefreshToken与WinAuth必要共享模型按反向引用最小保留。

四模块能正常运行，同时三删除模块不出现在加载清单、初始化/后台请求/计时器、UI/路由/托盘/CLI、设置注册、迁移和包依赖；残留目录或旧默认页不能复活它们。旧个人数据留原处不读取/导入/自动删除。

**C. 网络、系统接入、SDK与插件**

**SteamBox 网络、系统接入、插件迁移研究**

研究范围：`C:\Users\Colby\.gemini\antigravity\scratch\SteamTools` 当前工作目录中的源代码，只读研究。没有启动原程序、代理、驱动或迅游服务，没有修改目标仓库，没有读取用户配置、个人日志、证书或私钥。本文的源码路径均相对于该 Git 根目录。结论描述的是当前源代码；运行行为仍需将来在隔离测试环境实测。当前仓库存在本地改动，必须把当前工作树行为与上游基线分别建立清单。

**1. 四模块范围下的网络核心**

- Accelerator社区加速是.NET/Kestrel+YARP本地HTTP/HTTPS反向代理，Rust需要迁移域名/DNS/SNI/脚本链。
- 游戏加速是迅游原生SDK/服务/合法权益和区服测速，不等于HTTP数据面。
- Flutter+Rust AppEngine+按需net-host+最小权限helper+vendor-host构成终态。四模块以Rust领域和Flutter页面注册，不保留任意旧.NET程序集loader。
- 旧YARP仅作差分或明确过渡，Rust能力验收后退场；三个删除插件无任何兼容host或后台恢复路径。
**2. 已核对的源代码入口**

| 作用 | 源码路径与符号 | 已观察行为/迁移含义 |
|---|---|---|
| 加速插件启动 | `src/BD.WTTS.Client.Plugins.Accelerator/Plugins/Plugin.cs`：`OnInitializeAsync`、`OnPeerConnected`、`SubProcessPath`、`HasValue` | 主进程注册 IPC 守护子进程；子进程名为 `Steam++.Accelerator`，启动请求 `isAdministrator: true`。缺子进程文件时插件无值。只启动原 GUI 就可能请求管理员启动后台，研究阶段不要直接运行。 |
| 代理子进程 | `src/BD.WTTS.Client.Plugins.Accelerator.ReverseProxy/Program.cs`：`IPCSubProcessService.MainAsync` | 导出 `IReverseProxyService`、`ICertificateManager`，添加 DNS、网络、日志、安全存储服务。异常入口含 `Console.ReadLine`，重构不能让后台故障变成等待键盘。 |
| UI 操作总入口 | `src/BD.WTTS.Client.Plugins.Accelerator/Services/Mvvm/ProxyService.Operate.cs`：`StartProxyServiceCoreAsync`、`StopProxyServiceCoreAsync` | 读取设置、获取规则/脚本、选择模式、检查端口/证书、IPC 启停、hosts 写入、定时状态。原停止逻辑先撤销接入再停止代理；hosts 还原失败会阻止普通停止，避免域名被指向停止后的本机端口。 |
| 网络宿主 | `src/BD.WTTS.Client.Plugins.Accelerator.ReverseProxy/Services.Implementation/YarpReverseProxyServiceImpl.cs`：`StartProxyCore`、`StopProxyAsync` | 建 Kestrel，HostFiltering，自定义编码，无 body 上限，System/PAC/VPN 走 HTTP proxy listener，其余走 HTTPS reverse listener，开启 HTTP 到 HTTPS 时另开 HTTP listener。 |
| 中间件顺序 | `.../Services.Implementation/YarpReverseProxyServiceImpl.Startup.cs`：`StartupConfigure` | 本地脚本/xhr 请求 → PAC → 请求日志 → 反向代理。排序是兼容行为的一部分。 |
| listener | `.../Extensions/KestrelServerOptionsExtensions.cs`：`ListenHttpProxy`、`ListenHttpsReverseProxy`、`ListenHttpReverseProxy` | 正向代理先解析 HTTP/CONNECT、流量统计、TLS 判断，再 tunnel；HTTP 和 HTTPS listener 各有协议差异。声明 `Http1AndHttp2AndHttp3` 不能替代实际握手验证。 |
| TLS 判别与动态证书 | `.../Extensions/ListenOptionsExtensions.cs`：`UseTls`；`.../HttpServer/Middleware/TlsInvadeMiddleware.cs`；`.../HttpServer/Certificates/CertService.cs`：`GetOrCreateServerCert` | 检查首字节，ClientHello SNI 选择/生成终端证书。兼容必须涵盖无 SNI、IP、错误 SNI、CONNECT hostname 与 SNI 冲突。 |
| 正向 HTTP/CONNECT | `.../HttpServer/Middleware/HttpProxyMiddleware.cs`：`InvokeAsync`、`HttpRequestHandler` | CONNECT 先发 200，absolute URI 正向代理识别目标，解析失败 400。Rust 必须处理分片读取和客户端取消，不能假设单次 `read` 等于完整请求。 |
| CONNECT tunnel | `.../HttpServer/Middleware/TunnelMiddleware.cs`：`CreateConnectionAsync`、`GetUpstreamEndPointsAsync` | 非 TLS 的 CONNECT 走双向字节流，命中规则的 hostname 使用自定义 resolver，未命中走系统 DNS；有 10 秒连接时限和候选 IP 回退。 |
| HTTP 转发核心 | `.../HttpServer/Middleware/HttpReverseProxyMiddleware.cs`：`InvokeAsync`、`RecursionMatchDomainConfig`、`GetDestinationPrefix`、`SetWattHeaders` | 多层规则、原 URL/目标 URI 替换、HTTP→HTTPS、User-Agent 模板、静态响应、二级代理。云代理设置 `X-Watt-Origin-Dest-*` 和 `X-Watt-Token`，出站请求版本 3 + 向低版本回退。 |
| 出站拨号与 TLS | `.../Services.Implementation/Http/ReverseProxyHttpClientHandler.cs`：`SendAsync`、`ConnectCallback`、`ConnectThroughProxyAsync`、`ConnectAsync` | 固定 IP / ForwardDestination / 原域名 resolver 候选；自定义 SNI 与 Host；HTTP CONNECT、SOCKS4、SOCKS5 二级代理；cookie、重定向、自动解压均关闭。 |
| 客户端池 | `.../Services.Implementation/ReverseProxyHttpClientFactory.cs`、`.../Http/LifetimeHttpHandler.cs`、`.../Http/LifetimeHttpHandlerCleaner.cs` | 按域名/规则配置复用 handler。Rust 连接池 key 必须包含路由身份，避免修改 SNI、上游代理、目标、验证策略后误复用旧连接。 |
| DNS | `.../Services.Implementation/DomainResolver.cs`：`ResolveAsync`；`.../Net/DnsAnalysisServiceImpl.cs`、`DnsDohAnalysisService.cs`、`DnsAnalysisServiceSwitchImpl.cs` | Hosts/DNSIntercept 模式不能用被污染的系统 hosts 解析出站，否则回环递归。支持传统 DNS、DoH、IPv6 和缓存。 |
| DNS/TCP 拦截 | `.../Services.Implementation/PacketIntercept/DnsInterceptor.cs`、`TcpInterceptor.cs`、`InterceptHostedService.cs` | `WINDOWS && !REMOVE_DNS_INTERCEPT` 才编译；DNS 拦截条件为 UDP 目标端口 53，匹配 A/AAAA 回环响应；有 TCP/HTTP/HTTPS/SSH/Git 拦截类型。UDP 53 拦截不意味着拦截客户端 DoH/DoT。 |
| PAC | `.../HttpServer/Middleware/HttpProxyPacMiddleware.cs`：`CreateProxyPac` | `FindProxyForURL`，域名模式 `shExpMatch`，匹配返回本机代理，其余 DIRECT。需转义域名，保持用户模式和命中语义，规则变动时 PAC 与 resolver 同一配置版本。 |
| 本地脚本桥 | `.../HttpServer/Middleware/HttpLocalRequestMiddleware.cs`：`InvokeAsync`、`HandleScriptRequestAsync`、`HandleHttpRequestAsync` | 本地虚拟域名 status/xhr/脚本端点；脚本路径 `/.watt-toolkit-inject/{id}.js`；CORS/PNA；CookieHttpClient 发起脚本桥请求。不能把面向网页的脚本桥与高权限管理 IPC 混用。 |
| HTML 注入 | `.../HttpServer/Middleware/HttpReverseProxyMiddleware.cs`：`HandleScriptInject`、`GetStreamByContentCompression`；同目录 `HttpReverseProxyMiddleware.FindScriptInjectInsertPosition.cs` | GET、200、text/html，可限定 `Valve Steam` UA；gzip/deflate/br 解压、字符集/BOM，按脚本顺序插入 `<script src>`，GitHub 有特殊插入位置。已有实现删除 CSP，必须按脚本功能逐项确认需要的策略修改，避免整站无条件降低策略。 |
| 脚本管理 | `src/BD.WTTS.Client.Plugins.Accelerator/Services.Implementation/ScriptManager.cs`、`Repositories/ScriptRepository.cs`、`Entities/Script.cs` | 本地/商店脚本、匹配与排除域、排序、缓存/下载、启用状态和更新，是网络转发之外的完整业务模块。 |
| 流量与日志 | `.../Services.Implementation/FlowAnalyze/*`、`.../Models/FlowStatistics.cs`、`.../HttpServer/Middleware/RequestLoggingMiddleware.cs` | UI 图表/日志从 IPC 取得状态，Rust 应汇总后推送差量，保持原单位、累计/瞬时含义和重置行为。 |
| 证书管理 | `.../Services.Implementation/Certificate/CertificateManagerImpl.cs`、`CertGenerator.cs`；`.../Services/Certificate/ICertificateManager.cs` | 生成 CA/leaf，导入/移除平台根证书、过期检测/定时重启、CER/PFX。研究只读源代码，未访问文件内容。 |
| GOG CA | `.../Services.Implementation/ReverseProxyServiceImpl.cs`：`WirtePemCertificateToGoGSteamPlugins` | 向 GOG Steam 插件 certifi trust bundle 写公有 CA。需要文件版本/哈希、只添加自己的段、逐文件回滚及插件升级后的冲突识别。不能粗暴覆盖整个证书包。 |
| 系统代理 Windows | `src/BD.WTTS.Client/Services.Implementation/Platform/Windows/WindowsPlatformServiceImpl.SystemProxy.cs` | HKCU Internet Settings 的 ProxyEnable / ProxyServer / ProxyOverride / AutoConfigURL，通过 IPC/regedit 写入。必须按原用户身份操作；管理员 worker 的 HKCU 可能不是实际桌面用户 HKCU。 |
| 系统代理 Linux | `.../Platform/Linux/LinuxPlatformServiceImpl.SystemProxy.cs` | GNOME gsettings 模式、http/https host/port、ignore-hosts；当前源码关闭时设 none。GNOME 不能代表 KDE/所有 Linux 应用。 |
| 系统代理 macOS | `.../Platform/MacCatalyst/MacCatalystPlatformServiceImpl.SystemProxy.cs` | 对网络服务 `networksetup` 设置 web/securewebproxy、bypass，必须保存各网络服务的旧值。 |
| 游戏加速业务 | `src/BD.WTTS.Client.Plugins.Accelerator/Services/Mvvm/GameAcceleratorService.cs`、`Services.Implementation/BackendAcceleratorServiceImpl.XunYou.cs`、`Services/IAcceleratorService.XunYou.cs` | 安装/卸载、加速/停止、区服、VIP 到期、窗口显示、启动游戏、测速、SDK 回调映射。 |
| SDK 平台范围 | `src/XunYouSDK/XunYouSDK.cs`、`XunYouSDK.Constants.cs`；`src/BD.WTTS.Client.Plugins.Accelerator/XunYouSDK/Methods/*` | Windows，OS 架构 x86/x64，SDK appId 不为 0 时启用；区分 x86 `xunyoucall.dll` 与 x64 `xunyoucall64.dll`。未读取 AppId 文件；可用性、SDK 授权、动态库分发须单独验收。 |
| .NET 插件 | `src/BD.WTTS.Client/Plugins/PluginsCore.cs`、`Plugins/Abstractions/IPlugin.cs`、`IPlugin.Properties.cs` | `modules/{name}/BD.WTTS.Client.Plugins.{name}.dll`，AssemblyLoadContext、CompositionExport、禁用状态、插件菜单/设置页/生命周期/配置/IPC。 |

**3. 不应扩大已存在功能的声明**

1. `Socks5ProxyEnable`、`Socks5ProxyPortId` 在 settings、IPC DTO、网络基类中存在，但全 `src` 检索没有找到对应 SOCKS5 本地 listener 实现；只找到“出站二级 SOCKS5”握手。因此必须标记“保留设置、行为待验证”，不能把入站 SOCKS5 已完整可用写成事实。将来补齐属于修复/实现该缺口，不能用空开关假装已经迁移。
2. `ListenSshReverseProxy` / `ListenGitReverseProxy` 的当前宿主调用是注释；相关拦截器/handler 类存在不等于当前所有模式已启用 SSH/Git 加速。先验证模式与构建开关；移植清单记录 Active、Inactive、Conditional、External 四类状态。
3. `ProxyMode` 包含 VPN/ProxyOnly Android 条目；桌面 Accelerate 插件启动条件有平台宏。Flutter 移动端 UI 不自动带来这些系统能力。
4. `Http1AndHttp2AndHttp3` 的配置和出站 Version30 表示意图，不证明 HTTP/3 当前在目标 OS、TLS、QUIC 和该自定义连接链上均实际工作。Rust 不能为“性能”只支持 HTTP/1.1；也不能仅导入 quinn/h3 便宣布 HTTP/3/Extended CONNECT 完整。
5. 原项目 `StartProxyCore` 通过约 650ms 等待并检查异常报告启动态，重构要用真正的 listener ready + 健康探针 + 系统接入提交状态，不能复制时间猜测。
6. 原端口候选会从 443 向后找可用端口；hosts 只映射地址，不携带端口。Rust 的 Hosts 模式必须保证 443 可用或提供明确的重定向机制，不能悄悄改成 444 后显示加速成功。

**4. 网络数据面建议**

```text
Flutter + 托盘
    │ typed command/event IPC（request_id、配置 revision、取消、错误码）
Rust app-core（业务/唯一状态协调者）
    ├─ Rust net-host（单个网络 owner，listener / resolver / cert leaf cache / forwarder）
    │      ├─ Hosts / DNSIntercept 接入 → TLS终止 → HTTP规则 → 流式转发
    │      ├─ System / PAC → HTTP正向代理/CONNECT → 命中规则后终止或透传
    │      ├─ HTML可转换响应 → 有上限的解压/注入/重压缩
    │      └─ direct / 二级HTTP-SOCKS / 授权云代理
    ├─ os-helper（提升权限，仅声明的系统操作）
    │      └─ hosts / 指定 CA trust / WinDivert / 固定端口与系统恢复
    └─ vendor-host（仅迅游SDK，按需）
```

采用 Tokio async runtime、hyper/hyper-util 做底层 HTTP/1.1+HTTP/2、rustls/tokio-rustls 做 TLS、rcgen 做 CA/leaf、Hickory 做 DNS/DoH。具体版本与 features 在首次实现时核对、锁定和记录，本文不把滚动 latest 当工程锁版本。

普通 HTTP body、文件下载、Range、上传、chunked、trailer、SSE、WebSocket、gRPC 保持流式背压；Flutter 不承载、解析或拷贝代理 body。上游 cookies 和认证只逐请求透传，不让普通代理共享网站 cookie jar；脚本 xhr 的兼容 cookie jar 单独隔离。普通代理不自动解压、不跟随重定向。Hop-by-hop headers、Connection 指名头、重复 Set-Cookie、Host/:authority、HTTP/2 禁止头、100-continue、取消和半关闭需专门测试。

至少实现下列独立接口，而不是一个巨大 proxy 类：`IngressClassifier`、`RuleEngine`、`DomainResolver`、`UpstreamConnector`、`TlsPolicy`、`Forwarder`、`ScriptTransformer`、`LocalScriptBridge`、`FlowMeter`、`SystemIngressCoordinator`。RuleEngine 使用不可变 revision snapshot；单次连接/请求固定所属 revision；更新同时刷新 PAC、DNS规则、脚本元数据和池配置。

出站路径必须明确区分四个值：dial IP、HTTP Host/:authority、TLS SNI、证书应验证的服务身份。特定规则的 SNI 改写属于兼容功能；证书链/期限/用途验证仍必须成立。rustls 默认“一个 ServerName 同时驱动 SNI 与 hostname 验证”的直觉不足以覆盖该项目，需隔离的 per-rule verifier/connector spike；验证更改只作用于对应规则，不能污染全局配置。

连接池 key 建议包含目标 scheme、拨号/目的路由、Host identity、SNI policy、验证 policy、上游代理类型/地址/credential handle、HTTP policy、rule revision 的有效网络部分。用户名密码只使用 credential handle 引用，不进入 log/key 显示。公共 CA 与自建 CA 的 trust 范围明确区分。

CONNECT 首先解析 authority 和 allowed route，再选择透传或合法本地 TLS 终止；不匹配域默认透传，未知/恶意 authority 不签任意 leaf。端口白名单/范围、IPv6 authority、用户名信息、请求目标超长和解析歧义在统一 parser 处理。HTTP/2 CONNECT 与 RFC 8441 WebSocket Extended CONNECT 不是 HTTP/1 upgrade 的同一路径，必须另列实现验收。

HTTP/3 若属于基线实际活跃功能，则必须做单独 spike 并完成验收；若未活跃，也要保留配置/协议意图和未来实现槽位，最终文档明确能力状态。不能把 HTTP/3 将来实现写成当前“全功能已完成”。

**5. 系统操作 owner 与可恢复事务**

建议由 Rust app-core 作为系统接入配置唯一决策者，Rust net-host 作为数据面唯一拥有者，helper 只执行经过验证的动作。helper 不接收任意 shell、注册表路径、文件路径或 DLL 载入指令；命令枚举为 ReadSystemIngress、ApplyHostsBlock、RemoveOwnedHostsBlock、ApplyUserProxySnapshot、RestoreUserProxySnapshot、TrustOwnedCa、RemoveOwnedCa、OpenApprovedWinDivertHandle 等。命令包含调用用户 SID/UID、operation id、expected revision、session lease，并通过 Windows named pipe ACL/Unix socket owner+peer identity 校验。

避免为普通请求转发、网页 xhr 或插件操作保留高权限。低端口绑定/WinDivert 与数据面权限可采用提升的受限 net-host worker，或 helper 预绑定后通过经过测试的平台句柄传递；不得凭空承诺一个跨平台 FFI 调用就能安全传 SOCKET/FD。Windows 用户代理写入在实际桌面用户进程执行；HKCU 不等同 helper 自己的用户。

启动状态机：`Stopped → Validating → PreparingListener → PreparingTrust → ApplyingIngress → Running`。每步返回结构化 ready/errors；必须在真实端口绑定、叶证书准备、DNS 出站可用和 IPC peer 已就绪后写 hosts/系统代理。权限取消与端口占用回滚到 Stopped，并给用户准确原因；从不杀死只因名字包含 Accelerator/Steam++ 的任意进程。只管理经过 pid+启动时间+可执行路径+session id 验证的本次子进程。

停止状态机：`Running → RemovingIngress → Draining → Stopped`。先撤接入，再有限等待现存转发完成，之后关闭 listener/driver；若撤接入失败，保留服务并呈现“需要恢复”，不能假报停止。如果 OS 关机或进程已死，下一次 app/helper 启動先读 journal、发现并修复自己的遗留改动，再允许新的代理会话。

事务 journal 原子写入：operation id、用户/系统范围、旧值存在与否、已写入目标值、hosts 自有 block id、CA fingerprint 与 trust-store 位置、GOG bundle 文件摘要、driver session、当前步骤。恢复执行 compare-and-swap：只有现值仍等于自己的修改才恢复旧值；用户或其他代理已修改时标记冲突，不能覆盖。Hosts 仅撤销自己的标记段，保持 BOM/编码/换行、用户其他条目以及并发编辑。System/PAC 完整保存手动代理、例外、PAC URL、原始存在状态，并恢复旧值；不能只设 ProxyEnable=0 或 AutoConfigURL空字符串。

CA 正常停止不必自动卸载，否则每次启动都要重装；“移除信任/重置证书/卸载”操作须先停止相关网络会话并清理自有 trust。证书 journal 不记录私钥/口令。异常重启恢复前不能沿用失效 lease 上的管理权限。

**6. 本地 TLS/证书与网页脚本边界**

CA 仅用于用户明确启用、配置允许的社区网站流量和本机测试。只按原平台权限流程导入用户选择的 trust scope；不绕过 OS 权限交互，不向无关设备/其他用户 trust store 部署。以 fingerprint/owned certificate id 清理自己的 CA，不按 Subject 名称匹配删除所有同名证书。私钥由 owner 保存并受平台 ACL/原生密钥保护，Flutter、日志、IPC DTO、脚本端点只得到公有 CA 或 credential handle。过期/撤销/时钟偏差给出具体错误，轮换 journal 与 cache 一致。

未知 SNI/非选中站点默认透明 tunnel/pass-through 或拒绝，不任意扩大解密范围；无法支持的 certificate pinning/mTLS 应作为真实能力边界解释，不尝试禁用目标应用验证。HSTS 不等于无法本地 TLS 终止，是否成功由 client trust/pinning 决定，不能笼统承诺对所有程序生效。

已观察原源码两处行为风险：`ConnectThroughProxyAsync` 的 TLS callback 无条件 true；`ConnectAsync` 的 ValidateServerCertificate 在 NameMismatch 分支可能返回 true 而忽略其他错误。这里仅为迁移注意事项，并未开展安全审计或改代码。重构不得把它们当成必须复刻的交互行为；保留规则的 SNI/名称例外配置，建立更窄的 name-policy，同时独立验证链/有效期/用途，UI 如需异常名称规则则保留明确字段并说明作用。

脚本注入保留 Match/Exclude、启用、顺序、OnlyEnableProxyScript、Steam UA 限制、用户脚本/商店更新、GitHub特殊插入点。转换对内容长度、解压倍率、总时间和并发内存设界限；无法转换时返回原始正确响应并通知可诊断错误，不能截断网页或阻断所有代理。修改 body 后重新计算 length，并处理 ETag/Content-MD5/Digest、压缩和 cache validator；不会注入 HEAD、Range partial、SSE/WS/gRPC、非 HTML。

LocalScriptBridge 仍需复刻当前脚本使用的 API 与 cookie 语义，但它不暴露宿主文件/账号保险箱/系统控制。对网页 Origin、目标、脚本 id 与当前启用 capability 验证；避免脚本 xhr 成为任意 LAN/metadata/file 读取器；GM 对应权限应按脚本清单授予。CORS/PNA 只用于该桥所需的受控 origin，不把通配 origin、credentials、高权限 IPC 放到同一 listener。CSP 修改只针对已启用并命中的脚本，写进行为兼容清单，优先精确追加源/nonce/hash，必须与旧脚本实际需求验证。

**7. 全设置迁移中的网络字段**

来源 `src/BD.WTTS.Client.Plugins.Accelerator/Settings/ProxySettings.cs` 与 `Settings/Abstractions/IProxySettings.cs`。完整迁移必须保留字段拼写/旧 JSON alias，包括 `CustomDohAddres2` 的既有拼写；可内部规范化但读写兼容和升级回退需记录。

- 脚本/服务选择：IsAutoCheckScriptUpdate、IsEnableScript、SupportProxyServicesStatus、ScriptsStatus。
- 自动和接入：ProgramStartupRunProxy、SystemProxyPortId（源码默认 26561）、SystemProxyIp、ProxyMode、ProxyBeforeDNSCheck。
- 转发：OnlyEnableProxyScript、EnableHttpProxyToHttps、Socks5ProxyEnable、Socks5ProxyPortId（默认 8868，listener 未确认）、TwoLevelAgentEnable、TwoLevelAgentProxyType、TwoLevelAgentIp、TwoLevelAgentPortId、TwoLevelAgentUserName、TwoLevelAgentPassword。
- DNS：ProxyMasterDns、UseDoh、CustomDohAddres2；运行 DTO 包含 IsSupportIpv6 及 ProxyDNS，设置来源需要和全量字段索引继续核对。
- 扩展：IsProxyGOG、IsOnlyWorkSteamBrowser、AcceleratorTabsSelectedIndex、AutoShowWattAcceleratorWindow。
- 游戏加速：`GameAcceleratorSettings.MyGames`（game id → model）、WattAcceleratorDirPath，model 中选中区服、服务、状态字段要逐项索引。持久化状态必须区分用户选择与瞬时 SDK runtime state，不把上次“加速中”直接当当前真实状态。
- GeneralSettings 的 AppWebProxyMode / CustomWebProxyMode* 是“本应用 API 请求代理”，与系统加速和 TwoLevelAgent 是不同层。若集中 Rust HTTP 库，仍分别维护用途、DNS、认证、可信根与循环防护策略。

**8. 跨平台真实边界**

| 平台 | 原源码可见范围 | 重构承诺应如何表述 |
|---|---|---|
| Windows x64/x86 | Hosts、PAC、System，DNSIntercept受构建开关；迅游 x86/x64 SDK；管理员/证书/驱动 | Windows x64 作为完整迁移优先验收平台。Windows x86 若 Flutter工具链或第三方依赖不支持，保留用户数据和后端兼容方案，不能虚称完整支持。 |
| Windows ARM64 | Rust/Flutter本体可以单独评估；迅游源码判断只允许 x86/x64 OS | 本体/HTTP功能与游戏厂商 SDK 单列能力；需正式 ARM64 SDK 或独立 x64 worker在真实兼容环境验证，不能仅交叉编译宣称游戏加速。 |
| macOS | Hosts/System；networksetup/keychain/权限；无WinDivert或已证实迅游路径 | 每个网络服务恢复、keychain scope、签名/notarization、sleep/wake；游戏加速提示平台不可用并保留原支持差异。 |
| Linux | Hosts/System（实际GNOME gsettings），低端口能力/权限，证书发行版差异 | 区分 GNOME/KDE/headless、system CA/NSS/应用自带 trust、sudo/polkit、systemd权限。对原本不遵守桌面代理的程序不假报全局生效。 |
| Android/iOS | enum 或平台源引用不等于完整现成产品能力 | 本轮桌面重构不自动扩展移动系统功能。若实施移动端，单列 VpnService/Network Extension entitlement、用户CA信任/应用 pinning、后台限制和厂商 SDK。 |

需要注意原源码在 macOS/Linux 下 loopback listen 地址会被改为 0.0.0.0（权限理由）；Rust 必须对接入需求实测，默认 loopback。用户指定 LAN 暴露才扩大 bind，并定义认证/访问控制，避免把旧值简单当最佳实践。

**9. 严格四模块注册与供应商SDK**

保留Accelerator、GameAccount、GameList、SteamIdleCard四模块manifest/route/设置/生命周期；旧IPlugin携带Avalonia页面、.NET DI和IPC，按实际源调用重写，不承诺DLL直接兼容。注册/加载/更新都拒绝其他模块，残留目录不能触发旧.NET加载。

迅游adapter核对C ABI、calling convention、编码/packing/指针/callback寿命及线程要求；明确批准路径、厂商签名/摘要、架构和权益。阻塞/崩溃调用隔离vendor-host，以operation ID回调；超时重连查真实state，demo值/空回包不算完成。

Steam登录和本地网络证书依赖属于四模块共享支撑，不随OTP插件删除；驱动与低端口权限接口只提供社区加速/账号/下载的typed动作。
**10. 低一级模型可执行的网络任务序列**

1. **N0 行为夹具与构建清单**：固定当前工作树hash、upstream hash、构建宏、active/conditional/inactive功能；导出非敏感规则/脚本元数据schema；从 unit tests提取GitHub注入夹具。写 `network-contract.md` 和cases，不启动实网代理。
2. **N1 IPC与生命周期骨架**：app-core command/event、net-host mutual exclusion、helper最小命令、lease/journal mock。Fake OS adapter 下验证ready/rollback/crash recovery；到此只有隔离开发地址。
3. **N2 HTTP转发和protocol spike**：本地HTTP1/2 fixtures，streaming/WS/gRPC/CONNECT/SSE/Range/trailer/取消；pool reuse/identity。先准确实现YARP已有行为，量化吞吐和RSS后优化。
4. **N3 DNS/路由/TLS**：复刻递归rules、地址来源、SNI/identity分离、DNS/DoH/IPv6/循环排除；按允许域生成leaf；失败链/名称例外夹具。HTTP3/Extended CONNECT按真实基线单列gate。
5. **N4 脚本**：Match/Exclude/顺序、压缩/编码、GitHub特殊点、local script/xhr bridge、cookie隔离、CSP受控处理；金样HTML和可实际运行脚本fixture。
6. **N5 Windows系统接入**：先隔离VM测试CA/hosts/System/PAC，再driver mode；所有变更日志和恢复；无人值守mock可覆盖大多数故障，但真实权限/driver测试需用户批准的测试环境。
7. **N6 云API及迅游**：使用合法vendor服务/SDK测试账户集成，离线fixture记录契约。失败/会员过期/离线/安装路径/启动游戏行为完整，外部不可用状态不报成功。
8. **N7 插件/跨平台**：四模块白名单与删除门禁、macOS/Linux各自系统adapter；回滚与权限交互。在能力矩阵全部通过前不宣称跨平台同功能。

每项提交须附source mapping、覆盖settings字段、行为cases、真实结果与剩余gap。不能以mock成功代替厂商加速真实成功，也不能以Flutter页面截图代替网络兼容性通过。

**11. 必要测试矩阵**

- HTTP：status和重复headers、Set-Cookie、302不自动跟随、Range/206、大上传/下载、chunked与trailers、100-continue、gzip/deflate/br透传、HTTP1↔2、WebSocket双向/关闭码、SSE逐条到达、gRPCstream/trailer、HTTP2 extended CONNECT、HTTP3若活跃。
- CONNECT：absolute URI/authority/IPv6、分片/小包解析、证书pinning的明确失败/透传、client cancel、半关闭、timeout、允许站点/未知站点范围、SNI缺失和矛盾。
- DNS：A/AAAA/CNAME、NXDOMAIN、TTL/negative缓存/失效、DoH认证链失败、bootstrap DNS、client DoH无法被UDP53拦截的边界、Hosts防递归、proxy自身与上游DoH出口排除、并发缓存击穿、IPv6失败回退。
- 脚本：GET200 HTML与非命中原样、OnlySteam UA、禁用脚本/排除域、排序、UTF8/BOM/GB编码、gzip/br和decompression bomb、HTML无head/body/截断、GitHub最后外链script插入、CSP、content length/validators、xhr cookie与Origin权限、商店离线/更新失败。
- TLS：CA/leaf过期、电脑时钟、陌生CA、wrong identity、name exception仍拒绝坏链/过期、叶证书LRU/并发生成、证书轮换后cache/连接一致、不同SNI/pool不串。
- OS：端口80/443被占用、UAC/权限取消、启动任意阶段断电/kill、停服务任意阶段失败、reboot/sleep/wake、用户/其他代理并发修改、Unicode用户名路径、多Windows用户、多网络服务、hosts非法编码/只读、CA卸载只删owned、GOG升级冲突、WinDivert句柄清理。
- 状态/性能：UI掉线net-host不成为无人管理残留；listener ready与toggle一致；有request id取消；1GB流式下载RSS不会随body线性增长；script bounded buffering；resolver/pool/cert caches上限；代理数据面不经Dart bridge；无敏感URLquery/header/cookie/令牌进入普通日志。

**12. 已核对的官方资料**

- [Microsoft YARP direct forwarding](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/servers/yarp/direct-forwarding?view=aspnetcore-10.0)：`IHttpForwarder` 提供请求/响应转发、streaming、错误处理；routing/retry等由调用方负责。连接池复用与不buffer body对等迁移需保留。
- [Microsoft YARP WebSockets](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/servers/yarp/websockets?view=aspnetcore-10.0)：HTTP/1 upgrade与HTTP/2 WebSocket有不同headers/握手机制，源/目的版本可不同。Rust必须实际验收，不能借用了hyper便假定自动和YARP等价。
- [hyper 1 migration guide](https://hyper.rs/guides/1/upgrading/)：HTTP1/2双协议server可用hyper-util auto builder，池化client在hyper-util；库本身不是完整YARP替代品。
- [rustls docs](https://docs.rs/rustls/latest/rustls/) 与 [tokio-rustls docs](https://docs.rs/crate/tokio-rustls/latest)：TLS配置与异步stream候选，标准验证不应被全局绕过。
- [rcgen project](https://github.com/rustls/rcgen)：X.509生成候选；证书导入、平台密钥存储与权限仍由产品实现。
- [Hickory resolver docs](https://docs.rs/crate/hickory-resolver/latest)：DNS/DoH/缓存/IPv4IPv6候选，crypto与transport feature需锁定；防系统hosts回环由项目自己确保。
- [WinDivert 2.2 docs](https://reqrypt.org/windivert-doc.html)：Windows平台、管理员权限和签名驱动要求；包filter范围决定实际可拦截流量。Rust FFI不会消除驱动、权限和license分发要求。

源码与官方资料只用于研究。未把本地源代码或日志发送给外部服务。

**最终使用边界**

101保留字段列表已核对源名称、编号、类型及默认，仍需补四模块嵌套实体与运行行为。删除模块不因原代码存在或曾初始化而回到范围。其余源码风险只是验证线索，不是已修复或已审计结论。

常见误读：用户自行启动客户端也可在其主动账号切换中按路径/PID/身份管理；后台worker自动清理限自有进程。四模块数据兼容工具的退场不得损害保留格式导入。托盘关闭时主窗X真正退出，普通手动启动显示，显式silence才最小化。
