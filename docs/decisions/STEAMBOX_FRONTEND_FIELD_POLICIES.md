# SteamBox 前端字段提交策略

状态：`identified`。这是四个保留模块的确定交互契约，供实现与审查使用；不表示 Flutter/Rust 已实现或平台效果已验证。适用 Accelerator、GameAccount、GameList、SteamIdleCard，以及它们共同依赖的 General/UI/Steam 设置。Authenticator、ASF、GameTools 不进入此表。

依据是 `outputs/STEAMBOX_SOURCE_INVENTORY.md` §5 的 **101 个顶层保留字段种子**，并逐项核对当前源码。种子是源码 schema、序列化编号和默认值，**不是用户实际设置**。未读取任何用户配置、账号文件、数据库、cookie、凭据或日志。源目录只读：`C:/Users/Colby/.gemini/antigravity/scratch/SteamTools`，HEAD `a7a5ce7f0830b9b759090a75b59d82679084b478`；当前工作区内容是本次定位基线，不能用 HEAD 代替工作区内容。原 GUI、Steam、代理、迅游 SDK 和平台效果测试均**未运行**。

## 提交与效果必须分别表达

表中“提交 → 效果”是本次确定的前端契约，旁列“源码定位”说明已观察到的控制或消费。源码缺口应作为待实现工作，不能把建议契约反写为源码已有行为。

| 策略 | 确定语义 |
|---|---|
| `immediate` | 明确的开关、枚举选择或命令在一次用户操作完成时提交。先获 Rust 持久化确认，再显示“已保存”；运行效果另等相应事件。不得把逐次重建、选择恢复或初次绑定当用户提交。 |
| `commit_on_blur` | 文本/数字保存在当前字段草稿，失焦或 Enter 校验后提交；IME 组合输入期间不提交。无效值保留原文并就地报错，不能截断、清空或静默存入默认值。离开页面仍保留未提交草稿；回车与失焦同一次提交去重。 |
| `preview_only → commit_on_end` | 颜色、透明度、菜单拖动在拖动/键盘调整时只预览，释放、确认颜色、结束键盘调整或完成拖放后提交一次。Escape 撤回本次预览。不能用每帧值写磁盘或重启任务。 |
| `dialog_commit` | 进入弹窗复制独立草稿，确认并校验后一次提交；取消、关闭、返回、文件选择器取消均不改变既有字段。 |
| `internal_event` | 只能由已确认的业务事件或窗口事件写入。没有人工文本编辑入口，恢复事件不再写回自身。 |
| `migration_roundtrip` | 当前未确认活跃入口/消费，仅按原 schema 保留、导入、导出和重开恢复；不自动增加前端控件，不宣称效果。 |
| `restart_required` | 保存的是下次进程配置，当前效果保持旧值；提示重启，不自动退出或杀进程。只在真实重开并确认加载后更新 applied 状态。 |
| `next_run` / `next_cycle` | 保存立即完成，效果在表中指定的下一次启动、连接、安装、调度或等待周期采样。不得因字段保存自行启动 Steam、代理、SDK 或挂卡。 |

每个字段都有 `persisted_value`与必要的`draft_value`；有实际效果的字段才有可验证的`applied_value/applied_revision`。仅迁移保留项不虚构applied；网络配置分`desired_revision`与`applied_runtime_revision`。表中`next_run`有具体对象，不能统称“重启后生效”。未接通效果的活跃控件不能放行；实现阶段只显示准确的保存事实，产品提示不暴露“源码未确认/消费者”等开发术语。

## 所有行共同适用的恢复规则

这些规则逐行适用；表中代码用于指出额外要求。

| 代码 | 存储、失败、跨页与重开规则 |
|---|---|
| `B` 基础 | 持久化失败保持既有保存值，保留本次草稿/选择供重试，显示字段错误；不要先显示保存成功再悄悄回滚。跨锚点、根页切换、最小化保留草稿、滚动、焦点和稳定选择 ID。重开读取最后成功提交值；未提交草稿不当配置恢复。若做会话草稿恢复须另设非秘密会话文件并标“未提交”。 |
| `V` 外观 | 预览失败回到最后可用外观；保存失败保留草稿且撤回预览到已提交外观。已保存但平台拒绝材质/字体时保留 desired，显示实际 fallback；不静默改写原枚举/字段。跨页预览只能持续到当前操作结束，不能污染库编辑。 |
| `R` 运行 | 原运行配置继续有效，显示“已保存，待下次运行/重新应用”。运行中允许保存下次配置，但不能暗中重启。显式应用失败保留 desired 与旧 applied；已停止旧实例却新启动失败须显示停止/恢复结果，禁止假报仍在运行。 |
| `P` 平台 | 开机启动、托盘、文件、网络模式等按 operation_id 返回真实平台结果。保存成功/平台失败分别显示；有补偿的操作报告补偿结果。不能因 UI 勾选完成就宣称注册表、Hosts、系统代理或客户端变化成功。 |
| `D` 实体 | 弹窗草稿绑定 `(identity_epoch, stable_entity_id, base_revision)`；账号切换后旧草稿只读/失效，确认必须拒绝旧 epoch。持久化与外部写入分阶段显示；失败保留草稿，取消不触碰实体。导航、重新筛选、分页后按 ID 恢复，不能按行号恢复。 |
| `K` 秘密 | 使用受保护引用；密码遮蔽，默认不回填明文，空占位不表示清除。分别提供“保持”“替换”“清除”的意图。不得在日志、错误、事件、截图夹具或证据中输出值；不做含秘密的跨重开草稿恢复。写保护失败则不存明文。 |
| `M` 仅保留 | 保留 null、空集合、空字符串、未知枚举、复杂值与历史字段别名；无效果反馈、无无端网络请求、无原字段消失。当前平台不能用的原值不因安全 fallback 被覆盖。 |

bool 校验为真实布尔；枚举选择使用稳定枚举值，不能用显示文本/列表序号存储。数字拒绝 NaN、无穷、溢出、空白和不完整符号，单位在界面明确。所有提交由 Rust 验证；前端校验负责及时反馈。原 MessagePack/MP2Key/JSON order 不重排，JSON 枚举仍用原字符串语义；`SteamStratParameter`、`GameTypeFiltres`、`IdleSequentital`、`CustomDohAddres2` 保持兼容别名。

## Settings 长页与游戏编辑草稿

Settings 是同一长页的 General/UI/Plugin/Steam/About 五个滚动锚点，不拆成五个路由或五套表单，不增加全页保存/取消。开关逐项提交，文本逐字段草稿，颜色/透明度短期预览；“重置”只作用于源码明确范围且一次提交，不能顺便清空其他模块草稿。锚点滚动、语言变化、菜单重排和控件 rebuild 不能导致焦点丢失、重复保存或跳页。支持退出页后返回到原锚点、滚动与未提交字段草稿。

GameList 的“编辑游戏”独立于上述设置：复制元数据、路径、参数、图片/图片删除意图等完整实体草稿；图片重置也只改草稿。取消丢弃整个草稿；确认一次校验并按实体版本保存到本应用的编辑列表，**第二步“保存所有更改到 Steam 文件”才应用到 Steam**，两步成功状态分别呈现，不把第一步自动改成写 Steam。源 `src/BD.WTTS.Client.Plugins.GameList/UI/ViewModels/Pages/EditAppInfoPageViewModel.cs:40` 直接持有共享 App，`:186` 明确二段保存、`:191` 取消只刷新图片、`:236` 图片 Reset/Apply 会写出，因此不能沿用其共享引用和图片提前写出方式。游戏参数不是 S-0（Steam 客户端参数），游戏图不是 U-14（窗口背景）。这些实体字段不是 101 设置种子，另列领域表，不混入本表分母。

Steam身份变化使旧结果不能回写新账号投影；已提交的Cloud/成就等旧operation仍按原SID/AppId在journal结算/核对，不丢弃终态。窗口最小化不等于任务停止；重开从业务事实重连，不能把配置或旧MyGames.IsAccelerated当当前运行状态。

## 源码定位记法

下面 `文件别名:行号` 都相对于上述只读源根；G/U/S/P/A/L/I/X 是模型声明，给出原字段 Key 及确定定位。每行 `E` 为当前活跃前端入口，`N` 为内部消费/事件或未确认可达入口，`Q` 为 schema/注释入口或效果待定位。`E` 不表示效果已实测。

| 别名 | 真实文件 |
|---|---|
| G / U / S | `src/BD.WTTS.Client/Settings/GeneralSettings.cs` / `UISettings.cs` / `Steam/SteamSettings.cs` |
| P / X | `src/BD.WTTS.Client.Plugins.Accelerator/Settings/ProxySettings.cs` / `GameAcceleratorSettings.cs` |
| A / L / I | `src/BD.WTTS.Client.Plugins.GameAccount/Settings/GameAccountSettings.cs` / `src/BD.WTTS.Client.Plugins.GameList/Settings/GameLibrarySettings.cs` / `src/BD.WTTS.Client.Plugins.SteamIdleCard/Settings/SteamIdleSettings.cs` |
| SG / SU / SS | `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/Settings/Settings_General.axaml` / `Settings_UI.axaml` / `Settings_Steam.axaml` |
| SP / AP / SC | `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/ProxySettingsPage.axaml` / `AcceleratorPage2.axaml` / `ScriptPage.axaml` |
| AC / LP / IP | `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Pages/GameAccountPage.axaml` / `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/GameListPage.axaml` / `src/BD.WTTS.Client.Plugins.SteamIdleCard/UI/Views/Pages/IdleCardPage.axaml` |
| SVM | `src/BD.WTTS.Client/UI/ViewModels/Pages/SettingsPageViewModel.cs` |
| PO / PS | `src/BD.WTTS.Client.Plugins.Accelerator/Services/Mvvm/ProxyService.Operate.cs` / `ProxyService.cs` |
| LV / IV / XV | `src/BD.WTTS.Client.Plugins.GameList/UI/ViewModels/Pages/GameListPageViewModel.cs` / `src/BD.WTTS.Client.Plugins.SteamIdleCard/UI/ViewModels/IdleCardPageViewModel.cs` / `src/BD.WTTS.Client.Plugins.Accelerator/Services/Mvvm/GameAcceleratorService.cs` |
| Program / App / ResourceService | `src/BD.WTTS.Client.Avalonia.App/Program.cs` / `src/BD.WTTS.Client.Avalonia/UI/App.axaml.cs` / `src/BD.WTTS.Client/Services/Mvvm/ResourceService.cs` |
| NoticeService / IPlatformService.TextReader / HostsFileServiceImpl | `src/BD.WTTS.Client/Services/Mvvm/NoticeService.cs` / `src/BD.WTTS.Client/Services/Platform/IPlatformService.TextReader.cs` / `src/BD.WTTS.Client/Services.Implementation/Net/HostsFileServiceImpl.cs` |
| PluginVM / Window / MainVM | `src/BD.WTTS.Client/UI/ViewModels/Pages/SettingsPageViewModel.Plugin.cs` / `src/BD.WTTS.Client.Avalonia/UI/Views/Abstractions/Windows/ReactiveAppWindow.cs` / `src/BD.WTTS.Client/UI/ViewModels/Windows/Main/MainWindowViewModel.cs` |
| Settings_Plugin.axaml / SettingsPageViewModel.props / MainWindow.axaml.cs | `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/Settings/Settings_Plugin.axaml` / `src/BD.WTTS.Client/UI/ViewModels/Pages/SettingsPageViewModel.props.cs` / `src/BD.WTTS.Client.Avalonia/UI/Views/Windows/MainWindow.axaml.cs` |
| SteamConnectService / SteamServiceImpl2 / DownloadPage | `src/BD.WTTS.Client/Services/Mvvm/Steam/SteamConnectService.cs` / `src/BD.WTTS.Client/Services.Implementation/Steam/SteamServiceImpl2.cs` / `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/DownloadPage.axaml` |
| MessageBox / PlatformAccount / IPlatformSwitcher | `src/BD.WTTS.Client/UI/Widgets/MessageBox.cs` / `src/BD.WTTS.Client.Plugins.GameAccount/Models/PlatformAccount.cs` / `src/BD.WTTS.Client.Plugins.GameAccount/Services/IPlatformSwitcher.cs` |
| GameAccountPageViewModel / PlatformSettingsPageViewModel / SteamFamilyShareManagePageViewModel | `src/BD.WTTS.Client.Plugins.GameAccount/UI/ViewModels/GameAccountPageViewModel.cs` / `PlatformSettingsPageViewModel.cs` / `SteamFamilyShareManagePageViewModel.cs`（后两项同目录） |
| HideAppsPageViewModel / IdleAppsPageViewModel / IdleAppsPage | `src/BD.WTTS.Client.Plugins.GameList/UI/ViewModels/Pages/HideAppsPageViewModel.cs` / `IdleAppsPageViewModel.cs`（同目录） / `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/IdleAppsPage.axaml` |
| AcceleratorPage2.axaml.cs / AcceleratorPathAskBox.axaml.cs / ProxySettings.ProxyMode / SteamSettings.ValueChanged | `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/AcceleratorPage2.axaml.cs` / `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Controls/AcceleratorPathAskBox.axaml.cs` / `src/BD.WTTS.Client.Plugins.Accelerator/Settings/ProxySettings.ProxyMode.cs` / `src/BD.WTTS.Client/Settings/Steam/SteamSettings.ValueChanged.cs` |

源码普通 `.Value` setter 会改模型、通知再自动保存（`src/BD.WTTS.Client/Settings/Infrastructure/SettingsProperty.cs:98`、`.../Abstractions/SettingsStructPropertyBase.cs:60`），实际磁盘写入见 `.../Abstractions/ISettings.cs:467`；没有前端可核对的保存 revision 回执。本表的持久化确认、数字草稿、连续预览后提交是新契约要求，不能假称旧源码已有。

### General：23 行

| ID | 字段 | 源码入口/定位 | 前端输入与提交 → 效果 | 校验、依赖 | 恢复补充 |
|---|---|---|---|---|---|
| G-0 | AutoCheckAppUpdate | Q G:50；未定位消费 | 无新增控件；`migration_roundtrip` | bool；不由字段恢复触发更新请求 | M；下一检查效果未确认 |
| G-1 | UpdateChannel | Q G:56；未定位消费 | 无新增控件；`migration_roundtrip` | 保留原 UpdateChannelType 和未知值；不凭种子增加渠道菜单 | M |
| G-2 | AutoRunOnStartup | E G:62、SG:27 | 开关 `immediate → 平台注册结果`；下一系统登录启动 | 当前平台支持才可操作；不得因关闭清除 MinimizeOnStartup；不启动当前应用副本 | B/P；注册失败与保存结果分开 |
| G-3 | MinimizeOnStartup | E G:68、SG:36；当前启动只消费CLI silence，字段缺消费 | 开关 `immediate → 下一自动/后台启动`；修正缺口目标：普通人工启动始终显示，自动启动按此值，明确silence参数优先隐藏 | bool；独立于G-2是否开启；G-2已启用时同事务更新自启参数，不启动副本 | B/P/R；注册失败报告保存与接入差异；不改变本次窗口可见性 |
| G-4 | TrayIcon | E G:74、SG:45 | 开关 `immediate → 托盘实际显示事件` | 平台支持；关闭托盘且窗口隐藏时先确保可见返回入口，不制造无法恢复的隐藏应用 | B/P；重开按保存值建托盘 |
| G-5 | MessagePopupNotification | N G:80；NoticeService:43；GetNewsAsync:23 停用新闻获取 | 未定位设置控件；`migration_roundtrip`；保留残余通知判断 | bool；未读计数与弹窗许可分开，不因禁弹窗清空通知 | M；生产通知路线未确认，未来确认入口才用 immediate |
| G-6 | GameListUseLocalCache | Q G:86；未定位消费 | 无新增控件；`migration_roundtrip` | bool；不得据此假造缓存路线或删除缓存 | M |
| G-7 | TextReaderProvider | N G:92；IPlatformService.TextReader:14 | 平台→提供者映射，当前无直接入口；`migration_roundtrip → 下一打开文本命令` | 平台 Key、合法提供者/路径；不可用回退须报告；不主动运行编辑器 | M/P；字典整体保留 |
| G-8 | HostsFileEncodingType | N G:98；HostsFileServiceImpl:667 | 无当前设置入口；`migration_roundtrip → 下一 Hosts 写入` | 原 EncodingType；源 Auto 语义保持；不是打开设置即转码 | M/P；Hosts 操作失败不宣称编码应用 |
| G-9 | GPU | E G:104、SG:54；Program:174 | 开关 `immediate → restart_required` | bool；与 G-10 没有源码互斥关系，不能关闭 GPU 时顺手关 NativeOpenGL | B/R；展示下一启动请求与本次实际 renderer |
| G-10 | NativeOpenGL | N G:110；Program:176 | 无当前设置入口；`migration_roundtrip → 下一进程启动` | bool；Windows Wgl/CLI 优先关系按实际启动组合计算；不能宣称 GPU=false 必为软件 | M/R |
| G-11 | ScreenCapture | Q G:116；未定位消费 | 无新增控件；`migration_roundtrip` | bool；不据字段自行获取截图权限/开始捕获 | M |
| G-12 | DisablePlugins | E G:122；Settings_Plugin.axaml:55；PluginVM:21–35 | 插件开关 `immediate → restart_required`，只改对应插件 ID | UniqueEnglishName 稳定插件 ID；本项目仅四保留模块，已排除插件不可恢复成菜单；保存未知 ID 供 roundtrip | B/R；当前源提示重启，不假称已 unload |
| G-13 | PluginSafeMode | Q G:128；Startup.OnStartup:35 有重启提示，无消费 | 无新增控件；`migration_roundtrip` | bool；提示不证明插件加载采用该字段 | M；不得承诺重启真正生效 |
| G-14 | LastLookNoticeDateTime | N G:134；NoticeService:47、55、62 | `internal_event → 通知已读水位` | 合法 DateTimeOffset，保留 default；由确认已查看的事件写入，禁止浏览通知失败仍清已读 | B；跨页和重開保持水位，不自动归零 |
| G-15 | WebProxyMode | N G:142；Startup.Host:133、177 | 当前无路由；`migration_roundtrip → 下一应用 Host 初始化`；若以后暴露，用整组 `dialog_commit` | NoProxy/Custom/FollowSystem；这是应用 HttpClient 出站设置，不能和 P-18/宿主系统代理混为一项 | M/R；当前无热更新消费 |
| G-16 | CustomWebProxyModeHost | N G:148；Startup.Host:142 | 无当前直接输入；整组草稿，`migration_roundtrip → 下一 Host 初始化` | Custom 下合法 host；与 G-17 成对；不接受半个 host+port；host+port 与 Address 优先关系明确 | M/R；保留备用值，禁用不清空 |
| G-17 | CustomWebProxyModePort | N G:154；Startup.Host:142–144 | 同组数字草稿；`migration_roundtrip → 下一 Host 初始化` | 1–65535；原 0 保留为未配置，不静默回写1；仅 Custom+host 组有效 | M/R |
| G-18 | CustomWebProxyModeAddress | N G:160；Startup.Host:149–151 | 同组 URI 草稿；`migration_roundtrip → 下一 Host 初始化` | 有效受支持代理 URI；禁止 URI userinfo 混存秘密；host+port 不完整时才能按明确方案用 Address | M/R/K；不掩盖源 Custom 无地址回退系统 |
| G-19 | CustomWebProxyModeBypassOnLocal | N G:166；Startup.Host:146、151 | 同组开关；`migration_roundtrip → 下一 Host 初始化` | Custom 才有效；切换模式保持选择 | M/R |
| G-20 | CustomWebProxyModeCredentialUserName | N G:172；Startup.Host:158–163 | 无当前入口；将来凭据组 `dialog_commit`；当前 `migration_roundtrip` | Custom 且选择认证；受保护引用；保留未填/空值差异 | M/R/K；失败不写明文 |
| G-21 | CustomWebProxyModeCredentialPassword | N G:178；Startup.Host:161 | 同组密码保持/替换/清除；当前 `migration_roundtrip → 下一 Host 初始化` | 不可通过遮蔽占位值覆盖秘密；不自动填入实际内容 | M/R/K |
| G-22 | CustomWebProxyModeCredentialDomain | N G:184；Startup.Host:162 | 同组域文本；当前 `migration_roundtrip → 下一 Host 初始化` | 可为空；仅认证组有意义；无换行/控制字符 | M/R/K；切 NoProxy 不删除域 |

G-3 缺口依据：`src/BD.WTTS.Client/Startup/Startup.Commands.cs:104` 声明 silence，`:120` 设置 IsMinimize；App:107 消费 IsMinimize，当前未定位 MinimizeOnStartup 消费。G-9/10 渲染依据 Program:174：GPU 分支添加 AngleEgl/Vulkan，NativeOpenGL 分支仍可添加 Wgl，然后 Software fallback；不能自行增加未证实的 Cef 限制。G-2 订阅 App:206 立即请求平台，Windows 实现 `src/BD.WTTS.Client/Services.Implementation/Platform/Windows/WindowsPlatformServiceImpl.SystemOnOff.cs:26`；G-4 订阅 App:199 更新托盘，实际实现 `src/BD.WTTS.Client.Avalonia/UI/App.TrayIcon.cs:29`。

### UI：19 行

| ID | 字段 | 源码入口/定位 | 前端输入与提交 → 效果 | 校验、依赖 | 恢复补充 |
|---|---|---|---|---|---|
| U-0 | Theme | E U:52、SU:48 | 枚举 `immediate → 当前 UI 应用` | FollowingSystem/Light/Dark；系统跟随时仍保存原自定义颜色 | B/V；重开仍跟随系统，不固化当时主题 |
| U-1 | ThemeAccent | E U:58、SU:191、261 | 预置色点选 immediate；色板连续调整 `preview_only → commit_on_end` | 合法含 alpha 的颜色；自定义模式 U-2=false 才作为效果；源默认是 ARGB 不是 RGB | B/V；启用系统色不覆盖自定义色 |
| U-2 | UseSystemThemeAccent | E U:64、SU:70 | 系统/自定义开关 `immediate → 当前 UI` | 两种模式互斥；系统支持差异报告；自定义草稿保持 | B/V；恢复模式和用户色分别进行 |
| U-3 | Language | E U:72、SU:275；SVM:14 | 语言选择 `immediate → 当前资源应用` | 用 culture Key 匹配和存储，null/空 Key 都保留系统默认语义；不存本地化显示名称 | B/V；保持当前页、焦点、草稿和选择 ID |
| U-4 | MessageBoxDontPrompts | N U:78；MessageBox:124–142 | 确认框中的“不再提示”随有效确认 `dialog_commit → 下次同类提示` | 非 Undefined、OK 且 RememberChoose；native mbcs 路径先返回，不能假称所有对话框都有记忆；不是跳过业务验证或身份校验 | B；取消不能记录“不再提示”；无全局新增编辑器 |
| U-5 | IsShowAdvertisement | Q U:84；当前简化 Home 无消费 | 无新增控件；`migration_roundtrip` | bool；不据此恢复被简化广告服务 | M |
| U-6 | WindowSizePositions | N U:90；Window:70、144、180 | 正常窗口几何稳定/关闭 `internal_event → 下一窗口打开` | 稳定窗口 ID，finite 尺寸，最小尺寸/屏幕工作区；负坐标多屏合法；离屏只修正 effective placement，不破坏存储记录 | B/P；最小化零尺寸、关闭销毁尺寸不覆写正常尺寸 |
| U-7 | FontName | E U:96、SU:298 | 字体选择 `immediate → 当前字体资源` | 可用字体 Key；null 为默认；不可用字体保留 desired 并展示 fallback，不静默写入另一名称 | B/V；不因列表重载清空选中 |
| U-8 | GameListGridSize | Q U:102；未定位消费 | 无新增控件；`migration_roundtrip` | 原 int 保留；不凭150默认值造大小 Slider/范围 | M |
| U-9 | Fillet | Q U:108；未定位消费 | 无新增控件；`migration_roundtrip` | bool；不把圆角设计强行绑定到此字段 | M |
| U-10 | WindowBackgroundOpacity | E U:116、SU:344 | Slider `preview_only → commit_on_end → 当前窗口` | finite 0..1；材质/平台决定有效透明度；不可用保留 desired | B/V；拖动取消回到已提交值 |
| U-11 | WindowBackgroundMaterial | E U:122、SU:319 | 枚举 `immediate → 平台材质确认` | None/Transparent/Blur/AcrylicBlur/Mica 按平台能力可选；未知原值只保留，不自动转存默认 | B/V/P；效果 fallback 单独展示 |
| U-12 | WindowBackgroundDynamic | Q U:128；未定位消费 | 无新增控件；`migration_roundtrip` | bool；不据字段启动动态背景/动画服务 | M |
| U-13 | WindowBackgroundCustomImage | E U:134、SU:357 | 开关 `immediate → 当前窗口背景` | 图片可用才应用；关闭保留路径、透明度与 Stretch；不清空图片 | B/V；失败仍可取消/换图 |
| U-14 | WindowBackgroundCustomImagePath | E U:140、SU:409；SVM:78 | 文件选择成功后校验 `dialog_commit → 当前背景`；选择器取消无变化 | 文件存在、可读、真实图片格式；允许原内嵌 asset URI；空路径的“恢复默认”是明确命令 | B/V；文件之后丢失保留路径，显示 fallback/重新选择 |
| U-15 | WindowBackgroundCustomImageOpacity | E U:146、SU:394 | Slider `preview_only → commit_on_end → 当前背景` | finite 0..1；U-13=true 才有效，关闭时保留设置 | B/V；和 U-10 两个量分别存储 |
| U-16 | WindowBackgroundCustomImageStretch | E U:152、SU:428 | Stretch 枚举 `immediate → 当前背景` | 仅原 XamlMediaStretch；U-13=true 才有效；重开按 enum 恢复 | B/V |
| U-17 | SortMenuTabs | E U:160、SU:441、447；MainVM:216–224 在 Deactivation 保存 | 菜单拖放 `preview_only → commit_on_end → 当前菜单排序` | 稳定模块/页 ID 去重；仅四模块可见，未知/暂不可见 ID 保留；source HashSet 不能作为新契约顺序容器 | B/V；新契约每次结束保存，原源不是每次拖动即落盘；当前选择不随序号漂移 |
| U-18 | StartDefaultPageName | E U:166、SU:31；SettingsPageViewModel.props:48；MainWindow.axaml.cs:134 | 默认页选择 `immediate → 下一主窗口初始导航` | 稳定可达页 ID；Home/null fallback；排除插件或隐藏页保存原值但报告有效 fallback | B；设置此值不立即把当前页切走 |

语言存取缺口：ResourceService:18 初始选择按 `item.Value` 比较，SVM:14 存 `x.Key`；必须统一 culture Key，不能只实现写入，应用订阅/资源刷新见 ResourceService:116。U-2 自定义 RadioButton 在 SU:73 只是 `!Value` OneWay，必须把用户选择自定义显式提交为 false，不把视觉选中当保存成功。材质源 SU:327–330 的平台限制控件已注释，活跃 ComboBox 不限制 Mica；新实现以能力列表校验并保留原值。菜单原 HashSet schema 仍需 roundtrip，不应冒充保证顺序的类型。窗口位置源 Window:144 只接受 X/Y>0，负坐标多屏恢复需修正。窗口位置、DontPromptType 的更多实际入口是内部路线，不算设置长页新字段控件。

### Steam：11 行

| ID | 字段 | 源码入口/定位 | 前端输入与提交 → 效果 | 校验、依赖 | 恢复补充 |
|---|---|---|---|---|---|
| S-0 | SteamStratParameter | E S:50、SS:92；SVM:100–109 | 参数弹窗 `dialog_commit → next_run(Steam 客户端)` | 保留合法引号与用户参数；拒绝不可表示字符；结构化参数合成与 S-4/5/6/7 同事务，不用子串 Replace | B/D/R；取消零写入；不是游戏启动参数 |
| S-1 | SteamSkin | Q S:56；未定位消费 | 无新增控件；`migration_roundtrip` | 原可空字符串保留；不造皮肤选择 UI 或应用承诺 | M |
| S-2 | SteamProgramPath | E S:62、SS:26；SVM:62–75 | 文件选择校验成功 `dialog_commit → next_run(明确目标 Steam 操作)` | 存在/可执行/平台 Steam 目标；取消无变化；Mac/Linux 不能照搬 steam.exe filter | B/P/R；当前启动服务不消费此字段，须接通适配器后才宣称 next_run 有效 |
| S-3 | IsAutoRunSteam | E S:68、SS:35；SteamConnectService:270 | 开关 `immediate → next_run(应用初始化，Steam 未运行)` | bool；当前保存不启动/关闭 Steam | B/R；关闭不停止已运行 Steam |
| S-4 | IsRunSteamMinimized | E S:74、SS:56；SteamSettings.ValueChanged:38 | 开关 `immediate → next_run(Steam)`；同步规范化 S-0 的 -silent token | bool；不得替换 custom 参数中的同名子串或制造重复 flags | B/R；两字段事务失败整组不变 |
| S-5 | IsRunSteamNoCheckUpdate | E S:80、SS:65；ValueChanged:33 | 开关 `immediate → next_run(Steam)`；同步 -noverifyfiles | bool；源字段命名与实际 flag 不同，界面说明不能承诺任何版本的 Steam 都禁更新 | B/R；原参数不相关 token 保留 |
| S-6 | IsRunSteamChina | E S:86、SS:74；ValueChanged:43 | 开关 `immediate → next_run(Steam)`；同步 -steamchina | bool；不因此切换本应用登录身份、库或 epoch | B/R |
| S-7 | IsRunSteamVGUI | N S:92；ValueChanged:28，无活跃 SS 控件 | 无新增控件；`migration_roundtrip → next_run(Steam 参数合成)` | bool；保持历史 -vgui 语义，Steam 当前支持未实测 | M/R；与参数同事务 |
| S-8 | IsEnableSteamLaunchNotification | E S:98、SS:83；SteamConnectService:309 | 开关 `immediate → 下一成功 Steam 原生连接通知` | bool；不是任意进程启动瞬间通知，禁止重放旧连接事件 | B；禁用不关闭 Steam |
| S-9 | DownloadCompleteSystemEndMode | E S:104；GameList DownloadPage:35、57、61、67 | 电源菜单 `immediate → 下次显式下载监视任务` | Sleep/Hibernate/Shutdown 依平台；模式选择不等于监视已启用；监视/倒计时用独立运行态 | B/P/R；冻结任务 mode，变更时明确更新/取消；可取消真实倒计时 |
| S-10 | IsRunSteamAdministrator | E S:110、SS:47；SteamServiceImpl2:43 | Windows 开关 `immediate → next_run(Steam 请求)` | Windows 能力/用户明确启动动作；不自动提权重启；其他平台保留无效果 | B/P/R |

Steam 参数源 `src/BD.WTTS.Client/Settings/Steam/SteamSettings.ValueChanged.cs:18` 对字符串 append/Replace；迁移以完整 token 合成避免去除用户参数。S-2 的当前效果不能夸大：`GameAccount/UI/ViewModels/GameAccountPageViewModel.cs:19` 只初始化平台 DefaultExePath；`ref/SteamClient/src/BD.SteamClient/Services.Implementation/SteamServiceImpl.cs:55` 在构造时缓存系统探测路径，`.Abstract.cs:144` 从系统位置探测，未读 S-2。重建同样的旧 service 并不足以接通选定路径。下载源 `src/BD.WTTS.Client/Services/Mvvm/Steam/SteamConnectService.cs:450` 在完成时读模式，`:457` 通知“30秒”但 `:462–471` 直接调用电源动作；新契约必须真正等待、可取消并报告任务身份，不能照搬该矛盾反馈。

### Proxy：26 行

P 设置保存后均不等于宿主代理改变。P-5..22/P-25 是一次代理运行配置快照；运行中修改显示待下次运行，用户显式重新应用由后端串行停止/启动与补偿。新的流量不经 Flutter/FRB。测试严禁 127.0.0.1:10808；本项目若后续进行隔离测试，监听一律 ≥11808、先探测，且不得改宿主系统代理。

| ID | 字段 | 源码入口/定位 | 前端输入与提交 → 效果 | 校验、依赖 | 恢复补充 |
|---|---|---|---|---|---|
| P-0 | IsAutoCheckScriptUpdate | E P:50、SC:79；PS:743 无此条件，属于需修缺口 | 开关 `immediate → 下一自动检查触发`；自动job开始前读取保存值，关闭不创建新的自动检查 | bool；手动更新独立；已经开始的job按其取消契约处理，不借关闭开关伪报已停 | B/R；实现并核实触发策略后才宣告效果，原源码尚未接通 |
| P-1 | IsEnableScript | E P:56、AP:246；PO:48 | 开关 `immediate → next_run(代理)` | 有效脚本/支持运行方式；禁用不删除脚本，也不改变单脚本 Disable | B/R；运行中效果仍按 applied 快照 |
| P-2 | SupportProxyServicesStatus | E P:62、AP:498/517；PS:67–76 | 规则组勾选 `immediate → next_run(代理规则快照)` | 稳定规则 ID；三态父组计算子项；中间态不是可持久化第四布尔；未知规则保留 | B/R；刷新列表不重置用户选择 |
| P-3 | ScriptsStatus | Q P:68；PS:88–89 保存订阅被注释 | 无独立控件；`migration_roundtrip` | 原整数集合保留；当前单脚本有效启用由脚本实体 Disable 保存，不能另造双真值 | M；不得靠此集合重开覆盖脚本实体 |
| P-4 | ProgramStartupRunProxy | E P:76、SP:61、AP:253；PS:304、353 | 两处同一开关 `immediate → next_run(应用初始化)` | bool；两个入口共享一个保存值；打开设置不启动代理 | B/R；关闭不停止当前实例 |
| P-5 | SystemProxyPortId | E P:82、SP:49；PO:50 | 端口数字 `commit_on_blur → next_run(代理)` | 1–65535；源0作为默认哨兵保留，人工0须明确“使用默认”；应用前检查占用/绑定权限 | B/R/P；存储可合法但启动冲突须单独报错 |
| P-6 | SystemProxyIp | E P:88、SP:27；PO:51 | IP 文本 `commit_on_blur → next_run(代理)` | 可绑定 IP/原 Any 语义；明确0.0.0.0暴露范围；不把非法文本自动归一成 Any | B/R/P |
| P-7 | OnlyEnableProxyScript | E P:94、SP:110；PO:54 | 开关 `immediate → next_run(代理)` | P-1 启用且有适用脚本才有效；保留关闭时规则集合；运行模式能力可见 | B/R；不可把没有脚本伪装为加速成功 |
| P-8 | ProxyMasterDns | E P:100、SP:36–38；PO:72 | DNS 文本 `commit_on_blur → next_run(代理)` | 合法 DNS 地址与受支持表示；UseDoh=true 时控件禁用、备用值保留，不清空 | B/R；校验失败原文保留 |
| P-9 | EnableHttpProxyToHttps | E P:106、SP:68；PO:55 | 开关 `immediate → next_run(代理)` | bool；只影响适用请求转换，不承诺所有站点强制 HTTPS | B/R |
| P-10 | Socks5ProxyEnable | N P:116；PO:56，未定位活跃控件 | 无新增控件；`migration_roundtrip → next_run(代理)` | bool；运行适配器能力检查；不凭字段新增 Socks 服务页面 | M/R/P |
| P-11 | Socks5ProxyPortId | N P:122；PO:59–61，未定位活跃控件 | 无新增控件；`migration_roundtrip → next_run(代理)` | 原 ushort；1–65535 有效，0/不合法原值保存且 effective fallback 报告；仅 P-10 有效 | M/R/P |
| P-12 | TwoLevelAgentEnable | E P:132、SP:121；PO:62 | 开关 `immediate → next_run(代理)` | 启用前整组地址/类型/端口有效；禁用保留端点和凭据 | B/R/K；组内尚有无效草稿时阻止启用并定位字段 |
| P-13 | TwoLevelAgentProxyType | E P:138、SP:127；PO:63–65 | 枚举 `immediate → next_run(代理)` | 原 ExternalProxyType/实际支持项；仅 P-12 有效；unknown 保存不当有效选择 | B/R |
| P-14 | TwoLevelAgentIp | E P:144、SP:140；PO:66 | 地址 `commit_on_blur → next_run(代理)` | 合法后端支持地址；不能把拼错地址静默改成 loopback；P-12=false 保留值并禁用效果 | B/R |
| P-15 | TwoLevelAgentPortId | E P:150、SP:152；PO:67–69 | 数字 `commit_on_blur → next_run(代理)` | 1–65535；零原值 roundtrip/effective fallback 分离；P-12有效；不在保存时连接 | B/R |
| P-16 | TwoLevelAgentUserName | E P:156、SP:159；PO:70 | 用户名草稿 `commit_on_blur → next_run(代理)` | P-12有效；无控制字符；认证方式依协议；与密码保护引用原子一致 | B/R/K；关闭保留，不日志回显 |
| P-17 | TwoLevelAgentPassword | E P:162、SP:170；PO:71 | 遮蔽字段明确替换/清除后 `commit_on_blur → next_run(代理)` | P-12有效；空占位不清除旧秘密；不恢复明文草稿 | B/R/K |
| P-18 | ProxyMode | E P:172、AP:225–239；PO:126 | 停止时模式选择 `immediate → next_run(代理)` | 运行/启停中禁改此选择；平台合法枚举由 ProxySettings.ProxyMode:11 决定；不抹掉迁入的 unsupported 原值 | B/R/P；当前 applied 模式旁明示 |
| P-19 | IsProxyGOG | N P:180；PO:53、129，未定位活跃控件 | 无新增控件；`migration_roundtrip → next_run(代理)` | bool；平台/规则能力与 GOG 是否真实可用未实测 | M/R |
| P-20 | IsOnlyWorkSteamBrowser | E P:186、SC:80；PO:49、125 | 开关 `immediate → next_run(代理)` | 只在源适用脚本/浏览器作用域生效；不是全局 Steam/本应用网络代理开关 | B/R；保留切换前规则 |
| P-21 | UseDoh | E P:192、SP:75；PO:74 | 开关 `immediate → next_run(代理)` | 开启时自定义 URL 有效或明确使用默认；P-8仍保留；不在每次绑定时测速 | B/R；DNS 检查失败不假报启动成功 |
| P-22 | CustomDohAddres2 | E P:198、SP:87；PO:75 | URL 文本 `commit_on_blur → next_run(代理)` | P-21=true 才有效；合法 HTTPS DoH URL，空值=源默认；禁 userinfo/控制字符 | B/R；无效草稿阻止该组应用，旧 applied 保持 |
| P-23 | AcceleratorTabsSelectedIndex | N P:204；AcceleratorPage2.axaml.cs:36、60–65 | 真实用户切 tab `internal_event → 当前页面选择恢复` | legacy 可见序号映射稳定 tab ID；动态消失回退有效 tab；状态导致的自动跳转不覆盖用户偏好 | B；跨页/重开按稳定 ID，不按新可见数组同索引 |
| P-24 | AutoShowWattAcceleratorWindow | E P:210、AP:358；XV:312–314 | 开关 `immediate → 下一 SDK 加速成功事件的厂商窗口请求` | SDK 支持且该事件属于当前 operation/epoch；不是现在立刻显示/关闭厂商窗口 | B/R/P；厂商窗口请求结果未验证前不显示已应用 |
| P-25 | ProxyBeforeDNSCheck | E P:216、SP:94；PO:80–88 | 开关 `immediate → next_run(代理启动前 DNS 检查)` | bool；UseDoh 决定检查路径；用户取消/超时独立反馈；不长期占 UI busy | B/R；本次启动采样后设置变化用于下次 |

P-0 缺口：PS:743 的 CheckScriptUpdate 请求接口并标记版本，PS:577、694 调用，当前未读此开关。网络启动源 PO:211/227 先设置系统代理/PAC，PO:260 才启动监听，因此“保存/检查/平台改变/监听成功”必须分步，并要求失败补偿；本次未执行平台操作。重置源 `Accelerator/UI/ViewModels/ProxySettingsWindowViewModel.cs:48` 仅重置 P-6/P-8/P-5/P-4/P-9/P-21/P-22/P-7，`:61` 一次保存；不含二级代理、P-25或全部26项，按钮应明确范围，禁加一个模糊“重置所有”。

### Account：5 行

| ID | 字段 | 源码入口/定位 | 前端输入与提交 → 效果 | 校验、依赖 | 恢复补充 |
|---|---|---|---|---|---|
| A-0 | AccountRemarks | E A:50；PlatformAccount:57、IPlatformSwitcher:24 | 备注弹窗 `dialog_commit → 当前账号显示` | 稳定平台+账号 ID，保留 null/空备注；长度按存储/UI能力，禁控制字符；不是登录名/凭据修改 | B/D；账号切换旧备注草稿不可写新账号 |
| A-1 | DisableAuthorizedDevice | N A:56；SteamFamilyShareManagePageViewModel:56、172；页面入口注释 | 无当前可达新增入口；`migration_roundtrip` | 完整原DisableAuthorizedDevice复杂值保留；不据保留字段自动撤销授权或访问家庭分享 | M/D；本范围不恢复该路由 |
| A-2 | EnablePlatforms | E A:62；GameAccountPageViewModel:52、64、93 | 添加平台确认/删除命令 `dialog_commit 或 immediate → 当前平台列表` | 稳定平台 ID 去重；按平台能力；移除展示平台不删除账号/凭据/路径；Steam 基础平台不可误删 | B/D；恢复选择可达平台，不用序号 |
| A-3 | PlatformSettings | E A:68；PlatformSettingsPageViewModel:15、32–33 | 平台设置弹窗独立草稿 `dialog_commit → 下一平台操作` | 嵌套 PlatformPath(Key0) 文件存在、可执行、正确目标；取消不得要求先修好路径；不自动关闭/启动客户端 | B/D/P；平台与账号 epoch 校验；字典未知项保留 |
| A-4 | IsShowAccountName | E A:74、AC:130 | 显示账户名开关 `immediate → 当前账号卡片` | bool；控制显示，不改账号身份、别名或登录行为 | B；跨页/重开一致；不因开启输出身份到日志 |

### Library：7 行

| ID | 字段 | 源码入口/定位 | 前端输入与提交 → 效果 | 校验、依赖 | 恢复补充 |
|---|---|---|---|---|---|
| L-0 | GameInstalledFilter | E L:50、LP:324；LV:27、30–31 | 过滤开关 `immediate → 当前库过滤` | bool；过滤不删实体；移出可见范围的ID从active selectedIds剔除，可另存inactive恢复记忆，批量操作不能使用它 | B；解除过滤可按有效ID恢复，重开先读取再过滤，不由绑定回写 |
| L-1 | GameLibraryLayoutType | Q L:56；LP:240–251 选择器注释 | 无新增控件；`migration_roundtrip` | 原 GridLayoutType 保留；不能凭 schema 恢复多布局按钮 | M |
| L-2 | GameCloudArchiveFilter | E L:62、LP:334；LV:28、37 仅读取/过滤，无保存订阅 | 开关 `immediate → 持久化+当前库过滤` 为待实现契约 | bool；本地标记过滤不等于读回 Cloud/同步成功；保留实体选择与库草稿 | B；必须补持久化消费，不能仅局部过滤后承诺重开恢复 |
| L-3 | GameTypeFiltres | E L:68、LP:286；LV:41–48 | 类型勾选 `immediate → 当前库过滤` | 原 SteamAppType List，去重但保留未知原值；空选择=无匹配，显示空态/清筛选入口，不能自动重勾默认 | B；一次批量全选只提交一次 |
| L-4 | HideGameList | E L:74；LV:298；HideAppsPageViewModel:40 | 隐藏/取消隐藏命令 `immediate → 当前库可见集合` | uint AppId 为主键；可空名称只是缓存；同一身份 epoch；不从 Steam 卸载/删 Cloud | B/D；执行后就近选择，取消隐藏恢复稳定 ID |
| L-5 | AFKAppList | E L:80；LV:251、282–284；IdleAppsPageViewModel:69、99 | 加入/移除 AFK 命令 `immediate → 保存任务候选集合` | 去重 AppId；源上限32；加入不等于已启动；全删确认一次；活动 worker 需单独停止确认/结果 | B/D/R；与成就、Cloud、Idle worker 的资源互斥；重开不能假称运行 |
| L-6 | IsAutoAFKApps | E L:86；IdleAppsPage.axaml:101；SteamConnectService:314–317 | 开关 `immediate → 下一成功 Steam 原生连接自动启动` | bool；必须确认当前 SID/epoch、候选数与 worker 互斥；开启本次不立即启动 | B/R/D；关闭不偷偷停止已运行 worker |

### Idle：8 行

| ID | 字段 | 源码入口/定位 | 前端输入与提交 → 效果 | 校验、依赖 | 恢复补充 |
|---|---|---|---|---|---|
| I-0 | IdleTime | Q I:50；未定位持久化字段消费 | 无新增控件；`migration_roundtrip` | 原 TimeSpan；不得与 IV 运行 elapsed IdleTime 混淆，不把6分钟设成暂停/刷新按钮 | M；运行计时不写此字段 |
| I-1 | IdleRule | E I:56、IP:190；IV:325–339 | 规则枚举 `immediate → 串行调度重新配置` | FastMode/OnlyOneGame/OneThenMany/ManyThenOne；身份有效、任务快照锁；不是每帧 reset | B/R/D；运行中只排一个 operation，旧回调 epoch 失效 |
| I-2 | IdleSequentital | E I:62、IP:204；IV:346–358 | 顺序枚举 `immediate → 串行重排队列` | Default/LeastCards/Mostcards/Mostvalue 原拼写；排序取合法完整 badge 快照；当前 AppId 选择保持 | B/R/D；重排/重启结果分别反馈 |
| I-3 | MaxIdleCount | E I:68、IP:219–222；IV:678 | 数字 `commit_on_blur → next_cycle(调度边界)` | 整数2..32；仅并行挂时长场景有效；单游戏规则保留值不假称并行数生效 | B/R；降低数量在边界协调自有 worker，不硬杀非管理进程 |
| I-4 | MinRunTime | E I:74、IP:226；IV:561、590、603、675 | 数字 `commit_on_blur → next_cycle(队列分类边界)` | finite ≥2，单位小时；checked 换算不超过 TimeSpan.MaxValue（Int64.MaxValue 个100ns tick），不自行另加业务上限 | B/R；应用前展示下个边界，不改已有 badge 时长事实 |
| I-5 | SwitchTime | E I:80、IP:224；IV:792 | 数字 `commit_on_blur → next_cycle(切换 deadline)` | finite ≥5000，单位毫秒；checked 时长范围同 I-4；允许非100ms整倍数；单调时钟 deadline，禁止源码 modulo 导致永不命中 | B/R；显式更新下一 deadline 一次，不重开整个任务 |
| I-6 | RefreshBadgesTime | E I:86、IP:228；IV:841 | 数字 `commit_on_blur → next_cycle(下一等待周期)` | finite ≥1，单位分钟；checked 时长范围同 I-4；不重置现有等待；立即刷新是独立命令 | B/R/D；源当前 Task.Delay 周期采样，保存不承诺立即重置倒计时 |
| I-7 | BlacklistAppList | E I:92；IV:206–218、492 | 黑名单命令 `immediate → 串行队列 reconcile` | 稳定 AppId；备注可空；任务账号 epoch；禁/解禁不删除库存或 AFK 表 | B/R/D；重建失败保留 desired 与旧 applied 队列，显示状态 |

Idle 规则/顺序源订阅在 IV:34–35，运行变更会 stop/reset/start；后端必须串行化，快速多次选择合并到最新 desired，不能同时启动多个队列。所有 I 设置提交不绕过挂卡网页登录身份验证；旧网页登录 SID 与本机 Steam SID 不同必须阻止任务开始，不能仅 Toast 后继续。

### SDK：2 行

| ID | 字段 | 源码入口/定位 | 前端输入与提交 → 效果 | 校验、依赖 | 恢复补充 |
|---|---|---|---|---|---|
| X-0 | MyGames | E X:34；XV:302–307、424–435、489–508、516 | 收藏加删 `immediate`；游戏区服弹窗 `dialog_commit → 下次显式 SDK 加速` | 厂商 game/area/server 稳定 ID；重新取可用区服，不能按列表序号恢复；原复杂 VM 全值 roundtrip，但运行 flags 不当当前事实 | B/D/R；SDK epoch/operation 校验，旧测速 callback 不写当前新游戏；失败保留区服草稿 |
| X-1 | WattAcceleratorDirPath | E X:40；XV:373–380、623；AcceleratorPathAskBox.axaml.cs:18–41 | 安装路径弹窗文件夹草稿 `dialog_commit → next_run(显式安装)` | 目录可访问/可写；规范化且仅补一次 WattAccelerator 子目录；确认所选目标，选择器取消/弹窗关闭不改旧路径 | B/D/P；安装失败保留确认路径和进度结果，不假称 SDK 已安装 |

X-1 源在选择目录时（AskBox:34）已立即写字段，弹窗确认才（:41）Close(true)。本契约要求把选择移入独立弹窗草稿，确认后提交；这属于明确修复，不能声称旧取消语义正确。SDK 安装/区服/测速均未运行。

## 尚不能确认的路线与冲突处理

本表恰好 **101 行**：G23 + U19 + S11 + P26 + A5 + L7 + I8 + X2。前端入口定位分类为 E67、N21、Q13，均 `identified`，无 `verified`。N21 仅内部路线或未确认可达设置 route，不新增 UI；Q13 当前没有确认业务消费或有效入口：**G-0、G-1、G-6、G-11、G-13、U-5、U-8、U-9、U-12、S-1、P-3、L-1、I-0**。这些字段都必须保留，不能因 route 不存在降迁移分母或删除复杂值。

活跃入口也有应明确修复的闭环缺口：G-3 最小化存储但无启动消费；S-2 选定路径未接入实际 SteamService；P-0 自动检查开关未被 CheckScriptUpdate 读取；L-2 Cloud 过滤缺保存订阅；U-3 culture Key 写入与显示名称读回不一致；U-2 自定义 RadioButton 没有 TwoWay 写回。G-13 仅重启提示不足以证明有效消费。以上缺口不得通过 UI “已应用”文案掩盖。

冲突处理定为以下规则，避免执行模型自行选择：

1. **Settings 长页逐字段保存，库/平台/区服弹窗整份草稿提交。** 不新增全页 Save/Cancel，不共享游戏编辑引用；两种模式不互相借用取消语义。
2. **GPU和NativeOpenGL保留独立字段和请求意图。** 不制造互斥；源Avalonia的Wgl/Angle/Vulkan次序是证据，不能照抄成Flutter已支持。S01必须验证目标renderer适配与组合映射；未支持能力登记缺口，不用无效果开关宣布全移植。请求与实际renderer分开，用户提示使用平台可理解的原因。PluginSafeMode无消费不承诺重启效果。
3. **Steam 参数和四个 flag 以 token 合成事务维护。** 保留用户自定义 token，不用简单 Replace/append；手改参数后四开关反映明确 token 状态，歧义输入要求就地澄清，不能悄悄去掉参数。
4. **应用 WebProxy 和社区代理独立。** G15–22 当前只在应用 Host 初始化使用，不把它们当 P 组热更新，也不能连带修改宿主系统代理。
5. **代理组保存后待下次运行，模式仅停止时可选。** Reset 范围固定8项；规则三态只落子项，脚本实体 Disable 是已确认启用事实，P-3旧集合仅 roundtrip。
6. **有序菜单和动态 tab 用稳定 ID。** 保留旧 HashSet/Index 兼容数据，同时新模型存 ordered IDs；默认页变化不立即导航，动态消失 fallback 不覆盖用户偏好。
7. **运行事实不从配置恢复。** MyGames历史flags、L-6、下载监视与挂卡选项都不证明重开后任务仍运行。账号切换递增epoch；旧草稿提交拒绝，旧callback不写新UI，已提交旧Cloud/成就任务仍按原上下文结算/核对。
8. **未知值保存与有效 fallback 分离。** 无效人工输入不可提交；迁入历史未知值仍 roundtrip，当前效果按能力判断并报告。不要为了“校验通过”清掉秘密、未知页 ID、未知枚举或不可用路径。

嵌套 PlatformSettings.PlatformPath、SizePosition 四坐标、DisableAuthorizedDevice 和 XunYouGameViewModel 的详细 schema 仍属于领域普查；不计入这101行，也不意味着可以忽略。输入安全上限、平台支持、字体/材质真实结果、开机注册/托盘行为、Steam 参数版本效果、网络恢复补偿和 SDK 真实反馈皆**未验证**；本次只进行只读源码定位与文档字段完整性核对，产品/平台测试**未运行**。
