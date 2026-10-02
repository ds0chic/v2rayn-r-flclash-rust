# SteamBox 前端逐 action 固定策略

状态：本文所有动作合同均为 `identified`。这是 Flutter + Rust **规定要达到的目标**，不是原代码已实现这些逻辑的声明。原 GUI、Rust 实现、网络/SDK/Steam/Cloud/成就/系统效果与性能测试均未运行、未验证。文件名无日期；本轮静态读取日期为 2026-10-03（Asia/Shanghai）。

仓库 HEAD：`d7fbe8010f284609385466286360f7461bb1b574`。源根为 `C:/Users/Colby/.gemini/antigravity/scratch/SteamTools`，研究记录的 HEAD 为 `a7a5ce7f0830b9b759090a75b59d82679084b478`；读取当前工作树，HEAD 不替代 dirty 文件及子模块冻结。只新建本文；原源码、配置、日志、秘密、`work/`、`outputs/`、v2rayN 台账均未改动。

本文配合 [整体流程](STEAMBOX_FRONTEND_FLOW_SPEC.md)、[公共交互合同](FRONTEND_INTERACTION_CONTRACT_2026-10-02.md)、[字段策略](STEAMBOX_FRONTEND_FIELD_POLICIES.md)、[元素 CSV](steambox-frontend-elements-2026-10-02.csv) 和 [46 家族研究](../evidence/steambox-workflow-research-2026-10-02.md) 使用。后续实现不能只抄命令名，必须落实本文的提交目标、完成事实和失败结果。

## 1. 精确关联与共同边界

`SB-A-xxx` 是稳定动作合同 ID；不按表格排序重编号。源前缀全部相对上述 SteamTools 根：`Shell=src/BD.WTTS.Client.Avalonia`，`Shared=src/BD.WTTS.Client`，`A=src/BD.WTTS.Client.Plugins.Accelerator`，`GA=src/BD.WTTS.Client.Plugins.GameAccount`，`GL=src/BD.WTTS.Client.Plugins.GameList`，`IC=src/BD.WTTS.Client.Plugins.SteamIdleCard`。下面 `Views/…` 统一展开为 `<前缀>/UI/Views/…`；`VM/…` 展开为 `<前缀>/UI/ViewModels/…`；其他路径原样展开。冒号是源码一基行号。

CSV 的关联键为 `(source, command/handler 符号, 所属容器/参数)`；不能只用短命令名。例如网络页和账号页的 `RefreshCommand` 是不同动作。多个符号在同一行意味着同一明确目标或同一种事务，不意味着每个事件另发一次操作。通用 `Command` 根据 About 的 `Hyperlinks/OSL` 元素实体再分发；`ButtonCommand` 根据 `GameAppItem` 的父容器分发。未知容器/route 不执行默认命令，阻止该流程放行。表中 SB-J 列的 `01` 等价于流程规格的 `SB-J01`，其余同理。

所有业务写入冻结 `request_id + entity ID + scope + expected_revision + draft_revision`；Steam 相关再冻结 `SID + account_epoch + AppId`，Web 登录冻结 Web session epoch。持久去重回执丢失先按 request 查询，不能新建 request 盲重试。查询 latest-wins；写 single-flight/按冲突域排他；返回、导航、滚动不随业务写锁死。旧 epoch 的 journal 必须收敛到真实终态或待核对，不能只丢 UI 回调。状态、焦点与错误显示同时执行公共合同 P00–P25。

表中“提交目标”区分 UI 草稿、Rust 持久化、平台交付、外部副作用。纯 UI 事件不发 Rust 业务 operation；读取/校验可走 Rust query。OS 交付成功只能称“已交付打开请求”；本地 saved、Steam API accepted、外部 applied/身份确认是不同事实。取消仅在后台确认停止或尚未提交后终结为取消；结果未知时先核对。

## 2. 固定默认阶段 deadline

这是待实现的默认合同，不是性能实测。所有长动作 100ms 内有提交反馈，Rust 回执后才称“已接受”；>300ms 显示进行态，>2s 显示阶段、等待对象及合法取消/返回入口。每次请求在提交前固化阶段 deadline、提交点、补偿与核对来源，不能不断延长 spinner 或把所有动作统一成 10s。组合类别逐阶段执行，不用最大类别覆盖全部过程。

| 类别 | 固定默认 deadline | 超时与核对规则 |
|---|---|---|
| `UI` | 下一可用帧反馈，普通反馈≤100ms；视图变更 watchdog 1s | 结束本次 UI 进行态，保留原 view/draft；不造领域失败/成功。cached 切页仍按公共合同 p95≤150ms 验收 |
| `LOCAL` 本地提交 | 校验 3s + 持久提交 3s | 未提交可取消；已提交查本地 revision/journal。原子事务失败保留草稿；晚回执只清同 draft revision 的 dirty 字段 |
| `FILE` 文件读写 | 路径/权限预检 10s；读写阶段总 120s、连续无进度 30s | 临时文件+原子替换；大文件批次在提交前显式选择 `FILE-LARGE` 总 600s，仍无进度 30s。取消清自有临时文件；最终替换结果未知查 hash/revision/备份 |
| `NET` 网络查询 | connect 10s；单响应/分页 30s；一次查询总 120s | 保留最后可靠快照；过期响应不改新 view。流式资源下载 `NET-TRANSFER` 总 300s/无进度 30s；有界批次 `NET-BATCH` 总 600s，单项仍遵守上述限值 |
| `WORKER` 进程/Steamworks worker | 启动握手及 ready 15s；单次读/写确认 30s；停止确认 10s | 查拥有句柄、role、SID、AppId、epoch 与实际 API 状态；不得按进程名杀。长期 AFK/挂卡/反代本身无“30s 必停”，用心跳 15s、失联 30s 后显示 degraded/待核对 |
| `SDK` 厂商加速 | 调用接受 10s；终态回调 90s；状态核对 15s | 回调按 SDK epoch/operation 丢过期数据；超时查厂商真实状态再决定重试。安装 `SDK-INSTALL` 总 600s/无进度 30s，下载仍有独立网络阶段 |
| `WAIT` 登录/挑战/确认/picker 用户等待 | 单次等待 600s；提供方更短 TTL 优先 | 到期本次等待进入 expired，保留非秘密输入/草稿，用户可返回或重新开启等待；不自动重发验证码、写操作或确认。旧回调不提交新 revision。执行请求另用 NET/LOCAL/FILE/WORKER；不能把等待登录当网络请求卡死 |
| `OS` 剪贴板/外部窗口/浏览器交付 | 交付 5s | OS 接受即结束交付态；不保证网页加载或程序运行。失败明确目标与重试入口，不反馈业务已完成 |
| `HELPER` 权限/系统效果 | helper 建连 10s；用户授权 WAIT；已授权单步执行 30s | 先查系统实际效果及 owned 标记；失败报告补偿结果。宿主系统代理、10808 与非自有进程不可作为普通测试目标 |

用户等待超时不会自动“撤销”已经发生的外部副作用。SDK/Steamworks/Cloud 无可靠取消 API 的已提交阶段可以离开页面，显示 awaiting_result/unknown 并核对；禁用重复写，不能虚构“已取消”。数字是固定实施默认值，后续调整须用阶段测量证据更新合同，不能由每个按钮随意选择。

## 3. 公共壳、设置、About（F01–F06）

| ID | 源 scope / 确切符号 | action / 领域效果 | 提交目标 | 权威成功 | 取消、失败与恢复 | SB-J / 超时 |
|---|---|---|---|---|---|---|
| SB-A-001 | Shell Views/Pages/HomePage.axaml:87 `NavgationToMenuPageCommand`；Views/Pages/MainView.axaml.cs:26 `NavView_ItemInvoked` | 四根导航及原 Home fallback；解析已注册 PageType，不重复导航当前页 | UI viewKey/滚动锚点；可独立持久 view state | 目标 root 挂载、可交互，后台任务 owner 不变 | 未注册/已移除插件显示可返回错误；不初始化隐藏模块 | 01,02,06,14,17 / UI |
| SB-A-002 | Shell Views/Pages/MainView.axaml.cs:63,129 `OnFrameViewNavigated`, `OnNavigationViewBackRequested` | 前者同步高亮/返回按钮投影；后者回有效 backstack | UI only | 同 scope 恢复原筛选、选择、锚点与焦点 | 弹窗优先消费返回；dirty 按该草稿策略处理；不隐式取消持续任务 | 01,04,08,14 / UI,WAIT |
| SB-A-003 | A、GL Views/Pages/MainFramePage.axaml.cs:19/:16 `Tabs_SelectionChanged`；A Views/Pages/AcceleratorPage2.axaml.cs:45,61 `AcceleratorTabs_SelectionChanged` | 页内 tab 选择；网络运行状态只让当前已不可见 tab 回退到首个可见 tab | UI + 可选 view state LOCAL；不发启停 operation | 正确 tab 内容与选择一致 | 任务完成不抢用户仍可见 tab；程序赋值不触发循环保存/导航 | 01,02,05,09,17 / UI,LOCAL |
| SB-A-004 | Shell Views/Pages/Settings/SettingsPage.axaml.cs:14,23 `SettingsScrollTab_Tapped`, `SettingsScrollViewer_ScrollChanged`；A Views/Controls/ProxyLog.axaml.cs:54 `LogTextbox_TextChanged` | 设置锚点滚动/高亮；日志底部跟随只在用户原本位于底部时 | UI only | 滚动/焦点稳定，无业务命令 | 新日志不拉走历史阅读位置；滚动事件不写领域设置，不阻塞输入 | 02,16,17 / UI |
| SB-A-005 | Shell Views/Pages/About/AboutPage.axaml:27 `EasterEgg_PointerReleased` | About 彩蛋局部 UI | UI only | 本次动画/内容可见；事件已消费 | 不启动插件/网络/登录；离页清 UI 动画资源 | 01,17 / UI |
| SB-A-006 | Shell Views/Pages/About/AboutPage.axaml:43,54,88,112 `GPLv3_Tapped`, `Dotnet_Tapped`, `Avalonia_Tapped` | 分别打开 GNU GPL、.NET、Avalonia 官方链接 | OS 浏览器交付；固定 allowlisted HTTPS | OS 接受确切 URI | 失败仅打开失败；一次手势一次交付；不得称授权/安装完成 | 01 / OS |
| SB-A-007 | Shell Views/Pages/About/AboutPage.axaml:171,198 `Command`；Shared Models/HyperlinkModel.cs；Shared VM/Pages/AboutPageViewModel.cs:114–173,179–185 | 容器 `Hyperlinks` 的 OpenOfficialWebsite/RatingsAndReviews/Changelog/ContactUs/JoinUs/FAQ/Agreement/Privacy/DelAccount/BugReport/Crowdin/Source，以及 `OSL` 的资源库链接；DelAccount 实际是账户安全网页 | 经 typed HyperlinkModel 的稳定 item kind+URL 分发 OS；不是任意 ICommand，DelAccount 不删 Steam 账号 | 对应 URL 的 OS 交付成功 | 平台商店链接按实际平台；OSL 缺资源就显示不可用；不要从 label 猜删除/更新动作 | 01 / OS；OSL 资源展开待确认 |
| SB-A-008 | Shell Views/Pages/About/AboutPage.axaml:171 `Command`，`Hyperlinks` item `CheckUpdate`；Shared VM/Pages/AboutPageViewModel.cs:119–122 | 精简版本地版本说明；源只显示“当前已是最新精简版本 (SteamBox)” | UI notice；不发虚构更新 operation | 明确显示本地版本说明及来源，不能声称已联网检查 | 真远程检查需产品 route/更新源另决策；当前不能伪造“检查成功” | 01 / UI |
| SB-A-009 | Shell Views/Pages/About/AboutPage.axaml:171 `Command`，`Hyperlinks` item `CopyUserId`；Shared VM/Pages/AboutPageViewModel.cs:148 | 复制存在的产品用户 ID；不是 SteamID | OS 剪贴板；只读公开 ID，不存凭据 | 剪贴板交付确切 ID | 没有用户则说明不可用；产品账号路由未确认，不自动恢复 LoginOrRegister 页面 | 01 / OS |
| SB-A-010 | Shell Views/Pages/Settings/Settings_General.axaml:67,73,80,95 `OpenFolder_Click` | null→应用目录；AppData/Cache/Logs→相应自有目录 | OS 目录交付；Logs 不读取/上传日志 | 目标规范路径可交付 | 目录无权限/不存在就地失败；不创建任意用户路径、不伪报清理完成 | 16 / OS |
| SB-A-011 | Shell Views/Pages/Settings/Settings_Plugin.axaml:53 `SwitchEnablePlugin_Click` | 仅保留四插件的启用设置；明确需要重启 | Rust plugin-setting revision，restart_required | 保存 revision 确认；运行态保持真实已加载状态 | 保存失败留草稿/可靠值；不加载 Authenticator/ASF/GameTools，通用插件模板不是恢复许可 | 16 / LOCAL |
| SB-A-012 | Shell Views/Pages/Settings/Settings_Plugin.axaml:59,65,71 `OpenPluginDirectory_Click`, `OpenPluginCacheDirectory_Click` | 固定插件 AssemblyLocation/AppDataDirectory/CacheDirectory 打开 | OS；规范化并验证是选中保留插件拥有路径 | OS 交付具体目录 | 不信任任意绑定路径；找不到插件则说明，不换成其他插件目录 | 16 / OS |
| SB-A-013 | Shell Views/Pages/Settings/Settings_Steam.axaml:26 `SelectSteamProgramLocation` | picker 选 Steam 可执行文件并校验 | picker UI→Rust path validate→Steam path LOCAL；next_run | 合法路径持久化，不等于 Steam 已启动 | picker 取消零提交；错路径就地显示可重选；冲突账号操作不立即重启客户端 | 07,16 / WAIT,FILE,LOCAL |
| SB-A-014 | Shell Views/Pages/Settings/Settings_Steam.axaml:92 `EditSteamParameter`；Shared VM/Pages/SettingsPageViewModel.cs:100–110 生成 OK/Cancel | 独立启动参数草稿；确认保存供下次客户端启动 | Rust setting draft revision，next_run | LOCAL 确认最新冻结草稿 | Cancel 不改原值；保存失败留输入；不得用 shell 拼接执行参数 | 07,16 / WAIT,LOCAL |
| SB-A-015 | Shell Views/Pages/Settings/Settings_UI.axaml:408 `SelectImage_Click` | picker→格式/可读校验→背景预览与存储 | UI preview + Rust setting revision；持久资源引用 | 图片合法且保存回执；preview 与 saved 分开 | 取消不改原值；读图/保存失败留旧背景和错误；重开读取已保存资源 | 16 / WAIT,FILE,LOCAL |

## 4. 社区网络、检测、脚本、SDK（F07–F18）

| ID | 源 scope / 确切符号 | action / 领域效果 | 提交目标 | 权威成功 | 取消、失败与恢复 | SB-J / 超时 |
|---|---|---|---|---|---|---|
| SB-A-016 | A Views/Pages/AcceleratorPage2.axaml:31,86 `StartProxyCommand`；Services/Mvvm/ProxyService.cs:376,394,408 `UpdateProxyTrayMenuItems` 生成 start/stop Command | 按点击时可靠快照解析显式 start/stop desired，按钮与托盘同一网络 owner 事务 | Rust desired revision→listener/接入 transaction；持久 request | start=listener ready+系统接入实际确认；stop=接入撤销+自有 listener 退出 | 超时查 owner/接入；权限取消报告已完成阶段和补偿；不把 bool/toast 当成功；测试不用宿主代理/10808 | 02,03,17 / LOCAL,WORKER,HELPER |
| SB-A-017 | A Views/Pages/AcceleratorPage2.axaml:132 `RefreshCommand` | 查询规则/状态来源，保留有效服务选择与最后快照 | Rust read query；规则 snapshot 版本化 | 完整查询来源/时间与规则 revision | 离线不清空列表；不反转运行开关，不把空/残缺响应当成功 | 02,03 / NET |
| SB-A-018 | A Views/Pages/AcceleratorPage2.axaml:281 `ProxySettingsCommand` | 打开当前设置；字段按 FIELD_POLICIES 提交 | UI 页面/弹窗+字段策略；desired/applied 分离 | 正确 snapshot 展示；字段保存各自回执 | 关闭回启动按钮焦点；运行中模式选择禁用并说明先停止，不加热切换 | 02,03 / UI,LOCAL |
| SB-A-019 | A Views/Pages/ProxySettingsPage.axaml:19 `ResetSettings` | 明确代理设置默认项集合，需确认影响 | Rust setting revision；运行中待应用/需停止项逐项标出 | 保存默认集合 revision；不等于 runtime 已更新 | 确认取消零提交；失败保留输入和旧 snapshot；不重置无关壳/账号设置 | 03 / WAIT,LOCAL |
| SB-A-020 | A Views/Pages/AcceleratorPage2.axaml:290 `SetupCertificateCommand` | 安装自有证书信任 | Rust typed helper、owned fingerprint；分公有证书文件与信任 store | 查目标 store 实际 fingerprint/权限结果 | 权限取消/失败记录未安装和临时资源清理；不打印私钥；不伪称反代启动 | 02,03 / FILE,HELPER |
| SB-A-021 | A Views/Pages/AcceleratorPage2.axaml:294 `DeleteCertificateCommand` | 删除自有证书信任，先处理依赖会话 | Rust helper→仅 owned fingerprint | 相关 store 不再有 owned 信任，依赖状态一致 | 确认/权限取消保留；已提交未知先查 store，不删其他软件/用户证书 | 02,03 / WAIT,WORKER,HELPER |
| SB-A-022 | A Views/Pages/AcceleratorPage2.axaml:298,302 `ShowCertificateCommand`, `OpenCertificateDirCommand` | 查看公有证书/打开自有证书目录 | OS 公有证书/目录交付；必要 FILE read | OS 接受对应目标；只说明查看/打开 | 查看不安装；不显示/发送私钥或把目录打开当导出完成 | 02 / FILE,OS |
| SB-A-023 | A Views/Pages/AcceleratorPage2.axaml:318 `EditHostsFileCommand` | 交付 hosts 查看/编辑；若应用提交，只允许 owned 段 typed helper | OS editor；另有显式 helper commit，不能把打开当保存 | 编辑器交付；真正提交需 readback/hash+冲突检查 | 外部编辑结果未知先重读；无授权不改系统；非 owned 内容保持 | 02,03 / OS,HELPER |
| SB-A-024 | A Views/Pages/AcceleratorPage2.axaml:322 `ResetHostsFileCommand` | 只重置本应用 owned hosts 段 | Rust helper expected hash→原子修改+备份 | 读回 owned 段正确，非 owned 内容未变 | 确认取消零改动；并发 hash 冲突要求刷新后再确认，不整文件清空 | 02,03 / WAIT,HELPER |
| SB-A-025 | A Views/Pages/AcceleratorPage2.axaml:326,330 `OpenHostsDirCommand`, `OpenLogFileCommand` | 分别打开 hosts 目录、应用脱敏日志文件 | OS 交付；不读取/发送用户日志作本轮证据 | 确切路径交付成功 | 无文件/权限明确失败；日志打开不改网络状态 | 02 / OS |
| SB-A-026 | A Views/Controls/NetworkCheck.axaml:41,198,329 `NATCheckCommand`, `DNSCheckCommand`, `IPv6CheckCommand`；Views/Controls/AcceleratorServiceStatus.axaml:43 `ConnectTestCommand` | 固定测试目标/DNS输入/当前 runtime epoch；同种测试 single-flight | Rust read query；结果含目标、来源与时间 | 本次响应完整且 context 未失效；失败如实显示该测试结果 | 输入变更取消旧查询或拒绝旧回写；离页可取消，不自动停代理/改系统 DNS | 02,03,05 / NET |
| SB-A-027 | A Views/Controls/AcceleratorServiceStatus.axaml:43；.axaml.cs:17 `ConnectTestButtonOnClick` | 只展开祖先 SettingsExpander；与同按钮命令合成一次手势 | UI only；测速由 SB-A-026 唯一提交 | 面板展开 | 不发第二测速 request；重复展开幂等 | 02,05 / UI |
| SB-A-028 | A Views/Pages/ScriptPage.axaml:29 `AddNewScriptCommand`；VM/ScriptPageViewModel.cs:16–30 | picker 导入 JS/TXT，解析安全 staged 副本 | Rust script ID+文件+编译元数据原子提交 | 新 script snapshot/hash/revision，是否启用及 runtime 待应用分别显示 | picker 取消无提交；解析/编译/落盘失败不出现残缺已启用行；保留其他脚本 | 04 / WAIT,FILE,LOCAL |
| SB-A-029 | A Views/Pages/ScriptPage.axaml:164 `EditScriptItemCommand`；VM/ScriptPageViewModel.cs:120–143 `EditScriptItem` 生成外部 editor+OKCancel 确认 | 外部文本编辑保留原流程，改为 script ID 的 staged 文件；确认后校验/编译/替换 | UI/editor staged draft→Rust script revision；OK 确认是提交点 | script 新 revision 已保存；runtime 生效另确认 | Cancel 丢 staged 副本，不改已安装文件；源文件消失先显示缺失，可单独确认删除，不能取消编辑却删项 | 04 / OS,WAIT,FILE,LOCAL |
| SB-A-030 | A Views/Pages/ScriptPage.axaml:217 `DeleteScriptItemCommand`；VM/ScriptPageViewModel.cs:82,101 `DeleteScriptItemButton`, `DeleteNoFileScriptItemButton` 生成确认 | 删确切 script ID；缺文件仍显式确认条目删除 | Rust script revision + owned 文件；规则 snapshot 变更 | 本地记录/文件结果确认，runtime 引用收敛 | Cancel 零删除；失败保留条目和错误；不从旧 index 删除邻项 | 04 / WAIT,FILE,LOCAL |
| SB-A-031 | A Views/Pages/ScriptPage.axaml:139,202 `DownloadScriptItemCommand`；Views/Pages/ScriptStorePage.axaml:48,64 `DownloadScriptItemCommand` | 按 source/script ID 下载并校验安装；store 视图 route 未确认只能合同待用 | Rust 单项 operation→临时文件→校验→script revision | 完整合法文件原子提交，旧有效版本可恢复 | 同 ID 去重；进度真实；失败不替换旧版；store 未证明 route 不新增按钮 | 04 / NET-TRANSFER,FILE,LOCAL |
| SB-A-032 | A Views/Pages/ScriptPage.axaml:173 `RefreshScriptItemCommand`；VM/ScriptPageViewModel.cs:150 | 从当前 owned 文件重新解析/编译，不假设必为联网下载 | Rust 固定 script ID/hash/revision | 新有效本地快照；缺文件/编译错保留最后版本并说明 | 不因错误自动 disable/删记录；必要删除走独立确认 SB-A-030 | 04 / FILE,LOCAL |
| SB-A-033 | A Views/Pages/ScriptPage.axaml:48 `RefreshALLScriptCommand` | 全项按稳定 script IDs 的有界刷新；逐项区分本地重载/需下载 | Rust batch，单项 operation；summary 含成功/失败/跳过 | 全批次终态及逐项 revision，不用一个 toast 掩盖部分失败 | 取消停止未提交项；已提交项保留结果；不使用源 2s 任意节流代替去重 | 04 / NET-BATCH,FILE,LOCAL |
| SB-A-034 | A Views/Pages/ScriptPage.axaml:198、ScriptStorePage.axaml:69 `OpenBrowserCommand`，param=`SourceLink` | 打开该 script 的规范来源 URL | OS；entity ID→来源 URL，校验协议 | OS 接受 URL | 失败不改变安装/更新状态；store route 同 SB-A-031 | 04 / OS |
| SB-A-035 | A Views/Pages/AcceleratorPage2.axaml:392 `InstallAcceleratorCommand`；Services/Mvvm/GameAcceleratorService.cs:340–680 | SDK 探测→路径→下载/安装，显示真实步骤 | Rust SDK install operation+owned 下载资源；路径决定后才提交 | 厂商安装完成并探测实际 SDK 可用/版本 | 当前源安装框的 cancel 配置被注释，不能声称已存在按钮；目标实现按可取消阶段提供入口；超时查安装状态/下载 hash | 05 / WAIT,NET-TRANSFER,FILE,SDK-INSTALL |
| SB-A-036 | A Views/Controls/AcceleratorPathAskBox.axaml:36 `SelectWattAcceleratorInstallPath` | 路径 picker，只改安装 draft | UI picker→Rust query 校验路径 | 合法 draft 路径可见；未安装 | Cancel 保持原 draft；拒绝越权/错目标路径；不独立提交安装 | 05 / WAIT,FILE |
| SB-A-037 | A Views/Controls/AcceleratorPathAskBox.axaml:43 `OKButton_Click`；.axaml.cs:38；GameAcceleratorService.cs:373–374 生成对话框 | 验证路径并返回已确认决定，交给 SB-A-035 外层唯一安装 operation | UI dialog result +冻结 draft revision | 只称路径已确认，随后真实安装仍进行 | 关闭/取消返回 canceled decision；不得再从 OK 发第二次 Install；错路径留对话框 | 05 / UI,FILE |
| SB-A-038 | A Views/Controls/AcceleratorPathAskBox.axaml:52 `CustomInstallButton_Click` | 显示/收起 PathSelect | UI only | 面板可见/焦点正确 | 无路径写入、无安装副作用 | 05 / UI |
| SB-A-039 | A Views/Pages/AcceleratorPage2.axaml:396 `UninstallAcceleratorCommand` | 明确确认卸载及运行影响，先处理自有 SDK 会话 | Rust SDK uninstall operation | 实际安装探测未安装；清理结果明确 | Cancel 不卸载；未知查厂商安装状态，不靠目录不存在单独推断完整卸载 | 05 / WAIT,SDK-INSTALL,FILE |
| SB-A-040 | A Views/Controls/GameAccelerator.axaml:105 `GameAcceleratorCommand`；Views/Controls/GameAppItem.axaml:279 `ButtonCommand`；Views/Controls/GameDetail.axaml:189 `GameAcceleratorCommand`；Views/Pages/GameInfoPage.axaml:215 `ImmediatelyAccelerate` | GameAppItem 当前容器确切绑定 GameAcceleratorService.Current；SDK game+area+server desired start/stop。ImmediatelyAccelerate 提交有效区服决定给同一事务 | Rust SDK 单 operation；game/area/SDK epoch，按钮不执行任意 ICommand | SDK 实际加速终态/停止终态；101/“正在加速”只是进行态 | 权益/安装不足明确；区服取消保留有效原值；SDK 超时 query 再试；其他 ButtonCommand 容器必须注册 route | 05 / WAIT,SDK |
| SB-A-041 | A Views/Controls/GameDetail.axaml:99 `AcceleratorChangeAreaCommand`；GameAcceleratorService.cs:424,712 生成 GameInfoPage dialog | 打开当前 game 区服 draft；确认变更才进入 SDK 同一 operation | UI area/server draft→SB-A-040 SDK 事务 | draft 合法；真正生效仍以 SDK 回调为准 | Cancel 不改已生效区服；切 game 使旧对话框过期；原对话框无标准 OK，使用 ImmediatelyAccelerate 明确提交 | 05 / UI,WAIT,SDK |
| SB-A-042 | A Views/Pages/GameInfoPage.axaml:16 `BackSelectArea` | 回到区选择，清的是本次草稿 server/area 子步骤 | UI only | 当前 draft 和可选区服一致 | 不清真实运行区服，不发 stop；焦点回选区控件 | 05 / UI |
| SB-A-043 | A Views/Controls/GameDetail.axaml:170 `GameLaunchCommand` | 启动选中 SDK 游戏，与加速请求独立 | Rust/平台 game launch 交付，冻结可执行目标 | 交付与已运行探测分开显示 | 路径/权限失败不宣称已运行，不切换加速状态；不能启动任意绑定可执行文件 | 05 / OS,WORKER |
| SB-A-044 | A Views/Controls/GameDetail.axaml:180、Pages/AcceleratorPage2.axaml:387 `ShowXunYouWindow`，param=True | 显示厂商窗口 | SDK/OS show-window 交付 | 厂商窗口实际可见/交付状态 | 未安装提示，不隐式安装/重新加速；返回保持原 route/draft | 05 / SDK,OS |
| SB-A-045 | A Views/Controls/GameAppItem.axaml:134 `DeleteMyGameCommand` | 仅删当前“我的游戏”收藏关系 | Rust my-game ID LOCAL；若在运行，明确仅取消收藏 | 收藏记录 revision 和列表确认 | 不卸载/停游戏；目标消失幂等；失败保留原卡与可重试 | 05 / LOCAL |
| SB-A-046 | A Views/Controls/GameAccelerator.axaml.cs:35–43 `SearchGameBox_SelectionChanged`, `AddMyGame` | 搜索选中 XunYouGame 会新增“我的游戏”，不是纯选择事件 | Rust my-game ID 去重 LOCAL；成功才清输入 | 该稳定 game ID 收藏已保存 | 查询 late-wins；保存失败保留输入/目标；不得触发加速 | 05,17 / LOCAL |

## 5. 账号平台、卡片、动态菜单（F19–F25）

| ID | 源 scope / 确切符号 | action / 领域效果 | 提交目标 | 权威成功 | 取消、失败与恢复 | SB-J / 超时 |
|---|---|---|---|---|---|---|
| SB-A-047 | GA Views/Pages/GameAccountPage.axaml:156 `TabView_SelectedItemChanged`；.axaml.cs:45 | 平台 view 选择；未配置平台显示一次可取消路径流程 | UI viewKey；路径仍草稿 | 正确平台内容/选择，未配置状态明确 | 不陷取消关闭循环；保留其他平台可浏览；不把选 tab 当登录成功 | 06,07 / UI,WAIT |
| SB-A-048 | GA Views/Pages/GameAccountPage.axaml:156 `TabView_TabCloseRequested`；.axaml.cs:80–92 `RemovePlatform` | Steam tab 不允许删除并说明；其他平台确认后移除保存的自有平台关联 | Rust platform ID+revision LOCAL；范围/缓存清理单列 | 平台关联已删除；列表/selection 移至有效邻项 | Cancel 不改；运行依赖先处理；不删除其他程序安装目录/秘密，不能旧 index 误删 | 07 / WAIT,LOCAL,WORKER |
| SB-A-049 | GA Views/Controls/PlatformSettingsPage.axaml:33 `SelectProgramPath` | picker 只选/校验本平台程序路径 draft | UI+Rust read validation；不提前改共享设置 | 合法 draft 路径显示 | Cancel 零持久变化；错误就地标明且可以关闭；不强制重开对话框 | 07 / WAIT,FILE |
| SB-A-050 | GA Views/Pages/GameAccountPage.axaml.cs:24–40 `PlatformSettingButton.Click` 注册匿名 handler；:57–69 选择触发 ShowTaskDialogAsync；生成 OK/Cancel | 打开平台设置→确认合法 draft；取消总能返回未配置平台 | Rust platform setting revision；OK 提交，取消回原 view | setting 持久回执；不等于平台客户端可登录 | 源 CancelCloseAction 阻止关闭的缺陷不复刻；保存失败留路径，不禁导航 | 07 / WAIT,FILE,LOCAL |
| SB-A-051 | GA Views/Pages/GameAccountPage.axaml:36,208 `LoginNewCommand` | 交付平台新登录流程，处理依赖任务/客户端 | Rust platform login intent，Steam 排他 epoch；用户登录 WAIT | 实际新 SID/平台身份确认后才添加/更新卡 | 用户未登录/取消保留旧可靠身份；不提前创建“已登录”卡；超时重新查询客户端 | 06,07 / WORKER,OS,WAIT |
| SB-A-052 | GA Views/Pages/GameAccountPage.axaml:55 `SaveCurrentUserCommand` | 非 Steam 平台保存当前已验证本地缓存关联 | Rust platform+account ID 原子缓存记录；secret 只经安全存储引用 | 必需源文件完整、身份校验与 snapshot/hash 回执 | 缺文件失败不写空缓存；取消未提交清自有 temp；不在反馈/日志泄露缓存内容 | 06,07 / FILE,LOCAL |
| SB-A-053 | GA Views/Pages/GameAccountPage.axaml:95 `RefreshCommand` | 刷新平台账号/头像来源，真实当前身份单独核对 | Rust read query，完整快照后合并 | 来源、时间、有效账号 ID 和真实身份投影 | 暂未返回不删除已知账号；保排序/筛选/锚点，换账号清旧 selection；离线保缓存标签 | 06,07,15 / FILE,NET |
| SB-A-054 | GA Views/Controls/AccountItems.axaml:20,243,252 `SwapToAccountCommand`；.axaml.cs:15–26 DoubleTapped 注册匿名 handler；:43,53 `SteamPersonaStateSwapMenuItem_Click`, `SteamOfflineModeStartMenuItem_Click`；Services.Implementation/SteamPlatformSwitcher.cs:123–138 动态 tray 同一 Swap 命令 | 单击只选；双击/按钮/菜单/托盘同一 switch。persona Tag 与 WantsOffline/SkipWarning 是本次切换 draft，不先改权威账号 | Rust SwitchAccount request；全局 Steam epoch 排他，固定目标 SID+persona/offline | 验证→依赖任务终态→客户端退出→配置提交→启动→实际 SID 确认；配置写入不能当已登录 | 重复冒泡合并；只停拥有任务/授权测试客户端；未知先查 SID/journal；失败给 needs_login/补偿结果，不提前移动当前徽标 | 06,07,15 / LOCAL,FILE,WORKER,WAIT |
| SB-A-055 | GA Views/Controls/AccountItems.axaml:65,73,77 `CopyToClipboardCommand`；:83,88,93,98 `CopySteamIdMenuItem_Click` | 复制 profile URL/DisplayName/AccountName；SteamID Tag 对应 64/2/3/32 格式 | OS 剪贴板；冻结右键 account ID+公开文本，nested MultiBinding 解析 profile URI | 确切文本交付；一次简短反馈 | 文本框编辑优先；无合法 ID 不复制猜值；不切换/克隆账号 | 06 / OS |
| SB-A-056 | GA Views/Controls/AccountItems.axaml:106,114,122,130,138,146,154,162 `OpenBrowserCommand`；:187 `OpenUserDataFolderMenuItem_Click` | URL 为该 account 的 Steam community/SteamRep/SteamRepCN/SteamDB calculator/SteamGifts/SteamTrades/AchievementStats/Backpack.tf；userdata 按 SteamDir+该账户规范相对路径 | OS；冻结 account ID，URI 模板 allowlist/目录范围校验 | OS 接受目标 URL/目录 | 不从标签构造命令；目录不读秘密、不做清理；失败不影响登录状态 | 06 / OS |
| SB-A-057 | GA Views/Controls/AccountItems.axaml:172 `EditRemarkCommand` | 编辑该账号本地备注独立 draft | Rust account metadata revision LOCAL；生成确认才提交 | 回执对应目标和 draft revision | Cancel 不改；保存失败保输入；旧回执不覆盖随后再次编辑 | 06 / WAIT,LOCAL |
| SB-A-058 | GA Views/Controls/AccountItems.axaml:176 `SetAccountAvatarCommand` | 选择/验证自定义头像 | Rust account ID+owned image FILE+metadata LOCAL | 合法图片原子保存与记录 revision | picker Cancel 不改；解码/存储失败回旧头像；不改在线 Steam 头像 | 06 / WAIT,FILE,LOCAL |
| SB-A-059 | GA Views/Controls/AccountItems.axaml:182 `CreateShortcutCommand`；GA Models/PlatformAccount.cs:129–170 `CreateShortcut`, `CreateSystemProtocol`, `CreateLoginShortcut` | 创建该账号切换快捷方式；首次所需自有URI协议注册必须单列可见效果 | 平台 shortcut/icon 文件+自有protocol typed注册；固定 account ID URI、安全 argv与本应用可执行路径 | 实际文件/协议目标readback；双击仍走 SB-A-054；创建不等于已切账号 | picker/确认取消不写；已有路径确认/原子替换；注册失败显示文件/协议各结果；不把秘密写入快捷方式 | 06 / WAIT,NET,FILE,HELPER |
| SB-A-060 | GA Views/Controls/AccountItems.axaml:191 `DeleteAccountCommand` | 确认后删除明确本地账号记录/自有缓存；说明不注销 Steam 远程账号 | Rust account ID+revision；当前任务/会话先处理 | 本地事务完成、selection/任务一致 | Cancel 零删除；当前账号依赖明确；不能从列表删除即称远程退出；未知查缓存/journal | 06,15 / WAIT,WORKER,FILE,LOCAL |

## 6. 库、详情、overlay、AFK（F26–F35）

| ID | 源 scope / 确切符号 | action / 领域效果 | 提交目标 | 权威成功 | 取消、失败与恢复 | SB-J / 超时 |
|---|---|---|---|---|---|---|
| SB-A-061 | GL Views/Pages/GameListPage.axaml:172 `RefreshAppCommand` | 刷新该账户库来源/manifest；保持搜索、排序、筛选、锚点 | Rust library query，SID/epoch 与完整快照标志 | 来源与时间明确；完整后合并，10k增量可交互 | 失败保最后可靠库；空/半写 manifest 不推导下载完成/所有权消失 | 01,08,12,17 / FILE,NET |
| SB-A-062 | GL Views/Pages/GameListPage.axaml:21,27 `InstallOrStartAppCommand` | 依据可靠安装状态解析 install/start，两个来源同一 AppId 意图 | Steam client/URI 交付；SID+epoch+AppId | 交付、下载开始、实际运行分别核对/显示 | 重复交付去重；Steam 不可用明确；不因卡片 selected 就判游戏已运行 | 01,08,12 / OS,WORKER |
| SB-A-063 | GL Views/Pages/GameListPage.axaml:33、EditAppsPage.axaml:127 `EditAppInfoClickCommand`（后者由 AppItem.ClickCommand 容器传递）；VM/Pages/GameListPageViewModel.cs:190–205 | 打开 AppId 完整独立草稿；包括名字、启动项、四图及旧版本 | UI draft，源共享 App 引用不得复刻 | 独立 draft 与当前可靠 overlay 版本一致 | 未注册 ClickCommand 容器须 route 前置；关闭/切账号按草稿规则；不在打开时落 Steam 文件 | 08,09 / UI |
| SB-A-064 | GL VM/Pages/GameListPageViewModel.cs:194–203 生成 Save/Cancel；VM/Pages/EditAppInfoPageViewModel.cs:120,191 `SaveEditAppInfo`, `CancelEditAppInfo` | 保存完整草稿到本地 overlay；Cancel 丢整个草稿，含文本/启动项/图片 | Rust SID+AppId+draft revision 的 overlay/owned staged images；本地原子提交 | “已保存本地修改，待应用到 Steam”；最新 overlay revision | Save 失败保所有输入；Cancel 不写 Steam/原图；旧 save 回执不清新 dirty；不复刻 Cancel 只撤图 | 08,09 / WAIT,FILE,LOCAL |
| SB-A-065 | GL Views/Pages/EditAppInfoPage.axaml:179,235,290,345 `ShowGridDialog_Click`；.axaml.cs:36 `RefreshSteamGridItemList` | 打开对应 Grid/Hero/Logo/Icon 图片选择并查询 | UI panel + Rust read query，AppId+slot+draft revision | panel 可交互；最新 slot 结果/来源 | 切 slot 拒绝旧结果；失败保已有预览，网络不阻根导航 | 08 / UI,NET |
| SB-A-066 | GL Views/Pages/EditAppInfoPage.axaml:132 `SteamGridDBItem_Tapped`；.axaml.cs:60 `ApplyCustomImageToApp` | 选择确切 image ID 到当前 slot 的 staged draft preview | Rust read/download 到 owned temp；UI draft；不直接写 Steam 图片 | 图片完整合法，当前 slot preview 匹配 image/draft | 取消下载不改变原预览；旧 slot/epoch 回调无效；只有 SB-A-064 确认提交 overlay | 08,09 / NET-TRANSFER,FILE |
| SB-A-067 | GL Views/Pages/EditAppInfoPage.axaml:189,245,300,355 `ResetGridImage_Click` | 重置本次指定图 slot draft 到定义默认/原图来源 | UI draft；必要 image read query | 指定 slot 预览与 draft 一致 | 不当场删 Steam 文件；错误留原图；Cancel 完整撤草稿 | 08,09 / UI,FILE |
| SB-A-068 | GL Views/Pages/EditAppInfoPage.axaml:108 `HideGridDialog_Click` | 收起图片子面板，恢复 slot 焦点 | UI only | 子面板关闭且主 draft 保持 | 不触发 Save/CancelEdit，不中断其他合法草稿字段 | 08 / UI |
| SB-A-069 | GL Views/Pages/EditAppInfoPage.axaml:143,147,151 `OpenSteamGridDBImageUrlCommand`, `OpenSteamGridDBAppUrlCommand`, `OpenSteamGridDBAuthorUrlCommand` | 打开所选图片/App/作者来源 | OS validated URL；冻结该结果 entity ID | OS 接受对应 URL | 来源过期/缺失不给错误邻项 URL；不修改所选图片 | 08 / OS |
| SB-A-070 | GL Views/Pages/EditAppInfoPage.axaml:439,447,456,472 `UpLaunchItemCommand`, `DownLaunchItemCommand`, `DeleteLaunchItemCommand`, `AddLaunchItem` | 只改当前 draft 启动项集合/稳定 item ID 顺序；边界项禁用理由 | UI draft only；随 SB-A-064 统一保存 | draft 集合/焦点/顺序一致 | 不在每次 reorder 发 Rust 写 operation；删除不运行程序；Cancel 撤销所有 draft 改动 | 08,09 / UI |
| SB-A-071 | GL Views/Pages/EditAppInfoPage.axaml:80,535 `OpenFolder` param App.InstalledDir/FormatDirPath；GameListPage.axaml:78 `OpenFolderCommand` | 打开当前 App 安装/格式目录 | OS，AppId→规范目录；不能执行任意字符串 | OS 接受确切目录 | 不存在/无权限明确；不读或上传账号文件；不把打开当保存/安装 | 08 / OS |
| SB-A-072 | GL Views/Pages/EditAppInfoPage.axaml:495 `ManageCloudArchive_Click`；GameListPage.axaml:38 `ManageCloudArchive_ClickCommand`；VM/Pages/GameListPageViewModel.cs:207–217 | 打开 Cloud worker/window，两个入口同一角色与上下文 | Rust owned worker role=CloudManager+SID+AppId+epoch；只开窗口不写云 | worker ready 且身份/目标握手正确，窗口可交互 | 无 Steam/身份不符说明；旧 AppId-only 全局字典不得复用错角色；失败不半开窗口 | 10,15 / WORKER |
| SB-A-073 | GL Views/Pages/GameListPage.axaml:44 `UnlockAchievement_ClickCommand`；VM/Pages/GameListPageViewModel.cs:220–239 生成风险确认/记住选择 | 打开 Achievement worker；风险确认仅授权该范围，不执行 reset | Rust worker role=UnlockAchievement+SID+AppId+epoch；确认偏好单独 LOCAL | worker ready+目标身份正确 | 取消零写；类型不适用就地理由；不能复用 Cloud/用户游戏进程 | 11,15 / WAIT,LOCAL,WORKER |
| SB-A-074 | GL Views/Pages/GameListPage.axaml:102,201 `AddHideAppListCommand`, `ShowHideAppCommand`；HideAppsPage.axaml:26 `RemoveHideAppCommand` | 前后两者为本地 hide/unhide AppId；ShowHideApp 打开隐藏管理 view | Rust hide-set revision LOCAL；view 打开 UI | 保存的 hide set 与可见库/隐藏页一致 | 失败不丢 App；不远程隐藏/卸载；按ID保存焦点，返回恢复库筛选 | 08 / LOCAL,UI |
| SB-A-075 | GL Views/Pages/GameListPage.axaml:53 `AddAFKAppListCommand` | 将选中 AppId 加入本地 AFK 集合，去重；不等于启动 | Rust AFK set revision LOCAL | 列表保存且“未运行/运行”真实投影 | 冲突任务说明，不创建重复 worker；失败保库 selection | 14 / LOCAL |
| SB-A-076 | GL Views/Pages/GameListPage.axaml:62,70 `NavAppToSteamViewCommand`, `NavAppScreenshotToSteamViewCommand`；VM/Pages/GameListPageViewModel.cs:309–319；IC Views/Pages/IdleCardPage.axaml:46 `NavAppToSteamViewCommand` param App.AppId | 分别交付 `STEAM_NAVGAME_URL` 和 `STEAM_NAVGAMESCREENSHOTS_URL` 定义的客户端游戏/截图 view；挂卡只传稳定 AppId | OS Steam URI allowlist，SID/epoch context | URI 交付；页面加载由客户端负责 | 不用 AppId/index 猜账号；失败不影响 AFK/挂卡 | 08,14 / OS |
| SB-A-077 | GL Views/Pages/GameListPage.axaml:86,90,95；IC Views/Pages/IdleCardPage.axaml:55,59,64 `AppOpenLink_MenuItem_Click` | App 链接菜单按源码 Tag 的固定站点模板生成该 AppId URL | OS，右键 AppId+菜单 kind 冻结 | OS 接受确切 URL | 未解析 Tag 阻止动作；菜单打开后库刷新不换目标，不额外启动游戏 | 08,14 / OS |
| SB-A-078 | GL Views/Pages/IdleAppsPage.axaml:257,271 `RunStopBtnCommand` | 按可靠 owner 状态提交明确 AFK start/stop AppId | Rust owned AFK worker request；与挂卡 per-AppId ownership 协议 | ready/exit 确认；“停止 AFK”不误停挂卡/Cloud/用户游戏 | 重复按钮统一请求；冲突说明具体任务；未知查 handle/role，不按名字 kill | 14,15 / WORKER |
| SB-A-079 | GL Views/Pages/IdleAppsPage.axaml:32,52 `RunOrStopAllButton_Click` | 冻结本 AFK scope 的目标 IDs 和明确 start-all/stop-all；有界批次 | Rust batch，单 App worker operation，结果逐项 | 完整 summary 成功/失败/跳过；不一个全局 bool | 取消未提交项；部分运行保持真实，停止仅 AFK owner；新增项不偷偷加入已接受批次 | 14,15 / WORKER |
| SB-A-080 | GL Views/Pages/IdleAppsPage.axaml:285 `DeleteButtonCommand`；:102 `DeleteAllButton_Click` | 删单项/明确 AFK 集合；运行项先安全释放该 owner | Rust AFK set revision；单项稳定 ID或确认批次 | 自有任务与集合原子/协调结果确认 | 全删确认取消零删除；部分 stop 失败说明保留项，不停其他 owner；不删除游戏 | 14 / WAIT,WORKER,LOCAL |
| SB-A-081 | GL Views/Pages/IdleAppsPage.axaml:72 `Refresh_Click` | 重新读 AFK 集合/真实 worker 投影 | Rust read query | 新 epoch snapshot，保有效 selection/锚点 | 不以空快照 stop-all；失败保上次可靠状态 | 14,17 / WORKER,LOCAL |
| SB-A-082 | GL Views/Pages/EditAppsPage.axaml:41 `LoadSteamEditedApps` | 从实际 Steam/本地来源加载编辑列表；说明是否覆盖未保存 draft | Rust read/staged parse；SID+epoch+source revision | 完整合法列表并标来源；不能自动应用 | dirty 覆盖需确认，Cancel 保留；坏文件/身份变更保原 overlay | 09 / WAIT,FILE |
| SB-A-083 | GL Views/Pages/EditAppsPage.axaml:23 `SaveSteamEditedApps` | 应用当前最新有效 overlay 到 Steam；保留两阶段体验 | Rust apply transaction SID+epoch+overlay revision+expected file hashes；owned backups | 文件替换 readback/hash、已应用 revision；若需 Steam 重启明确 pending_restart | 冲突先刷新/重建 draft，不覆盖外部改动；部分失败列资源及补偿；未核实不绿；不因保存失败引导 reset | 09,15 / FILE,LOCAL,WORKER |
| SB-A-084 | GL Views/Pages/EditAppsPage.axaml:70 `ExportSteamEditedAppsBackup` | 导出明确 overlay snapshot 合成/本地编辑备份 | picker→Rust owned export FILE，冻结 revision | 文件原子写入/readback；显示内容版本 | picker Cancel 不写；失败无残缺成功文件；不包含凭据/订阅/会话 | 09 / WAIT,FILE |
| SB-A-085 | GL Views/Pages/EditAppsPage.axaml:76 `ImportSteamEditedAppsBackup` | 导入备份到 staged overlay，校验 schema/图片/目标范围，确认合并 | picker→Rust import draft→LOCAL；不直接写 Steam | 合法 overlay revision 已保存；应用另走 SB-A-083 | 错包/冲突保原 overlay；Cancel 无提交；跨 SID 数据须显式目标说明，不偷偷写当前账号 | 09 / WAIT,FILE,LOCAL |

## 7. Cloud 与成就（F36–F39）

这些窗口从 SB-A-072/073 冻结上下文建立；即使切账号后还保留旧窗口，写权限必须失效。任何副作用未知先按旧 SID+AppId/operation 查真实结果，不能让 UI 丢弃结果后直接给新账号再执行。

| ID | 源 scope / 确切符号 | action / 领域效果 | 提交目标 | 权威成功 | 取消、失败与恢复 | SB-J / 超时 |
|---|---|---|---|---|---|---|
| SB-A-086 | GL Views/Windows/CloudArchiveWindow.axaml:25 `RefreshList`；VM/Windows/CloudArchiveAppPageViewModel.cs:104–118 | 查询真实 Cloud 文件/配额完整 snapshot；worker 后台读取 | Rust query，SID+AppId+epoch；配额 u64 | 来源/time、完整文件 IDs+版本、合法 quota | 请求中保旧列表；失败不清空；换账号拒绝旧回写，refresh不等于云同步完成 | 10,15 / WORKER |
| SB-A-087 | GL Views/Windows/CloudArchiveWindow.axaml:40 `UploadFile`；VM/Windows/CloudArchiveAppPageViewModel.cs:133–160 | picker→名字/冲突/大小预检→写确切云 key；保留大小写语义 | Rust Cloud write request+stream FILE；SID+AppId+expected version+hash | Steamworks FileWrite accepted 与平台 readback metadata/hash 分阶段；同步状态未知要说明 | picker Cancel 零写；超时查远端该 key/hash 再重试；不 ReadAllBytes 大文件、不开新请求重传未知写 | 10,15 / WAIT,FILE,WORKER |
| SB-A-088 | GL Views/Windows/CloudArchiveWindow.axaml:118 `DeleteFile`，旧 param #CloudGrid.SelectedItem；VM/Windows/CloudArchiveAppPageViewModel.cs:49–82 | 右键菜单冻结被右键 file ID/key+version，删除需确认 | Rust Cloud delete request；禁止依赖更新后 SelectedItem/index | 该 key 的实际删除确认，完整刷新后合并 | Cancel 零删；目标变化/冲突先刷新；超时查 key 存在性，不能删邻项/新版本 | 10,15 / WAIT,WORKER |
| SB-A-089 | GL Views/Windows/CloudArchiveWindow.axaml:55 `ClearAllFiles`；VM/Windows/CloudArchiveAppPageViewModel.cs:121–130 | 明确当前 Cloud 目标 scope 全清；确认总数/范围，固定文件版本集合 | Rust batch，单 file operation；账号排他协调 | 全部逐项终态与失败集合；最终完整查询 | 取消未提交项；已删不可虚构恢复；部分失败不报“全部成功”；超时逐 key 查，不重新清新文件 | 10,15 / WAIT,WORKER |
| SB-A-090 | GL Views/Windows/CloudArchiveWindow.axaml:113 `DownloadFile`，旧 param #CloudGrid.SelectedItem | 冻结右键目标→选择输出→读取匹配版本→原子文件落盘 | Rust query+FILE，SID+AppId+key+version+hash；目标路径确认 | 文件字节/长度/hash与所选版本一致，原子替换成功 | picker Cancel 不写；同名确认；取消清 owned temp；旧账号 callback 不写新文件/弹成功 | 10,15 / WAIT,WORKER,FILE |
| SB-A-091 | GL Views/Windows/AchievementWindow.axaml:27 `RefreshStats_Click`；VM/Windows/AchievementAppPageViewModel.cs:530 | RequestCurrentStats 等真实 callback 后刷新成就/统计 | Rust worker query SID+AppId+epoch，旧表 retained | 正确 app callback+完整数据 snapshot | 读失败保可靠表；有 dirty 明确 refresh 冲突/保草稿，不能清表冒充加载完 | 11,15 / WORKER |
| SB-A-092 | GL Views/Windows/AchievementWindow.axaml:57 `SaveChange_Click`；VM/Windows/AchievementAppPageViewModel.cs:559–581 | 校验成就/统计 draft，提交 StoreAchievements/StoreStatistics/StoreStats 同一 operation | Rust worker write SID+AppId+epoch+expected stats version+draft revision | 实际 StoreStats callback/重读匹配；列部分失败，不能仅 API bool/toast | 保存失败保输入；**不得调用 ResetAllStats_Click**；结果未知重读目标后再试；取消仅未提交阶段 | 11,15 / WORKER |
| SB-A-093 | GL Views/Windows/AchievementWindow.axaml:42 `ResetAllStats_Click`；VM/Windows/AchievementAppPageViewModel.cs:541–556 生成三个MessageBox | 保留三步位置和确认流程；第二步两选择明确命名“统计与成就”“仅统计”，不将后者标为取消；第三步最终确认冻结范围 | 独立typed reset request，范围选择与dismiss为不同结果；不是Save的恢复路径 | ResetAllStats accepted+正确App callback/重读证实所选范围 | 任一步Esc/X/真正取消均终止且零重置；旧源码第二步Cancel表示仅统计，不照搬到新UI取消语义；未知先重读，不自动retry | 11,15 / WAIT,WORKER |

## 8. Steam Web 登录与挂卡（F40–F46）

| ID | 源 scope / 确切符号 | action / 领域效果 | 提交目标 | 权威成功 | 取消、失败与恢复 | SB-J / 超时 |
|---|---|---|---|---|---|---|
| SB-A-094 | IC Views/Pages/IdleSteamLoginPage.axaml:53,78 `Login`；VM/IdleSteamLoginPageViewModel.cs:17,21–78 `SteamLoginAsync` | 登录/验证码/二次认证步骤为同一 Web auth session | Rust auth session+secure secret references；非秘密用户名 draft 保留；SID 校验 | Steam Web 会话实际有效且 SID 与客户端要求一致；remember 提交另有本地确认 | challenge 返回/取消不清原页面，不自动重发；过期新挑战；秘密不进日志/CSV；网络错不当空掉卡成功 | 13,15 / NET,WAIT,LOCAL |
| SB-A-095 | IC Views/Pages/IdleSteamLoginPage.axaml:105 `CookieLogin`；VM/IdleSteamLoginPageViewModel.cs:18,81–121 `CookieLoginAsync` | Cookie 格式校验后必须实际验证会话/SID | Rust auth verify+secure storage reference；禁止仅格式成功 | 有效 Web session+正确 SID | 非法/过期/错 SID 就地错误留非秘密上下文；取消不保明文；不得直接显示已登录或启动挂卡 | 13,15 / NET,LOCAL |
| SB-A-096 | IC Views/Pages/IdleCardPage.axaml:307 `LoginSteamCommand`；VM/IdleCardPageViewModel.cs:47–74 | 依据权威 Web session 显式 login 或 logout；不是点击即反向改 bool | Rust session intent；logout停 owned card worker→会话失效→Web epoch 更新 | logout清自有会话/worker事实；login后仍需 SB-A-094/095验证 | logout失败保 truthful 状态；已提交旧账号操作必须 journal 终态；切页不自动 logout | 13,14,15 / WORKER,LOCAL,NET,WAIT |
| SB-A-097 | IC Views/Pages/IdleCardPage.axaml:86 `IdleRunStartOrStop`；VM/IdleCardPageViewModel.cs:269–364 | 检查客户端+Web SID+合法徽章 snapshot后明确 start/stop 调度 | Rust card scheduler request+owned per-AppId workers；SID/epoch/queue revision | start实际 scheduler/worker ready；stop自有引用释放并确认；零掉卡须完整可靠来源 | 失联/身份不符阻止启动；不把Forbidden/空/半快照当全部掉完；离页继续，未知查scheduler/worker | 13,14,15,17 / NET,WORKER |
| SB-A-098 | IC Views/Pages/IdleCardPage.axaml:27 `PriorityRunIdle`；VM/IdleCardPageViewModel.cs:175–224 | 固定 AppId 优先调度；说明暂停/改变哪项自动调度 | Rust scheduler queue revision + owned worker 协调 | 队列顺序与实际运行 snapshot一致 | 不用瞬时 UI reorder冒充已切游戏；冲突原因可见；失效App/epoch安全拒绝 | 14,15 / LOCAL,WORKER |
| SB-A-099 | IC Views/Pages/IdleCardPage.axaml:33,39 `ToggleBlacklistIdle` | 点击时可靠值解析明确 add/remove blacklist；运行项按调度协议停/换 | Rust SID+AppId+expected blacklist revision +scheduler更新 | 已保存 blacklist 与实际运行/队列事实分别确认 | 两菜单入口不二次toggle；失败保真实旧集合与待处理值；不误停 AFK owner | 14,15 / LOCAL,WORKER |
| SB-A-100 | IC VM/IdleCardPageViewModel.cs:403–423 `RunLoadBadges` 生成 `TextBoxWindowViewModel.ShowDialogAsync` PIN 确认/取消，`UnlockParental` | Forbidden 后家长 PIN challenge；成功再取完整徽章 | Rust auth session epoch 的 unlock query；secret不落普通状态；Web请求独立 | unlock实际响应+随后完整徽章结果；PIN成功不等于挂卡启动 | Cancel/空输入返回原登录/队列上下文，不抛成空队列完成；失败允许重试挑战而非清库 | 13,14,15 / WAIT,NET |
| SB-A-101 | IC VM/IdleCardPageViewModel.cs:366–428 `LoadBadges`、`RunLoadBadges` 自动 token refresh/恢复分支（见46家族研究）；非独立可见按钮 | Web会话核对/refresh→徽章获取是登录/启动 operation 的子阶段 | Rust actor内部阶段，同 SID/epoch，不另造用户点击命令 | 有效会话/身份/完整徽章来源，合法零数量区分明确 | refresh失效回needs_login；旧epoch终态核对；自动重试只可限次只读/刷新，禁止无限登录弹框 | 13,14,15,17 / NET |

## 9. 主窗、动态 tray、代码生成入口

| ID | 源 scope / 确切符号 | action / 领域效果 | 提交目标 | 权威成功 | 取消、失败与恢复 | SB-J / 超时 |
|---|---|---|---|---|---|---|
| SB-A-102 | Shell UI/App.TrayIcon.cs:37,149 `RestoreMainWindow` Command；UI/App.Interface.cs:14–75 窗口唤回；Views/Windows/MainWindow.axaml.cs:116–124 主窗 Closing 匿名 handler | restore/第二实例唤回同一主会话；Tray=true X隐藏，Tray=false X转真实退出 | UI lifecycle；还原订阅权威 snapshots | 当前 route/锚点/draft与后台任务一致，同一owner，无第二worker | 隐藏不stop/login；焦点恢复，渲染停但数据源有界；真正退出走SB-A-103 | 02,14,16,17 / UI,LOCAL |
| SB-A-103 | Shell UI/App.TrayIcon.cs:155 `Shutdown` 生成 Command；Tray=false 主窗关闭路由 | 显式退出：协调未提交draft/不可取消写，停止本应用 owned资源，持久journal | Rust application lifecycle+owned worker cleanup；UI shutdown 末步 | 已授权不可取消动作终态/unknown记录持久化，owned资源清理确认后退出 | 有待确认副作用明确；超时写可恢复journal，不假取消；不杀其他进程，重开先探活/恢复结果 | 10,11,15,16 / WAIT,LOCAL,WORKER |
| SB-A-104 | Shell UI/App.TrayIcon.cs:110,133–142 `UpdateMenuItems` 生成 Steam start/close 匿名 Command，内调 `StartSteamWithParameter`, `TryKillSteamProcess` | tray显式客户端生命周期，复用账户冲突域；不能把源按名字停止当新合同 | Rust Steam lifecycle request，明确受管/隔离授权客户端与依赖任务；next_run参数快照 | 实际客户端PID/身份/退出结果；启动交付与登录分开 | 缺停止权限/非owned目标拒绝并解释；测试只自有或明确授权隔离客户端；未知先核对 | 06,07,15 / WORKER,OS,WAIT |
| SB-A-105 | Shell UI/App.TrayIcon.cs:159–194 `TrayMenus[].Command/CommandParameter`；A ProxyService.cs `UpdateProxyTrayMenuItems`；GA SteamPlatformSwitcher.cs 账号菜单 | 动态菜单只由保留四模块typed action注册表生成；网络→SB-A-016，账号→SB-A-054；root还原/退出→102/103 | 同一领域 request，与按钮复用去重；匿名Command不是任意执行许可 | 各目标 action 自己的权威成功；menu可见不是业务完成 | 菜单打开后目标消失/epoch改变拒绝旧参数；未注册插件不恢复；未发现GL/IC tray就不编造挂卡按钮 | 02,06,15,16 / 依目标类别 |

代码后置生成入口均使用对应主 action：设置参数 OK/Cancel→014；代理 reset 确认→019；证书/hosts确认→020/021/024；脚本外部编辑确认与缺文件删除确认→029/030；SDK路径自带OK→037且外层install仍035；区服确认→040，返回→042；平台设置OK/Cancel→050；账号备注/头像/删除的picker/确认→057/058/060；库元数据Save/Cancel→064；Cloud/成就风险确认→073/088/089/093；登录验证码/二次验证→094；家长PIN→100。生成按钮必须追加独立元素ID与明确返回焦点，不能因为不在AXAML就略过。

下载页/下载监控三态选择/完成动作配置（F33/F34）、成就表保护和数值编辑（F38）、挂卡调度字段（F44）及无命令的选择/搜索/窗口尺寸由字段策略和P规则决定。它们不是遗漏的匿名业务命令；**不为XML属性、样式、布局、普通投影和纯选择造operation**。真实下载完成后电源动作是Rust监控 actor 的授权后继：锁实际目标完整快照、可取消倒计时、到期再次核对；不能只因一次空manifest触发。实施须按Flow SB-J12补动态倒计时/取消元素及平台授权测试，当前未运行。

## 10. 候选、非适用及未证明 route

本节同样是 `identified` 合同/边界；源 applicability 仍保持原值。列出符号不代表恢复入口、提高适用分母或实现这些功能。

| ID | 源 scope / 确切符号 | 明确边界 / 可能提交目标 | 成功、取消与失败要求 | SB-J / 超时 |
|---|---|---|---|---|
| SB-A-106 | Shell Views/Pages/User/LoginOrRegisterPage.axaml `ChooseChannel`, `ChangeState`, `ManualLogin`, `OpenHyperlink`, `SendSms`, `Submit`；CSV candidate_requires_route_check | 商业/产品账户候选route。ChooseChannel/ChangeState为UI；OpenHyperlink只交付协议/隐私OS；SendSms/Submit/ManualLogin若将来获准才有auth operation，不能与Steam Web Login混用 | 必须先确认四模块实际所需route与账号角色；不从About CopyUserId恢复页面。若启用须实名 challenge session、provider TTL、发送去重/限频、用户取消及身份验证 | 仅route获准后关联13/16，当前不放行 / UI,OS,NET,WAIT |
| SB-A-107 | Shell Views/Pages/User/NoticeFlyout.axaml `OpenBrowserCommand` param MessageLink；CSV candidate_requires_route_check | 产品通知候选；链接须来源/URL白名单；未证明通知源与四模块相关 | route不恢复；若启用仅OS交付，不称通知已完成业务；未知MessageLink拒绝 | 未确认 / OS |
| SB-A-108 | A Views/Pages/AcceleratorPage.axaml `StartProxyCommand`, `RefreshCommand`, `ProxySettingsCommand`, `SetupCertificateCommand`, `DeleteCertificateCommand`, `ShowCertificateCommand`, `OpenCertificateDirCommand`, `EditHostsFileCommand`, `ResetHostsFileCommand`, `OpenHostsDirCommand`；CSV not_applicable_source_inactive | 旧网络页，不恢复；对应当前 AcceleratorPage2 的合同仅用于当前scope，不让同名符号误匹配旧页 | 不导航/初始化旧route，不新增按钮；保留静态分母与source applicability | 无 / 无执行 |
| SB-A-109 | GA Views/Pages/SteamFamilyShareManagePage.axaml `RemoveButton_Click`；CSV not_applicable_source_inactive | 非当前入口FamilyShare页；不恢复family share删除 | 不执行，不假设适用于账号本地DeleteAccount | 无 / 无执行 |
| SB-A-110 | A VM/ScriptPageViewModel.cs `ScriptStoreCommand`, `OpenScriptStoreWindow`；Views/Pages/ScriptPage.axaml.cs 被注释store入口 | store视图虽然CSV为in_scope_source_view，当前可达route未证明；SB-A-031/034合同已覆盖静态命令 | 实现前记录真实注册来源或保持无入口；不能靠构造ViewModel证明用户可达 | 04前置待确认 / UI,WAIT |
| SB-A-111 | Shared VM/Pages/SettingsPageViewModel.cs `DeletePlugin_Click`, `ResetImage_Click`, `CheckUpdate_Click`；IC VM/IdleCardPageViewModel.cs `IdleManualRunNext`；GL VM/Pages/EditAppInfoPageViewModel.cs `ResetEditAppInfo`；本轮未找到当前live入口 | 方法存在不等于route；不用它们补新增删除插件/重置背景/远程更新/手动下一游戏/恢复库按钮 | 若后续发现真实代码生成入口，追加元素与独立合同并核查效果；否则不执行。GL库layout切换按钮在AXAML注释中，不恢复 | 未确认 / 按获准action |

## 11. CSV 精确 source/symbol → action 关联索引

本索引只收 `in_scope_source_view` 的29个源views。每个分号分隔项是确切归一化符号及合同ID；命令参数/容器仍执行第1节及对应主表。它不是把source代码string当可执行路由的注册表。

| CSV source（相对源根，完整路径） | 归一化 symbol → SB-A ID |
|---|---|
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/About/AboutPage.axaml` | `EasterEgg_PointerReleased=SB-A-005`; `GPLv3_Tapped=SB-A-006`; `Dotnet_Tapped=SB-A-006`; `Avalonia_Tapped=SB-A-006`; `Command=SB-A-007/SB-A-008/SB-A-009`（171行Hyperlinks按item区分；198行OSL只007） |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/HomePage.axaml` | `NavgationToMenuPageCommand=SB-A-001` |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/Settings/Settings_General.axaml` | `OpenFolder_Click=SB-A-010` |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/Settings/Settings_Plugin.axaml` | `SwitchEnablePlugin_Click=SB-A-011`; `OpenPluginDirectory_Click=SB-A-012`; `OpenPluginCacheDirectory_Click=SB-A-012` |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/Settings/Settings_Steam.axaml` | `SelectSteamProgramLocation=SB-A-013`; `EditSteamParameter=SB-A-014` |
| `src/BD.WTTS.Client.Avalonia/UI/Views/Pages/Settings/Settings_UI.axaml` | `SelectImage_Click=SB-A-015` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Controls/AcceleratorPathAskBox.axaml` | `SelectWattAcceleratorInstallPath=SB-A-036`; `OKButton_Click=SB-A-037`; `CustomInstallButton_Click=SB-A-038` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Controls/AcceleratorServiceStatus.axaml` | `ConnectTestCommand=SB-A-026`; `ConnectTestButtonOnClick=SB-A-027` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Controls/GameAccelerator.axaml` | `GameAcceleratorCommand=SB-A-040` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Controls/GameAppItem.axaml` | `DeleteMyGameCommand=SB-A-045`; `ButtonCommand=SB-A-040` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Controls/GameDetail.axaml` | `AcceleratorChangeAreaCommand=SB-A-041`; `GameLaunchCommand=SB-A-043`; `ShowXunYouWindow=SB-A-044`; `GameAcceleratorCommand=SB-A-040` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Controls/NetworkCheck.axaml` | `NATCheckCommand=SB-A-026`; `DNSCheckCommand=SB-A-026`; `IPv6CheckCommand=SB-A-026` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/AcceleratorPage2.axaml` | `StartProxyCommand=SB-A-016`; `RefreshCommand=SB-A-017`; `ProxySettingsCommand=SB-A-018`; `SetupCertificateCommand=SB-A-020`; `DeleteCertificateCommand=SB-A-021`; `ShowCertificateCommand=SB-A-022`; `OpenCertificateDirCommand=SB-A-022`; `EditHostsFileCommand=SB-A-023`; `ResetHostsFileCommand=SB-A-024`; `OpenHostsDirCommand=SB-A-025`; `OpenLogFileCommand=SB-A-025`; `ShowXunYouWindow=SB-A-044`; `InstallAcceleratorCommand=SB-A-035`; `UninstallAcceleratorCommand=SB-A-039` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/GameInfoPage.axaml` | `BackSelectArea=SB-A-042`; `ImmediatelyAccelerate=SB-A-040` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/ProxySettingsPage.axaml` | `ResetSettings=SB-A-019` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/ScriptPage.axaml` | `AddNewScriptCommand=SB-A-028`; `RefreshALLScriptCommand=SB-A-033`; `DownloadScriptItemCommand=SB-A-031`; `EditScriptItemCommand=SB-A-029`; `RefreshScriptItemCommand=SB-A-032`; `OpenBrowserCommand=SB-A-034`; `DeleteScriptItemCommand=SB-A-030` |
| `src/BD.WTTS.Client.Plugins.Accelerator/UI/Views/Pages/ScriptStorePage.axaml` | `DownloadScriptItemCommand=SB-A-031`; `OpenBrowserCommand=SB-A-034`（route前置SB-A-110） |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Controls/AccountItems.axaml` | `SwapToAccountCommand=SB-A-054`; `SteamOfflineModeStartMenuItem_Click=SB-A-054`; `SteamPersonaStateSwapMenuItem_Click=SB-A-054`; `CopyToClipboardCommand=SB-A-055`; `CopySteamIdMenuItem_Click=SB-A-055`; `OpenBrowserCommand=SB-A-056`; `OpenUserDataFolderMenuItem_Click=SB-A-056`; `EditRemarkCommand=SB-A-057`; `SetAccountAvatarCommand=SB-A-058`; `CreateShortcutCommand=SB-A-059`; `DeleteAccountCommand=SB-A-060` |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Controls/PlatformSettingsPage.axaml` | `SelectProgramPath=SB-A-049` |
| `src/BD.WTTS.Client.Plugins.GameAccount/UI/Views/Pages/GameAccountPage.axaml` | `LoginNewCommand=SB-A-051`; `SaveCurrentUserCommand=SB-A-052`; `RefreshCommand=SB-A-053`; `TabView_SelectedItemChanged=SB-A-047`; `TabView_TabCloseRequested=SB-A-048` |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/EditAppInfoPage.axaml` | `OpenFolder=SB-A-071`; `HideGridDialog_Click=SB-A-068`; `SteamGridDBItem_Tapped=SB-A-066`; `OpenSteamGridDBImageUrlCommand=SB-A-069`; `OpenSteamGridDBAppUrlCommand=SB-A-069`; `OpenSteamGridDBAuthorUrlCommand=SB-A-069`; `ShowGridDialog_Click=SB-A-065`; `ResetGridImage_Click=SB-A-067`; `UpLaunchItemCommand=SB-A-070`; `DownLaunchItemCommand=SB-A-070`; `DeleteLaunchItemCommand=SB-A-070`; `AddLaunchItem=SB-A-070`; `ManageCloudArchive_Click=SB-A-072` |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/EditAppsPage.axaml` | `SaveSteamEditedApps=SB-A-083`; `LoadSteamEditedApps=SB-A-082`; `ExportSteamEditedAppsBackup=SB-A-084`; `ImportSteamEditedAppsBackup=SB-A-085`; `EditAppInfoClickCommand=SB-A-063` |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/GameListPage.axaml` | `InstallOrStartAppCommand=SB-A-062`; `EditAppInfoClickCommand=SB-A-063`; `ManageCloudArchive_ClickCommand=SB-A-072`; `UnlockAchievement_ClickCommand=SB-A-073`; `AddAFKAppListCommand=SB-A-075`; `NavAppToSteamViewCommand=SB-A-076`; `NavAppScreenshotToSteamViewCommand=SB-A-076`; `OpenFolderCommand=SB-A-071`; `AppOpenLink_MenuItem_Click=SB-A-077`; `AddHideAppListCommand=SB-A-074`; `RefreshAppCommand=SB-A-061`; `ShowHideAppCommand=SB-A-074` |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/HideAppsPage.axaml` | `RemoveHideAppCommand=SB-A-074` |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Pages/IdleAppsPage.axaml` | `RunOrStopAllButton_Click=SB-A-079`; `Refresh_Click=SB-A-081`; `DeleteAllButton_Click=SB-A-080`; `RunStopBtnCommand=SB-A-078`; `DeleteButtonCommand=SB-A-080` |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Windows/AchievementWindow.axaml` | `RefreshStats_Click=SB-A-091`; `ResetAllStats_Click=SB-A-093`; `SaveChange_Click=SB-A-092` |
| `src/BD.WTTS.Client.Plugins.GameList/UI/Views/Windows/CloudArchiveWindow.axaml` | `RefreshList=SB-A-086`; `UploadFile=SB-A-087`; `ClearAllFiles=SB-A-089`; `DownloadFile=SB-A-090`; `DeleteFile=SB-A-088` |
| `src/BD.WTTS.Client.Plugins.SteamIdleCard/UI/Views/Pages/IdleCardPage.axaml` | `PriorityRunIdle=SB-A-098`; `ToggleBlacklistIdle=SB-A-099`; `NavAppToSteamViewCommand=SB-A-076`; `AppOpenLink_MenuItem_Click=SB-A-077`; `IdleRunStartOrStop=SB-A-097`; `LoginSteamCommand=SB-A-096` |
| `src/BD.WTTS.Client.Plugins.SteamIdleCard/UI/Views/Pages/IdleSteamLoginPage.axaml` | `Login=SB-A-094`; `CookieLogin=SB-A-095` |

## 12. 覆盖与真正待确认项

静态覆盖以最终 CSV 的 `applicability=in_scope_source_view` 且 command/handlers 非空计算：168 行动作节点，其中129行含命令、40行含handler，1行同时有两者。归一化后170次符号出现、95种command+19种handler=114种符号、123个 `(kind, source, symbol)` 键；逐符号文本检查遗漏0。本文111个稳定ID，其中001–105为当前源及代码补充合同、106–111为候选/非适用边界。`TabView_TabCloseRequested` 已由 CSV 的 Requested 事件识别追加在原 TabView 同一行；它不是额外增加第169个节点。另列代码注册、生成对话框、动态 tray 和内部后继。**静态覆盖不是运行可达性或全流程通过率**。

真正待确认的是：OSL bundled资源每个实际链接实体及本轮未冻结的dirty/递归子模块版本；ScriptStore实际用户route；产品账户/Notice候选route及精简版是否要真正远程更新；未找到live入口的候选方法；厂商SDK取消/查询能力、Steamworks写入确认/同步可观测边界及各平台helper能力。数字deadline已有默认合同，不把“自行选超时”留给实施者；真实平台测量后可有证据调整。

实施验收：每个SB-A至少覆盖正常、同目标重复、失败/超时核对后重试、取消/返回、跨页后台完成、重开恢复；有SID/AppId的动作另覆盖账号epoch改变、窗口仍开与晚回调；FILE动作覆盖坏文件/权限/并发hash冲突；批次覆盖部分失败和取消未提交项。SB-J01–17中的交错动作必须串联测试，不以孤立controller测试或局部编译作为真实验证。静态关联检查可确认本文无漏符号，真实UI/平台/性能测试本轮均未运行。
