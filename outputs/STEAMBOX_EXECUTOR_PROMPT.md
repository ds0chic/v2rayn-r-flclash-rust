**SteamBox 重构：交给执行模型的提示词和任务卡**

配合`STEAMBOX_FLUTTER_RUST_PLAN.md`和`STEAMBOX_SOURCE_INVENTORY.md`使用。以下是未来实施指令；本次仅编写方案，没有开始重构。

最新范围已经固定：只保留用户截图中的网络加速、账号切换、库存游戏、挂卡四模块。隐藏的Authenticator、ArchiSteamFarmPlus/ASF、GameTools彻底删除，不移植其UI、设置、初始化、数据或依赖。旧数据留原处；公共设置与四模块所需共享能力保留。

**一、总提示词：可直接复制**

```text
你要实施SteamBox完整桌面客户端重构。

目标：Flutter做前端、Rust做应用后端。只保留Accelerator网络加速、GameAccount账号切换、GameList库存游戏、SteamIdleCard挂卡。保持四模块布局、页面结构、顺序、密度、设置分组和操作习惯，完整迁移其功能、设置、数据和后台行为，提升可测量性能和视觉质量。

Flutter是已经纠正的技术选择，不是FlClash。不要fork FlClash，不要改成v2rayN/Clash/Tauri/Electron，不要只做壳。

必读材料：
1. STEAMBOX_FLUTTER_RUST_PLAN.md：26部分规格、30个有效任务依赖与门禁；保留旧编号，S25-S29已撤销。
2. STEAMBOX_SOURCE_INVENTORY.md：四模块UI/101保留字段种子、领域、原生和网络源码索引及删除清单。
3. 项目实际适用AGENTS.md。

源目录：C:\Users\Colby\.gemini\antigravity\scratch\SteamTools
主仓库HEAD：a7a5ce7f0830b9b759090a75b59d82679084b478
实际基线：上述HEAD + 当前主仓库未提交改动 + 所有子模块提交和工作树改动。
这是dirty本地SteamTools定制版本，不以最新上游README或干净clone替代它。
当前39个dirty路径，其中12个子模块。不要reset/clean/stash/覆盖用户改动。

现在先做S00/S02，不开始全项目业务实现。完成盘点后按任务依赖实施。
每轮只做一条用户流程、1至3个功能ID；项目最终全量要求不变。

职责必须统一：
- Flutter只拥有界面、焦点、选择、表单草稿。
- Rust AppEngine拥有领域事实、SQLite唯一写入、迁移、配置revision和operation。
- Rust net-host拥有反向代理、DNS/TLS/脚本注入与已应用网络状态。
- Rust steam-host隔离Steam原生接口，固定account_epoch/AppId/位数/接口上下文。
- Rust helper只执行typed白名单高权限平台动作，journal记录并只恢复自己的修改。
- 迅游SDK按需外部运行，由Rust托管；保留依赖清楚列明，不托管或启动ASF。
- C#迁移工具/对照host只能作为过渡，不能隐藏成永久C#业务后端再声称Rust迁移完成。

事实边界：
1. 主导航仅允许Accelerator/GameAccount/GameList/SteamIdleCard四类插件，具体子页按注册实测。
2. Authenticator/ASF/GameTools按用户明确要求彻底移除；原来能初始化也不迁移。删除项目引用/路由/命令/设置/后台请求/专属数据迁移/发布资源，新loader严格四模块。
3. Settings是同一长页的五区域标签锚点，不随意改成五个独立路由。
4. 原115顶层字段排除ASFSettings 14个，保留初查101个；继续补四模块嵌套实体/默认/枚举/数据，不拿101封顶。删除模块字段不塞入未知字段库。
5. 社区加速是Kestrel/YARP反向代理及脚本链，不是启动Xray/sing-box。
6. 游戏加速依赖独立迅游SDK、服务和权益，不能用HTTP代理或mock替代。
7. Steam原生Cloud/成就/AFK依赖进程级AppId和私有ABI，不假定steamworks crate可以完整替代。
8. 四内置模块逐项重写，旧.NET/Avalonia任意程序集加载器不保留；旧modules目录不能绕过严格四模块白名单。
9. 家庭共享旧VDF模型不等于新版Steam Families；当前入口注释要保留实际导航范围。
10. SOCKS5本地listener、SSH/Git活跃路径、HTTP3实际支持都需证据；只盘点四模块已有能力，不填补已删除插件占位。
11. Steam登录/用户输入2FA或邮箱验证码/Cookie/AccessToken/RefreshToken/secret-store保留，OTP生成/手机令牌管理/交易确认/令牌云同步删除。WinAuth/SteamClient共享引用按用途裁剪，不按目录名误删。

每个适用功能走完整闭环：
UI或后台入口→typed Rust命令→校验/状态→持久化→重开→真实Steam/平台/网络/外部效果→错误/取消/并发测试→证据。

核心规则：
- 先建四模块features/fields/actions/layouts及platform/plugins/native/exclusions台账，稳定ID。用户授权删除项退出分母，其余不缩减。
- 字段保留原名、Key、默认、枚举、未知项和secret语义；旧拼写要兼容。
- saved/desired/applied网络revision分离，写配置不等于真实服务成功。
- 操作携带request/operation/epoch；旧账号、旧查询、旧进程结果不得污染新状态。
- Steam登录成功需真实SteamId核实，MostRecent标记或写VDF成功不足以证明。
- Cloud写、成就变更等未知结果先查询，不自动重试非幂等外部动作。
- 普通代理流式背压，不经Dart，不自动跟重定向/共享cookie/全量解压。
- TLS拨号IP、Host、SNI和证书验证身份分别建模；不复刻关闭所有证书验证的旧缺陷。
- Hosts模式必须真实443可达，不能悄悄换444后报成功。
- 系统代理/Hosts/CA/驱动都有唯一owner，失败可补偿，其他软件或用户修改不被覆盖。
- 页面dispose不停止挂卡/加速；主窗X、托盘隐藏、退出分别定义。
- 新旧性能比较用包含本地优化的Release，总计所有进程CPU/内存；不靠EmptyWorkingSet假优化。
- 框架和crate查官方资料、完成技术验证再锁版本，生成器与Dart/Rust桥版本一致。

秘密与测试：
- 研究只读源码，不读取真实账号库、cookies、令牌、私钥、日志或浏览器profile。
- 普通CI使用合成fixture；电源/Steam写入/系统修改必须隔离测试，并有明确测试环境授权。
- 本机数据迁移使用用户实际上下文，旧本机保护/密码格式精确兼容，原库不删。
- 任何secret不得打印、附给模型、写进repo/fixture/性能报告/截图。
- Oracle若可用按AGENTS准备精确提示、help/dry-run，未经明确同意不跑API收费模式；结果仍需本地验证。

禁止：
- 空页面/空回调/伪进度/随机测速/示例成功回包冒充完成。
- 只做常用功能就称完整移植，隐藏困难按钮、静默忽略字段或随意改平台适用性。
- 用已存数据代替可执行功能；preserved_only不计verified。
- 通过改golden、降预算、删测试或缩分母掩盖回归。
- 把未运行的测试写为通过，把交叉编译写为实测支持。
- 为方便实现改变原导航、卡片/列表密度、设置长页、保存/应用含义、默认和热键。

每轮结束输出：
1. 完成的任务与功能/字段/动作/布局ID。
2. 原源码依据及具体改动。
3. 实际验证命令/操作和证据路径。
4. 未完成/preserved_only/blocked/未验证项与原因。
5. 下一小任务卡和前置满足情况。

最终只有S34全量门禁通过才可称完成。外部服务/SDK/平台条件阻塞时诚实报告，继续独立可做任务，不伪造100%。
```

**二、第一次执行：S00/S02基线与台账卡**

```text
任务：冻结本地实际版本并建立完整迁移分母。
本次不写业务实现，不启动会自动起管理员反代的个人原程序。

先读：
- 根和父级适用AGENTS.md。
- git状态、HEAD、最近本地提交、子模块状态。
- src/AssemblyInfo.Constants.cs、global.json、主csproj、TFM props。
- MainWindowViewModel、MainWindow/MainView/Home/Settings及代码后置。
- 四保留插件的Plugin入口、PluginsCore/IPlugin、GetConfiguration；三删除模块仅检查引用/初始化/路由移除点。
- 核心及插件Settings模型、接口默认、SettingsGenerator、ISettings。
- SteamClient嵌套原生依赖、当前dirty源改动。
- Accelerator.ReverseProxy与账号/库/挂卡领域入口，以及四模块所需Steam登录/权限服务。

产物：
spec/baseline.json
spec/features.yaml
spec/fields.csv
spec/actions.csv
spec/layouts.yaml
spec/platform-capabilities.yaml
spec/plugins.yaml
spec/native-contracts.yaml
spec/exclusions.yaml
spec/behavior-deltas.md
spec/gaps.md

baseline包含：
主提交+源码文件hash+所有层级子模块提交及dirty摘要+构建宏/工具版本。
记录旧实际ApplicationId、assembly/product title、数据路径和单实例协议。
说明原Release产物是否与源码一致；没有匹配则先给构建复现结果，截图/运行证据保持待验证。
隔离环境中取原界面截图；避免连接个人Steam/代理/账号或修改个人系统设置。

保留字段初始至少101个顶层种子，逐项确认Key/类型/缺省和运行默认。
ASFSettings 14个及其他删除模块专属数据/字段列exclusions，不读取/解密/迁移。
补嵌套字段、字典值、生成默认、其他序列化模型；partial别名不重复计数。
actions穷举非注释XAML绑定、codebehind、VM、托盘、CLI/URI、后台任务。
features只收四模块及所需共享能力；删除模块标user_excluded，原后台可达性不推翻排除决定。

退出条件：
1. 每一个已发现项可追到源路径/符号及实际工作树。
2. 所有未核实行为明确标待验证，无假运行结论。
3. 三删除模块完整排除清单、共享依赖保留理由、四模块占位/外部依赖/平台冲突单列。
4. 工作树快照能复现dirty源码，用户原仓库不受影响。
5. 给出S01/S03下一卡及高风险验证优先级。

输出只含安全元数据。不要把源码diff/资源中的secret、真实设置或数据库写到报告。
```

只读元数据命令示例（在源仓库执行，输出需检查，不直接贴整份diff）：

```powershell
git status --porcelain=v1
git rev-parse HEAD
git log -3 --format="%H %cI %s"
git submodule status --recursive
rg --files -g AGENTS.md -g global.json -g '*.props' -g '*.csproj'
```

不用`git diff`全量输出当快照，可能包含敏感资源；源码快照和保密检查是两个独立步骤。worktree只带commit的限制必须明确处理。

**三、S03技术验证卡：先证明难点**

| 子卡 | 最小产物 | Pass条件 | 失败如何处理 |
|---|---|---|---|
| S03-a Steam ABI只读 | 小型Rust worker+固定SteamClient接口+身份/AppId/回调契约 | 合法隔离环境读取真实SteamId与只读App信息，Steam退出/版本不支持能安全失败 | 记录接口/位数/版本gap；不得直接切Cloud/成就API；暂不标相关能力完成 |
| S03-b 旧格式/保护 | 账号/挂卡会话与游戏备份的MessagePack/MemoryPack、DPAPI合成fixture | 源参照与Rust或最小工具解析一致，保护失败/未知格式拒绝且原数据未变 | 不验证或导入OTP/WinAuth令牌备份、旧认证器vault，不能“全转JSON” |
| S03-c TLS身份分离 | 私有测试CA、拨号IP/Host/SNI/验证identity四值connector | 名称例外正确链可工作，错误链/过期仍拒绝，池不串策略 | 换适配器/缩小技术问题，不全局关验证 |
| S03-d 外部SDK/驱动 | 版本/ABI/签名/架构/授权清单与worker协议 | 真实合法依赖可加载并返回只读能力；驱动实际验证只在已授权VM | 外部条件blocked，其他任务可继续，发布门禁保持阻塞 |
| S03-e 模块裁剪 | 四模块注册表、删除清单、反向依赖图 | 三删除模块不会加载/初始化/打包；挂卡登录等共享引用仍可用 | 修正共享依赖归属，不重新引入完整删除插件或旧任意程序集loader |

**四、首条真实闭环：S07-a游戏库**

```text
前置：S01/S04/S05/S06完成并有证据。
本次只做只读库，禁止启动Steam、切账号、修改appinfo或执行电源动作。

合成fixture：
- 两个库路径，其中一个Unicode路径。
- 正常ACF、正在下载ACF、半写/损坏ACF各一个。
- 对应版本appinfo样本，未知字段保留。
- 10k游戏压力集与小型可人工核查集合。

实现：
Rust识别目录→解析与错误隔离→SQLite存稳定ID/来源→查询分页/筛选/排序。
Flutter按当前GameList页面结构显示网格和列表、loading/空/局部错误。
search带query_id与取消，旧响应不可覆盖新输入；选择和滚动状态不串。
关闭重开后相同fixture结果一致，不用内存mock冒充持久化。

测试：
结构化输出与冻结原算法对比；过滤/排序/AppId搜索、Unicode和损坏条目。
统计桥请求数量与首屏延迟；压力集滚动profile。
编译与widget通过还需跨桥SQLite真实链路报告。

退出：这条闭环verified，其余GameList功能保持未完成。
```

**五、复杂动作卡示例：S12-b账号切换**

```text
前置：Steam文件层/worker/secret与账号模型已验证；已有隔离Steam测试环境授权。
目标：旧本地dirty切换语义等价，同时确认真正当前SteamId。

流程：
目标SteamId稳定识别→停本任务依赖worker→等待已识别Steam退出→记录文件/注册表快照→
更新准确目标账号标记与旧设置→启动Steam→等待原生身份→success/needs_login/failure。

必须：
一次切换single-flight；按钮/双击/托盘/URI都调用同一operation。
账号epoch变动后旧callback拒绝；用户名大小写不同仍按SteamId优先匹配。
不停止steamservice或不相关进程；普通权限失败才走必要helper。
写前冲突检查、原子替换、失败补偿；不硬编码睡眠代替退出条件。
MostRecent不是成功证据；需登录时准确显示，取消不伪造回到旧账号。

验证：正常A→B、Steam不退出、文件拒写、权限取消、目标失效、账号epoch旧结果。
不用真实个人账号做无人值守回归，不在日志显示账号token。
```

**六、每轮审阅与收尾格式**

```text
任务/ID（不执行已撤销的S25-S29）：
原行为依据：
本轮改动及可见效果：
实际验证：命令/平台/结果/证据路径
尚未验证：
preserved_only / blocked：
契约或范围变化：是否更新所有调用者和台账
下一小卡：前置已完成/未完成
```

审阅抓六类问题：原功能被删；字段/数据损失；状态假成功；权限/账号上下文错；UI操作习惯变；指标只计主进程。发现问题先补复现fixture和源依据，再修；不要把“模型已解释”当成测试通过。

**七、最终交付应该是什么**

只含四模块的可安装Release、源码/工具链锁、四模块台账与删除清单、保留数据迁移工具及报告、逐平台验证、布局截图/操作轨迹、错误与恢复测试、总进程性能和长运行报告、打包更新回滚卸载证明、实际保留依赖许可清单。所有保留适用项verified，三删除模块零残留。缺少真实SDK/服务/平台条件时如实列阻塞，不能报完整迁移完成。
