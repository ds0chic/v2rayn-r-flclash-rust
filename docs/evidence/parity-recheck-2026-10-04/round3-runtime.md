# 第三轮运行领域复核（7edf1ee）

日期：2026-10-04。审查 HEAD：`7edf1ee4e64590093e39444188229a69beb59908`；RC 编译源码：`a4ceab5dcf9c752e324be7dbf5d35ae128119a3d`。冻结原版：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`，Windows 对照 WPF。

本报告重新检查当前源码、RR 任务卡、相关 compat 条目及冻结源。旧 RR 编号只是问题对应关系，不把旧实现描述当当前事实，不把历史测试写成本轮实测。生产源码只读；未改用户正在调整的 profiles 文件，未提交。本文的 `implemented` 表示找到实际接线并说明证据边界，不等于整项用户流程通过验收。

路径约定：仓库根为 `C:/Users/Colby/Documents/Codex/2026-10-01/v2rayn-flclash-rust-v2rayn`；以下 `U/` 指该根下 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。行号对应本轮读取的当前源码。

## 复核结论

安装/定位同根、默认路由命令、系统代理入口统一、runner 入包与真实启动代码、Custom JSON 端点解析、切换前预检/恢复、托盘监听、监控 store 重绑、Inno 非递归删除安装根均有新实现，上一轮“完全未接”的判断应撤销。

仍不能宣称全部运行功能迁移完成。最优先的问题是：PAC 返回串格式错误；预 SOCKS 图的执行顺序与配置设置不正确；其它内核的原生 Custom 配置仍被 Xray JSON 端点约束拒绝；首次 TUN 仍依赖尚未创建的接口；TUN 成功授权后侧车失败存在清理缺口；sing-box 统计后台任务没有获得持续运行的 executor。应用发行源仍有明确阻断，而且普通检查列表与应用自身更新的源分流不一致。

## RR-01 至 RR-11 当前状态

| 原问题 | 状态 | 当前结论与证据边界 |
|---|---|---|
| RR-01 下载/运行核心目录 | `implemented` | `engine.rs:94-103,183-186,291` 定义并传递同一个 managed root；`t16.rs:693-695` 委托 engine；`net_host_client.rs:466-474` 给新子进程传根；`adapter.rs:33-50,132-149` 对齐默认根且显式根禁开发回退。本轮 adapter 纯测试 10/10。普通 GUI 下载→真实核运行本轮未运行。已有 net-host 连接不重新协商根，见附加边界。 |
| RR-02 托盘默认路由 | `implemented` | `main_shell.dart:56-58` 与 `status_bar_view.dart:127` 都调 `setDefaultAndReload`；`routing_controller.dart:340-344` 保存默认后调用运行时 reload，托盘依据 `isActive` 勾选；原版 `U/ServiceLib/ViewModels/StatusBarViewModel.cs:391-417` 保存默认并发布 Reload。真实托盘点击→配置重生成→核心重启本轮未运行。成功文案没有判断 reload 结果，见 R3-10。 |
| RR-03 系统代理/PAC/重开 | `identified` | 三入口已共用 `PlatformController.applyModeFromConfig`，`app.dart:105-110` 在恢复核心后恢复保存模式，旧“只提示 PAC/不保存”断点已改。新 PAC 格式问题仍导致功能效果不正确，见 R3-01；SOCKS-only Custom 端点也需协议区分。本轮未写读宿主系统代理。 |
| RR-04 应用更新 | `blocked` | Dart 真实 detached 启动 runner 和退出代码已接（`update_controller.dart:21-37,246-308`）；后端写 plan/result 并产出完整参数（`update_service.rs:650-689`）；平铺覆盖与真实 CLI 一致，当前 ZIP 有 runner。本轮真实 runner 对合成临时安装目录的 3 项测试通过。自有发行源在 release 为 None，真实下载→验签→覆盖→重开仍 blocked；普通更新检查源分流还存在 R3-08。 |
| RR-05 TUN/授权 | `blocked` | `tun_plan.rs:47-87` 增加适配器查询；`helper_client.rs:97-105,736-754` 增加 token、runas 启动路径；打包有 helper。首次无适配器时的因果顺序仍断，见 R3-04；部分新侧车失败分支不释放 lease，见 R3-05。真实 UAC、接口、地址/路由、DNS、开关恢复本轮均未验证。 |
| RR-06 前置 SOCKS/其它核心 | `identified` | 确实新增了 12 adapter 和 sidecar spawn/等待/清理代码，应撤销“只有 2 adapter、图完全不执行”的旧描述。执行顺序、配置内容、原生格式和环境注入仍有源码缺陷，见 R3-02/03；14 个 adapter 有值不能代表 14 个核心可运行。 |
| RR-07 完整 Custom 真实端点 | `implemented` | 对 Xray/sing-box 常见 JSON，`endpoints.rs:65-136` 读取真实代理/API 端口；`engine.rs:2506-2551` 进入 plan；`engine.rs:1589-1604,1609-1636` 给代理下载选 HTTP/SOCKS scheme。本轮 parser 6/6。适用范围有限：其它核心、不绑定本机回环、端点认证、没有普通代理 inbound、API secret 等未形成完整迁移，见 R3-03/07。未运行真实 Custom 核心。 |
| RR-08 托盘持续同步 | `implemented` | `desktop_integration.dart:184-205` 监听五个 provider，节点/路由/模式变更会触发重建，不再仅启动/点击时刷新。图标状态仍全部映射同一 ico（`:188`），视觉区别没有实现。菜单与真实托盘绘制/异步并发刷新本轮未实测；未将历史 synthetic surface 测试当实机。 |
| RR-09 监控/Clash/统计显示 | `identified` | 新重绑避免每次无关 RuntimeView 变化重建 source，store 打开/加载失败可见且可重试（`bridge monitor.rs:1216-1287`）；节点 today/total 数据确实有 DTO（`:346-355`）。持续 sing-box WS 运行仍有 R3-06，Custom API secret 有 R3-07，状态栏“今日”仍使用 session 累计（`status_bar_view.dart:208-209`）。本轮 application monitor 17/17 仅验证聚合、临时 SQLite 等。 |
| RR-10 切换故障回滚 | `implemented` | `session.rs:480-561,703-778` 先 adapter/hash/端口/locator/config test/sidecar 预检，再切换；失败 best-effort 恢复上一 plan/exe；`fail_operation:368-390` 撤下死端点。`server.rs:123-128` 等 apply 结果后才返回 Accepted，`engine.rs:1470-1482` 仅 Accepted 更新目标/facts，因此不能沿用“失败把新节点标成已应用”的无条件推测。真实切换/失败/恢复本轮未运行，TUN+侧车资源清理仍见 R3-05。 |
| RR-11 Inno 卸载 | `implemented` | `tools/release/v2rayn-r.iss:71-77` 已删除整根 `filesandordirs {app}`，只显式删除 `.staging`、`app.previous` 并 `dirifempty {app}`。不再将旧整根递归删除风险写成当前事实。安装/卸载和升级后 Inno 文件清单实际效果本轮未运行；ZIP 检查不等于安装器验收。 |

## 当前问题及最小复现

### R3-01 / P1 / `identified`：PAC 用了系统代理服务器串，而非 PAC 指令串

正常流程：已运行普通节点 → 状态栏/托盘选择 PAC → 用实际 pac.txt → 返回的 PAC 内容应引导浏览器经代理访问。原版 `U/ServiceLib/Manager/PacManager.cs:49` 将 `__PROXY__` 替换为 `PROXY 127.0.0.1:{httpPort};DIRECT;`。

当前 `platform_controller.dart:203-209` 向 `startPacFromConfig` 的 `proxyRule` 传 `buildProxyServer`；`proxy_settings_view.dart:50-62` 默认返回裸 `127.0.0.1:port`，高级协议设置时返回 WinINET 的命名代理模板。`platform_bridge.dart:328-333`→`bridge platform.rs:517-538`→`platform_service.rs:294-323` 原样传递；`platform/pac.rs:56-57,202-203` 仅做原样替换。模板首行是 `var proxy = '__PROXY__';`，故真实入口会生成裸地址而不是 `PROXY` 指令；高级协议模板同样不符合 PAC 返回格式。

源码证实，不是本轮浏览器实测。现有 FIX-15C 用例直接传合法 `PROXY ...;DIRECT;`，RR-03 测试只核对保存模式/文件路径，不能验证此入口。

最小安全复现：合成 Running runtime 端口 ≥11808，记录 `applyModeFromConfig(Pac)` 真正传给 bridge 的 `proxyRule`，再用纯 `render_pac` 或 PAC evaluator 执行固定测试域名，断言完整合法指令。修复应拆出 PAC renderer，普通路径对齐原版字符串，不能借 WinINET `buildProxyServer` 代替。真实浏览器/系统 PAC 验收需经批准的隔离环境。

### R3-02 / P1 / `identified`：预 SOCKS 多进程执行顺序反了，且配置没有消费真实设置

原版 `U/ServiceLib/Manager/CoreManager.cs:94-96` 是 main core → `WaitForProxyPort(preContext)` → `CoreStartPreService`；`U/ServiceLib/Handler/ConfigHandler.cs:1555-1584` 创建 SOCKS **出站节点**，其 port 指向主核；前置服务经完整 context 生成自己的入站/TUN 配置。

当前 `engine.rs:2570-2574,2614` 的图仍表达 sidecar 依赖主核，但 `session.rs:572-576` 将主核从 prepared 顺序移除，`:979-1101` 先启动全部 sidecar 并等待 `sidecar.port`，`:1103-1145` 才启动主核，跨越了图依赖。`codegen.rs:281-294` 将 `listen/port` 赋给 SOCKS 出站节点后用 `CodegenInput::default` 生成，没有传真实入站/TUN/DNS/路由/统计设置；`config_codegen/input.rs:351-367` 的实际默认入站是 **11808**。

可推导触发：Custom.PreSocksPort=12080、主 Custom 核将监听12080、用户本地端口11980。前置配置却监听11808、出站拨12080，net-host 在主核尚未启动时等待12080，导致 ready 超时；如果同端口已有不受本项目管理的 SOCKS 服务，还可能错认其 ready。LegacyProtect+TUN 也未把 TUN 设置传入前置 config；默认入站与上游主核端口重合时可能冲突/自指。以上是源码推导，不写成真实流量实测。

最小安全复现：纯生成断言侧车出站12080、入站11980、TUN开关和相关设置；受控 stub记录主→ready→pre 顺序。真实双核只用隔离临时根、先探测 ≥11808 端口并持句柄清理。修复必须执行整个拓扑顺序并区分“侧车入站”与“拨向主核的 SOCKS 出站”，不能仅在图里将端口 `exclusive=false`。

### R3-03 / P1 / `identified`：14 adapter 存在，完整 Custom 原生配置链仍未迁移

本轮 `adapter_for` 在 `adapter.rs:718-734` 已覆盖14代理核心，纯候选测试通过。但 `codegen.rs:263-267` 除 SingBox 外均走 Xray 生成器；Custom raw 在 `config_codegen/xray/mod.rs:110-122` 被 JSON 解码或包装为 JSON string。`engine.rs:2497-2517` 重新序列化后强制 `parse_custom_endpoints`；该函数在 `endpoints.rs:65-70` 除 SingBox 外全部按 Xray `inbounds` 解析（`:74-99`）。Mihomo YAML 与 naive/tuic/mieru 等原生结构无该数组，运行计划被 `error.custom_endpoint_parse_failed` 拒绝。包装 YAML 为 JSON 字符串也没有保持原生文件内容。

冻结原版 `U/ServiceLib/Handler/CoreConfigHandler.cs:16-21` 给 mihomo 专用 Custom 服务，其它 Custom `:44-83` 实际复制原始文件；没有要求所有核心都采用 Xray inbounds。

另有确定的环境合同遗漏：Mieru `adapter.rs:705-706` 只返回 `run`；`session.rs:1007-1012`（侧车）与主核 command 构造不注入配置环境。原版 `U/ServiceLib/Manager/CoreInfoManager.cs:282-289` 要求 `MIERU_CONFIG_JSON_FILE={0}`，`CoreManager.cs:337-348` 注入该环境。故即使解除 JSON 拦截也仍缺原生 config 路径。其他核心的资产/证书环境亦需要逐核核对，不能从候选名测试推定有效。

最小安全复现：合成 mihomo YAML（mixed-port11980）、naive JSON（listen 字段）、mieru 配置，不启动核心，走真实 `build_runtime_plan`，记录准确拒绝路径；记录 Command spec/env。修复按冻结核心合同分别保持 raw/config格式、端点可解析性/无代理端点状态、args/env/工作目录。不能删除其它核心或将其统一标“不适用”。

### R3-04 / P1 / `blocked`：首次打开 TUN 仍需要尚未由核心创建的接口

`engine.rs:2467-2473` 在 build plan 前调用 `tun_hints_from_env`；`tun_plan.rs:47-87` 查找已有适配器，找不到保持0，`:170-174` 拒绝生成 TunSpec。即使已有接口，`session.rs:898-975` 顺序仍是 helper 设置地址/路由后才启动核心；`privileged_helper/src/windows.rs:408-420` 给既有 interface 设置地址，并不创建该设备。

因此“加了 netsh 和 runas”解决了已有接口定位/授权启动代码，并未使干净主机上首次打开 TUN 成为完整用户流程。冻结原版 `U/ServiceLib/Manager/CoreManager.cs:88-96` 清理旧设备后启动相应核心，设备创建合同不能被“必须先存在”替代。

最小复现应首先纯 stub返回没有 v2rayn-tun 的列表，确认目前在核心 spawn前就被拒绝；之后需隔离 VM验证核心创建设备→发现接口→地址/路由设置→ready→退出回收，包含UAC拒绝和重开。真实平台写入本轮未运行；不能用已有设备的dry-run验证首次TUN完成。

### R3-05 / P1 / `identified`：helper 成功后侧车失败分支漏释放 TUN lease

`session.rs:963-975` 已将成功 lease/link/session_id 写入 Inner。后续侧车目录/配置失败（`:989-1004`）或ready超时（`:1088-1098`）只 `fail_operation`→`stop_sidecars`→return，没有 `release_tun_lease` / `finalize_journal`。`fail_operation:368-390` 只更新状态和端点；首次启动 `had_session=false`，`apply_plan:746-749` 不走旧会话恢复。所以一旦隔离环境中 helper 真设置成功，紧随其后的侧车失败可以留下租约/路由与未终结journal，同时UI看到Stopped。

这是条件性源码证实，未执行宿主TUN操作。最小安全复现用记录型 HelperLink 成功 Apply + sidecar ready失败，断言 Stop/Tun清理调用、lease/link/session_id与journal都归零。修复应用统一的失败清理作用域，覆盖所有 helper成功之后的early return；不能只在主核spawn失败分支清理。

### R3-06 / P1 / `identified`：sing-box 统计 WS 后台任务仍缺调度

旧风险第1项（无关RuntimeView反复重绑）与第3项（store失败被静默当已绑定）已有源码修复：`bridge_api/api/monitor.rs:1216-1287` 和 Dart `monitor_controller.dart:217-264`。不再写成未修。

第2项仍存在：生产 `poll_loop` 用 `new_current_thread`（`bridge monitor.rs:1325-1327`），`:1402-1405` 的 `block_on` 只创建 `SingboxTrafficSource`；该source在 `core_adapters/src/stats/singbox.rs:121-136` 使用 handle.spawn启动WS任务，其 `poll()` 在 `:168-170` 立即返回内存snapshot。生产loop只 `rt.block_on(src.poll())`（`:1366`），随后阻塞当前线程sleep（`:1386`）。这一组合不会持续驱动同一executor上的WS连接/接收任务；可能一直是空统计。

冻结 `U/ServiceLib/Services/Statistics/StatisticsSingboxService.cs:18,35-58` 持续驱动 Run/WS接收。当前adapter自身测试用持续运行的Tokio上下文，不能证明production hub调度正确。本轮没有运行production hub + WS server，故结论为高可信源码缺陷，不声称亲眼看到0速。

最小复现：同一生产poll loop和本地合成WS服务持续发送非零增量，端口≥11808且先探测，读取production stats snapshot/node SQLite，比较无流量、流量、断线恢复。修复将poll loop置于持续async executor或用独立运行线程，保持统计在页面隐藏时继续收集。不要靠给测试单独调用sleep/yield避开真实入口。

### R3-07 / P1 / `identified`：Custom 的监控端点信息仍不完整

`CustomEndpoints`（`endpoints.rs:49-55`）只有inbounds与api_port，没有listen地址、代理认证、Clashsecret、API类型；`parse_singbox:127-132` 只取external_controller的端口。`AppliedFacts`/`MonitorSession` 的engine路径也没有将Custom secret传给hub。`sync_from_engine_session:1238-1243` 更新core/port/节点/开关，不更新secret；Clash client `monitor.rs:739-756` 使用hub.secret，这个值只有独立 `monitor_configure:403-416` 写入。普通应用的自动sync路径未从Custom配置读取它。

另外 `engine.rs:1612-1616` 先用通用opts统计端口初始化；Custom缺少api_port时 `:1626-1632` 不清零，会试图访问配置里并不存在的默认统计API。Custom仅SOCKS的local_proxy_url已修scheme，但系统代理/PAC入口仍只取port（`platform_controller.dart:244-249,191-194,206-209`），没有协议区分；不能以via-proxy下载用SOCKS正确推定WinINET/PAC也正确。

触发与最小复现：合成sing-box mixed11980+Clash API11990并设置公开合成secret；用记录型API服务要求同一secret，走普通应用sync/连接/策略选择；再对没有API的Custom断言没有错误的API访问。协议/地址/认证事实应由已应用计划发布，秘密不应进入日志/证据。本轮未读用户Custom文件，未运行真实API或系统写入。

### R3-08 / P1 / `identified`：普通更新检查仍查原版 App 发行源，按钮却查重构版源

`BUILTIN_TARGETS`（`update_service.rs:114`）仍含v2rayN，`t16_check_updates:790-802` 对所有target统一 `check_core`；`check_core:399-406` 不传app_repo，`updater/channel.rs:45` 映射`2dust/v2rayN`。相反“应用自身更新”按钮走 `t16_apply_app_update_spec:922-925`→`check_app_update:415-425`，release版自有源None（`:83-94`），报`error.update_app_source_unconfigured`。UI检查行会展示原版应用版本/资产，但点击执行的是另一份发行源。这仍是用户逻辑差异，虽然 `t16_apply_core_updates:849-855` 已阻止把App当核安装。

修复普通检查应给App分流到自己的源/当前版本/资产合同；源未配置必须在检查行同样明确blocked，不能用原版发行信息伪装本产品可升级，也不能伪造发行URL。最小复现用双记录release provider断言App和核分别请求正确repo，不需网络。

RR-04 flags不再硬编码true/None，当前 `remember_update_flags:43-54,788,843` 实现记忆。但仅切换窗口复选框后直接点应用更新、不先运行一次check/apply-core，stage接口仍取上次全局flags而非当前窗口参数；应有真实当前选择→stage合同测试。该部分可作为P2跟进。

### R3-09 / P2 / `identified`：托盘图标状态和“今日”文本仍与实际数据不匹配

`TrayIconStatus`和provider监听确实存在，但 `desktop_integration.dart:188` 忽略status，所有状态都映射 `:157-162` 同一tray_icon.ico；`:449-455` 调setImage仍不会产生视觉差异。需要真实资源映射与状态视觉验收。

状态栏 `status_bar_view.dart:208-209` 将 `monitor.proxyUp/proxyDown` 标为“今日”，而bridge/app聚合数据来自session累计；真正节点today值在 `bridge monitor.rs:346-355`。原版 `U/ServiceLib/Manager/StatisticsManager.cs:115-125,158-159` 单独维护今天累计和日期清零。最小复现用合成“上次会话今天100、新会话20”及跨日数据，断言显示语义；不能只加统计列而保留错标签。

### R3-10 / P2 / `identified`：路由重载失败仍提示“已重载”

`routing_controller.dart:340-344` 只判断保存默认是否成功，然后await reload，不检查RuntimeView.error/实际applied修订，无条件设置“已切换默认路由并重载”。当新路由导致codegen/config check错误，RR-10会保存旧会话，默认desired已改变，但这里仍提示重载完成。

最小复现：合成成功保存+失败runtime reload，断言默认设置保存、旧applied保留、具体失败呈现。修复成功提示应以新applied事实为依据，不回滚原版“先保存默认再Reload”的业务顺序。

## 其它审查边界

- RR-01 的根只在**新launch**时传递：`NetHostClient::connect:409-429` 先打开已有固定pipe，engine默认pipe不按data dir区分（`engine.rs:183-186`、`session.rs:68-69`）。两个实例只改data dir、未改pipe，或更新后复用旧net-host，不能据此保证隔离/根热切换；本轮未连接现有pipe。需要结构化启动握手/实例归属，不把独立DATA_DIR单独当完整运行隔离。
- 自更新默认Dart退出是 `exit(0)`（`update_controller.dart:34-37`），不走普通桌面 `exitApp` 的PAC/代理所有权/热键清理（`desktop_integration.dart:353-358`）。runner只等待UI PID；net-host有自己的断线宽限（`session.rs:71`）。源blocked时这条生产链到不了末端，但发行前须在隔离环境验证已有运行会话时的升级交接、旧net-host退出/复用、安装目录权限、结果读回与重启。仅“Process.start返回成功”不代表覆盖成功。
- 本轮纯runner测试没有在真实安装目录运行，没有覆盖完整RC四exe都正在参与时的更新、签名发行源、实际GUI重开或Inno记录。旧“runner根本没入包/CLI参数不匹配”已撤销；未知锁文件效果不能作为已实测缺陷。
- 页面参数消费/日志环/Clash数据结构已有后端，不等于实际hide/show节流、带secret Custom策略切换、真实非零流量、跨日/重开SQLite全链通过。安全组件测试只覆盖对应组件。

## 本轮实际命令与结果

| 命令/检查 | 结果 | 能证明的范围 |
|---|---|---|
| `cargo test -p runtime --lib --locked adapter::tests` | exit0，10passed | 候选名、adapter存在、参数形状、临时目录定位、默认/显式根；不启动真实核。 |
| `cargo test -p runtime --lib --locked endpoints::tests` | exit0，6passed | Xray/sing-box合成JSON端点解析；不证明其它核心Custom可运行。 |
| `cargo test -p application --lib --locked monitor::tests` | exit0，17passed | 聚合、日期、世代、日志、合成/临时SQLite存取；不运行production bridge的WS poll loop。 |
| `cargo test -p upgrade_runner --test app_overlay --locked` | exit0，3passed | 真实runner binary覆盖临时合成文件、注入失败回滚、缺失重启目标错误；没有启动实际GUI。 |
| Python `zipfile` + SHA256检查当前RC | 哈希吻合、CRC完整 | 包内容/来源；未启动包。首次按archive-root查build-info失败后已按真实顶层目录重查通过，不把首次脚本失败算包损坏。 |

当前ZIP：`dist/v2rayN-R-1.0.0+1-windows-x64.zip`。SHA256：`6e9e28551376537f6dd10c244f05ffd0b03e5ae8e0878a4ee0bfd447d76dac21`，与SHA256SUMS一致。顶层目录为`v2rayN-R-1.0.0+1-windows-x64/`；其内平铺四exe：`v2rayn_desktop.exe`、`net_host.exe`、`privileged_helper.exe`、`v2rayN-upgrade.exe`；没有Xray/sing-box/mihomo核心。ZIP内build-info：source a4ceab5、dirty=false、Windows x64、smoke_armed=false，`testzip()`返回None。当前HEAD多一个dist刷新提交，源码构建来源差异是明确登记的正常关系。

未运行：全workspace门禁、Flutter构建/GUI操作（根代理负责）、真实核心下载/运行/代理流量、真实WS/API、真实UAC/TUN/路由、系统代理/自启/证书写入、安装器安装/卸载、签名发行源自替换。未读取真实用户配置/节点/日志；未碰10808；没有杀外部进程。仅新增本文。

建议实施顺序：先修 R3-01、R3-02、R3-03 与 R3-06，并用生产入口的合成测试暴露各断点；随后做 R3-04/05 的TUN生命周期与故障清理，再经授权隔离VM验证；应用更新源/检查分流/退出交接应合并为一条可审阅的更新流程；最后补R3-09/10的用户反馈与视觉效果。每项都保留原冻结需求，不能把未完成的内核/TUN/更新降为“不适用”。
