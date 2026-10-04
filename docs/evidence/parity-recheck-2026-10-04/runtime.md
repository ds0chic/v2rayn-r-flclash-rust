# 2026-10-04 运行、平台与发布包第二轮复查

状态：`identified`。开始 HEAD `efd6b2f`，结束复核 HEAD `a2b6905`。两者间仅节点选择四文件与 dist 元数据变化；本文引用的运行/更新/平台源码未变化。原版冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`，Windows 主合同取 WPF。当前 RC 的 build-info 指向 `5471e4dcf53c9cc8d20521627bc86805050c44ab`、dirty=false、smoke_armed=false。

上游路径前缀 `U/` 在本文统一代表 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`；下面行号均是此次重新读取的当前文件或冻结文件，不沿用 1251cbc 的旧定位。本文只改这一份报告，不修改生产代码、用户节点选择四文件、台账、冻结源码或发布包，不 commit。

本轮是生产调用链源码核对、公开 RC ZIP 内容/哈希检查和两组安全纯 Rust 检查。没有启动原版双窗口、没有操作发布包 GUI、没有远端下载/自替换/安装卸载、没有监听端口、没有读用户配置/节点/日志，也没有写宿主代理、自启、证书或 TUN。源中可确定的断点与尚需实测的运行风险分列，不能把修复任务卡或历史系统探针当作本轮已通过。

## 已补上的实现，不能继续照搬旧报告

- `apps/desktop/lib/app/app.dart:104-109` 普通启动已调用 `restoreActiveOnLaunch()` 和订阅 scheduler；`features/runtime/runtime_controller.dart:115-143` apply 前会重新 refresh 修订。不能再说普通启动只 snapshot、scheduler 全无入口。
- `crates/application/src/engine.rs:1459-1507` 已有 Running 会话代理端口发布/停止撤销；`1527-1558` 有监控会话与 SQLite TrafficStore。`crates/bridge_api/src/api/monitor.rs:1197-1238` 和 Flutter `monitor_controller.dart:233-248` 已消费正常 RuntimeView。旧“仅证据环境配置监控/只有内存统计”不再准确。
- `crates/runtime/src/install_layout.rs:41-60,116-149` 已统一 singbox 名及版本子目录，locator 也使用该类型。旧“目录名一定 sing-box/singbox 分叉、只按字符串选版本”不应照搬；剩余根目录与生产消费问题见 RR-01。
- `desktop_integration.dart:73-94,171-249,326-340` 已实现 Windows X 隐藏、动态菜单、热键 dispatcher；`app.dart` 语言 delegates 已接。不能再说这些全部不存在；动态菜单对象及同步仍需检查。
- TUN UI 已保存并调用 apply，RuntimePlan 已 `attach_tun_to_plan`；预 SOCKS 已写 graph。新增图和描述符不等于 net-host 真正执行完整生命周期，见 RR-05/06。
- PGP verifier 已接应用签名 staging、App/core 安装分流已做、防 WPF payload 的代码已存在。当前缺的是普通检查分流、发行源、runner/CLI/包布局/退出执行等合同，见 RR-04。

## 当前可由生产源码确定的问题

### RR-01 · P1 · 普通下载核心后仍可能完全找不到它

触发：新鲜 RC 普通启动→更新 Xray/sing-box→应用节点；不设开发用 `CORES_ROOT`/`XRAY_BIN`，或使用独立 `V2RAYN_R_DATA_DIR`。

当前链：`crates/bridge_api/src/api/t16.rs:661-670` 安装根为 `V2RAYN_R_CORES_DIR` 或 `engine.data_dir()/cores`；`crates/application/src/engine.rs:221-228` 默认 data 为 `%LOCALAPPDATA%/v2rayn-r/data`，所以默认下载位置是 `.../data/cores`。net-host 的 `CoreLocator::from_env` 在 `crates/runtime/src/adapter.rs:28-35` 只取 `V2RAYN_R_CORES_ROOT`、`.../v2rayn-r/cores` 和开发祖先 `tools/cores`；`crates/application/src/net_host_client.rs:430-447` 启动子进程没有传 engine data 目录或同一 cores 根。共享 layout 修好了根之下的名字，但两个根仍不同。

原版 `U/ServiceLib/ViewModels/CheckUpdateViewModel.cs:377` 安装至 GetBinPath(core)，`U/ServiceLib/Manager/CoreInfoManager.cs:32-47` 用相同 GetBinPath/core 候选 exe 定位。当前开发目录里的 tools/cores 可以掩盖普通用户失败，纯 layout 单测也未覆盖真正跨进程根传递。

风险：下载显示成功、再启动报 core_not_found，或者运行已有开发/旧版本。修复须把 engine 确定的根写入不可变运行合同/受管子进程，而不是要求用户手动设置两个环境变量。验收用离开仓库的干净 RC、合成下载源和独立 data dir，核对下载文件与实际进程 exe 一致。本轮未运行下载/核心启动。

### RR-02 · P1 · 托盘“切路由”仅改变编辑选择；状态栏切路由仍不重载

触发：运行节点→从托盘路由菜单选另一方案；或从状态栏选择默认方案。

当前 `apps/desktop/lib/app/shell/main_shell.dart:53-58` 将 tray onSelectRouting 接 `RoutingController.select`；`features/routing/routing_controller.dart:275-283` 只写 selectedId 和 reloadRules，不写默认路由。托盘 checked 又取 `desktop_integration.dart:181-188` 的 selectedId，所以能出现勾选变化但运行/默认配置没变。状态栏调用 setDefault，该方法 `routing_controller.dart:322-330` 保存后明确“未应用”；未调用 runtime apply。

原版 `U/ServiceLib/ViewModels/StatusBarViewModel.cs:400-417` SetDefaultRouting 成功后发 ReloadRequested 和图标刷新；MainWindow 的 reload 订阅消费该请求。应统一默认路由业务命令和原版生效时机，而不是复用编辑选中方法。未运行真实托盘或核心路由测试。

### RR-03 · P1 · 状态栏、托盘、重开三条系统代理路径仍分叉

触发：状态栏选 PAC/其他模式→重开；或托盘选 PAC 后在设置修改 pac.txt/绕过规则；普通重开期望继续使用已保存 ForcedChange/PAC。

状态栏 `apps/desktop/lib/app/shell/status_bar_view.dart:304-335` PAC 仍只是提示先启动服务，其他模式只 controller.apply，未保存 SysProxyType。托盘 `desktop_integration.dart:285-323` 有 PAC/保存，但默认用该文件 `361-362` 的极简“所有 URL 返回 __PROXY__”模板，没走已存在的 `PlatformController.startPacFromConfig` (`features/settings/platform_controller.dart:112-159`)；自定义路径与默认 pac.txt 用户链不一致。两边 server 仍取期望第一入站，而非实际 AppliedSession 地址。

更重要的是正常重开：`desktop_integration.dart:81-85` 仅回读 proxy state；随后 `app.dart:104` restore core，runtime plan 的 `engine.rs:2423-2426` system_proxy=None，没有再次按保存模式设置系统代理。原版 `U/ServiceLib/ViewModels/MainWindowViewModel.cs:704-705` 每次 LoadCore 后 UpdateSysProxy；`U/ServiceLib/ViewModels/StatusBarViewModel.cs:362-378` 模式改变会保存；`U/ServiceLib/Manager/PacManager.cs:13,32` 初始化实际 PAC 内容。

风险：显示保存模式却未恢复系统效果，同名菜单在不同入口行为不同，PAC 用户规则未消费。应共享 applied 地址、模式保存、PAC文件/服务和回读命令，原版启动/Reload时机贯通。ownership 退出恢复是规格要求的有意变化，不要求删掉外部更改保护。本轮未写宿主系统代理/PAC监听。

### RR-04 · P1 · 自身更新仍没有可执行产品闭环，RC 漏包且布局/CLI 不兼容

触发：更新页仅检查选中 v2rayN，或点击应用自身更新，然后按提示退出。

已有进展必须保留：core 安装在 `update_service.rs:522-525` 拒绝 v2rayN，`t16.rs` core loop 也跳过 App；应用 root 改为当前 exe 目录 (`t16.rs:673-691`)；verified staging 现会强制 detached signature (`update_service.rs:656-663`)。问题是：

1. 普通 `t16_check_updates` (`t16.rs:773-776`) 对所有选项调用 check_core；`check_core` (`update_service.rs:398-405`) 仍用 channel 的原版应用源。单独自身升级则用 `check_app_update` (`415-425`)，生产 app_repo 为 None（`83-94,282`）。因此普通检查与实际自身升级来源分叉；没有自己的发行源是已登记 blocked 条件，不能称现有 RC 已支持真实自身升级。
2. `t16_apply_app_update_spec` (`893-895,954-956,969-975`) 固定 prerelease=true、proxy=None，只返回 spec。Flutter `features/update/update_controller.dart:202-224` 仅存 lastSpec 并显示“退出后由外部 runner 执行替换并重启”，没有启动 runner/计划写入/退出交接。该提示不是已发生效果。用户 flags 不能贯通该动作。
3. 默认 runner 名是 `crates/updater/src/app_upgrade.rs:26-27` 的 v2rayN-upgrade.exe；实际 Rust binary 是 upgrade_runner。`tools/release/build_windows.ps1:120-141` 仅要求/拷贝 net_host、privileged_helper。当前公开 ZIP 重算 SHA256 `894d1569eeb397909162490b405ea59b148128f995fc091fbf75c1829a322fe6`，exe 清单恰为 net_host.exe、privileged_helper.exe、v2rayn_desktop.exe，无任何 upgrade 文件。Inno `tools/release/v2rayn-r.iss:62` 从同一 stage递归打包。
4. `crates/updater/src/install.rs:315-331` spec.args 仅 `[source]`，而 `services/upgrade_runner/src/main.rs:46-86` 必须有 --plan 和 --result。即使简单 Process.start(spec.helper,spec.args)，现 runner 也会报未知参数/缺计划。
5. `app_upgrade.rs:92-95` active payload 是 `<install_root>/app`，restart 也指向那里；RC 和 Inno (`v2rayn-r.iss:62-69`) exe/快捷方式平铺于 install_root。替换子目录不能自动更新原快捷方式/旧平铺 exe。

原版 `U/ServiceLib/ViewModels/CheckUpdateViewModel.cs:341-349` 检查 helper、启动 helper，`U/AmazTool/UpgradeApp.cs:106` 替换后 StartV2RayN。修复须先固定平铺/受管子目录合同、实际 runner 名/CLI、计划/结果文件和持久安装模式，再接发行源/签名和退出；不要只给提示文案或一组 DTO。未运行真实远端、验签、自替换、安装器，因此本轮 ZIP 内容证据仅证明漏包，不证明全部升级结果。

### RR-05 · P1 · 普通 TUN 仍依赖人工环境提示与未提权 helper

触发：普通 RC 从状态栏打开 TUN，不设 V2RAYN_R_TUN_* / HELPER_* 环境变量。

TUN已不是只改 Dart：`status_bar_view.dart:264-285` 保存 EnableTun再apply；`engine.rs:2431-2436` 附真实 descriptor。但 `engine.rs:2316` 从环境读取 hints；`crates/application/src/tun_plan.rs:39-58,75-83,117-123` 默认 interface_index=0，启用时立即拒建。没有普通 UI/平台设备创建后的接口发现路径。helper 参数在 `services/net_host/src/helper_client.rs:63-80` 只读环境，默认 token空、bin无；`:524-541` 因此不能连接/启动。`:666-686` 使用普通 Command.spawn，没有UAC/runas提权。

net-host `session.rs:549-578` 顺序是 helper 写接口地址/路由后才 spawn核心；当前 Windows backend `services/privileged_helper/src/windows.rs:408-420` 给已有 interface 写地址，设备本身由核心创建的合同与此顺序尚未解决。原版 `U/ServiceLib/Manager/CoreManager.cs:91-96` 有设备清理和实际核心/前置服务启动，Linux/macOS有独立CoreAdminManager授权合同。

错误文案还有两条问题：`status_bar_view.dart:288-293` 仅 value=true检查runtime.error；关闭TUN若apply失败仍显示已关闭并应用；helper失败提示运行状态未改变，但 net-host `session.rs:408` 已先停旧会话，实际可能已停止。应据 snapshot报告 desired/applied 和保留/停止事实。

需要设备发现、受控授权/取消、helper安全握手、完整计划及失败恢复；不能把 dry-run/人工interface_index 条件视为普通用户功能。未运行任何真实TUN、授权或宿主路由写入。

### RR-06 · P1 · 预 SOCKS 有图但未执行，其他12核心仍缺adapter

触发：Custom.PreSocksPort、LegacyProtect+TUN，或选择 v2fly/mihomo/其他原版外部核心。

`engine.rs:2376-2407` 新增 pre-socks graph；配置体却是 `v2rayn.presocks.plan.v1` 描述而非对应核心配置。`services/net_host/src/session.rs:440-445,676-681` 只解析 plan.target并启动一个exe，没有遍历 graph/start_order。`engine.rs:2253-2255` 已明确 FIX-13B 未执行第二核心，必须保留这一限制。`crates/runtime/src/adapter.rs:224-230` 仍只有 Xray/SingBox；其他枚举/版本URL不构成适配。

原版 `U/ServiceLib/Manager/CoreManager.cs:94-96,194,219` 主核心→等待代理端口→前置服务；`U/ServiceLib/Manager/CoreInfoManager.cs:120-283` 每核心候选exe、参数和env。Custom前置核心当前 `engine.rs:2293` 仍直接用node.core_type，原版 `U/ServiceLib/Handler/ConfigHandler.cs:1555` GetPreSocksItem以Custom默认核心解析，应逐对象核对。需要真正生成/启动/清理多个受管进程，而不是删枚举或将graph写入称为完整运行。未启动这些核心。

### RR-07 · P1 · 完整 Custom 配置的真实监听仍被普通节点期望端口代替

触发：导入有效完整 Xray/sing-box JSON，其实际 inbound/API端口与参数页第一入站不同，再应用/经代理更新/打开监控。

原文文件已能消费，这是 FIX-04B 的进展。当前 `engine.rs:2341-2364` 仍统一按 opts.local_port构造 plan port；`session.rs:465-476` 用该port进行preflight/ready，`:757`以后发布ports。`engine.rs:1462-1479` 将first reported port当proxy端点，`:1507` 固定http://；monitor accepted facts `:1386-1392` 则按普通runtime_codegen_options推API端口，未解析/验证Custom实际配置端点和协议。有效 Custom 使用其他端口就可能启动后ready超时；只有SOCKS的配置也可能被当HTTP下载代理。

原版完整 Custom passthrough与PreSocksPort/运行端口合同在 `U/ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs` 和 `U/ServiceLib/Handler/ConfigHandler.cs:1555` 明确区分。补普通/Custom/辅助端点的类型与来源，发布实际可验证地址；不能再次用期望值冒充applied事实。未以真实 Custom 核心复现timeout或下载，本文确定的是生产来源错误。

### RR-08 · P2 · 托盘菜单快照和主窗口操作未持续同步

触发：主窗口增删/切换节点、改默认路由或状态栏模式，再打开托盘；或托盘异步切节点后立即重开菜单。

动态构造已有，但 `desktop_integration.dart:94,269,279,323` 只在startup及tray动作即时刷新，没有ref.listen业务/运行/配置变化；节点 delegate返回前触发刷新不能等异步apply完成。节点checked/routechecked来自不同对象，图标固定初始icon，没有按proxy/core状态更新。原版 `U/ServiceLib/ViewModels/StatusBarViewModel.cs:385-387,413-417` 与RefreshServersMenu由业务事件刷新图标/菜单。应由共享read model稳定ID和实际状态订阅驱动；本轮未实际操作系统托盘。

### RR-09 · P2 · 刷新参数已消费，但隐藏门控/列状态/今日量仍不齐

Clash刷新已读ClashUIItem并持久化开关，不再是硬编码2秒。可定位缺口：`features/monitor/proxies_view.dart:49-76` / `connections_view.dart:49-75` timer仅看mounted，不看主窗口ShowInTaskbar；dispose不撤pageVisible；`desktop_integration.dart` hide只调用windowManager.hide，未通知这些页面。原版 `U/ServiceLib/ViewModels/ClashProxiesViewModel.cs:115`、`ClashConnectionsViewModel.cs:131` 的刷新有ShowInTaskbar门控。日志页dispose已有setVisible(false)，但窗口隐藏不dispose它，也需区分页面生命周期。

“今日” `status_bar_view.dart:209-210` 仍读 monitor.proxyUp/Down；`crates/bridge_api/src/api/monitor.rs:342` traffic DTO从StatsService的session_proxy投影，而 `crates/application/src/monitor.rs:258-264` 会话量随generation重置，和已落库node.today量不是同一范围。今日节点统计应从正确范围读取，不能以已接SQLite消除此标签差异。连接表的完整列宽/顺序交互和Groups→Details结构仍需原版同DPI实机对照；本轮未完成视觉或刷新节奏实测。

隐藏只暂停受可见性约束的UI刷新；后台统计不得因为窗口隐藏停止（原版StatisticsXrayService/StatisticsSingboxService没有ShowInTaskbar门控）。

### RR-10 · P1 · 无效切换会先停好会话，失败仍未恢复上次运行

触发：运行良好节点→切换缺exe/无效配置/不支持核心/失败helper。

`session.rs:408` 先stop，随后`:442,532,556`才检查adapter/locator/TUN；没有先执行adapter.test_args。rollback在`:840`清候选/拥有资源，不恢复之前核心。原版也先停后启，不应谎称原版绝对无中断；这是方案要求的“保留或恢复前一良好运行”仍未落实，且缺exe切换可在停旧前预检。applied事实忙碌时保留旧端点（engine.rs:1455-1490）还需与旧核心实际已停的时刻同步，避免下载指向已不存在的监听。未运行故障注入/真实核心切换。

### RR-11 · P1 · 卸载规则递归删除整个用户所选安装目录

触发：安装选择已有目录，或用户在安装目录放入未由安装器登记的自定义文件/便携资源，再卸载。

`tools/release/v2rayn-r.iss:71-73` 明确 `Type: filesandordirs; Name: "{app}"`，因此整个选定目录内容都在卸载删除目标。仅“限制在{app}”不等于仅删除本程序拥有文件；已有测试选空临时目录不会覆盖此风险。这里是新增installer所有权风险，不是宣称原版有同一安装器。建议让Inno删除自身登记文件、仅删确认空目录；若删受管残留，使用明确清单并保留外来文件/用户数据。本轮没有安装/卸载任何程序，数据删除是静态规则推断风险。

## 新接监控仍需重点实测的调度风险（未当成已复现）

1. `MonitorController` 在 `monitor_controller.dart:233-248` 对每个RuntimeView变化调用syncSession；`runtime_controller.dart:60-98` 对流seq更新也改RuntimeView；`monitor_bridge.dart:191` syncSession总调monitorStartPolling；`monitor.rs:1205` 每次清source_sig。poll_loop因此可因无关日志/heartbeat重新创建source。Xray source 的generation/last在 `crates/core_adapters/src/stats/xray.rs:101-106` 重建归零；SingBox source drop中止WS collector。需只在applied session/core/endpoint/统计开关改变时重新绑定，并实测普通日志下采样/跨会话归属。
2. `monitor.rs:1243` 使用current-thread Tokio，`:1320-1322` block_on即时new创建SingboxTrafficSource，source内部 `crates/core_adapters/src/stats/singbox.rs:130-147` spawn后台pump；`:169-170` poll是立刻返回的snapshot，外循环`:1304`用阻塞sleep，缺持续驱动后台task的await节奏。存在pump不获调度/长期空采样的结构性风险；独立adapter测试运行在自己的活跃Tokio环境不能证明这一生产loop。应在同一production hub +真实受管sing-box +Flutter同屏非零流量的合成场景验证。未启动WS服务器/核心验证，所以本项标源码风险，不写“实测全部恒0”。
3. `sync_from_engine_session:1210-1214` Store打开/加载失败被忽略且store_bound=true；若第一次失败或恢复换Engine/dataDir，后续不再尝试绑定。必须有可见故障和恢复后的hub/store重建合同。相关restore跨域由settings代理复查。

## 后台与跨平台余项

订阅scheduler正常入口已存在，不再重复“所有定时任务都没启动”。仍未找到原版总TaskManager的小时cleanup/Geo与每日CheckHasUpdateOnlyAll/提示入口；原版 `U/ServiceLib/Manager/TaskManager.cs:23-83,127` 与当前只有SubsController.startScheduler的范围应分开登记。已有即时保存可以替代无必要的20分钟重写，但不得替代Geo/每日检查。

Windows helper已打包仍不等于普通授权TUN可用；非Windows helper `services/privileged_helper/src/main.rs:7`、transport与真实平台backend仍各有缺口。Linux关闭行为也不能照当前注释的“非Windows允许destroy”认作原版：冻结Linux为Hide2TrayWhenClose=false时最小化。macOS/Linux/ARM64本轮未构建、未验证。

## 本轮实际命令与边界

| 检查 | 实际结果 | 说明 |
|---|---|---|
| git status/log/diff、rg和固定源文件读取 | 完成 | 开始的用户四个profiles未提交文件未修改；随后根提交/刷新后重新确认运行源码未变 |
| Python zipfile清单与SHA256复算 | 当前RC哈希894d1569…；仅3个exe，无upgrade文件 | 没解压/运行ZIP，没有读取个人数据 |
| `C:/Users/Colby/.cargo/bin/cargo.exe test -p runtime --lib install_layout::tests --locked` | exit0，4 passed | 纯目录/版本映射测试，未证明两进程同根 |
| `C:/Users/Colby/.cargo/bin/cargo.exe test -p application --lib monitor::tests --locked` | exit0，17 passed | 合成计数/日志/SQLite测试，未验证production Tokio loop或Flutter/核心整链 |

没有执行完整workspace门禁、Flutter构建/GUI、远端TLS/代理下载、自替换、Inno安装卸载或平台写入。中途一次读取不存在的coordinator.rs失败，之后按install.rs定位；不存在文件的输出没有作为功能证据。此报告中的源合同差异不声称本轮全流程verified。

优先下一步：先修RR-01真实下载到运行的根合同；共享RR-02/03的路由/代理命令与applied状态；RR-04固定发布包/runner/CLI/布局，保留发行源blocked边界；RR-05/06完善设备发现、授权和受管进程执行；最后以实际会话验证监控与生命周期。每张卡只承诺一个完整用户流程，不能把UI提示、图节点或单测绿当作效果闭环。
