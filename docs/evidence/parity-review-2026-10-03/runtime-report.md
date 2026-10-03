# 内核、运行效果、桌面集成与后台任务差异审查

审查日期：2026-10-03（北京时间）。应用基线 `1251cbc6821276e35d082f7739b8b9c15b6f93dd`；冻结上游 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。Windows 以 WPF 为准，非 Windows 单列 Avalonia 合同。分配 107 行，逐行结论见 [runtime-items.json](runtime-items.json)。

逐行文件已自检：分配 key 集合与输出完全相等、107 行无重复、任务卡要求字段齐全、所有上游与当前引用路径存在。构成为功能 42、动作 19、布局 7、窗口 11、内核矩阵 15、后台任务 6、定时任务 7；状态为 `identified` 76、`implemented` 12、`preserved_only` 19，无 `verified`。`implemented` 仅表示文中指出的已有生产实现，仍有明确限制，不能据此计算产品完成率。

本报告没有修改应用、台账、冻结源码、方案原件或发布包，也没有运行真实用户节点、改写宿主代理、自启或 TUN。这里的“已确认差异”主要是重新核对当前生产调用链的源码事实；原版实机逐事件对照、重构版全部 UI 流程及跨平台运行均未验证。底层测试通过不能证明最终用户入口已接入。

## 主结论

当前差距不只在界面。很多 UI 可以编辑或显示一个选项，Rust 也有对应函数和测试，但生产启动、运行会话和实际效果没有接上。最明显的是：普通重开不启动活动节点；活动节点与运行节点分离后没有等价的自动应用时机；TUN 开关只改 Dart 状态；普通会话不配置监控；经代理下载拿不到本地会话端口；内核更新成功后路径与运行定位规则不一致；托盘、多数热键、定时任务和自身升级未完成用户闭环。

同时，已有真实实现不能被忽略：Xray/sing-box 的结构化配置、节点与设置的 SQLite/JSON 保存、net-host 的受管进程与日志转发、Clash 的选择/断开 HTTP API、ownership 代理恢复、独立 OpenPGP 验证器和外部 upgrade_runner 都已经存在。问题必须落在缺失的具体连接上，不能把这些全说成“没实现”。

## 优先修复的用户流程

| 编号 | 优先级 | 用户触发 | 当前与原版差异及定位 | 必须达到的结果 |
|---|---|---|---|---|
| RT-01 | P1 | 保存活动节点后正常关闭/重开程序 | 原版 MainWindowViewModel.cs:334–342 注册后台任务、统计并 `Reload()`；当前 app.dart:17 的 applyPlanOnLaunchProvider 默认 false，自动 apply 只为 debug/armed smoke。普通重开只有 snapshot。 | 合成隔离目录重开后按原版活动节点和设置恢复本地运行；避免把证据环境变量当生产入口。 |
| RT-02 | P1 | 启用节点、改设置/路由后点击应用 | profile_actions.dart:134 只改 active ID，重复启用会置空；runtime_controller.dart:100 使用旧 snapshot revision，保存事件不在 refreshKinds 中。原版默认服务器/路由切换触发 Reload。 | 启用同一活动节点保持原版语义；根据操作的生效时机刷新 revision 并应用，错误留在用户可见位置。 |
| RT-03 | P1 | 已启动核心后选择经代理更新/订阅 | engine.local_proxy_port 初始 None；全仓 production setter 只有 FRB/Bridge 定义，没有会话回填。t16_check_updates / apply 和订阅走代理据此返回 E_PROXY_UNAVAILABLE。 | 从已应用会话获取实际 mixed/SOCKS 地址，停止时撤销；不能把期望配置端口冒充运行端口。 |
| RT-04 | P1 | 下载内核后重新应用 | updater 安装到 data/cores/<core> 根目录，sing_box 目录为 `singbox`；runtime 只搜索版本子目录、sing-box，并使用另一个环境变量和默认根目录。 | 更新、版本显示、运行定位共用一个安装合同；下载后必须真实启动下载的那个版本。 |
| RT-05 | P1 | 打开 TUN 状态栏开关 | ui_shell_controller.dart:334 仅修改 Dart 内存；未保存/授权/Reload。即使设置窗口保存 EnableTun，build_runtime_plan:1799 仅放 tun_enabled，没有 `attach_tun_to_plan`；net-host 的 tun_spec_from_plan 会拒绝缺失 tun 节点。 | 控制开关→保存→权限/取消→完整计划→helper→核心→真实状态/回滚闭环，保持未授权测试不修改宿主。 |
| RT-06 | P1 | 使用预 SOCKS 或 LegacyProtect | pre_socks_decision 存在且纯测试通过，build_runtime_plan 没使用结果，ProcessGraph 只有主核心。自定义节点前置核心选择还直接使用 node.core_type，而原版用 Custom 类型默认内核。 | 按冻结 CoreConfigContextBuilder/CoreManager 的主/辅助关系生成进程与端口，准备与失败反向清理；不得把单个决策函数当作双进程支持。 |
| RT-07 | P1 | 选择 v2fly/mihomo/其他自定义核心运行 | CoreType 枚举保留 15 项，但 adapter_for 仅 Xray/SingBox；12 个其他代理核心缺启动适配。Mihomo YAML Mixin 只有纯函数/测试，没有生产生成入口。 | 逐个迁移原版候选 exe、参数、环境变量和配置形态；应用更新标识 v2rayN 不作代理内核。 |
| RT-08 | P1 | 选择文件型自定义配置/Outbound 并应用 | build_codegen_input 只取 proto_extra.extra.customConfigText，不读取 Address 文件/导入 RawConfig；非 JSON 内容被 Value::String 包装后再次 JSON 序列化。 | 受控读取与透传原始配置字节，Mihomo 走 YAML 修改/Mixin；文件型及内联路径均用真实核心校验。 |
| RT-09 | P1 | 正常运行核心后看速度/代理/连接 | 普通 runtime 没调用 monitor.configure/startPolling/setActiveNode；唯一 configure 在 main_shell.dart:100 的环境证据钩子。MonitorHub 默认 Xray、端口 0、统计 disabled。 | applied 会话事实注入监控，并在换核/停核及时更新；普通 GUI 操作即可出现真实数据。 |
| RT-10 | P1 | 运行多天、重开、清除历史统计 | MonitorHub 使用 InMemoryTrafficStore；真正 SQLite ServerStatItem 没接。clearStats 只清内存库；状态栏“今日”显示 session_proxy 累计，而非当天节点统计。 | 真实累计/当日/跨日/节点归属/重开/全清都有持久化证据，且旧会话流量不会归到新节点。 |
| RT-11 | P1 | 托盘切节点/路由、导入、扫码、更新、复制命令 | desktop_integration.dart:143 动态子菜单 children=[]；其余动作落 default 尚未接入，节点数量上限函数也未使用。 | 使用与主窗口相同的应用命令，按原版作用域和刷新规则同步菜单、运行状态与图标。 |
| RT-12 | P1 | 注册全局热键后按下 | hotkeys.dart:155 manager.register 只有 HotKey 参数，没有 keyDownHandler/keyUpHandler，也没有业务 dispatch。 | OS 接受注册与实际触发分别验收；显示/代理四动作执行同一个入口。 |
| RT-13 | P1 | Windows 点 X 或启动隐藏/启动第二实例 | 当前 preventClose/onWindowClose 用 Avalonia/Linux 的 Hide2TrayWhenClose 字段决定 destroy；WPF Closing 恒 Cancel+Hide。AutoHideStartup 仅解析未使用。SingleInstanceGuard 只有库/测试，应用启动未调用。 | WPF 的 X 隐藏、菜单关闭隐藏、托盘退出恢复分开；启动隐藏、第二实例唤回与退出按上游合同验收。 |
| RT-14 | P1 | 从状态栏或托盘选 PAC | 两入口只提示先启动服务；Flutter 无 pac_start/pac_start_from_file 调用。系统代理其他模式能调用 Windows backend，但新模式不写 settings，重开回到旧 mode；状态栏改变后托盘也不自动刷新。 | 模式选择即管理 PAC 生命周期并保存，临时监听 URL、绕过项、退出恢复及重开行为完整。 |
| RT-15 | P1 | 正常检查更新/应用自身更新 | 默认选中 v2rayN 后 checkAndApply 把应用当 core 安装到 cores/v2rayN；单独自身更新固定 prerelease=true、proxy=None、2dust/v2rayN 原版包，安装根是数据目录父级，helper 名 v2rayN-upgrade.exe；UI 只返回 spec，没有启动已存在的 upgrade_runner。 | 新客户端自身更新来源/安装根/runner 和参数相符；保留原版支持的 stable/prerelease/viaProxy/安装模式语义，实际退出替换重启。 |
| RT-16 | P1 | 设置自动更新间隔后等待 | start_sub_scheduler 只有定义，没有正常启动调用；1 分钟总调度、每小时清理/Geo、每日更新检查不存在。Geo 资源更新也无生产入口。 | 受控 fake clock 验证到期条件+真实合成订阅下载闭环；任务生命周期随应用启动/退出，不能要求用户手动调用 API。 |
| RT-17 | P2 | 改代理/连接自动刷新、排序、列宽后返回或隐藏窗口 | 页面本地变量每次重建归零，2 秒硬编码，不读 ClashUIItem；Connections 为完整 DataTable 无列宽/索引恢复；dispose 不 setPageVisible(false)，隐藏窗口不停止刷新。 | 参数保存/重开/返回一致，选中组+明细布局及连接列状态与原版一致，隐藏时正确暂停 UI 刷新。 |
| RT-18 | P2 | 打开代理面板切模式/延迟测试 | 原版有 RuleMode、排序、当前组→明细；当前仅名称 A/Z 切换及混合竖向列表。GET/PATCH 模式接口未接；delay URL 用 gstatic 常量；已有 retry 函数未由服务调用。 | 按原版控件/模式/排序/测速 URL与三次重试规则走实际 Clash API，保留真实 selector PUT 与 connection DELETE。 |
| RT-19 | P2 | 日志自动刷新/暂停采集/复制 | 当前自动刷新只控制滚动；原版另有自动刷新与滚动到底两个状态。无复制入口；采集暂停后 Rust buffer 拒收，但 ingest_runtime_event 把原始行继续 fan-out，Dart _onLogs 无暂停过滤。 | 区分采集、呈现与滚动；暂停、恢复、复制、过滤、窗口隐藏均有真实输出证据。原版 ClearMsg 已注释，保留入口的处理不得当作必须新增清空业务。 |
| RT-20 | P1 | 自启开关后取消；管理员重启/UWP菜单 | 自启立即写 Run 键、取消仅关闭；settings 域已对照原版 SaveSettingAsync 确认原版保存后才 SetFull，取消不执行。管理员重启直接 notImplemented；UWP 仅判断 exe 存在。非 Windows platform 服务仍 fake，helper仅 Windows。 | 保存/取消与平台生效时机完整；Windows 管理员计划任务、自启回读、真实 runas/工具启动和非 Windows 各后端分别验收。 |

## 边界与有意变化

- 系统代理退出恢复：原版非 Unchanged 强制清除；方案 §13 已要求按字段 ownership 恢复，并保留外部更改。当前 restore_on_exit 符合这一安全方向。应在迁移合同明确记录这项有意差异，不能为追求逐字一致删除 ownership。缺口是所有真实退出路径尚未统一进入恢复，取消/失败反馈与持久模式同步也未完整。
- 配置保存：大量用户命令已即时写 JSON/SQLite，比原版约 20 分钟兜底保存更早。不应仅凭“没有20分钟计时器”断言设置必定丢失，也不应额外造一个无意义的周期写入。需逐实体证明等价耐久性；统计/节点扩展数据仍存在未接持久化问题。
- TUN：T21 的独立系统测试能证明指定设备/路由 API 在其测试条件下工作，不等于状态栏→完整运行计划→权限→helper 的产品闭环。此轮没有再触发宿主 TUN。
- 签名：当前 PgpDetachedVerifier 与 public key、测试已经新增；不能继续声称仓库只有 UnsupportedSignatureVerifier。普通 UpdateService.stage_artifact 仍未消费签名 URL或调用验证器，只做可选 checksum/`.dgst`，失败取得 digest 还可退为空；签名能力存在与用户升级已验证是两回事。
- 运行回滚：net-host rollback 能清理失败候选与拥有的 TUN 资源，未恢复之前已停的核心。原版也先停后启；这属于方案要求的恢复能力未落实，不能误写为原版必定保留旧运行。官方 core `test_args` 已定义，但 apply_plan 没在停止旧会话前调用；就绪仅看一个 TCP 端口，辅助监听/API/UDP/自定义端口未形成完整计划。
- 非 Windows/ARM64：未运行。某些平台函数根本不存在（如非 Windows helper、真实系统代理 backend），另一些已有可编译源码但尚缺构建/真机证据，二者分开登记。Linux sudo 独立视图没有实现，不能把 Windows helper测试替代它。

## 实际运行的检查

环境 `login:false` 不包含 cargo PATH，直接 `cargo test -p config_codegen --locked` 首次失败，提示 command not recognized；随后按 AGENTS 固定绝对路径运行。

| 实际命令 | 结果 | 可以说明什么 |
|---|---|---|
| `C:/Users/Colby/.cargo/bin/cargo.exe test -p config_codegen --locked` | exit 0；83 tests 全通过 | 已有纯配置生成/错误/模板/形状测试成立。没有完整原版 golden 输出或远端握手验收。 |
| `C:/Users/Colby/.cargo/bin/cargo.exe test -p runtime --lib adapter::tests --locked` | exit 0；5 passed | 已有适配参数/locator范围测试；`unknown_core_has_no_adapter` 正说明缺失其他核心，不能当作它们已支持。 |
| `C:/Users/Colby/.cargo/bin/cargo.exe test -p application --test t18b_runtime_plan --locked` | exit 0；11 passed | 合成配置→RuntimePlan 的代表路径成立；使用 NullRuntimeClient，没有启动核心、系统代理或 TUN。 |

每项的 tests_run 字段区分上述实际命令和未运行的 UI/重开/系统场景。没有执行整个 workspace、T21 真机平台测试、用户订阅或远端网络。Flutter/Windows 实机场景由根代理串行协调，此报告不将其他代理未完成场景冒充已通过。

## 实施顺序

1. 修用户主线：普通启动与激活/路由切换的生效合同、revision刷新和可见错误；把实际会话端口/内核/API/节点归属注入下载和监控。
2. 修安装合同：统一 updater/runtime 的根目录、目录名、版本布局和当前版本识别；先合成下载→校验→安装→真实启动，再接应用自身升级。
3. 逐核与特殊配置：Custom 文件/原始格式、Mihomo/Mixin、其他 exe 参数环境；预 SOCKS/LegacyProtect进程图和完整端口/资源图。不得删掉不支持的内核选项作为“完成”。
4. 系统与桌面：PAC与模式持久化、所有退出、托盘业务动作、热键回调、单实例、权限流程；平台写入仅在隔离授权环境做实际验收。
5. 后台与长期数据：启动所有到期任务、接 SQLite 统计、当日/跨日、刷新/隐藏/重开；以用户可见结果验收，不以计时器存在验收。
6. 原版界面与细节：状态栏可用性点击、代理组/明细和模式控件、连接表列状态、日志复制/刷新/滚动，跨字体与 DPI 再验证。

跨领域依赖：profiles-report 负责特殊类型入口、订阅转换/PrevProfile/NextProfile与节点键鼠；settings-report 负责字段UI/保存/取消和路由/DNS编辑对象。这里的 `build_codegen_input` 不读取订阅前/后置节点表达式、平台/IPv6探测、本地 SRS库存与用户 SrsSourceUrl，也没有把它们转成运行上下文。相关字段不能只因为 JSON能保存而标已生效。

下一轮验收必须使用固定应用HEAD、公开冻结来源、合成配置和独立数据目录，逐功能走 UI→Rust→保存→重开→配置/实际效果；本轮审查完成不等于产品功能完成。
