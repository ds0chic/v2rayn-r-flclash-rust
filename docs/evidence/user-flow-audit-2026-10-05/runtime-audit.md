# 运行、平台效果与发布链路：用户流程审查（2026-10-05）

基线：应用 HEAD `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`；冻结上游 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`，Windows 以 WPF 为对照。本文 `U/` 指 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。当前 ZIP 内 build-info 的源码为 `73da06eb52a5989aeb31f0071b7ef03a72c4bd19`，HEAD 的最后提交只更新 RC/证据；不能把新未发布修复自动算入该包。

本轮只读生产代码、公开包和冻结源；只写本报告。没有读取用户配置、订阅、节点、日志或凭据，没有连接现有 named pipe，没有监听/修改 10808，没有写系统代理、自启、TUN，也没有停止外部进程。旧任务卡/审查只作导航，每项以下均重新看当前源。报告中的 `identified` 表示当前源码确认缺口，`blocked` 表示当前所需产品输入/真实验收环境缺失；不以历史台账状态或函数存在认定迁移完成。

## 1. 用户报告“应用、双击、Enter 无效”的生产链

### RUN-01 / P1 / identified：表格选择、持久化默认、实际会话三者没有完整用户契约

当前表格单击只改 `selected/primaryId`（`apps/desktop/lib/features/profiles/profiles_controller.dart:905-923`），这是合理的原版选择语义。顶部“应用”直接调用 `RuntimeController.applyActive`（`apps/desktop/lib/app/shell/main_shell.dart:676-677`）；bridge 给 `applyRuntime` 传空 `targetId`（`apps/desktop/lib/features/runtime/runtime_bridge.dart:192-194`）；Rust 空目标只解为持久化活动 ID，缺失即 `error.no_active_profile`（`crates/bridge_api/src/api/engine.rs:734-748`）。因此“导入/新增后单击一行→应用”并不把该行作为目标；已有 A 默认时单击 B→应用仍使用 A。

当前 controller 初始化/重新加载只读取 active（`profiles_controller.dart:337-338,496-505`），没有冻结原版的默认恢复过程。上游 `ProfilesViewModel.GetProfileItemsEx` 调用 `ConfigHandler.SetDefaultServer`（`U/ServiceLib/ViewModels/ProfilesViewModel.cs:397-402`）；默认不在当前列表、但仍在数据库时保留它，完全缺失时从当前列表或数据库取首个 `Port>0` 的候选并保存（`U/ServiceLib/Handler/ConfigHandler.cs:427-444`）。这能使首次有可用节点后的默认状态稳定，不能拿“请先设活动”永久代替原版行为。

双击/Enter 的相同默认返回本身是原版幂等：上游 `ProfilesViewModel.cs:575-577`；台账 `ACT-PROF-005`（`compat/actions.yaml:1813`）。当前 `setActiveSelected` 相同活动时提示并返回，`activateProfileDetailed` 相同活动则直接给 `persisted=true, applied=true`（`apps/desktop/lib/features/profiles/profile_actions.dart:509-515,546-555`），后者把落库事实冒充实际运行。在第一次新活动 ID 已保存而 apply 失败后，再双击/Enter 不会再次启动。这个无操作合同应保留在“设为默认”动作，但显式应用/F5 必须是可靠重试入口，反馈也不能称未运行会话已应用。

最小条件（源码确认，真实 GUI 未运行）：空 active + 合成有效节点 + 普通单击→顶部应用；或 A 活动 + 选择 B→顶部应用；或首次设 B 默认后核心缺失→重复双击 B。应分别显示清楚目标/默认恢复或执行原版设默认流程、明确应用的是 A，以及保留同默认幂等并给可靠的显式重试。

### RUN-02 / P1 / identified：历史错误在重试前否决新命令

生产链：`net_host.apply_plan` 预检失败把错误留在 `inner.detail.error`，旧会话可继续 Running（`services/net_host/src/session.rs:926-945`）；`NetHostClient.map_snapshot` 原样映射 detail error（`crates/application/src/net_host_client.rs:260`）；Dart `applyActive` 清本地错误后立即刷新，读回历史错误，然后 `if (state.error != null) return`（`runtime_controller.dart:111-114`）。这时不会再次发 apply，也不会检查原因是否已修。F5 的 `reload` 自己也在刷新后遇 error 返回（同文件 `:148-169`）。核心已下载、路径已纠正、配置已保存均不能越过旧快照错误。启动恢复亦有同类 gate（`:133-138`）。

安全故障注入已证实这个控制器条件：首次 apply 故意返回 `error.core_not_found`，随后让 mock 可成功，第二次 apply 仍只有一次 bridge 调用。根代理的可复跑证据：`startup-contract-repro.dart` / `startup-contract-repro.log`（本目录），显式应用预期 2 次实际 1、F5 预期 2 次实际 1。mock 只注入故障，不代表真实内核/GUI已测。

修复时必须区分快照请求失败与上次运行失败：前者影响读最新 revision，后者仅作为历史反馈，不能禁止用户新的 apply。保留旧错误、重试操作 ID 与新尝试结果的关联，防止后台事件覆盖新结果；不能仅清 UI 文本伪造恢复。

### RUN-03 / P1 / identified：请求在途时 UI 尚未 busy

`RuntimeView.isBusy` 仅看 net-host 状态 `Validating/Preparing/Starting/Checking/RollingBack`（`runtime_bridge.dart:78-83`）；顶部按钮只用此字段禁用（`main_shell.dart:677`）。`applyActive` 开始 snapshot/FRB 等待时没有 controller 命令在途标记，也没有通用 apply 重入闸。F5 另有 `_reloadInFlight/_reloadPending`，不覆盖顶部应用、节点激活和停止。根安全探针等待 apply 闸期间，`isBusy` 实际 false，契约期望 true（本目录 `startup-contract-repro.log`）。

不可用编造的 Running 状态修复：应分别表示“命令正在提交/排队”与“后端实际运行状态”；明确重复 apply 合并、最后 desired 补跑、stop 排序/取消安全点合同。否则按钮可以继续点击，新的 snapshot/apply 排到共享队列，用户感到每次都迟钝。

## 2. 延迟来源：已确认的等待链与尚未量化的风险

### RUN-04 / P1 / identified：IPC 总时限不包括排队，超时没有取消阻塞工作

`NetHostClient.do_request` 先 `req_mutex.lock()`，取得锁后才新建线程并 `recv_timeout`（`crates/application/src/net_host_client.rs:272-297`）。snapshot 设置 5s、apply/stop 60s（`crates/ipc_contract/src/lib.rs:32-35`），因此这些是锁后等待上限，排队等待没有上限。例如一个 apply 占用锁 60s，后面的 snapshot 总耗时可超过 65s；连续点击再叠加更多请求。worker 对同步 `File` pipe `read_exact` 没有自己的截止时间或取消（`net_host_client.rs:299-336,445-459`），外层超时只丢弃等待，worker 可继续阻塞。超时后返回错误亦不保证后端 apply 未稍后成功，必须能按 operation 查询和对账。

当前 getSnapshot、applyRuntime 是 FRB `executeNormal` / Dart Future（`apps/desktop/lib/bridge/frb_generated.dart:653-678,1741-1762`），不能说它们直接同步冻住 Dart UI线程。已确认的是用户命令迟迟完成、忙碌反馈滞后和队列积压；真实 frame time/调用耗时未量化。应测队列等待、执行耗时和帧时间，分别报告。

`RuntimeController.refresh` 无在途合并或过期响应丢弃（`:95-101`）。命令主动 refresh 与状态事件 120ms debounce refresh 可能重叠（`:48-58`），都进入同一 IPC 锁。**当前已过滤事件种类，只接受 runtime_detail/runtime_state_changed/error_raised/lease_reclaimed；heartbeat/log 不刷新，旧“任何事件刷新”已修，不能重复列为缺口。**

预检另有真无界点：`session.run_config_check` 注释称 bounded，实际 `spawn_blocking → Command.output()` 无子进程超时（`services/net_host/src/session.rs:839-875`）。客户端 60s 时限不能停止这个校验进程。必须使用本项目持有的子进程句柄做有界等待与安全清理，验证合成挂起校验核心；不可按进程名杀外部程序。

### RUN-05 / P1 / identified：监控同步 FRB 争用持有磁盘工作的锁

`MonitorController.syncRuntimeSession → MonitorBridge.syncSession → monitorStartPolling` 是同步端点（Dart `monitor_controller.dart:293-303`、`monitor_bridge.dart:191`；Rust `crates/bridge_api/src/api/monitor.rs:1323-1325`）。不过 engine `monitor_session` 只读 applied_session/apply_facts 缓存（`crates/application/src/engine.rs:1783-1808`），**不调用 runtime.snapshot、不进入 req_mutex**。不能误称它直接等待60s named pipe。

真正同步 UI风险是 MonitorHub 共用 mutex：`with_hub` 在锁内执行 closure（`monitor.rs:392-395`）；sync `stats_snapshot/get_logs/monitor_start_polling` 等争这把锁（`:452-469,511-534,1323-1325`）。后台 poll 对网络 `src.poll().await` 时已释放锁，网络5s超时本身不会占该锁；但得到样本后锁内 `h.stats.flush_store()`（`:1396-1399`）遍历全部节点逐行 upsert（`crates/application/src/monitor.rs:357-361`），SQLite 写入路径是 `crates/application/src/store_repo.rs:806-815`。首次绑定也在同一锁内加载全库（bridge monitor `:1263,1292-1295`）。慢盘/多统计行时同步端点能阻塞UI。这是源链确认的性能风险，未做定量磁盘/帧实测，不能认定已解释用户所有延迟。

现有修复要保留：session签名过滤无关事件（Dart `monitor_controller.dart:293-297`），sing-box WS poll loop 已在持续 Tokio executor 内（Rust monitor `:1334-1350`），网络请求与锁分离；不能复述旧“WS从未被调度/每个心跳都重绑”。修复应提供后台存储 worker/批量事务/增量dirty行，读模型短锁复制，再把大读/加载端点改异步；stats开关、日期 rollover、隐藏页仍收集、断连重连、节点归属、存储失败反馈均须保留。

## 3. 生效与恢复事实

### RUN-06 / P1 / identified：活动节点改变未推进 desired，重连缺少真实会话元数据

`AppEngine.set_active` 只验证节点、写 active、persist_config，持有的 revisions 不 `bump`（`crates/application/src/engine.rs:912-930`）。不同活动 ID 已保存、apply失败时 desired/applied可仍相等；Dart“未应用”只看两 revision 不等（`runtime_bridge.dart:72-75`），因此缺少差距提示。相同默认保持幂等是上游要求；不同默认必须推进本重构的 desired版本/目标事实。

成功 apply 后 engine 记录 `apply_target/apply_facts`（`engine.rs:1584-1597`），预检失败前不会替换它们，这是已有保护。但全新 GUI engine 重连旧 Running net-host 时这些是内存缓存；`reconcile_applied_session` 缺 apply_target 就回退 desired active（`:1669-1674`），`monitor_session` 缺 apply_facts 就回退 Xray/default API（`:1790-1798`）。host快照目前没有完整已应用 node/core/version/endpoint protocol/API facts，须由唯一进程持有者在协议提供，而非从当前设置猜。典型待验收条件：A真实运行→设 B默认但预检失败保留A→快速退出/重开复用host；不得将A流量记为B，sing-box不得当Xray轮询。

这是源码缺口/待实测，未运行真实快速重开，也未读取用户会话。另 app engine 明确 `target_id` apply时也从 `self.active_profile()` 记录目标而非plan携带目标，现UI传空目标掩盖了它，后续若显式应用选择必须同时修结构化目标合同。

### RUN-07 / P1 / identified：成功手动重载没有系统代理/PAC对账；Custom协议信息也未到Dart

上游每次 Reload 完成 `LoadCore` 后调用 `SysProxyHandler.UpdateSysProxy`（`U/ServiceLib/ViewModels/MainWindowViewModel.cs:702-705`）。当前普通启动有 restore mode（`apps/desktop/lib/app/app.dart:105-110`）；状态栏/托盘/热键可调用 `applyModeFromConfig`。但是手动 Apply/F5/切换节点的成功链不调用它；desktop runtime listener只同步托盘（`desktop_integration.dart:216-226`），全库 applyMode入口没有成功应用监听。因此先选强制代理/PAC、再改端口/应用Custom或恢复新会话时，系统设置/PAC脚本可继续指旧端点。Stop后的模式处理也应按上游和所有权对账，不能仅更新图标。

`_appliedProxyInbound` 只得到扁平 runtime.ports，再优先匹配当前desired设置的端口与协议，匹配不上则用 ports.first + 当前desired协议/default HTTP（`platform_controller.dart:248-260`）。Rust Custom 已解析真实入站/secret等（`engine.rs:1742-1759`），但桥接没把代理协议/listen/auth事实交Dart；SOCKS-only Custom可被误当HTTP，认证/非loopback不可用也未在此判定。应扩充只含所需事实的端点DTO，敏感认证不可进日志/证据；不把第一个任意监听端口当系统HTTP代理。

本轮未写宿主系统代理/PAC，以上为源码确认断点。真实验收需授权隔离VM，普通四模式先读取实际系统状态，再用合成端点验证新端口生效、PAC内容/来源、失败保留旧端点、用户外部更改字段的所有权保护。

### RUN-08 / P1 / identified：退出/升级交接未统一；断连回收已有，不能称永久泄漏

原版 `AppManager.AppExitAsync` 按顺序恢复代理、保存配置/节点统计、CoreStop、关闭统计，再shutdown（`U/ServiceLib/Manager/AppManager.cs:122-146`）。当前 Windows X隐藏至托盘（`desktop_integration.dart:356-361`），符合WPF；不能把X当退出。真正 tray `exitApp` 只有 stopPac/restoreProxy/unregisterHotkeys/destroyWindow（`:375-380`），bootstrap dispose 停订阅 scheduler（`app.dart:80-83`），没有 await显式 runtime stop、统计flush和完整受管host终止交接。

net-host确有最后连接断开后6s grace的受管会话回收（`services/net_host/src/session.rs:71,436-459`；`server.rs:228-239`），Job Object持有子进程；所以不是“退出永久留下core”。但这是崩溃兜底，正常退出/安装不能依靠时间竞赛。host本身继续accept，`NetHostClient.connect`优先复用固定pipe（`:409-410`），不会将新cores根或新服务版本传给已活host。

App升级在stage/runner成功后500ms直接 `exit(0)`（`apps/desktop/lib/features/update/update_controller.dart:34-37,313-320`），绕过DesktopIntegration退出（系统代理/PAC/统计/调度清理不完整）。runner只等给定GUI PID，不等host/helper（`services/upgrade_runner/src/main.rs:35-42,107-123`）。旧 net_host持有旧程序文件，正式换包/快速重开可能重用旧服务或遇文件占用：这是尚未跑的风险，不是本轮实机失败结论。

Inno已改为仅删本应用 `.staging/app.previous`、空目录，未整目录递归删用户文件（`tools/release/v2rayn-r.iss:71-78`）；此保护必须保留。仍需正常关闭应用→停自己服务→确认包文件可替换→安装/卸载的统一有界流程；不得按名字批量kill进程。

## 4. 安装、更新、后台任务与发布实际状态

### RUN-09 / P1 / identified + blocked：无核心首用恢复、活核更新与自有发行信任根

包不捆绑内核是发布政策，不是漏包。已读当前ZIP：含 `v2rayn_desktop.exe`、`bridge_api.dll`、`net_host.exe`、`privileged_helper.exe`、`v2rayN-upgrade.exe` 和Flutter/plugin资源；无内核exe、节点/用户数据库。安装与运行共用engine data_dir/cores、CoreInstallLayout（`AppEngine::cores_root`；`update_service.rs:599`；`runtime/adapter.rs:33-45,79-98`；NetHostClient `apply_launch_env`）。旧“下载到A、运行从开发B找”已修；本文不重开旧问题。

首用缺核 `CoreLocator` 如实 `error.core_not_found`（`crates/runtime/src/adapter.rs:103-115`），但状态栏仅打印 code+messageKey（`apps/desktop/lib/app/shell/status_bar_view.dart:175-179`），没有清楚核心/修复动作；结合RUN-02，下载后还无法重试。应给正常产品化安装引导，并保留取消/离线安装/详细失败/安装→定位→真实运行链。

更新后UI只显示“已更新x个内核”，没有重载当前核心的链（`update_controller.dart:204-249`；Rust `t16.rs:946-1015` 仅循环检查/下载/安装）。安装器替换目录并保留previous（`update_service.rs:588-639`），但该入口不停止旧活核，不判断旧进程的文件句柄，也不让用户知道“磁盘版本已安装、实际仍旧版运行”。原版更新完成可发布ReloadRequested（`U/ServiceLib/ViewModels/CheckUpdateViewModel.cs:320-329`）。新合同须下载/验摘要→预检候选→安排安全切换/回滚→确认实际进程exe/version，不将“装好了”当“用上了”。活核Windows目录替换文件锁是否失败未实测，勿写成必然失败。

自身更新当前代码有真实runner和rpgp/GPG验签，不能复述“仍默认Unsupported/只返回spec”。预发布/经代理 flags已被 `last_update_flags` 读取（`crates/bridge_api/src/api/t16.rs:1022-1024`），旧固定true/None也已修。

然而正式产品源/信任根仍未产品化：`app_repo`只可通过debug `test_app_repo_override`，release分支恒None（`crates/application/src/update_service.rs:83-94`），正式build不靠普通env就能启用自更新；`app_signature_verifier→v2rayn_app_verifier`仍固定上游2dust的公钥（`update_service.rs:880-882`；`crates/updater/src/signature.rs:448-456`），不能用来签本产品自主发布的Flutter包。要提供编译固定的本产品repo、资产命名、版本策略和自有可信公钥；保留上游核心源与许可归属，拒绝把原版WPF包覆盖进本产品。无这些发行输入时保持 `blocked`，不得跳过签名，也不得填虚构repo。

### RUN-10 / P1 / identified：后台TaskManager只迁移了订阅周期部分

当前普通启动已 `SubsController.startScheduler`（`app.dart:113-114`），关停/restore也有专项生命周期，不再是旧“scheduler只定义不调用”。SubItem.AutoUpdateInterval的due判定在 `crates/application/src/subs.rs:841-842`。但全库 GuiItem.AutoUpdateInterval只有设置/DTO保存，没有全局geo周期消费者；上游 `TaskManager.UpdateTaskRunGeo`按此字段按小时更新（`U/ServiceLib/Manager/TaskManager.cs:118-127`），另有周期check更新（`:131`起）。因此订阅调度已接不能代表原版TaskManager完整迁移。

应核对任务全表（订阅、geo/规则资源、检查App/核心、定时保存/统计、失败重试/并发/退出/恢复），逐项输入→持久化→重开→实际下载/生效。用虚拟时间测周期，不靠sleep等数小时；真实网络仅使用合成隔离服务，未知来源不得在审查期下载。设置消费者详表由settings审查交叉确认。

### RUN-11 / P2 / identified：apply任务没有终态接线

engine在 runtime已返回Accepted之后创建 `jobs.start("apply_runtime")`（`engine.rs:1584-1601`），全库生产 finish调用只见订阅/speedtest域，没有apply operation→job终态关联；snapshot把这些非终态任务保留（`:2889`；`jobs.rs:203-216`）。重复apply可积累active_jobs、让取消/进度含糊。没有把它夸大为restore永久失败：restore drain明确只筛 `update_subscription`（`engine.rs:418-423`）。应修操作ID/jobID映射、成功/失败/取消终态、在途清理与bounded历史。

### RUN-12 / P1 / identified：TUN已有后端实际租约，桥接丢弃；关闭失败仍反馈成功

当前首TUN创建顺序已修（见下节既有实现），本项是普通操作到实际结果的缺口。net-host `RuntimeDetail.tun` 已带脱敏 adapter/index/route_count/dry_run（`crates/runtime/src/wire.rs:52-64`），client也透传至 `RuntimeSnapshot.tun`（`net_host_client.rs:263`起），application snapshot保留（`crates/application/src/snapshot.rs:101`）。但是 `bridge_api.snapshot_to_dto` 构造SnapshotDto时没有s.tun（`api/engine.rs:373-405`），DTO/RuntimeView没有该字段，Dart拿不到lease事实。

`tunActualLabel`依据desired决定“未启用”，再按是否Running给“已请求(未验证)”（`apps/desktop/lib/features/runtime/tun_toggle.dart:44`起），没有实际TUN指标。关闭开关desired=false立即显示“未启用”，即使新计划预检失败保留旧TUN会话。`toggleTunDesired`以void apply返回就标runtimeApplied=true（同文件`:26-42`）；`_onTunToggle`只在value=true时检查runtime.error，value=false有错误仍走“ TUN已关闭并应用”（`status_bar_view.dart:271-297`）。开启失败的所有错误（包括core缺失/配置失败/revision失效）也统一误报“授权被拒绝/helper不可用”。

上游开关改变后保存并发布Reload（`U/ServiceLib/ViewModels/StatusBarViewModel.cs:430-460`），取消授权重置desired（`:450-454`）；本产品可用最小helper提权改进权限，但必须明确取消、失败保留会话和desired/applied的语义，不把保存成功等于平台关闭。

最小安全故障条件：fake runtime仍Running且有旧TUN事实、关闭desired、apply返回失败；预期“关闭未生效/旧TUN仍运行”，当前文案成功。真实设备/UAC/路由未运行。本项须贯通新增脱敏DTO、FRB生成、运行真实事实与四种反馈（未启用/已生效/请求中/失败保留或回滚）；不能只改汉字或在错误时推断设备已清掉。

交叉ROOT存储卡的运行影响（P1 / identified）：`bridge_api.engine()` 打开存储失败静默fallback `AppEngine::in_memory`（`api/engine.rs:77-82`），后者使用NullRuntimeClient（`application/engine.rs:151-152`）。其snapshot给host_alive=true（`runtime_client.rs:182`），apply返回Accepted却不启动任何核心（`:189-195`）。当前profile动作 `_applyRuntime` 只检查runtime.error是否为空，仍可将Stopped/无PID视为applied。这是条件性源码结论，未故意破坏用户存储、未证实用户本次就是此原因。ROOT初始化修复须生产fail-closed、显示持久化初始化错误、禁止自动换stub；只在测试显式使用in-memory，并让运行成功以正确operation/session/applied结果确认。

## 5. 本轮可直接交给实施模型的任务卡

以下为运行领域修复卡输入。主方案应按固定小任务模板拆文件，每卡只完成一个主用户流程，所有功能/fields/action ID分母保留；接口缺口登记建议，不降低要求。不宜让同一低一级模型一次修改全部运行与平台模块。

| 卡 | 唯一用户流程 / 前置 | 允许模块 / 必须保留 | 具体输出与完成条件 |
|---|---|---|---|
| UF-RUN-01 | 新数据目录导入合成节点→默认恢复→顶部应用，前置冻结ACT-PROF-005/FIX-07 | profiles controller / application default resolution / shell入口；普通单击不自动激活、同默认幂等保留 | 迁移ConfigHandler.SetDefaultServer候选/过滤/数据库回退语义；顶部文案/目标明确；空数据、首节点、已有默认选另一行、默认缺失、仅Custom/Port0等fixtures逐项对照；真实包正常UI验证 |
| UF-RUN-02 | 首次缺核心失败→安装/修复→显式应用或F5成功，前置01目标合同 | RuntimeController/bridge snapshot错误合同、net_host历史错误；desired/applied分离保留 | 新命令不被历史错误否决；snapshot通信失败不返回空成功（get_snapshot:443要结构化保留error）；旧错误/新operation正确关联；根3个失败断言全部通过，真实独立目录缺核→安装→同入口Running |
| UF-RUN-03 | 连点应用/重载/停止有即时反馈且最终使用最新desired，前置02 | controller command state与runtime IPC/client/server；UI不拥有实际Running状态 | pending与实际状态分开、请求合并/排序/截止时间含排队、refresh去重/过期丢弃；合成挂起校验/断pipe/60s超时不得积累worker或迟到假成功；只终止本项目句柄；Windows真实隔离pipe与release frame trace |
| UF-RUN-04 | 改活动节点→失败保旧→重开仍正确显示实际A和待应用B，前置02/03 | ipc_contract/runtime detail/application applied facts/FRB DTO/runtime UI；失败保留/回滚不回归 | 只在ID改变bump desired；host发布目标/core/version/hash/typed端点/API身份；新engine重连不猜Xray/desired节点；DTO重生成no-diff；真实合成A→B失败→快速重开检查PID/hash/统计归属 |
| UF-RUN-05 | 強制代理/PAC已有→成功切新端口→系统真实跟随，前置04typed端点 | platform controller/runtime成功协调/bridge/platform；所有权保护与四模式、真实PAC来源保留 | 应用完成后的platform对账，Custom SOCKS/HTTP/mixed/auth/listen明确；失败保旧、stop/restore符合冻结规则；纯平台fake先验证，最终授权隔离VM读实际注册表/WinINET/PAC及流量；本宿主禁止写 |
| UF-RUN-06 | 有流量统计时滚动/菜单/应用保持顺畅，前置04正确归属 | monitor hub/store读模型/FRB监控端点；统计开关/日期/隐藏页/断连保留 | 磁盘工作离开读锁、dirty批量事务、异步加载；慢store故障注入保证sync/UI不等FS；10k统计行+连续日志+代理面板release trace，分别报告p95/p99命令与帧耗时，无网络统计假成功 |
| UF-RUN-07 | 真退出→快速重开或自身升级→旧服务清理/恢复正确，前置03/04/05/06 | desktop/bootstrap统一lifecycle、runtime Shutdown IPC、updater handoff、installer协作；X隐藏保留、不得删用户文件 | 所有真实退出共用有界scheduler drain/statsflush/PAC代理恢复/managed stop/hotkey清理；正常无wait6s竞赛，崩溃仍watchdog回收；service版/根握手；旧进程终止后换包并读runner结果；隔离真实安装/升级/卸载复验 |
| UF-RUN-08 | 缺核→正常安装→运行；运行中更新→候选/回滚→确实用新版，前置02/03/07 | update service/controller/core locator/runtime协调；同根/摘要/签名/previous回滚/许可保留 | 安装状态与运行版本分开；验证磁盘/exe/version/会话而非“applied count”；升级时机按原版和本产品事务合同；错误/取消/代理下载/离线路径均有反馈；14核心适用矩阵逐核保留，不能将typed节点裁成仅Xray |
| UF-RUN-09 | 本产品自身检查→下载验签→退出换包→重启，前置07，缺真实发行repo/公钥则blocked | build发行常量、App updater/signature/runner；core/App分流/拒WPF包/不绕验签 | 自有发行源/信任根/资产协议固定并允许受控测试注入；签名错/缺/下载断/替换失败/rollback/重启结果；真实签名fixture与隔离正式包端到端；用户取消不改运行状态 |
| UF-RUN-10 | 开启全局周期更新→重开→到期geo/检查等真实任务，前置08/09 | TaskManager对应application scheduler及settings消费者；已有subs schedule/停机恢复保留 | 全表逐项任务迁移，虚拟时钟/失败退避/无重复并发/取消退出；合成下载→资源替换→必要生效动作；字段持久化不是完成证据 |
| UF-RUN-11 | 一个apply从排队到终态进度可追踪，前置03操作协议 | application jobs/runtime event mapper/bridge；不伪造进度百分比 | operation与job同一关联；success/error/cancel终态，一次完成一次撤下active_jobs，恢复后可查询；多次apply后活动任务清空且历史有上限 |
| UF-RUN-12 | 运行TUN→关闭失败/取消→正确保留或清理实际租约，前置02/03/04/07 | RuntimeDetail到SnapshotDto/FRB/RuntimeView/TUN toggle/controller；首TUN deferred与失败清理保留 | void apply改结构化结果，desired/actual两套事实，开启/关闭/取消都检查真实结果；透传脱敏lease，不暴露token；FakeHelper只注入拒绝/超时，最终隔离VM真实UAC/设备/地址/路由/停止/重开验收，不能将stub当实测 |

每卡证据至少含：固定HEAD/包hash、原版符号与合同、合成输入ID、用户入口、bridge/engine/host持久化及重开、实际进程/配置/平台事实（所需值脱敏）、真实命令与exit、未运行/未验证项。控制器/内存mock只证明控制流；配置生成单测只证明文本；实际效果不能用二者代替。测试端口≥11808先探测，只停本项目持有的会话；系统写入/提权只在明确授权隔离环境。

## 6. 现有实现与不可误写为仍缺失的事项

1. 安装与运行的核心根已统一；定位支持受管版本目录/alternate exe names，没有重新发现旧分叉。
2. `build_runtime_plan`生产确实读持久化profile/settings/routing/DNS，生成后hash进入plan（`engine.rs:2648-2658,2717-2718`），net-host预检早于停旧会话（`session.rs:918-947`），失败保旧/失败启动恢复已有；不是“启动仍硬编码冒烟配置”。
3. 原生Custom正文、Xray/sing-box解析真实端点、pre-SOCKS sidecar图、first-TUN deferred核心先创接口再helper路径已在当前源码；先前未实现项不能原封不动抄回来（`engine.rs:2660-2702,2757-2801,2825-2841`；`session.rs:1285`起）。本轮未复跑逐核、TUN真机，不将旧证据当新验证。
4. sing-box统计executor、session签名过滤、Custom API secret同步已有，监控非每个心跳重绑；网络poll不持hub锁。剩余typed平台端点与存储锁应修。
5. Windows X隐藏，菜单/托盘显式退出区分已有；Job Object和断连6s回收已有。剩余正常退出/升级交接及新engine重连事实需补。
6. 自升级runner已打包、实际detached launch与exit接线、App/core发行分流和OpenPGP验签已有；App flags已透传。正式自有源/信任根仍blocked，不把原版签名源当本产品源。
7. 订阅scheduler普通启动已有；全局geo/检查TaskManager仍缺消费者，二者不要混同。
8. Inno用户/外来文件保护已有；安装/卸载中的活会话停机和服务版本升级仍须真实验证。

## 7. 本轮实际命令与证据边界

- 只读 `git rev-parse HEAD / git log -4 --oneline / git status --short`；当前HEAD如上。`profile_actions.dart` 在并行协作者工作中有未提交状态，未修改/恢复/提交它；本报告对其引用是本轮读取的行为。源引用以77c74ed为审查基线，实施前要核对最新版本。
- `rg`检索、`Get-Content`按行读取当前模块、四台账相关项、FIX-07/RR-01/RR-04/RR-10/R3-ROOT-01/R3-04/R3-06/R3-07任务卡和冻结上游；未运行cargo全workspace、真实native UI、平台写入测试。若检索旧路径不存在，已找当前模块位置；不以路径缺失判功能缺失。
- 自建临时外部Flutter故障探针：`flutter test --no-pub <TEMP>/runtime_start_probe_<guid>_test.dart`，只override synthetic bridge/runtime/store。一次retry故障条件通过（修复后仍applyCalls=1）；same-active探针的provider/mock目标不匹配导致其预期断言失败，**不把这个失败用作缺口证据**。临时文件已按LiteralPath删除。same-active事实以上游与生产源码为证。
- 根代理正式保留探针 `startup-contract-repro.dart/.log`：3个期望修复后合同断言在当前实现失败（retry=1而非2、F5=1而非2、pending busy=false），无native crash；本报告引用其故障注入证据，不称本代理真实UI验证。
- ZIP通过 `.NET ZipArchive`只读枚举；含四个本产品exe与bridge/plugin/Flutter资源，无core exe或用户DB。`Get-FileHash`复算SHA256与dist/SHA256SUMS一致：ZIP `45a702b7527843fe4344c11260d6ae2e2510700f1654880f09dfd9dceb01484c`；setup `a8fd08e29e0331ff8fb9cb15601cc5f0b1c32a7b5f52e172ad947b7e22608b64`。只检查包内容与hash，没有执行安装器/正式包。

下一步优先 UF-RUN-01/02/03，给用户可启动、可重试、即时反馈的完整基础流程；然后04/05/06/07；其余更新/周期功能需按完整移植范围保留。报告不是完成率证书；本轮没有生产修复，没有把尚未运行的真实验收标成verified。
