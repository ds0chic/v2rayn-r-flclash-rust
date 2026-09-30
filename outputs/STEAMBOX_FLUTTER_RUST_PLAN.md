**SteamBox：Flutter 前端 + Rust 后端完整重构实施方案**

编写日期：2026-10-01。用途：交给执行模型逐项实施。本次只研究本地源码和编写方案，没有修改 SteamBox 源码、运行加速器、切换账号或操作 Steam 数据。

配套文件：`STEAMBOX_SOURCE_INVENTORY.md` 是本地源码与设置索引；`STEAMBOX_EXECUTOR_PROMPT.md` 是可直接复制的执行提示词与任务卡。三份文件一起使用。本方案独立于之前的 v2rayN 方案，可以单独执行。

范围修订：用户明确要求“隐藏插件直接去掉”“只保留截图中的几个”。本方案仅保留网络加速（Accelerator）、账号切换（GameAccount）、库存游戏（GameList）、卡片图标对应的挂卡（SteamIdleCard）四个功能模块。Authenticator、ArchiSteamFarmPlus/ASF、GameTools彻底移出重构范围：页面、设置、初始化、后台任务、命令、专属数据迁移和发布依赖全部不移植。四模块所需公共设置、托盘、Steam登录、权限助手等支撑能力保留。

**1. 目标和完成定义**

用 Flutter 重做桌面界面，用 Rust 实现应用自己的领域逻辑、持久化、任务调度、网络数据面、平台操作和进程管理。保留本地 SteamBox 的界面结构、页面位置、信息密度、设置分组、快捷键、菜单含义和操作习惯，在这些约束内改善字体、间距、图标、材质、反馈和可测量性能。

技术栈已经确定为 Flutter + Rust；“FlClash”是之前的口误。不要 fork FlClash，也不要把 SteamBox 改造成 v2rayN、Clash 客户端、Tauri 或 Electron 应用。SteamBox 的网络加速包含本地反向代理和脚本注入，不能用启动一个通用代理内核代替全部实现。

“完整移植”针对上述四模块及其共享支撑能力：每个适用功能有可达入口或原有后台触发；每个适用设置能保存、重开并产生原有实际效果；保留范围内数据可无损迁移；操作状态和错误可核实；平台支持有逐项证据。已明确删除的三个模块不参与迁移分母。只有界面、只有序列化、只有 mock、只有源码编译，均不算完成。

“最佳性能”以同一机器、同一数据、同一服务端条件下的 Release 实测为依据。“最佳颜值”以保留布局和操作习惯的视觉验收为依据。不能预先宣称达到绝对最优，不能因为 Flutter 或 Rust 的语言名称就宣称性能提高。

**2. 冻结本地版本，不能拿上游 README 代替基线**

| 项目 | 当前证据与执行要求 |
|---|---|
| 项目目录 | `C:\Users\Colby\.gemini\antigravity\scratch\SteamTools` |
| 解决方案 | `WattToolkit.slnx`；本地品牌和构建产物有 SteamBox/Steam++ 混用，执行前核实 |
| 分支/提交 | `develop`；`a7a5ce7f0830b9b759090a75b59d82679084b478`，比 origin/develop 多两个提交 |
| 本地状态 | 39 个 dirty 路径，其中 12 个子模块；另有未提交源码改动，HEAD 不等于实际工作树 |
| 近期本地优化 | 删除冗余代码、ReadyToRun 优化、HarmonyOS Sans SC 字体修复；须以包含这些改动的原版 Release 作性能对照 |
| 框架 | Avalonia/FluentAvalonia + C#/.NET；`global.json` 指定 SDK 10.0.300 及滚动策略 |
| 当前主导航 | Accelerator、GameAccount、GameList、SteamIdleCard 四类插件；实际注册出的子页面另行普查 |
| 删除模块 | Authenticator、ArchiSteamFarmPlus、GameTools原来有源码及初始化路径；依据用户明确要求，新版必须彻底排除 |
| 设置初查 | 原115个顶层字段中ASFSettings 14个退出范围；保留初查101个，另有嵌套字段、实体和默认值；101不是最终验收分母 |
| 本次未证明 | 本地运行截图、全部 Release 构建成功、当前服务端接口可用性、各平台/架构运行成功、性能数值 |

S00 必须记录主仓库提交、修改文件 SHA-256、所有子模块提交及其工作树改动、构建工具版本、目标框架和插件加载清单。元数据可以入规格库；未经检查不要把整份工作树或数据目录发给外部模型。禁止 reset、clean、stash 或覆盖用户已有改动来“恢复干净”。

源码需建立只读冻结副本或可靠快照，包含未提交改动和子模块；重构在隔离目录实施。单独 `git worktree add HEAD` 不会带上工作树修改，不能据此自称复现了本地基线。源码快照不能夹带账号数据库、cookies、密钥、浏览器 profile 或个人缓存。

当前 `APPLICATION_ID` 未提交改动从 `net.steambox.app` 改为 `net.steampp.app`，涉及数据路径、单实例、IPC、协议注册及共存行为。先建立旧标识→实际目录→新标识迁移表，再确定新版本标识。项目名称不能作为推导数据路径的唯一依据。

**3. 完整性台账：先确定分母，再允许实现**

在重构仓库建立以下规格文件，使用稳定 ID，新增发现只能扩充分母，不能通过删除难项提高覆盖率。

| 文件 | 最少内容 |
|---|---|
| `spec/baseline.json` | 工作树摘要、哈希、子模块、Release构建及截图来源、工具链锁定 |
| `spec/features.yaml` | 功能ID、原入口、背景触发、平台、源符号、依赖、正确输出、失败语义、测试证据 |
| `spec/fields.csv` | 字段ID、原名称、类型、JSON/MessagePack键、默认、枚举、作用域、敏感性、UI、保存/实际生效方式、迁移测试 |
| `spec/actions.csv` | 按钮、命令、双击、右键、热键、协议链接、托盘、后台计时、跨页动作；输入和副作用 |
| `spec/layouts.yaml` | 页面、弹窗、窗口、导航位置、像素/逻辑尺寸、DPI、各状态截图、键盘和焦点顺序 |
| `spec/platform-capabilities.yaml` | 平台×架构×功能×原版支持证据×新实现×验证状态 |
| `spec/plugins.yaml` | 被发现/加载/初始化/可见/可达是五个独立事实；含禁用与安全模式行为 |
| `spec/native-contracts.yaml` | Steam接口版本/位数、供应商SDK、WinDivert、ABI、外部协议、故障隔离 |
| `spec/exclusions.yaml` | 三个已删除模块、专属字段/数据/依赖、删除依据、共享依赖保留理由与排除验证 |
| `spec/behavior-deltas.md` | 必要的错误修正或安全改进；原行为、新行为、理由、回归用例、用户可见差异 |
| `spec/evidence/` | 合成数据、脱敏操作轨迹、截图、差分报告、性能数据与测试平台信息 |

每项区分 `navigation_visibility`、`runtime_reachability`、`baseline_applicability` 和 `implementation_status`。实现状态至少有 `discovered / specified / implemented / preserved_only / verified / blocked`。只存住旧字段、不能执行原功能，标为 `preserved_only`；它不能满足完整迁移门禁。

适用性分类：A四模块当前界面能力；B四模块所需共享/后台能力；C本地已裁剪且无当前可达行为；D现有占位；E平台条件不支持；U用户明确删除的三个模块及其专属能力。A/B必须迁移；C/D/E有源码证据；U依据最新用户指令退出移植与验收分母，转为“确保不存在”的排除验收。原有初始化、深链接或后台入口不能推翻U分类。家庭共享等保留模块内部被注释入口仍按原本地事实记录，不擅自恢复。

发现来源要覆盖 XAML、代码后置、ViewModel 命令、服务、定时任务、设置生成器、实体/数据库、序列化属性、插件初始化、CLI 分支、单实例消息、IPC和原生子模块。仅 grep 设置页绑定会漏掉大批高级字段。

**4. 平台与产品范围**

优先在本地主要使用的平台 Windows x64 完成全量闭环。其他平台/架构必须先与冻结版本比较，凡属于原版实际支持且本次承诺的平台，均进入发布门禁；不能把“Windows先做”当作取消后续要求的理由。

| 平台/架构 | 实施原则 |
|---|---|
| Windows x64 | 第一验收平台；覆盖Steam原生接口、系统操作、WinDivert、游戏加速SDK和托盘生命周期 |
| Windows ARM64 | 分别验证Flutter、Steam原生库、驱动和供应商SDK；x64模拟运行不等于ARM64原生支持 |
| Windows x86 | Flutter官方桌面支持表没有原生x86目标；若旧版承诺x86，这是产品范围冲突，明确呈报，不能假装支持 |
| macOS/Linux | 对照源码平台分支迁移可用能力；Hosts、系统代理、托盘和Steam接口单独适配；Windows专属SDK不可编造跨平台替代 |
| Android及其他架构 | 源码或README出现不等于本地已发布支持；必须先核实冻结构建产物、UI基线与Steam相关能力 |

Flutter 的实际平台下限和架构需按实施时固定版本查证并锁定；官方桌面支持以 x64/ARM64 为主，不能据此推导所有第三方依赖也支持。[Flutter 官方支持平台](https://docs.flutter.dev/reference/supported-platforms)

**5. 界面复刻与视觉改进规范**

当前本地主窗源码尺寸为 960×680，最小 528×508；左侧 NavigationView 窄导航约72宽，项目图标28、项目最小高度75、标签约10；内容顶部留约50空间。保留这一骨架，不套用 v2rayN 的表格主界面，也不随意改成大侧栏或移动端底部导航。

启动默认页回退到 HomePage，但构造器会用过滤后的插件 tabs 替换传入 tabs，首页按钮不一定留在导航。S00 用包含当前改动的构建截图确定实际行为，不凭 XAML 或旧 README 想象首页入口。首页欢迎当前 Steam 用户、已安装功能磁贴、标语；插件页和首页磁贴仍根据本地注册顺序与设置生成。

原设置页的五个标签是同一长页面的锚点滚动，不能改成五个独立路由导致保存、滚动、返回习惯变化。其他页面的列表/网格、工具栏、右键菜单、账号“切换”按钮和双击、Steam挂卡列表、加速器列表按原版分别复刻。

视觉改进按三步实施：先灰度复刻几何与行为；再使用统一 tokens 替换样式；最后优化高DPI、深浅主题和动效。tokens 包含字体族与 fallback、文字层级、原有紧凑密度、4/8/12/16/20间距、圆角层级、边框、选中/悬停/禁用/错误/进度颜色、材质强度。具体数值必须由原版截图校准，不把这里的建议当成更改布局的授权。

字体保留当前 HarmonyOS Sans SC 设计意图，核实随包字体许可和字重；中英文、数字、Steam昵称、长路径、RTL文本、100/125/150/200% DPI分别验证。Flutter逻辑像素不能机械等同物理截图像素。字体、边线、图标按DPI正确渲染，不通过全局位图化或模糊缩放掩盖问题。

保持原列顺序、排序语义、行高、图块密度、拖动/选择行为；用户昵称不是稳定主键。悬停才出现的操作保留键盘入口；异步返回不能抢焦点或滚回顶部；执行成功不自动跳走，除非原版就是这样。窄窗优先保留原最小尺寸和已有折叠方式，禁止为“响应式”任意重排桌面操作。

动效建议100–180ms，仅用于状态过渡；支持减少动画。耗时任务立即显示真实任务状态，不能阻塞鼠标反馈，不能为凑动效而延迟操作。视觉验收分开比较几何、字体和材质，允许经过审核的样式差异，不以“完全像素相等”否定授权的美化。

**6. 架构和职责，谁拥有状态要明确**

```text
Flutter桌面UI：渲染、焦点、路由、选择、表单草稿
          │ typed flutter_rust_bridge：命令/分页快照/事件
Rust AppEngine：领域服务、数据库唯一写者、任务与权限编排
          ├── Rust net-host：本地反向代理/TLS/DNS/脚本注入运行状态
          ├── Rust steam-host：按用户/AppId/位数隔离的Steam原生调用
          ├── Rust privileged-helper：Hosts/证书/系统代理/必要驱动操作
          └── vendor-host：已核实的游戏加速供应商SDK
```

| 组件 | 拥有 | 不承担 |
|---|---|---|
| Flutter | 页面草稿、焦点、选择、滚动、可见范围 | Steam文件写入、秘密存储、反向代理流量、后台业务事实 |
| AppEngine | 配置revision、账号/游戏/插件实体、迁移、动作状态、业务持久化 | 高权限常驻GUI、把每个数据包经桥发送 |
| net-host | 已应用的网络revision、监听socket、连接、DNS会话、证书缓存、运行统计 | 直接修改应用主库或接收未校验任意命令 |
| steam-host | 当前Steam用户/AppId上下文、原生句柄和回调 | 一进程随意切AppId、把崩溃传播到界面 |
| helper | 已授权白名单平台动作、系统资源记录与恢复 | 通用shell执行、解析不受限插件代码、持有全部账号数据 |
| vendor-host | 游戏加速SDK运行状态 | 成为隐藏的永久C#应用业务后端 |

网络域用一个专用进程是为了窗口关闭后仍可继续加速、崩溃隔离和权限分离；只在需要时启动。Steam 原生接口按实际上下文需求启动 worker；不要无差别每个游戏常驻一个进程。驱动句柄需要的高权限数据处理归最小特权组件，普通HTTP转发仍在普通权限进程。

Unix的80/443绑定必须有早期技术证明：优先受限helper预绑定并经peer身份校验传递FD，或仅给专用进程最小绑定能力；Windows句柄交接也要按真实实现验证。S03/S22记录支持平台、句柄生命周期、重启/升级/睡眠恢复。不能临时把完整HTTP宿主长期提升权限便结案。

Rust 内部优先模块化单工作区；不为每个模块增加HTTP微服务。进程间用本地有权限控制的IPC，协议带版本、长度限制、session ID、请求 ID；认证peer和安装路径，防止低权限进程伪装命令。网络流量和原生二进制数据不通过Dart事件流绕行。

主应用内业务最终全部由 Rust 承担。旧 C# 代码允许作为四模块的差分测试参照或按需数据迁移工具；如保留能力需要过渡.NET业务host，必须列未完成项和替换任务。游戏加速供应商二进制单独记录边界、版本和许可；已删除模块不允许借兼容host重新加载。[YARP 官方说明](https://learn.microsoft.com/en-us/aspnet/core/fundamentals/servers/yarp/getting-started?view=aspnetcore-10.0)明确它依赖.NET，因此Rust数据面需要迁移行为，不能直接换语言调用同一库。

**7. 技术选择与仓库组织**

| 层 | 推荐候选与决定条件 |
|---|---|
| UI | Flutter stable；Dart页面控制器统一一种状态方案，建议Riverpod；不混用多套全局状态容器 |
| UI桥 | flutter_rust_bridge；首个技术验证锁定Dart/Rust/代码生成器相同兼容版本 |
| Rust异步 | Tokio；同一AppEngine runtime；阻塞文件和原生调用放有界worker |
| HTTP数据面 | Hyper/http-body/Tower及明确TLS适配；reqwest供控制面请求，不把方便的Client自动行为当成透明转发 |
| TLS | rustls及平台验证适配为优先候选；先验证原版目标站/SNI/算法兼容，再固定实现；不能全局关闭验证 |
| DNS | 优先成熟DNS协议实现，保留原始查询/EDNS/IPv6行为；逐项兼容测试 |
| 存储 | SQLite，建议rusqlite+专用DB worker；唯一写者、有版本迁移和未知字段侧袋 |
| 日志 | tracing，结构化错误/操作ID；敏感字段脱敏，所有日志与事件有界 |
| Win32/IPC | windows crate、受限named pipes、Job Objects/子进程生命周期 |
| 原生ABI | 小C/C++ shim仅在ABI确有必要时；Rust封装unsafe；worker隔离；禁止直接仿写未知vtable |
| 图片 | Rust磁盘缓存+Flutter可见图块解码、按显示尺寸缩略图、预算式LRU |

这是一组实施候选，不是已验证兼容清单。S01/S03锁定版本、许可证、feature flags、ABI和打包链，提交`pubspec.lock`、`Cargo.lock`及工具链文件。若候选库不能满足差分用例，替换适配器并记录依据，不删除原功能。官方[Hyper资料](https://hyper.rs/)和[reqwest文档](https://docs.rs/reqwest/latest/reqwest/)分别用于底层HTTP与控制面客户端能力查证。

```text
app/                     Flutter桌面入口、平台runner
app/lib/{shell,design,features,bridge}/
crates/contracts/        版本化DTO、错误、枚举、协议
crates/app_engine/       领域服务、快照、唯一DB写者
crates/storage/          Schema、迁移、旧格式适配
crates/steam_files/      VDF/ACF/Steam目录解析与保守写回
crates/steam_native/     ABI适配和worker协议
crates/accounts/         各平台账号策略
crates/library/          游戏、图片、云文件、成就领域
crates/idle/             挂卡调度和可恢复会话
crates/proxy/            路由、TLS、HTTP、DNS、注入
crates/steam_session/    账号/挂卡Web登录、验证码挑战、Cookie/token
crates/plugins/          四模块白名单注册、禁用、安全模式
crates/platform/         普通权限平台接口
bins/{net_host,steam_host,privileged_helper,vendor_host}/
spec/ fixtures/ tests/ bench/ packaging/ tools/
```

**8. 数据、事件和操作契约**

所有跨桥对象是明确的领域DTO，不暴露SQLite row、不传ViewModel、不每次广播整份游戏库。枚举有显式稳定值；SteamId64等64位ID用无损类型/字符串，不经过可能损失精度的JSON number路径。

```text
CommandEnvelope { request_id, expected_revision?, operation_id?, payload }
Snapshot<T>     { schema_version, revision, epoch, items, cursor? }
Event          { epoch, sequence, entity_id?, changed_fields?, operation_state }
Operation      { id, target_ids, state, progress?, started_at, error?, retry_policy }
Error          { code, domain, user_message_key, retryable, safe_details, operation_id }
```

进程重启递增epoch，序列跳跃触发重取快照；不能错过一个事件就永久显示假状态。分页/搜索有取消和query ID，旧查询结果不得覆盖新查询。排序要与原版一致，稳定ID作为最终次序，选中集合独立于当前可见行。

网络配置至少区分 `saved_revision`、`desired_runtime_revision`、`applied_runtime_revision` 和 `runtime_epoch`。保存成功仅表示持久化成功；启动/重新应用成功需要真实监听、目标规则探测和资源安装结果。原版立即生效设置仍立即请求apply，不增加不必要的全局“保存”步骤。

Steam账号切换/云写入/成就变更不能套用网络revision收敛。用operation ID、用户/AppId/账号上下文、前置检查和实际结果核对；每类动作规定幂等性。外部写入超时结果未知时标记`unknown_result`并重新查询，不盲目自动重试。

状态例：`queued → validating → running → succeeded/failed/cancelled/unknown_result`；长操作进度只有取得真实总量时显示百分比，其他阶段用文字/不定进度。取消要定义可取消点；不可回滚的外部动作不能假装取消成功。

日志批量限频，普通运行统计每秒2–5次足够；关键状态即时。线程/worker/队列都设置容量和超时。Flutter页面dispose只取消页面订阅，不自动停止用户已经启动的挂卡/加速任务。

**9. 持久化和旧数据迁移**

AppEngine是应用SQLite唯一写者；其他进程发结果或领域消息。不要让net-host、Flutter和steam-host各建一套数据库。普通读分页，写事务短小；在锁定文件、原生调用、网络请求期间不持有数据库事务。

通用secret-store从S04提供：实际用户上下文的平台密钥包裹、版本化认证加密blob、受限秘密引用与保护失败语义。保留Steam账号/挂卡Web token、Cookie、代理凭据和本地网络证书私钥的必要保护；不建立OTP令牌库或迁移已删除模块数据。

建议表族：`settings/settings_unknown`、`accounts/platform_profiles`、`games/library_locations/artwork`、`idle_profiles/idle_runs`、`proxy_profiles/rules/scripts`、`steam_sessions`、`plugin_states`、`operations`、`owned_system_resources`、`migration_runs`。秘密只保留引用/加密blob和密钥标识，不以可检索明文散落普通表。

迁移流程仅覆盖保留范围：识别目录和版本→建立只读一致备份→解析到中间模型→逐字段验证→写入新事务→核对数量/值/引用→只读回放→报告→标记完成。可重跑不重复账号、会话或游戏；失败回滚新数据，保留原目录。删除模块的旧库、设置和个人文件不读取、不解密、不导入，也不自动删除。保留模块未知字段策略不得反向导入已明确排除的模块命名空间。

旧SQLite WAL/SHM不能在旧程序仍写入时靠逐文件copy制造一致备份；优先SQLite backup API或确保旧程序退出再复制整组文件。旧设置JSON和MessagePack明确键号、缺省行为和生成器默认值；缺字段、显式null、零值、空列表不混为一谈。新版本无法理解的字段按原路径保留，并允许导出回查，但不能声称其功能已经生效。

目录识别必须复刻WindowsFileSystem中便携/安装模式规则，含根目录AppData、历史Steam++目录、特殊安装路径及用户SID上下文。迁移扫描只覆盖明确候选目录；用户可以指定原目录；不递归遍历整个磁盘搜索秘密。

DPAPI保护的保留数据要在可解密的原用户/机器上下文内迁移，不能让管理员helper变成另一个用户而丢失密钥。跨机器导出使用可验证保护格式；Steam登录token/Cookie、代理凭据、云代理token和证书私钥不能写入日志。[微软DPAPI资料](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)用于核实保护作用域。

所有测试用合成账号和令牌。每组字段至少验证旧→新、缺省→新、保存→重开、未知字段→再次保存、损坏输入→可解释失败。实际用户数据迁移是实施阶段的明确动作，本方案研究没有读取用户账号库。

**10. 四模块白名单与隐藏插件彻底移除**

只将Accelerator、GameAccount、GameList、SteamIdleCard移植为Rust领域模块+Flutter页面注册器。保留四模块标识、顺序、设置、disable/safe mode和初始化语义；插件管理页仅显示这四项，计数与首页磁贴同步。原名称/GUID按台账固定，禁用和安全模式不等于允许添加其他模块。

删除三个模块的项目引用、页面/资源、服务注册、GetConfiguration、初始化/退出钩子、托盘项、CLI/URI/路由和专属定时/网络任务；发布包与更新清单不带这些程序集、SDK/ASF资源或专属数据。新注册器严格允许四项，不兼容加载其他旧.NET/Avalonia插件，不能被残留modules目录、深链接或更新恢复绕过。

依赖按反向引用图裁剪：ASF、OTP令牌库、交易确认、游戏窗口工具/VAC修复等专属能力全部删除。SteamClient/WinAuth等名字可能同时承载挂卡Web登录模型、验证码挑战、Cookie/token，不能按目录名整包误删；只保留四模块实际用到的最小共享部分并记录理由。CA私钥/DPAPI、Cloud/成就/AFK、权限helper、WinDivert、迅游SDK仍在保留范围。

排除验收：构建引用与打包产物无删除模块；旧默认页指向删除模块时回退到原Home/有效四模块；旧URI/命令返回明确不可用，不加载程序集；启动/托盘/插件页/日志中无其初始化、定时器或后台请求。源码冻结副本只用于研究，不能混入新版可执行资源。

**11. 社区加速：Rust要迁移的真实数据面**

原链路为 Kestrel → 自定义HTTP/CONNECT/TLS中间件 → YARP `IHttpForwarder` → 自定义出站连接。Rust实现至少拆成`IngressClassifier / RuleEngine / DomainResolver / UpstreamConnector / TlsPolicy / Forwarder / ScriptTransformer / LocalScriptBridge / FlowMeter`，不能写成难以验证的单个巨类。

| 能力 | 必须保留的语义与验证 |
|---|---|
| 规则匹配 | 多层/递归域名规则、匹配/排除、目标URI、固定IP、ForwardDestination、HTTP→HTTPS、UA模板、静态响应；同一请求固定规则snapshot |
| 正向HTTP/CONNECT | absolute URI、authority、IPv6、分片请求解析；命中站点按原语义处理，其他透传；双向背压、半关闭、取消、连接候选回退 |
| 反向HTTP | HTTP/1.1与HTTP/2，上传/下载/Range/206、SSE、WebSocket、gRPC/trailer、100-continue、重复Set-Cookie及hop-by-hop头处理；适用的RFC8441 Extended CONNECT及HTTP1 upgrade↔HTTP2 WebSocket转换单列测试 |
| HTTP/3 | 原源码有协议声明，先验证是否实际活跃；若适用单独实现和验收，不能降级到HTTP/1后称全部兼容 |
| 出站 | 原域名、拨号IP、HTTP Host/:authority、TLS SNI、证书验证身份分别建模；出站HTTP CONNECT/SOCKS4/SOCKS5、用户名密码 |
| DNS | 系统/自定义DNS、DoH、IPv6、A/AAAA/CNAME、TTL/负缓存、失败回退、并发去重；Hosts/DNSIntercept时禁止出站解析回本机 |
| 授权云代理 | 复刻X-Watt相关目标头/token契约；token由秘密句柄取得；服务端不可用、过期或拒绝时报告实际失败 |
| 统计 | 原瞬时/累计流量、单位、重置、连接状态、请求日志；Rust汇总限频，不发送每包事件 |

代理普通响应保持流式，不自动跟随重定向、不共享网站cookie jar、不自动解压后误改变转发。脚本xhr需要的cookie行为独立管理。控制面HTTP代理、本地系统接入代理、出站二级代理是三个不同用途，必须各自保留认证、DNS和循环防护设置。

云代理保留头是控制输入：删除或覆盖客户端传入的保留`X-Watt-*`字段，仅向批准的云代理目的地附加token；目的地址改变/跨origin重定向不得泄露凭据。伪造控制头和错误出口列入测试，不能直接照搬TryAdd语义。

连接池身份至少包含目标/路由、Host、SNI、验证策略、二级代理和HTTP策略。规则更新时使受影响池失效，不能只按域名复用导致凭据或TLS策略串用。DNS、证书、规则和脚本缓存都设置预算和失效条件。

本地发现：二级代理TLS callback存在无条件接受证书的实现，另一名称不匹配分支也可能忽略其他链错误。重构保留规则的SNI/名称例外用途，独立验证链、期限和用途；不复刻全局放弃验证。以`behavior-deltas`记录这项修正，用正确链/错误链/过期/名称例外四类夹具证明行为。这里是迁移注意项，不代表已经完成安全审计。

现有`Socks5ProxyEnable/Port`只有设置/DTO证据，尚未找到本地SOCKS5 listener；SSH/Git listener调用有注释；不能将“出站SOCKS5支持”写成“入站SOCKS5已可用”。S00标为行为待核实，按实际基线确定迁移或明确缺口，禁止交付一个无效开关。

**12. 网页脚本与证书：不能遗漏的兼容细节**

脚本管理包含本地/商店脚本、下载与更新、启停、Match/Exclude、排序、数据缓存、OnlyEnableProxyScript、Steam浏览器UA限制、本地脚本与xhr桥。它是一组完整功能，不是简单替换HTML字符串。

原注入条件包括GET、200和HTML；支持gzip/deflate/br、字符集/BOM、脚本顺序及GitHub特殊插入点。Rust先准备原始响应可回退路径，再在有界内存中转换；限制解压倍率、总大小、并发与时间。超界时正确透传并给诊断，不能截断网页。HEAD、206、SSE、WS、gRPC和非HTML不注入；改变body后更新长度、压缩、缓存验证相关头。每种场景用金样页面和浏览器执行结果验证。

可执行回退算法：在决定转换成功前不提交变换后的头或body；读原压缩数据时保留有容量上限的原始前缀，再供受限decoder处理。原始前缀、解压量或时间触及上限，停止转换，先回放全部已读原始字节（含read-ahead/current chunk），再流式透传未读原流。小响应完整转换后才提交新头和body。两套buffer及并发总预算独立计数；已提交混合body后不能声称还能回退，不能为了回退先无限读完整响应。

原实现有CSP移除逻辑。对确需脚本功能的规则逐项验证策略调整，优先精确源/nonce/hash；不要给所有页面无条件删除CSP。网页脚本桥保留原API和必要cookie语义，但与管理IPC分离，不暴露文件、账号秘密或系统操作；用脚本ID、启用能力、Origin和允许目标校验。

CA与leaf生命周期包括生成、信任安装、过期/轮换、公有证书导出、原有PFX等操作和GOG插件公有CA写入。按当前适用性迁移对应按钮及数据格式。私钥保护、信任scope、证书fingerprint必须明确；停止加速与移除信任是不同动作，不每次停止都清空证书库。

只为启用规则允许的目标生成leaf，未知SNI、无SNI、IP、CONNECT与SNI冲突分别定义行为。应用pinning/mTLS可能限制兼容，不以关闭应用验证冒充解决。GOG certifi bundle只合并自有段，记录原摘要、升级冲突和逐文件恢复；不能覆盖整个证书包。研究与普通自动测试采用私有测试CA，不直接装进个人系统信任库。

**13. 系统接入：Hosts、PAC、系统代理与DNS拦截**

接入模式与HTTP数据面是不同层。Hosts/DNSIntercept将站点指向本机TLS服务；System/PAC通过HTTP proxy listener/CONNECT接入。PAC命中规则与DNS/路由使用同一revision，域名模式正确转义，非命中保留DIRECT语义。

Hosts只能映射地址，不能携带端口。原源码有443向后找空闲端口的行为；重构必须保证Hosts模式实际443可达，或实现且验证明确端口重定向。端口冲突不能偷偷换成444后显示启动成功。

Windows DNSIntercept依赖WinDivert和构建条件；UDP53的DNS拦截不等于覆盖DoH/DoT。完整验证IPv4/IPv6、查询/回复校验、TTL、回环排除、driver句柄释放、停止时恢复。Rust FFI复用经过核实的签名驱动，不为“全Rust”重写内核驱动。[WinDivert官方仓库](https://github.com/basil00/WinDivert)说明其用户态捕获/修改/回注结构，驱动与平台依赖必须单独记录。

系统操作journal记录operation、实际用户SID/UID、旧值/是否存在、自有Hosts段、写入目标、CA指纹与store、GOG文件摘要、driver会话和已完成步骤。恢复采用比较当前值与本次写入值的方式，只撤自己的改动；用户或其他代理改变后的值不能覆盖。Hosts保留未知条目、编码、BOM和换行，并处理并发编辑。

启动：`Stopped → Validating → PreparingListener → PreparingTrust → ApplyingIngress → Running`。先有真实socket和探活，再写系统接入。停止：`RemovingIngress → Draining → Stopped`。撤接入失败时保留服务或明确“需要恢复”，不能让Hosts继续指向死端口却说已停止。任一步失败都有补偿；异常退出/重启从journal恢复自己的遗留变更。

Windows HKCU代理在实际桌面用户上下文读写；提升权限worker的HKCU不一定是同一用户。保存ProxyEnable/Server/Override/PAC及原存在状态，停止恢复原值。macOS按每个network service保存；Linux区分GNOME gsettings与其他桌面，不能声称对所有Linux应用有效。

helper仅接受枚举白名单命令、预定义路径和结构化输入，拒绝任意shell/注册表路径/DLL。进程管理核对pid+启动时间+路径+session，只终止本程序拥有的worker。UAC取消、端口占用、只读Hosts、其他代理介入、睡眠唤醒、关机都要有可重现失败测试。

**14. 游戏加速SDK是独立能力**

迅游游戏加速与社区反向代理分别建模：安装/卸载、客户端或服务路径、厂商账户/权益、游戏与区服、加速/停止、测速、显示窗口和启动游戏。写出Rust HTTP代理不会自动获得游戏UDP加速。

本地SDK源码有Windows x86/x64 OS架构判断、不同DLL、AppId启用条件。S03核对合法SDK来源、版本、签名、调用约定、字符串编码、结构packing、callback线程与释放、授权和可分发条件；没有真实SDK/权限，状态是blocked，不能用demo值或模拟测速报完成。

SDK放隔离vendor-host，安全路径加载，回调转operation事件；callback不能直接调用Flutter。超时不等于SDK已停止，重连查真实状态；上次持久化“加速中”不能成为当前状态。测速的延迟、丢包和线路语义要与原版一致，原启动游戏动作复用实际路径/参数检查。

**15. Steam账号、多平台账号与家庭共享**

Steam账号切换按本地代码及子模块改动复刻：稳定SteamId64/AccountName匹配、MostRecent/RememberPassword/AllowAutoLogin/AutoLogin/Timestamp等写入、登录缓存更新、进程退出等待、重启/登录结果。原dirty修改处理了名称匹配和等待顺序；用明确进程退出与文件就绪状态替代任意300/200ms睡眠，不丢失其正确性目的。

每个平台独立策略接口：`inspect / backup / validate_target / stop_verified_target_client / apply / launch / verify / compensate`。切换操作允许管理用户当前操作授权的、路径/PID/身份明确验证的Steam/Epic等目标客户端，包括用户自行启动的客户端；本应用worker的后台自动清理严格限制自身所有权。Steam、Epic、Ubisoft等只有源证据对应能力，不能用同一目录copy算法假装通用。使用平台设置里的路径、注册表/安装发现、关联进程和备份格式；多平台独立恢复、离线和错误路径反馈。

账号卡顺序、当前账号状态、显式“切换”与双击、头像、重命名/移除/备份等按actions台账迁移。防重复点击依靠单一operation与原反馈，不让用户连击触发两次关客户端。外部Steam登录失败时显示实际结果，不能只写VDF就把目标账号标成已登录。

VDF/ACF必须支持重复键、未知字段、大小写语义、文本/二进制KeyValues、编码、换行与类型；以最小patch/可证明等价写回，原子替换前备份。写前比对文件版本/摘要与当前Steam写入状态，禁止覆盖并发变化。路径支持Unicode、多个库、自定义磁盘和符号链接边界。

本地家庭共享页入口被注释，代码使用旧AuthorizedDevice/VDF模型；不能宣称支持当前Steam Families完整服务。保留本地可达行为和数据兼容，当前不可达部分分类留证。若未来新增新Families，另列新需求，不把它计入已有功能迁移进度。

**16. 游戏库、下载、云文件与成就**

游戏库迁移要覆盖本地扫描/缓存、搜索过滤排序、列表和网格、封面/自定义图片、显示名/备注/分类等已有实体、启动/安装/卸载/查看目录/打开相关网页、下载管理、下载结束动作、Steam设置联动。实际动作以源码索引和运行台账为准；不能只实现游戏列表。

库扫描做分页增量，文件watcher合并去抖，整个扫描操作可取消；下载运行状态来自真实Steam接口/文件状态，不使用定时随机进度。关机等后续动作按原设置时机触发，测试用平台adapter拦截并在隔离环境验证，普通测试不得关当前机器。

云存档/成就原来通过按AppId启动进程和Steam原生上下文访问。Rust steam-host要隔离用户/AppId/位数/接口版本，准确初始化、callback泵、释放顺序及退出。不能在一个进程中同时随意改Steam AppId，也不能默认某个steamworks crate覆盖私有接口。

具体调用依据是`ref/SteamClient/src/BD.SteamClient/Services.Implementation/SteamworksLocalApiServiceImpl.cs → SAM.API.Client`；ABI初始化在嵌套`ref/SteamClient/ref/SteamAchievementManager/SAM.API/Client.cs`，当前使用SteamClient018、pipe/global user和Apps/UserStats/RemoteStorage等接口。顶层Steam4NET/Facepunch存在不证明它们就是此功能的当前绑定入口，必须沿实际调用链移植。

官方Steamworks文档用于理解SDK上下文与Cloud API，但不证明任意客户端可以修改任意应用数据。现有Steam4NET/SteamClient等私有接口必须以本地接口版本、合法测试账号/应用和目标Steam版本实测。[Steamworks SDK](https://partner.steamgames.com/doc/sdk)、[ISteamClient](https://partner.steamgames.com/doc/api/ISteamClient)、[ISteamRemoteStorage](https://partner.steamgames.com/doc/api/ISteamRemoteStorage)

云操作分别列读取元数据、下载、上传、覆盖、删除、冲突/配额/离线、用户/AppId变化和未知结果；删除与写入不得因网络超时自动重复。成就读取、状态编辑、批量操作、提交/重置按当前原版可用语义迁移，实际写入验证用合法测试应用，不能拿空callback成功作证据。

自定义图片保存原路径与新缓存映射；生成显示尺寸缩略图，懒加载与有界解码。用户原文件不变成仅临时cache，也不因LRU淘汰而丢失。下载/云文件/图片的路径规范化和文件句柄生命周期必须统一。

**17. 挂卡：保留完整调度与Steam登录**

| 模块 | 全量移植重点 |
|---|---|
| SteamIdleCard | 账户/应用上下文、掉卡数据获取、挂卡队列/模式/顺序、启动/停止/暂停/恢复、运行时长与剩余卡、自动行为、页面关闭后继续、Steam退出/断网/换账号 |

挂卡是独立调度域，保持用户/AppId切换约束和worker生命周期，禁止为一个账号同时启动相冲突会话。运行记录与选择配置分离；重启确认Steam真实状态后恢复，不能仅凭上次标记认定仍在挂卡。调度定时使用单调时钟，系统时间改变不让统计跳变；远端时间/日期展示按原时区语义。

SteamIdleCard独立注册Steam账号服务，登录使用用户输入的2FA/邮箱验证码挑战、Cookie、AccessToken/RefreshToken。删除认证器插件不删除这些登录流程。支持记住/退出、token刷新、错误账号、过期、限流；不提供OTP自动生成、手机令牌绑定/解绑、交易确认或令牌云同步。

**18. 设置迁移：保留范围101字段只是起点**

| 字段族 | 初查顶层数 | 需覆盖的后端效果 |
|---|---:|---|
| GeneralSettings | 23 | 更新渠道/检查、自启/最小化、托盘通知、缓存、目录/编码、GPU/原生渲染、插件禁用、安全模式、本应用HTTP代理及凭据 |
| UISettings | 19 | 主题/强调色/语言/字体、窗口位置、列表密度、圆角、背景材质/透明/动态/图片及提示选择 |
| SteamSettings | 11 | Steam扫描/启动/修改相关行为、显示与自动项；保留每个具体字段作用 |
| ProxySettings | 26 | 社区加速模式、端口/地址、脚本、DNS/DoH、二级代理、GOG、Steam浏览器限制、启动项 |
| GameAccountSettings | 5 | AccountRemarks、DisableAuthorizedDevice、EnablePlatforms、PlatformSettings、IsShowAccountName；平台路径另有嵌套字段 |
| GameLibrarySettings | 7 | 已安装/云存档/类型筛选、布局、隐藏游戏、AFK列表及自动AFK；排序等其他状态继续查实际来源 |
| SteamIdleSettings | 8 | 挂卡选择、模式/规则与定时行为 |
| GameAcceleratorSettings | 2 | 游戏/区服选择数据、客户端路径；MyGames模型字段继续递归盘点 |

保留8组共101顶层字段：23+19+11+26+5+7+8+2。原ASFSettings 14字段全部取消读取/注册/UI/迁移/效果验收，不能塞进settings_unknown重新导入。完整保留字段见源码索引；继续普查四模块嵌套实体、动态字典、枚举和默认代码。partial别名不重复计数；保留字段历史拼写如CustomDohAddres2保持兼容。DisablePlugins/SortMenuTabs/StartDefaultPageName等共享集合中的删除模块ID忽略并规范化，其他有效值保留。

每个字段写明：加载/旧缺省、草稿校验、保存、触发时机、应用确认、失败回退、重启要求、导入导出、敏感属性和平台条件。UI渲染设置即使改为Flutter新实现，也要保留旧值与可解释映射；例如GPU/NativeOpenGL不能只留下无效开关，要定义Flutter等价渲染/软件路径与必须重启行为，无法等价项列差异和阻塞。

配置编辑和生效验证分两阶段：先完成schema/导入/页面编辑；后续对应业务与平台模块完成后再验证效果。台账在第二阶段之前不标verified。某个高级字段原本无实际实现，留原事实分类，不能为了“每个字段都有效”编造旧行为。

**19. 顺畅的前端操作与生命周期**

窗口生命周期不能只看Program初始OnExplicitShutdown：App.TrayIcon会再次设置，TrayIcon=true时主窗X取消关闭并隐藏、显式退出才结束；TrayIcon=false时OnMainWindowClose，主窗X真正退出并执行后台清理。当前dirty普通手动启动应显示窗口，显式`-silence`才最小化；MinimizeOnStartup旧值保留并按启动来源解释，不能无条件覆盖手动启动。托盘开/关×普通/silence四格均验收。后台加速/挂卡不因页面dispose消失。单实例第二启动通过IPC唤回窗口；已有窗口缺失/最小化的恢复路径和焦点处理按原体验重做。

点击切页沿用原导航语义；本地侧栏`useCache:false`清BackStack的行为需要验证用户感受后准确保持。Flutter可以只保留低成本页面草稿和必要快照，不让无限缓存页面造成内存增长。返回、模态窗口、深链接、任务完成回页的顺序写成actions测试。

前端为每个操作提供一次明确反馈：立即接受/拒绝、可取消阶段、具体失败原因、真实完成。只禁用会冲突的目标操作，不把整个页面锁住。输入法组合态、Tab顺序、Enter/Escape、快捷键冲突、菜单焦点、文件选择返回、双击和滚动分别覆盖。

原本有二次确认的外部破坏性操作保留目标和范围；不要给普通开关、查看详情、读库增加额外弹窗。下层服务失败用可恢复状态和原页面提示承接，不靠无限toast，也不把错误吞成空列表。

**20. 关键格式和状态的实施补充**

Steam格式层至少覆盖文本loginusers/config/localconfig/libraryfolders/ACF、非Windows registry.vdf、三种magic的二进制appinfo及V3 string pool、UserGameStatsSchema、MessagePack modifications.vdf与`.stmbak`。不能把所有`.vdf`当文本：本应用modifications.vdf实际是MessagePack。具体路径、字段和magic见源码索引。

游戏元数据编辑存在“保存到应用overlay”和“批量应用到Steam appinfo”两步；封面Grid/Logo/Hero/Header可来自文件及SteamGridDB，要落到正确SteamId32目录。保留保存/取消/重置和`.stmbak`备份。旧写appinfo备份曾被注释，新实现写前备份、复读验证和冲突检测列入行为改进。离线缓存含有某游戏不等于当前账号拥有它；所有权来源明确。

GameList AFK源码上限32与IdleCard默认并发30不同。先用小型Rust worker替换每AppId完整GUI进程，再研究合并，不能先假定单一Steam连接等价。相同AppId worker若共享，明确引用计数和任务所有权，停止挂卡不误停用户游戏或另一AFK任务。

IdleCard至少测试FastMode/OnlyOneGame/OneThenMany/ManyThenOne × Default/LeastCards/Mostcards/Mostvalue，保留旧枚举数字和拼写；IdleTime默认6分钟、MinRunTime默认2小时、SwitchTime默认5000ms等单位明确。徽章HTML解析需判断合法完整页面，登录页/限流/网络错误不可解释为空队列已完成；价格失败只降级价值排序，不取消整个挂卡能力。Web session与Steam原生SteamId必须相同。

本地token加载有“仅有效token进入分支，再判断过期刷新”的逻辑风险。Rust新状态用Valid/ExpiredRefreshable/RequiresLogin并做合成过期测试；账号epoch贯穿异步结果，换账号时旧callback不得污染新会话。SteamLoginState本地从MemoryPack改为MessagePack，调用者与版本需追踪，不能盲猜格式。

保留Steam登录会话和游戏备份格式的兼容工具只处理四模块数据；Rust等价覆盖前允许按需使用，不成为常驻业务后端。已删除模块的OTP/旧vault/WinAuth令牌备份导入不实施。Steam登录所用WinAuth类型与协议如仍被引用，应最小化保留并标共享用途。

下载完成电源操作的原通知写30秒，但读取的方法没有证明真实可取消等待。新版本实现可取消倒计时、到期重新确认完整下载快照，列入行为差异；断网、manifest半写或列表短暂空不能触发电源操作。云配额用u64，区分API写入成功与真正同步完成。成就int/float范围和protected标志保留。


**21. 性能预算与测量方法**

先测冻结原版包含本地字体/ReadyToRun/GC/GPU改动的Release。原Program将GPU资源预算降为64MiB的改动必须纳入对照；不能拿未优化上游Debug当基准。原版与新版都按四模块等价负载测量，另记录原版隐藏初始化额外开销。新版覆盖UI、Rust库、net-host、steam-host、helper、SDK及所有子进程，不通过拆进程隐藏内存，也不把删除功能的收益包装成Rust本身更快。

| 指标 | 初始验收目标；S00/S32确认测量条件 |
|---|---|
| 冷启动 | 原版同数据p95至少改善20%为目标；并且首窗可交互p95≤1.5s；不足时记录瓶颈和差异，不报已经达到 |
| 热启动/唤回 | 窗口可交互p95≤350ms；单实例IPC不丢命令 |
| 输入/导航 | 普通反馈≤100ms；缓存数据切页p95≤150ms；I/O任务异步反馈不等待实际完成 |
| 渲染 | 固定60Hz预算16.7ms；库快速滚动/多选UI与raster p95均≤8ms为目标，99%以上帧不过预算；120Hz另记录 |
| 搜索过滤 | 本地1万游戏，包含桥和首屏渲染p95≤120ms；旧query可取消；1千账号作为压力样本，不假定普通用户都有此数据 |
| 常驻资源 | 空闲总CPU与私有提交内存不高于原版为硬门槛，争取总内存下降20%；记录RSS/working set/commit，不能靠trim制造假下降 |
| 代理数据面 | 相同协议/路由/TLS/脚本条件下吞吐不低于原版95%，附加延迟p95不高于原版+1ms为初始目标；实际公网抖动不用作微基准 |
| 大文件 | 1GiB代理下载/上传为流式，内存不随body线性增长；云接口若只能有界buffer需明确限制与测试 |
| 长运行 | 8小时常用组合和24小时后台场景，进程/句柄/连接/缓存无持续增长，停止后资源回到可解释基线 |

绝对值是工程目标，不是本次实测成绩。硬件不同可在任何优化实现前冻结等价条件与预算修订依据；禁止看到失败后降低指标或挑最优单次成绩。外部API限流和Steam登录耗时单独计，不把它们塞进纯UI数字，也不能在端到端报告中隐藏。

优化顺序：先正确性和测量→启动关键路径→虚拟列表/网格→分页快照和桥调用批次→图片缩略/缓存→有界解析和DB查询→小型Steam worker→连接/证书/DNS池→Release链接/打包。先分析profile，再引入SIMD、自定义内存池或复杂zero-copy。

图片初始预算：仅可见区及一屏预取；内存解码cache64MiB起步，GPU资源以64MiB对照目标测试，磁盘cache可配置且有总量/LRU；预算不足以维持原页面体验时记录测量后调整，不靠清空用户图片解决。保留模块日志采用有界ring及明确预算，统计不超过必要频率。

每项测试预热、冷缓存定义、输入数据hash、运行次数（启动至少20次、微基准至少30轮）、p50/p95/p99、峰值资源、环境与版本均入JSON/CSV。Flutter看DevTools timeline，Rust用对应平台profiler与tracing跨度；Release/profile mode下测，Debug只用于定位。[Flutter性能实践](https://docs.flutter.dev/perf/best-practices)、[Flutter帧性能资料](https://docs.flutter.dev/perf/ui-performance)

**22. 验证矩阵：差分、真实闭环与故障注入**

| 测试组 | 代表用例和真实完成依据 |
|---|---|
| 基线/UI | 四类主导航及注册子页、首页有无入口、五锚点设置、深浅主题、4种DPI、窗口/托盘/焦点/双击/返回；截图+实际操作轨迹 |
| 字段 | 保留范围至少101种子字段及后续发现项，默认/零值/null/未知/枚举/秘密引用，导入保存重开后实际生效；ASF14字段及删除模块命名空间不注册/导入 |
| 文件 | Unicode/重复键/未知字段、旧新库格式、3代appinfo、损坏/截断/长度炸弹、stats与stmbak、外部并发写；读写差分+fuzz |
| 账号 | A/B名字大小写差异、离线与Persona、Steam退出超时/需登录、错误SID、文件拒写；真实SteamId验证+补偿结果 |
| 库/下载 | 所有过滤排序布局、编辑两阶段、四种封面、AFK、全选三态、半写manifest/断网不关机、倒计时可取消 |
| Steam ABI | 固定接口版本/位数、Steam升级不兼容、callback超时、用户/AppId错配、worker崩溃/Steam退出；主UI稳定和明确错误 |
| Cloud/成就 | 合法测试应用的读写/重名/配额/删除/未知结果、protected stats、int/float、批量与重置；mock与真实证据分别标记 |
| 挂卡 | 4算法×4排序、30并发、手动优先/下一、黑名单/私有、HTML改变、价格降级、身份不符、睡眠唤醒、恢复 |
| HTTP | status/重复headers/Set-Cookie、302、Range、chunk/trailer、大body、SSE/WS/gRPC、CONNECT分片/半关闭、HTTP1/2及适用HTTP3 |
| DNS/TLS | A/AAAA/CNAME/NXDOMAIN/TTL、DoH bootstrap、回环排除、SNI/Host/IP/身份分离、坏链/过期/名称例外、缓存/轮换 |
| 注入/脚本 | GET200条件、UA、Match/Exclude/排序、各压缩/编码、超大HTML回退、GitHub插入、CSP、XHR origin/目标/cookie、更新失败 |
| 系统接入 | UAC取消、80/443冲突、Hosts编码/并发、其他代理介入、每阶段崩溃、恢复journal、owned CA/GOG、WinDivert释放 |
| SDK/远端 | 合法SDK安装/权益过期/区服/回调/测速/断线、云代理拒绝；没有服务条件就是阻塞，不报通过 |
| 登录/秘密 | 账号/挂卡验证码挑战、Cookie/token刷新、DPAPI作用域、异机保护失败、账户epoch、日志脱敏；不包含OTP令牌模块 |
| 移除门禁 | 严格四模块，构建/包/路由/托盘/设置无删除模块，旧modules/默认页/URI不能复活，启动无其后台请求/定时器，四模块共享能力不被误删 |
| 生命周期 | 页关闭/主窗关闭/退出分别验证；托盘开/关×手动/silence四格，第二实例、后台继续、升级/卸载恢复、系统重启、sleep/wake |
| 资源 | 四场景总进程内存CPU、10k库滚动、1GiB流、大量回调、8/24h、失败后无残留资源 |

测试分层：纯Rust格式/算法/状态机→隔离本地HTTP/Steam协议fixtures→Flutter widget/交互→跨桥端到端→隔离VM系统接入→合法外部服务与真实平台。差分可以比较结构化结果、HTTP字节语义、编辑后文件、动作状态，不要求时间戳和随机operation ID相同。

Flutter集成测试不覆盖所有原生窗口/权限/托盘，需要平台测试和人工验收补齐，不能用integration_test全绿声称UAC与驱动都通过。[Flutter集成测试说明](https://docs.flutter.dev/cookbook/testing/integration/introduction)

操作会改系统或Steam数据的测试使用已授权隔离VM、测试账号和可恢复样本。普通CI用合成fixtures并阻止真实电源/外部写入；这些mock是测试隔离，绝不是产品功能的替代。测试证据明确记录实测、隔离模拟、尚未验证三种状态。

**23. 实施任务依赖：按闭环推进**

每行是一个里程碑，执行模型还需拆成一条用户流程、1–3功能ID的小任务卡。依赖均指前置实际完成，不是“文件已经创建”。高风险技术验证在早期完成；设置生效验收在相关业务完成后执行，避免循环依赖。

| 任务 | 前置 | 产物与退出条件 |
|---|---|---|
| S00 冻结与安全基线 | 无 | 工作树/子模块快照、构建宏、Release匹配、只读研究清单；个人研究环境不运行会自动起管理员代理的原GUI，运行截图在隔离授权环境获取 |
| S01 工具链/桥最小验证 | S00 | Flutter↔Rust typed调用、流/取消/错误、打包和版本锁；没有业务完成声明 |
| S02 四类台账与范围 | S00 | 四模块features/fields/actions/layouts+platform/plugins/exclusions；101字段种子扩成完整保留清单 |
| S03 难点技术验证 | S00,S02 | Steam ABI只读worker、登录会话/游戏备份双格式与保护、SNI身份分离、低端口FD/句柄、驱动/SDK、共享依赖裁剪；各自pass/blocked |
| S04 契约与持久化 | S01,S02 | DTO、epoch/revision/operation、SQLite唯一写者、通用secret-store、原子迁移与事件gap恢复 |
| S05 主壳与生命周期 | S04 | 单实例、导航、窗口/托盘/退出及四格启动语义；已实现AppEngine状态、未初始化业务准确显示不可用；geometry golden |
| S06 Steam格式只读/写回库 | S02,S04 | VDF/ACF/appinfo/stats/MessagePack corpus、未知保留、冲突/备份；纯样本不碰个人Steam |
| S07 首条真实纵向链路 | S05,S06 | 合成Steam目录→Rust扫描→SQLite→分页桥→Flutter原布局列表→重开；排序过滤可验证 |
| S08 视觉与交互规范 | S05,S07 | 原布局tokens、字体/DPI/主题、焦点/键盘、操作状态；先几何再美化 |
| S09 全设置schema/旧数据读取 | S02,S04,S03相关格式验证 | 每字段类型/默认/Key/未知项与旧→新测试；尚未实现效果仍标未验证 |
| S10 设置页阶段A | S05,S08,S09 | 原五锚点/插件设置/高级字段草稿、校验保存重开；不宣称业务效果已完成 |
| S11 Steam worker完整协议 | S03 Steam验证,S04 | native能力表、固定上下文、回调、身份/崩溃隔离、进程所有权 |
| S12 Steam账号切换 | S05,S08,S06,S09,S11,S22权限能力 | 原账号卡/双击/托盘/URI→单切换事务，本地dirty语义、身份核实、快捷方式、文件/注册表补偿 |
| S13 多平台账号 | S05,S08,S06,S09,S12 | 原平台tab/路径/菜单→Platforms.json各有效适配器、缓存迁移/恢复、错误、平台范围证据 |
| S14 库编辑/备份/AFK | S07,S08,S09,S11 | 原库/编辑/挂机页面→两阶段编辑、封面、stmbak、隐藏、AFK worker与所有权 |
| S15 下载与后续动作 | S07,S09,S11,S22电源adapter | 原下载页→真实下载状态、选择/取消倒计时、异常不触发动作、隔离实测 |
| S16 Cloud与成就 | S07,S08,S06,S11,S12 | 原库入口/独立窗口→合法应用真实读写、账户/AppId隔离、配额/受保护统计/未知结果 |
| S17 Steam Web会话 | S05,S08,S03格式验证,S04,S12 | 原登录弹窗→挑战、Cookie/token状态、刷新/限流/epoch、秘密保护 |
| S18 挂卡调度 | S05,S08,S09,S11,S17 | 原挂卡页→徽章parser、4×4模式、并发/黑名单/优先、后台恢复和真实会话 |
| S19 HTTP/CONNECT数据面 | S03网络验证,S04,S05,S08 | Rust net-host，本地受控HTTP1/2转发、Extended CONNECT/WS/SSE/gRPC/大流/取消；原社区页服务状态/日志/统计接入，系统接入仍未启用 |
| S20 路由/DNS/TLS | S09,S10,S19 | 原规则/网络检测/代理设置→规则差分、DoH/IPv6、SNI身份、池/cache；适用HTTP3另独立gate |
| S21 脚本/本地桥 | S20 | 原脚本页/商店→增删更新启停、注入/压缩/编码/GitHub/CSP、xhr权限与cookie兼容 |
| S22 权限helper与journal | S04,S05,S03相关平台证明 | 提前并行：typed白名单、已核实客户端停止、电源adapter、低端口/句柄、身份/权限、资源快照/补偿/恢复；先fake故障覆盖 |
| S23 实际系统接入 | S20,S21,S22 | 原社区页启动/停止/模式/证书/Hosts动作→隔离VM的Hosts/PAC/System/证书/GOG/WinDivert与崩溃恢复全闭环 |
| S24 游戏加速SDK | S03 SDK验证,S05,S08,S09,S10,S22 | 原游戏/区服/安装/测速/窗口动作→vendor-host、合法SDK/权益、启停/启动游戏与真实失败 |
| S30 四模块注册与删除验证 | S03依赖裁剪,S05,S09,适用的S12–S24模块 | 四模块加载/禁用/安全模式/初始化；删除模块构建/打包/路由/后台零残留 |
| S31 设置阶段B与全流程联调 | S10,S30,所有适用的S11–S24业务子卡 | 保留字段真实效果、保存/失败/重开/回退；四模块actions端到端证据 |
| S32 性能优化 | S07基准采集,最终S31 | 场景profiles、预算、总进程测量；优化不能使功能门禁回退 |
| S33 平台扩展验收 | S03平台策略,S31相应模块与各平台adapter | 每承诺平台/架构实测，差异/驱动/原生SDK明确，不靠交叉编译 |
| S34 打包更新与发布候选 | S30,S31,S32,S33承诺项 | 签名/路径/更新/回滚/卸载、全量门禁报告、无个人数据发布包 |

任务编号沿用原表，原S25–S29五项已撤销，不实施、不计入任务分母；当前30个有效里程碑。执行波次：S00–S04基础与高风险；S05–S10首闭环/UI/设置；账号库挂卡与网络系统/SDK并行；S30–S34整合。共享契约/数据库/窗口生命周期/系统owner只能有一个负责人。

S22在S05之后提前并行，供账号退出回退/下载电源等任务使用，编号22不代表必须等S21结束。各领域可先做无UI技术子卡，但退出必须包括四模块原页面→Rust→真实效果→反馈→重开的纵向链路；不创建删除模块的测试页面或兼容路由。

S03需要真实外部条件的验证可以blocked，但不能把该阻塞丢到发布之后。其他不依赖它的任务可继续；相关功能保持未完成。每个验证产物包括成功证据或具体阻塞、可替代路线、影响的平台/功能ID。

**24. 给低一级模型的任务卡规范**

```text
任务ID：Sxx-a / 一条明确用户流程
前置：已完成任务与证据路径
功能/字段/动作/布局ID：明确列出，禁止范围外改动
源依据：冻结仓库相对路径、符号、工作树hash/宏
输入：fixture、目标平台、账户/AppId上下文；全部合成或合法测试样本
本次产物：具体Rust服务/DTO/Flutter页面/迁移或测试报告
必须行为：正常路径、外部失败、取消/重开/并发/权限中适用的路径
不得：占位、伪进度、静默删字段、擅改导航/默认、用mock标实测
验证：命令、人工动作、预期结果、证据文件；未运行明确写未运行
退出条件：该条UI→Rust→持久化→实际平台/原生/网络效果完整
收尾：改动、ID、实际验证、剩余gap、下一任务及依赖
```

例：S07-a先实现只读游戏库首闭环。输入合成libraryfolders/ACF/appinfo，不扫描个人目录。Rust识别多库→持久化稳定游戏ID→分页/search→Flutter网格/列表原布局→关闭重开→内容一致。加入一个Unicode路径和损坏manifest，损坏单项报错而非全库消失。网络、成就、切账号不在本卡范围；也不以本卡通过宣称游戏库全功能完成。

每轮模型先读台账、相关原实现和契约，再动手。卡做不完就拆分并保留原完成标准，不减少功能。两轮同类失败或接口不清时先复现、缩小fixture、请独立审阅，再改；不反复猜依赖版本或通过删除模块让编译绿。

**25. 打包、更新和许可**

发布包包含匹配架构的Flutter runner、Rust库/worker/helper、四模块字体/资源和已获许可的必要依赖。Debug残留/旧二进制不能证明当前源码构建。Steam SDK/私有接口、迅游DLL、WinDivert驱动分别记录来源、版本、摘要、许可/授权和更新策略；排除三个删除模块及其专属资源，检查GPLv3和实际保留子模块许可。

更新流程：下载/校验签名与摘要→确认磁盘空间→按用户设置安排重启→优雅停止自有worker→数据库一致备份→替换→版本迁移→探活。失败回滚程序与可逆数据迁移，原账号数据不能被覆盖；回滚/旧目录/增量更新不能恢复已删除模块。后台网络先撤接入或交接有效owner，不留下Hosts指向死进程。

卸载区分程序文件、用户数据、系统自有变更与外部SDK。先撤自有接入和owned证书/注册项，再按用户明确选择删除应用数据；不删除Steam安装、平台账号源目录、旧删除模块的个人数据或其他代理设置。便携/安装、非ASCII路径、多用户、自启、旧URI/快捷方式一并验证。

**26. 最终门禁与当前边界**

发布候选只有在以下条件同时成立时才能称“完整移植”：

1. 基线包含当前dirty工作树及子模块，源码/构建/界面来源一致；范围严格四模块，用户授权删除记录完整。
2. 所有A/B保留功能、字段、动作、布局在承诺平台均verified；`preserved_only/blocked/未验证`数为零。C/D/E有源证据，U有用户明确删除依据与零残留证据。
3. UI入口和后台触发完整，旧数据迁移可回放，原业务实际生效；没有空回调、伪状态或只mock路径。
4. 四模块Steam ABI、外部SDK/云代理条件完成真实验证；过渡.NET业务host退场；三个删除模块无引用、打包、初始化、路由或后台残留，共享登录/权限能力仍可用。
5. Hosts/证书/代理/驱动/平台文件变更有正确owner和恢复；失败/取消/重启测试通过。
6. 原布局与操作习惯、设置长页锚点、主题/DPI、托盘/单实例/焦点通过审阅；所有授权差异有记录。
7. 性能与长运行报告覆盖总进程资源，既达冻结门槛也无业务回归；未达目标明确呈报，不能口头写“最佳”。
8. 打包、更新、回滚、卸载和许可清单完整，没有任何用户秘密或真实账号数据进入发布物/测试物。

当前交付是方案与源码索引，未完成上述实现与实测。源码研究发现的风险已经转成验证/改进任务，不表示原程序已被修复。

本次未找到可读的Oracle skill，因此没有运行Oracle，也没有开启收费API模式。已用独立源码盘点和交叉审阅降低遗漏。实施阶段若Oracle skill可用，遵守AGENTS.md：准备精确栈/错误/尝试/约束/安全附件，先确认CLI help并dry-run/files-report；API模式必须已有明确同意，任何秘密不得发送。Oracle结果是建议，仍须以本地代码与测试验证。
