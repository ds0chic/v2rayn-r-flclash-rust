# SteamBox 四模块跨页流程与交互家族只读研究

状态：`identified`。文件名沿用本轮研究任务约定；本轮读取持续到 2026-10-03（Asia/Shanghai）。

范围：Accelerator、GameAccount、GameList、SteamIdleCard，及串联这四模块必需的主壳、托盘、Steam 身份与后台任务。Authenticator、ArchiSteamFarmPlus/ASF、GameTools 不进入流程、实现或验收分母。

本报告只读取源码与方案，并新建本证据文件。原 GUI、Steam、网络加速、第三方 SDK、系统代理、证书/Hosts 操作、账号切换、Cloud/成就写入、电源动作：**未运行**。所有平台结果与视觉/流畅性：**未验证**。以下“源码事实”表示事件、调用及分支已定位；“合同建议”表示重构时应明确的行为，不表示产品已经实现或验证。

## 1. 基线与适用台账

源码根：[SteamTools](C:/Users/Colby/.gemini/antigravity/scratch/SteamTools)。本轮 `git rev-parse HEAD` 返回 `a7a5ce7f0830b9b759090a75b59d82679084b478`；读取的是当前工作树，不能用 HEAD 代替冻结 dirty 文件及递归子模块。

已阅读 `outputs/STEAMBOX_SOURCE_INVENTORY.md`、`outputs/STEAMBOX_FLUTTER_RUST_PLAN.md` 的范围、UI、契约、账号、库、挂卡、网络和任务章节。`outputs/` 保持只读。已有 `docs/tasks/T01.md` 和 `compat/{features,actions,fields,layouts}.yaml` 锚定 v2rayN 7.25.4，不是 SteamBox 台账；没有把本报告的发现写进这些台账或降低其分母。SteamBox 本轮对应方案 S04/S05/S08、S10、S12–S24、S31，正式进入实现前仍需 S00/S02 冻结专属 actions/layouts/fields。

下文源定位使用这些前缀，全部相对上述源码根：

| 前缀 | 实际目录 |
|---|---|
| A | `src/BD.WTTS.Client.Plugins.Accelerator` |
| GA | `src/BD.WTTS.Client.Plugins.GameAccount` |
| GL | `src/BD.WTTS.Client.Plugins.GameList` |
| IC | `src/BD.WTTS.Client.Plugins.SteamIdleCard` |
| Shell | `src/BD.WTTS.Client.Avalonia` |
| Steam | `src/BD.WTTS.Client/Services/Mvvm/Steam/SteamConnectService.cs` |
| IdleWeb | `ref/SteamClient/src/BD.SteamClient/Services.Implementation/SteamIdleCardServiceImpl.cs` |

不读取真实账号目录、配置、数据库、日志、浏览器资料或任何秘密。源码内字段名如 AccessToken、Cookie 是契约符号；报告不包含其真实值。

## 2. 主长流程：操作相连时必须保持的事实

| 阶段 | 源码已定位的入口及事实 | 前端必须承接的结果与后继 |
|---|---|---|
| 手动打开 → 进入网络页 | `Shell/UI/App.axaml.cs:63` 注释及启动分支区分显式 silence；`Shell/UI/Views/Pages/MainView.axaml.cs:26–45` 同页不重复导航，根导航 `useCache:false`；`Shell/Services.Implementation/UI/Widgets/NavigationService.cs:58–102` 清 BackStack | 普通启动显示窗口；silence 语义单独验收。四根页切换保持领域任务，恢复轻量页面状态；不能把隐藏窗口等同进程退出 |
| 选择平台规则 → 修改代理/脚本设置 → 启动 | `A/UI/Views/Pages/AcceleratorPage2.axaml:229` 运行时禁模式选项；`:498/:517` 三态规则；`A/Services/Mvvm/ProxyService.cs:117–155` 用 ProxyStarting 防并发；`ProxyService.Operate.cs:47–75/:99–118` 启动时取设置/脚本快照 | 勾选结果、持久化结果和已应用规则分别显示。启动即时接受，阶段反馈来自真实操作；保持一个 runtime owner。页切换不停止加速 |
| 运行 → 日志/流量/网络检测 → 脚本更新 | `AcceleratorPage2.axaml:443–470` 按 ProxyStatus/IsAnyProxyScripts 改变可见 tab；`.axaml.cs:52–55` 状态变化强制切 0/1；`ProxyService.cs:701–737` 下载替换脚本行 | 启停时当前 tab 消失才回退到可见 tab；用户正在检测/查看日志时不无条件抢焦点。脚本更新保持原 script ID 行与选择，失败不丢旧版本；说明本次更新何时应用到运行服务 |
| 社区加速旁的 SDK → 选游戏/区服 → 加速 → 启动游戏 | `GameAcceleratorService.cs:340–486` 安装检查、区服对话框、发送开始/停止；`:83–181` SDK 事件更新状态；`:438–456` 101 仅显示“正在加速”；`GameInfoPageViewModel.cs:23–49` 返回清区/服，提交要求完整选择 | SDK 能力独立于社区代理；安装/权益/请求发送/加速完成/游戏启动是不同结果。外部 SDK 窗口出现和返回不能丢原路由。取消区服不改变已生效区服 |
| 加速保持运行 → 进入账号页 → 切 A 到 B | `GA/UI/Views/Controls/AccountItems.axaml:245–252` 显式切换与双击；`GA/Models/PlatformAccount.cs:26–29` 成功 toast；`SteamPlatformSwitcher.cs:14–66` 停客户端、写用户、延迟、启动并 return true | 单一切换事务。冻结旧 Steam 身份关联写操作/worker，处理不可取消动作结果，再切身份；网络页继续运行。B 的真实 SteamId 尚未核实时显示“已应用登录配置，等待客户端确认”，不能直接标成 B 已登录 |
| B 确认 → 游戏库搜索/过滤 → 打开游戏/Cloud/成就 | `GL/UI/ViewModels/Pages/GameListPageViewModel.cs:27–89/:180–245` 搜索/筛选、编辑、Cloud 与成就独立进程；`Steam:128` RuningSteamApps 仅按 AppId 索引 | 库数据带 account epoch/来源（当前在线账户或本地缓存）。Cloud/成就窗口标题与操作上下文固定 SteamId+AppId；切账号后旧窗口可查看旧快照，不能继续向新账户写入 |
| 游戏详情编辑 → 选图/改启动项 → 保存 → 编辑列表“保存到 Steam” | `EditAppInfoPageViewModel.cs:38–40` App 直接共享；`:120–188` 图片先写、IsEdited、提示下一步；`EditAppsPageViewModel.cs:44–58` 最终写 Steam，可提示重启 | 保留原两阶段流程，清楚显示“已保存本地修改”和“已应用到 Steam”。完整草稿副本隔离文本、启动项、图片；取消撤草稿；部分图片失败必须列出失败项，不能全局显示已完成 |
| 下载页 → 选择监听游戏 → 设置结束动作 → 等待完成 | `DownloadPageViewModel.cs:24–65` 三态全选与 WatchDownloadingSteamAppIds；`Steam:89–100/:450–473` 最后一个 ID 不再下载后调用电源操作 | 选择集合独立于可见行；完成、暂停、断网、不可见/半写 manifest 不能混为一类。后续动作有可取消倒计时，到期重新验证，换账号/Steam 失联使倒计时失效；不能因一次空快照触发 |
| 进入挂卡 → Web 登录/验证码或 Cookie → 徽章读取 → 启动 | `IC/.../IdleSteamLoginPageViewModel.cs:21–78/:81–121` 两种登录与记住；`IdleCardPageViewModel.cs:94–154/:269–308` 客户端/Web 身份分别判断；`:366–428` 刷新 token、徽章、家长 PIN | 区分客户端未启动、客户端未登录、Web 待登录/挑战/会话失效、身份不符、徽章读取失败、有效零掉卡。身份不符必须阻止启动；验证码/PIN 取消返回原上下文，不把错误当空队列完成 |
| 挂卡运行 → 优先/黑名单/规则 → 去别页 → 再切账号 | `IdleCardPageViewModel.cs:175–224/:325–364/:691–723` 调度操作和停止 worker；`GL/.../IdleAppsPageViewModel.cs:120–201` AFK 共用 AppId 进程字典 | 调度命令串行，明确优先操作暂停了哪种自动切换。AFK 与挂卡各有任务 owner，停止一个不误停另一任务/Cloud/成就/用户游戏。账号切换必须停止或冻结旧 epoch 的挂卡会话 |
| 最小化/关闭到托盘 → 还原 → 真退出 → 再开 | `Shell/UI/Views/Windows/MainWindow.axaml.cs:116–124` TrayIcon=true 取消关闭并 Hide；`App.TrayIcon.cs:60–64` 改 ShutdownMode；`App.Interface.cs:14–75` 复用/重建窗口、恢复焦点 | 隐藏继续后台工作、还原订阅最新快照并恢复当前页/滚动/选择；显式退出执行资源清理。重开先探活/恢复 journal，再显示实际运行状态，不照搬上次布尔值。托盘开/关×普通/silence 四格独立验收 |

以上每个后继阶段均需使用前一阶段的真实完成事实。不能把“请求已发出”“登录配置已写”“Cloud API 接受写入”“缓存为空”升级成完整业务成功。

## 3. 源码状态断点与会别扭的路径

这些是静态可证的调用/状态风险；实际症状和发生频率未验证。重构应记录有源依据的纠正项，不能把旧缺陷当必须复刻的交互习惯。

| 编号 | 源码事实 | 串联流程中的影响与纠正要求 |
|---|---|---|
| R01 | `AccountItems.axaml:251–252` DoubleTapped 命令，同时 `.axaml.cs:15–23` 祖先路由再发命令；后者设 Handled，但事件先后未实测。`PlatformAccount.cs:26` 使用 `ReactiveCommand.Create<IAccount>(async ...)`，未见业务切换 operation/互斥 | 双击/显式按钮/托盘/URI 统一一个入口与 idempotency key；同目标合并，多目标排队/拒绝并解释。不要声称已验证双执行，但必须覆盖一次双击仅产生一个切换事务 |
| R02 | `SteamPlatformSwitcher.cs:16` 未检查停止返回值；`:17/:59` 固定 300/200ms；`:63` 用户列表缺失报错，`:65–66` 仍启动并 return true；`:30–46` 先更新 MostRecent | UI 可先绿后客户端失败，后页取得错误身份。成功分为配置应用/客户端启动/SteamId确认；保留原卡片位置，失败显示可恢复操作并恢复本应用已改资源 |
| R03 | `GameAccountPage.axaml.cs:29–40/:57–69` CancelCloseAction 在无效路径时返回阻止关闭；`PlatformSettingsPageViewModel.cs:32–33` 选文件即修改共享设置 | 首次选平台被无效路径弹框困住。合法取消应回原平台页并显示该平台未配置，保留平台/目标账户；路径草稿提交时核验，不让“取消”成必须填正确路径 |
| R04 | `ProxyService.Operate.cs:211–217/:227–231` 先写 System/PAC 接入，`:260` 后启动监听；该 start 失败分支未见恢复此前 System/PAC；Hosts 失败则在 `:296` 停监听 | 启动失败 toast 后可能留下失效接入。前端需要“启动失败且恢复成功/需要恢复”两个结果；服务先探活监听，再写接入。恢复入口在网络页，不凭 toast 假定停止 |
| R05 | `AcceleratorPage2.axaml.cs:52–55` 每次 ProxyStatus 变化切 0/1；`:27–33` 仅初始化时记录 visible indices；ProxyService runtime 设置取启动快照；`ProxySettingsWindowViewModel.cs:48–61` 重置只覆盖部分字段 | 切日志/检测被抢到服务页、可见 tab 集合过时；设置写了却不生效或“重置”范围不明。tab 用稳定 ID 和实时可见集合，缺失时才回退；逐字段标明保存/应用范围；重置准确列出受影响组 |
| R06 | `ProxyService.cs:701–737` async void 下载仅开头/结尾置 IsLoading，无 finally；`EditAppInfoPageViewModel.cs:216–233` 图库同样无 finally；SDK `DownloadCallbackAsync:659–661` 失败 code 范围分支空处理 | 异常后行/图库/安装弹窗可能持续忙态。operation terminal event 必达，finally 释放局部忙态；旧可用内容保留，失败含原因和重试，不锁整个页面 |
| R07 | `GameAcceleratorService.cs:316–321` 测速回调写当前全局 CurrentAcceleratorGame，不检查回调对应 game/area/server/epoch | 用户换游戏/区服后旧测速污染新游戏。回调绑定 vendor epoch+operation+game+area+server；过时回调丢弃；断开恢复明确标无测量值，0 不能代表已测零丢包 |
| R08 | `GameListPageViewModel.cs:28` 读取 Cloud filter；在该模块搜索仅见这处读取和 schema，未定位类似 `:30–31` installed filter 的保存订阅；布局切换 `GameListPage.axaml:240–251` 是注释 | 重开/切页过滤可能丢失；不能凭 schema 增加当前不存在的布局按钮。把 filter 恢复合同写入 S08/S14，布局入口以当前可达性冻结 |
| R09 | `EditAppInfoPageViewModel.cs:38–40` 共享 App；Cancel `:191–194` 只刷新图片；文本/启动项直接绑定 App；图库无选择 reset 分支 `:236–244` 立即写 Steam 图片 | 对话框取消不能保证撤文本/启动项/图片真实副作用。使用完整 draft 和 base_revision；图片 reset 也先预览。明确哪些旧写入已发生、哪些仍本地待应用；关闭后保存继续时不可失去结果承接 |
| R10 | `CloudArchiveAppPageViewModel.cs:104–118` 同步取列表并清空、int MB 配额；`:121–130` 清全部不收集每项结果；`:145` 上传名 ToLowerInvariant，`:149` ReadAllBytes，`:154` API写成功toast；`CloudArchiveWindow.axaml:115/:120` 右键用 Grid.SelectedItem | 右击不同于既有选中行时可能操作错目标；大文件 UI/内存压力、同名大小写冲突、部分失败无结果、API接受误认云端完成。菜单冻结被右击 file ID；上传 preview 目标/冲突，逐项结果与真实同步状态；配额 u64；后台 I/O |
| R11 | `AchievementAppPageViewModel.cs:123–152` 回调弹框 `.Wait()` 与循环 Thread.Sleep(2000)；`:532–536` 刷新先清列表；`:575–578` StoreStats=false 转 ResetAllStats_Click；StoreAchievements `:415` 写请求前改 IsAchieved | 保存失败落入三段重置确认，草稿消失/空列表/假状态。保存失败只保留 draft 并报告失败或未知结果；重置只来自用户的重置命令。请求状态与实际成就分离，回读确认；刷新不静默丢 draft |
| R12 | `Steam:97–100` 任一 watched app 从 IsDownloading 变 false 即移除并可能完成；`:457` 通知文案用30，`:459–473` 随即调用电源 adapter，所读方法未含等待/取消 | 下载暂停/状态瞬变与完成不清，通知期限不可操作。真实完成判据、可取消倒计时、重新核验全部 watched ID；后续动作按当前身份/下载任务 epoch 锁定；普通测试不得实际电源动作 |
| R13 | `IdleCardPageViewModel.cs:131–136` SID 不同只 warning，阻止启动 return 已注释，`:139` 继续 ReadyToGoIdle；`:273–281` 已存会话直接 Success；CookieLogin `:89–114` 格式通过便 Success | 账号页刚换 B，而 Web A 挂卡仍能进入启动；Cookie 可显示已登录后徽章失败。Web/原生身份必须相同、有效会话确认后才标登录。当前 token refresh 在 `LoadBadges:370–389` 已有 valid/else refresh 分支，本报告不沿用旧索引的“刷新不可达”说法 |
| R14 | `IdleWeb:43–48` 成功 HTTP 就解析 HTML，`:136` 返回 OK；未在这条路径定位完整徽章页身份检查。IC `:426–428` 替换徽章，`LoadBadges:383–388` 重新打开登录后还继续读取 | 200 登录页/限流页/缺结构页不能当有效空徽章；重新登录未完成不能继续发旧会话任务。有效零掉卡、黑名单全排除、私密排除、网络失败、格式变化分别反馈 |
| R15 | `Steam:128` 全模块 RuningSteamApps 只按 AppId；GL Cloud/成就 `GameListPageViewModel:217/:238` 写此字典；IC `StopIdle:695–699` 按 AppId取得记录并 Kill/remove；GL AFK亦读写/kill同一字典 | 同AppId但不同任务用途必须有 owner/role，不允许挂卡停止杀掉 Cloud/成就工作器。worker key 至少包含 SteamId、account epoch、AppId、role；复用必须先证明 ABI/生命周期等价并做引用计数 |

## 4. 交互元素 family 全表

表中每行是一类可重复生成的交互元素，行内列出该家族已定位的入口。全部源码定位状态 `identified`，行为/可达性实测 **未运行**。资源存在但未定位活跃入口的行标明这一限制，不自动恢复入口。静态文本、分隔线、纯装饰图形不算动作；状态显示归入反馈家族。

### 4.1 主壳与共有交互

| ID / family | 元素与源码位置 | 输入、反馈与恢复合同 |
|---|---|---|
| SB-F01 根导航 | 四模块左栏、设置入口；MainView.axaml.cs:26–45 | 同页幂等；侧栏切根清子返回栈，保存模块轻量状态；异步完成不擅自切回发起页 |
| SB-F02 模块内部 tabs | A/GL `UI/Views/Pages/MainFramePage.axaml.cs:26–42/:26–39` | 固定原结构；按稳定 tab ID 保存，不用仅 index；不叠加无界返回栈，切 tab 不销毁领域任务 |
| SB-F03 返回/对话框/菜单 | NavigationService.cs:106–128；四模块 TaskDialog/MenuFlyout/右键 | 回原发起控件；关闭菜单恢复焦点；取消先关最上层。原有确认保留目标/范围；普通读操作不新增强制确认 |
| SB-F04 窗口/托盘/单实例恢复 | MainWindow.axaml.cs:116–124；App.TrayIcon.cs:37/:60–64/:149/:155；App.Interface.cs:14–75 | 最小化/隐藏/退出分别处理；还原重取事件快照后恢复页与焦点；后台事实不挂在 Widget 上 |
| SB-F05 表格/卡片/滚动/焦点 | AccountItems、GameList、CloudArchiveWindow、AchievementWindow、IdleCardPage | 稳定实体 ID；右键明确操作目标；保持搜索、筛选、排序、selection、scroll anchor和激活控件。刷新失败留旧内容并标过时；键盘/输入法与框架隐含键位未实测，不凭习惯增加快捷键 |
| SB-F06 任务反馈 | ProxyStarting/IsLoading/RunLoaingState/SDK事件/ContentLoader | 立即接受或解释拒绝；真实阶段；真实total才百分比。每操作一个终态；toast仅补充，持续问题留就地状态；失败重试保留用户输入 |

### 4.2 网络加速

| ID / family | 已定位的元素与源码位置 | 输入、反馈与恢复合同 |
|---|---|---|
| SB-F07 社区启停 | AcceleratorPage2.axaml:35/:88；ProxyService.cs:117–155 | 单owner串行，按钮反映阶段而非乐观布尔；失败区分恢复结果，关页继续 |
| SB-F08 刷新/规则三态 | AcceleratorPage2.axaml:134/:498/:517；ProxyService.cs:67–76；AcceleratorPageViewModel.cs RefreshCommand | 规则组/叶三态，稳定rule IDs；刷新受正在运行限制时就地解释；不因刷新抹勾选；持久化与运行快照分离 |
| SB-F09 模式/启用脚本/启动时运行/侧栏设置 | AcceleratorPage2.axaml:225–253/:283；ProxyService.cs:235–261 | 模式运行时原禁用保留；设置即时保存语义与运行应用时机明确。解释next-start字段，不加额外全局保存步骤 |
| SB-F10 代理设置各输入 | ProxySettingsPage.axaml:19/:27/:38/:49/:61/:68/:75/:87/:94/:110/:121/:127/:140/:152/:159/:170 | reset、监听IP、DNS、端口、启动时运行、HTTP→HTTPS、DoH与地址、先检测DNS、仅脚本、二级代理开关/类型/IP/端口/用户名/密码；完整/无效草稿分离，依赖enable，敏感值不进入事件/日志，保存与apply结果分别反馈 |
| SB-F11 证书与Hosts工具 | AcceleratorPage2.axaml:292–332 | 安装/删除/显示/目录；编辑/重置/目录/日志。系统修改以同owner journal协调；重置/删除目标明确。普通审计/CI未运行实际平台修改 |
| SB-F12 运行 tabs与状态/连通性测试/日志/流量 | AcceleratorPage2.axaml:443–470；AcceleratorPage2.axaml.cs:36–73；AcceleratorServiceStatus.axaml:45–51；Models/ProxyDomainGroupViewModel.cs:21–53；ProxyServiceStatus/ProxyLog/ProxyChartView | tab动态可见；每服务组ConnectTestCommand、单选TreeView与展开状态恢复；点击codebehind只展开组，不是第二次测速。检测绑定runtime revision/operation并保留逐项error/timeout；服务状态基于实际探活、规则与resources；有界批量日志，页面dispose解除订阅；保留当前可见tab，不抢焦点 |
| SB-F13 NAT/DNS/IPv6检测 | NetworkCheck.axaml:33/:45/:200/:275/:331；NetworkCheckControlViewModel.cs:92–147 | STUN选择、域名/DNS/DoH输入、三类检测；各自busy，不锁其余；query ID+配置revision绑定结果，旧检测不能覆盖新输入；unknown与失败区别 |
| SB-F14 SDK安装/卸载/外部窗口/自动显示 | AcceleratorPage2.axaml:350/:358/:389/:394/:398；GameAcceleratorService.cs:599–700/:780 | capability unavailable时说明具体条件；真实安装状态/错误终态；返回仍在网络页，社区runtime不随SDK失败变状态 |
| SB-F15 SDK游戏搜索/卡片/收藏增删/轮播 | GameAccelerator.axaml:31–39/:79/:90/:105；GameAppItem.axaml:126/:139；GameAcceleratorService.cs:489–554 | search和game ID选择保持；移除收藏不暗示停止加速；列表刷新保持已运行游戏，返回/重开按MyGames恢复 |
| SB-F16 SDK区/服选择与返回 | GameInfoPage.axaml:20/:38/:115/:222；GameInfoPageViewModel.cs:23–49；GameAcceleratorService.cs:424/:702–755 | 区变更清无效server；选服页返回只退一级；取消恢复旧选择；提交完整目标，对已运行目标apply失败恢复展示旧applied区服 |
| SB-F17 SDK加速/停止/启动游戏/测速 | GameDetail.axaml:101/:174/:184/:192；GameAcceleratorService.cs:83–181/:316–331/:438–484/:768 | vendor request accepted与completed分离；start/stop互斥；测速附目标与epoch；启动游戏单独结果，无旧callback串台 |
| SB-F18 脚本导入/刷新全部/设置 | ScriptPage.axaml:31/:50/:79–80；ScriptPageViewModel.cs:16–30/:45–56 | 文件取消回原按钮，无改动；重复脚本目标确认；刷新失败保留旧脚本；自动更新和仅Steam浏览器各自真实效果明确 |
| SB-F19 脚本行启停/下载更新/编辑/刷新/删除/来源 | ScriptPage.axaml:126/:140/:165/:174/:199/:203/:218；ScriptPageViewModel.cs:82–170；ProxyService.cs:620–737 | 稳定script ID，局部忙态/finally、替换原子完成；外部编辑返回后检验文件；缺文件保留真实错误与原删除确认；删除不误删其他脚本 |
| SB-F20 脚本商店资源 | ScriptStorePage.axaml:50/:65/:70；ScriptPageViewModel.cs:39/:68–74 | 下载/更新/来源已定位，当前ScriptPage活跃XAML未见ScriptStoreCommand入口，codebehind旧入口注释。实测前不声称当前可达，不擅自新增导航；如冻结确认可达则回原行并显示下载结果 |

### 4.3 账号切换

| ID / family | 已定位的元素与源码位置 | 输入、反馈与恢复合同 |
|---|---|---|
| SB-F21 平台tabs/新增移除平台/路径 | GameAccountPage.axaml:161–162；GameAccountPageViewModel.cs:23；GameAccountPage.axaml.cs:45–96；PlatformSettingsPageViewModel.cs:15–33 | 平台稳定ID；Steam不可移除；无效平台cancel可退出；回正确原平台/目标账号；平台路径草稿与保存分离 |
| SB-F22 新登录/保存当前/刷新/显示账号名 | GameAccountPage.axaml:37/:56/:96/:130；GameAccountPageViewModel.cs:24–26/:132–163 | 非Steam save-current与Steam新增登录差异保留；async打开对话框前冻结platform ID，后继不能读取用户已切到的SelectedPlatform；刷新保留已知用户与顺序 |
| SB-F23 卡片单切换/双击/右键切换/离线/Persona | AccountItems.axaml:21/:25/:245–252；.axaml.cs:43–59；PlatformAccount.cs:26–29 | 所有入口同事务；MostRecent与真正online身份分离；离线/Persona写入目标固定；保持卡片选中/位置，失败留原页 |
| SB-F24 复制/外链/账号文件夹/快捷方式 | AccountItems.axaml:相关MenuFlyout；.axaml.cs:63–88；PlatformAccount.cs:32–35/:71–78/:129–172 | 文本复制明确成功，外链/路径存在性解释；快捷方式目标稳定ID/账户参数；无效路径不吞错误；返回原焦点 |
| SB-F25 备注/头像/删除账号 | PlatformAccount.cs:37–68；SteamPlatformSwitcher.cs:153–170；AccountItems.axaml:192–193 | 文本与头像有draft；cancel保留原；删除原两次提示区分登录记录和用户数据；进行切换的同账号禁止删除并解释；非Steam策略独立 |

### 4.4 游戏库、编辑、下载、AFK、Cloud、成就

| ID / family | 已定位的元素与源码位置 | 输入、反馈与恢复合同 |
|---|---|---|
| SB-F26 库搜索/类型/已安装/Cloud过滤/侧栏/刷新 | GameListPage.axaml:137/:174/:238/:286/:324/:334；GameListPageViewModel.cs:27–89 | 拼音匹配/原排序语义按源；稳定查询ID；同账号切页恢复。零结果解释当前filters；Cloud filter保存待修正。布局按钮240–251注释，不计当前活跃入口 |
| SB-F27 游戏卡主动作/上下文菜单 | GameListPage.axaml:22–103；GameListPageViewModel.cs:180–245 | 安装/启动、编辑、Cloud、成就、加入AFK、Steam详情/截图/目录、商店/SteamDB/卡片站、隐藏；固定右击target；请求启动与实际游戏启动分离；不自动增加未定位的卸载入口 |
| SB-F28 隐藏列表恢复 | GameListPage.axaml:201；HideAppsPage.axaml:27；GameListPageViewModel.cs:73–77 | 弹框原位置/滚动恢复；移除隐藏后原库查询恢复；取消无副作用；hidden ID保存后重开可验证 |
| SB-F29 游戏详情基本信息/页内tabs | EditAppInfoPage.axaml:31–52/:398/:487；EditAppInfoPageViewModel.cs:38–65 | Name/SortAs/Developer/Publisher等活跃文本与只读事实分开；完整draft，页内tab保存。SteamReleaseDate编辑块源码注释，不凭rg算活跃字段 |
| SB-F30 四种自定义图/图库/文件选择/恢复 | EditAppInfoPage.axaml:113/:144–152/:182–388；.axaml.cs:20–64；EditAppInfoPageViewModel.cs:216–281 | Header/Grid/Hero/Logo分别记录draft、请求ID、旧图；取消选择不删除；图库请求失败清busy并保留旧结果；按图片类型+epoch丢过时回调 |
| SB-F31 启动项/保存文件入口 | EditAppInfoPage.axaml:413–475/:496/:538 | Label/Executable/Arguments/WorkingDir/Platform、新增/上下移/删除；目录只读入口、Cloud入口。排序索引变化用launch ID；草稿校验和target revision；新账号不能接受旧草稿写入 |
| SB-F32 编辑保存/取消/reset/应用Steam/备份 | GameListPageViewModel.cs:190–204；EditAppInfoPageViewModel.cs:120–210；EditAppsPage.axaml:25/:43/:71/:77；EditAppsPageViewModel.cs:44–164 | 完整cancel；reset范围明确；本地保存→列表→真实apply两阶段；import/export文件取消无操作，错误保留原；apply后重启Steam是单独结果/可取消建议 |
| SB-F33 下载监控与结束动作 | DownloadPage.axaml:35–70；Steam:450–473 | 启用监听、Sleep/Hibernate/Shutdown选择；实际授权/阶段/倒计时cancel可见；页离开后台仍监控；真实完成与失联区别 |
| SB-F34 下载行选择/全选三态/进度 | DownloadPage.axaml:82/:105/:123–127；DownloadPageViewModel.cs:24–65 | ID选择集合；true/false/null三态；预定目标范围明确，新增下载不静默加入已有承诺；selection持久化/重开再确认，进度不因缺样本显示完成 |
| SB-F35 AFK全部/行启停/移除/刷新/自动 | IdleAppsPage.axaml:34/:54/:74/:101/:103；IdleAppsPageViewModel.cs:42–107/:120–247 | 上限32与挂卡配置分开；每任务真实worker状态，单/批partial结果；自动恢复须身份确认；移除只停own任务；刷新有timeout/cancel，不无限等全库loading |
| SB-F36 Cloud刷新/上传/批量清空/配额 | CloudArchiveWindow.axaml:30/:45/:60/:81/:101；CloudArchiveAppPageViewModel.cs:104–163 | background I/O，错误与有效空不同；目标AppId+SID清楚；上传同名预览/部分失败；APIaccepted与synced区别；刷新保旧行 |
| SB-F37 Cloud单文件下载/删除/选择 | CloudArchiveWindow.axaml:111–122；CloudArchiveAppPageViewModel.cs:49–82 | 右击实体独立于旧selected；下载picker cancel不写；原删除确认保留；删除/上传未知结果禁止自动重试；完成回正确文件行或最近邻 |
| SB-F38 成就/统计搜索/逐项编辑/全选 | AchievementWindow.axaml:24/:97；AchievementAppPageViewModel.cs:30–114/:203–330 | protection不可编辑且说明；int/float/min/max正确；全选作用范围按源（所有未保护source items），不能悄悄变为只筛选可见项；draft+baseline分离 |
| SB-F39 成就刷新/保存/重置 | AchievementWindow.axaml:32/:47/:62；AchievementAppPageViewModel.cs:530–581 | refresh dirty需明确保留/丢弃选择；save失败不发reset；reset原多次确认含是否包括成就，固定AppId+SID；callback确认成功/失败/未知；不强杀UI承接错误 |

### 4.5 挂卡与登录

| ID / family | 已定位的元素与源码位置 | 输入、反馈与恢复合同 |
|---|---|---|
| SB-F40 Web账号登录/挑战/记住 | IdleSteamLoginPage.axaml:21–82；IdleSteamLoginPageViewModel.cs:21–78 | Username/Password、2FA/Email code、Remember、Login；挑战state machine与operation绑定；只保必要草稿，敏感输入不写持久化页面状态；失败保账户名并聚焦正确输入；取消challenge停止该operation |
| SB-F41 Cookie登录tab/输入/记住 | IdleSteamLoginPage.axaml:97–110/:132/:153；IdleSteamLoginPageViewModel.cs:81–121 | sessionid/steamLoginSecure格式校验＋真实会话确认；SID不符解释；无refresh token明确RequiresLogin；秘密不回显进日志；cancel不会保存 |
| SB-F42 登录/注销/客户端与Web身份反馈 | IdleCardPage.axaml:249–320；IdleCardPageViewModel.cs:47–74/:269–308 | 客户端账号与Web账号并列可辨；logout先停相关会话/取消请求，不仅清cookie；旧callback不能复活登录。保存remember是秘密引用，当前真实状态由backend确认 |
| SB-F43 挂卡总启停/运行统计 | IdleCardPage.axaml:91/:149/:433–516；IdleCardPageViewModel.cs:94–169 | 前置Steam运行、原生登录、Web有效、身份一致、队列有效；busy/finally/partial失败；停止后worker真实退出才完成；统计单调时钟、未知/断连显示真实含义 |
| SB-F44 设置侧栏/算法/排序/数值 | IdleCardPage.axaml:163–228；IdleCardPageViewModel.cs:34–35/:325–364 | 4规则×4排序，MaxIdleCount2–32、SwitchTime≥5000ms、MinRunTime≥2h、RefreshBadgesTime≥1min按当前UI；数字单位明确；变更运行任务串行重排、失败说明applied旧值；默认并发仍需冻结schema |
| SB-F45 游戏优先/黑名单/链接/排除说明 | IdleCardPage.axaml:26–68/:562/:736；IdleCardPageViewModel.cs:175–224 | 右键/More同target；优先会暂停自动next须可理解；黑名单持久化但与私密状态分离；全排除不显示全部掉卡完成；Steam/商店/SteamDB/卡片站链接回原页 |
| SB-F46 家长PIN/徽章读取/token刷新/自动调度 | IdleCardPageViewModel.cs:366–428/:790–918；IdleWeb:32–136 | PIN取消回挂卡；token过期可刷新/需登录分清；HTML结构、身份及完整分页验证；价格失败降级排序；自动任务owner与account epoch固定，sleep/wake重新探活 |

`IdleManualRunNext` 方法/命令位于 `IdleCardPageViewModel.cs:46/:175`，本轮当前活跃 `IdleCardPage.axaml` 未定位直接按钮绑定；仅作为后台/未来可达性待核对，不擅加按钮。旧 `RunStopBtnCommand` / `IdleAppsPageViewModel` 错类型按钮在 `IdleCardPage.axaml:607–619/:679–725` 均是注释，不能认定为当前死按钮缺陷。脚本商店、库布局选择、家庭共享入口也必须区分资源/命令存在与活跃入口。

## 5. 互斥、身份 epoch、草稿与返回的统一规范

### 5.1 互斥范围

| 资源 | 必须串行/互斥的操作 | 可继续操作 | 取消与失败承接 |
|---|---|---|---|
| 社区runtime/系统接入owner | start/stop、模式apply、Hosts reset、删除运行中必需证书、恢复journal | 读日志、滚动、切页、无写入网络检测、账号/库只读 | 重复同意图合并；冲突拒绝说明。不可取消外部写入进入恢复/未知结果，不能发虚假cancelled |
| 某个平台客户端切换 | 同平台switch/new-login/清登录，以及依赖该身份的写事务 | 社区runtime、其他平台、只读缓存、修改独立本地draft | 等已提交写入收束；未提交可取消；结果不明先核实/补偿。不是全应用busy遮罩 |
| Steam账户epoch | switch与Cloud写/成就store、账户目录图片写、账号关联AFK/挂卡native session | 旧快照查看、搜索/滚动、将草稿保存在旧账户上下文 | 切换开始即冻结写按钮；明确当前阻塞operation，完成/取消再切；新身份核实后启新epoch |
| 原生worker role/context | 同SID/AppId/role的生命周期操作；如果API上下文不支持并发则额外worker锁 | 经证明独立的其他AppId/role | 使用own worker handle/session记录；不得按进程名kill；共享worker引用计数，停止不伤另一owner |
| SDK vendor session | start/stop/change-area/install/uninstall及对单一current target写入 | 社区代理、库查询、SDK列表浏览 | request发送与terminal callback分开；超时state=结果不明，查询SDK状态后决定，不盲目repeat |
| 同一script/file/achievement batch | update/delete/edit/import冲突；同cloud目标重名write/delete；stats store/reset | 其他行/普通读取 | 目标revision、idempotency与partial结果；保存失败保草稿，unknown destructive结果先查证 |

### 5.2 不同 epoch 不混为一个 bool

至少区分 `app_engine_epoch`、`steam_account_epoch`、`web_session_epoch`、`community_runtime_epoch`、`sdk_vendor_epoch`。操作context固定 `{operation_id, stable_entity_id, account_id?, app_id?, epoch, base_revision}`，事件携带sequence；旧epoch丢弃，sequence gap重取snapshot。搜索/图片/网络检测另有query ID。

账号切换开始进入身份转换态：停止接收依赖旧身份的新写命令；终结已提交动作、取消可取消读取；清除旧原生连接/own worker；完成客户端切换并验证真实SID；递增并发布新账户epoch。若新Steam未登录或停客户端失败，停在可解释状态，原页面可继续只读/重试，不发布“登录成功”。Web会话与原生SID相等是挂卡启动硬前置。

Cloud/成就窗口、图库下载、徽章parser、SDK测速callback都必须捕获context，不能处理结果时读全局“当前用户/当前游戏”。Flutter只投递意图，Rust/domain/worker拥有实际事实。Widget dispose仅取消该页订阅，不停用户已经启动的业务。

### 5.3 草稿规则

1. 模态编辑使用完整副本，包含文本、启动项顺序、图片/删除意图；可取消层内不直接写Steam。base_revision变化时做冲突处理，不静默覆盖。
2. 两阶段编辑保持：保存本地draft形成pending edit；用户在编辑列表“保存到Steam”才进入外部apply。图片若原流程必须即时应用，必须明确告知结果和取消范围；优先改为同一draft原子提交，作为有源依据的纠正项记录。
3. 直接开关/原即时设置不增加多余全局Save。文本组合输入过程可保本地无效draft，blur/Enter等提交点按冻结行为；提交校验失败在字段旁说明。runtime取启动快照的字段展示“已保存，下一次启动应用”。
4. 错误保留用户投入；成功保持当前页与焦点。不要因为网络返回、列表刷新、后台任务完成覆盖正在输入的数据。
5. 账号关联draft按旧SID/AppId隔离。换账号后可保留/导出旧draft，不能自动套用到新SID。秘密、password/code/PIN/Cookie不进页面恢复缓存或证据。

### 5.4 选择与返回恢复

模块页面状态键建议为 `{module_id, tab_id, account_scope?}`；内容包含search、filters、sort、stable selection IDs、scroll anchor ID＋offset、pane state、focus ID。根切换清导航stack仍可恢复这些低成本状态。同账户返回恢复完整选择；账户改变时保留通用display设置，账户实体选择隔离并重新验证存在性。

右键菜单捕获被右击实体，不以旧selected row当目标；键盘上下文菜单用当前focus实体。删除后选择原位置最近邻，刷新保存在的stable IDs。filtered-empty和source-empty区别，仍给清筛选入口。全选必须明确原语义：下载全选针对当前DownloadingApps；成就源码针对所有source中未protection项，不能迁移时悄悄改变。

模态栈返回到发起控件；图片选择/区服选择先返回其父编辑层；文件picker取消恢复原focus；异步操作完成如果用户已走开，只更新领域状态与可见任务，不强制跳回。SDK callback、网络启停、登录完成后只在原逻辑确需且当前上下文仍匹配时转页；不可见tab回退到最近可见稳定tab。

## 6. 下一步可执行的全流程验收轨迹

下表是后续 S31 的场景合同草案，**未运行**。全部使用合成资料/已授权隔离测试账号与平台adapter；实际系统接入、电源、Steam写入需要隔离环境。测试监听端口一律≥11808且预探测，不触及127.0.0.1:10808，不改宿主系统代理、不终止非项目管理进程。

| 轨迹 | 操作串联 | 关键期望与证据 |
|---|---|---|
| W01 首次启动与网络 | 普通启动→四模块导航→规则三态→无效端口→修正→start→网络检测→日志→stop | 每步即时反馈；无效字段不落runtime；保存/desired/applied分明；阶段错误在原页；root导航及tab恢复截图/事件 |
| W02 网络失败恢复 | start在listener/trust/ingress各点失败→去账号页→回网络→恢复→restart | 失败补偿准确；不假Stopped；journal结果；在失败处可重试；宿主无修改（adapter/VM证据） |
| W03 运行中脚本 | 社区运行→脚本import/duplicate/取消→更新失败→重试→enable→去库→回脚本→stop/start | script stable IDs/旧版本保留；局部busy释放；持久化与apply时机可核实；返回选择及滚动 |
| W04 SDK完整与失败 | 未安装→选路径取消→安装失败→恢复→选游戏/区/服返回→开始→换区失败→旧测速late callback→stop→external窗口返回 | capability与安装terminal；旧区服保持；late callback不串target；社区runtime正常；取消无假成功 |
| W05 账号A→B→库 | 账号列表刷新→双击B同时按钮→client退出失败→重试→B未登录→登录→库查询 | 每次输入最多一事务；真实SID确认；不同目标冲突明确；库在线身份/本地缓存来源与账户epoch证据 |
| W06 平台路径取消 | 进入未配置平台→file picker取消→invalid path→对话框Cancel→回Steam→再进入平台 | 无取消陷阱，无路径副作用；原平台/目标focus恢复 |
| W07 编辑整条链 | 选游戏→文本/launch顺序/四图→图库超时→返回→Cancel→重开→本地保存→编辑列表→apply失败→重试→是否重启Steam | 完整draft回滚；两阶段反馈；失败保pending；部分图片结果；关闭/重开真实persist/apply证据 |
| W08 库/选择恢复 | search/pinyin→type/install/cloud→滚到中段→Cloud窗口→返回→另tab→另模块→回库→重开 | search/filter/selected AppId/scroll/focus稳定；有效零结果与错误区别；不自动解除用户filter |
| W09 Cloud身份与未知 | A/AppX打开→右击非selected file→save picker取消→重名upload→部分失败→delete超时→切B→旧结果到达 | target正确；cancel无写；partial清单；unknown无重试；旧epoch无污染；APIaccepted/sync result各自证据 |
| W10 成就编辑失败 | search→全选→protected/int/float edit→refresh→保draft→save fails→retry→reset→逐级取消 | 保存失败不进入reset；draft/actual独立；全选源范围明确；合法测试应用callback+回读，不能只mock验收 |
| W11 下载闭环 | 选部分/全选三态→go elsewhere→pause/断网/manifest半写→恢复下载→真实全部完成→倒计时Cancel→再开始→切账号 | 选择不随列表消失而完成；电源adapter调用记录/隔离验收；cancel可见有效；身份改变撤过时倒计时 |
| W12 挂卡登录路径 | 原生A→WebB→attempt start→WebA 2FA错码/邮箱→重试→PIN取消→Cookie过期→有效登入→徽章200登录页 | 不同SID拒start；challenge可恢复；非法HTML不是零队列；token刷新/需登录分明；日志无秘密 |
| W13 挂卡/AFK并存 | 合法SID→4×4模式→优先→黑名单→AFK同AppId→Cloud同AppId→stop挂卡→切账号 | 单调计时与队列状态；role/owner互不杀；换身份旧session终结；排除原因明确；小fixture与真实测试分开 |
| W14 后台与重开 | 社区/挂卡运行→切根页→最小化→X到托盘→tray restore→第二实例→真正退出→重开→sleep/wake | 主壳四格；任务持续/真实清理；重取snapshot；保存页/scroll；epoch变化无假状态；resources探活及journal |

完成每个family需得到入口→Rust command→持久化→重开→真实配置/平台效果→事件/视觉反馈的证据。暂时无外部条件可做合成故障注入，但状态只能保留 `identified/implemented/blocked` 等真实进度，不能写 `verified`。

## 7. 本轮实际运行与未完成项

实际运行的只读命令：`git rev-parse HEAD`（exit 0，上述hash）；`rg --files`（定位四模块和共享源码）；`rg -n`（定位命令、绑定、状态与调用）；PowerShell `Get-Content -LiteralPath`（读取源和任务/台账）；PowerShell只在内存中剔除AXAML注释、保持行号后枚举活跃命令/选项（exit 0）。一次把 `SteamConnectService*.cs` 放进Windows路径参数的rg返回os error123，已改用 `rg --files` 找到 `Services/Mvvm/Steam/SteamConnectService.cs` 后读取；该失败不涉及运行产品。

本轮唯一新增文件是本报告。源码、work、outputs、现有台账、用户运行中的进程与网络设置未修改。产品单元/集成/Flutter测试、原GUI对比、真实平台与性能检查：**未运行**；报告不宣称UI顺畅或全部逻辑已修复。

未完成：SteamBox冻结工作树/构建产物一致性，完整专属actions/layouts/fields编号，原框架隐含键盘/焦点行为，script-store/ManualNext等可达性，四模块真实账号/合法测试App的native与Web验证，SDK权益/安装/回调平台实测，真实资源恢复、每个family的视觉及长流程证据。

下一步前置：确认本轮用户目标是SteamBox四模块后，由共享契约owner把本报告的R01–R15、SB-F01–F46、W01–W14落入S04/S05/S08及S12–S24纵向任务；先实现身份epoch/operation/草稿/worker owner，再逐流程联调，不能只给按钮加loading。若目标是当前v2rayN实现，此文保留为SteamBox研究材料，不将两套源码的逻辑混入同一应用。
