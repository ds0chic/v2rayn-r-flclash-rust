# R3-WPF-COMPARE — 冻结原版 7.25.4 WPF 构建并做三窗口可见结构对照

状态：`implemented`（冻结源已用 dotnet 10 SDK 成功构建并隔离启动原版；主窗口 + 参数设置 + 路由设置三窗口真实截图与 RC 同三窗口截图已完成可见结构对照。**未做全流程逐事件对照、未改任何生产代码、未做行为等价判定**，故不写 `verified`）。

任务 ID：R3-WPF-COMPARE

本次唯一用户流程：在隔离数据目录下构建并启动冻结原版 v2rayN 7.25.4（WPF），截主窗口、设置窗口、路由设置窗口；用同一个 RC 包（`dist/v2rayN-R-1.0.0+1-windows-x64/v2rayn_desktop.exe`）以隔离 data dir 截同三窗口；逐项记录可见结构差异并按「一致 / 差异 / 无法判定」分类。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；对照要求见 `docs/evidence/parity-recheck-2026-10-04/round3-README.md`（第 5 行「需要原版双窗口与真实平台效果验收」、第 17 行「没有原版双窗口」）与 `round3-root.md` 第 24 行。任务卡格式参考 `docs/tasks/FIX-01.md`（§19）。

对应 feature / field / action / layout ID：`F-DESKTOP-001/002/003`、`LAY-OPTSET-001/002`、`LAY-PROFILES-004`、`ACT-MAIN-024`（OptionSetting）、`ACT-MAIN-025`（RoutingSetting）、`ACT-MAIN-001..018`、`ACT-MAIN-035`、`FLG-ENT-*`、`CFG-*`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `v2rayN/v2rayN/v2rayN.csproj`（`net10.0-windows10.0.19041.0`、UseWPF）、`v2rayN/Views/MainWindow.xaml`（菜单/表头）、`v2rayN/Views/MainWindow.xaml.cs`、`ServiceLib/Common/Utils.cs:1152`（`StartupPath` = exe 目录，除非 `V2RAYN_LOCAL_APPLICATION_DATA_V2=1`）、`ServiceLib/Handler/ConfigHandler.cs:19/165/191`（配置加载与 `SystemProxyItem` 默认）、`ServiceLib/ViewModels/StatusBarViewModel.cs:217`（启动 `ChangeSystemProxyAsync`）、`ServiceLib/Handler/SysProxy/SysProxyHandler.cs:8`（`forceDisable`/`ForcedClear`）、`ServiceLib/ViewModels/OptionSettingViewModel.cs`、`ServiceLib/ViewModels/RoutingSettingViewModel.cs`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：冻结源（只读）、已装 `dotnet` 10.0.300（`C:\Program Files\dotnet\dotnet.exe`）、RC 包。
- 输出：本卡、证据目录 `docs/evidence/recheck-fixes/R3-WPF-COMPARE/**`（六张截图、UIA 树、探针 JSON、两个 capture 脚本）、compat 台账追加块。
- 错误/取消：构建失败或窗口超时写 blocker 并如实记录，不伪造截图。
- 权限：只用普通权限启动；不触发 UAC、不写系统代理/TUN/注册表/路由。
- 持久化：原版仅在临时 exe 目录写 `guiConfigs`/`guiLogs`；RC 用 `V2RAYN_R_DATA_DIR`。
- 生效：原版进程强制结束（不走 `AppExitAsync`），RC 只停本脚本启动的 PID 及其后代。

安全隔离要点（硬约束）：
- 原版数据目录 = 临时副本 exe 所在目录（`AppDomain.BaseDirectory`），未触碰用户真实配置或 `V2RAYN_LOCAL_APPLICATION_DATA_V2`。
- 原版启动前在临时 `guiConfigs/guiNConfig.json` 预置 `SystemProxyItem.SysProxyType=2`（不改变系统代理）：否则 `StatusBarViewModel.Init()` 会按默认 `ForcedClear` 调 `ProxySettingWindows.UnsetProxy()` 清空宿主系统代理。启动后状态栏显示「不改变系统代理」证实生效。
- 原版以 `Stop-Process -Force` 结束，使 `AppManager.AppExitAsync` / `UpdateSysProxy` 不执行，避免退出清代理。
- 端口：RC 隔离配置写 `Inbound.LocalPort=11808`；未对 10808 做任何监听/占用（10808 由用户正在跑的原版 PID 9584 持有，未触碰）。
- 手动枚举并排除非本脚本启动的进程：用户真实 `v2rayN.exe`（`Desktop\v2rayN-windows-64`，PID 9584）与另一处从 `dist` 启动的 RC（PID 6948 + `net_host` 35992，18:14 启动）均未处理。
- 未使用任何真实节点/订阅/证书；未读取用户凭据。

允许修改的模块：本卡、`docs/evidence/recheck-fixes/R3-WPF-COMPARE/**`、`compat/actions.yaml`（仅追加注释块）。**未改** repo 内任何生产代码、`crates/**`、`apps/desktop/**`、`tools/**`、`work/**`、`outputs/**`。

禁止改变的已有行为：菜单结构/条目/顺序、表头、布局；不删入口、不降分母；不把可见差异写成已修复。

测试夹具和原版预期：原版为合成/空配置（无节点、无订阅）；RC 用隔离 data dir 合成配置（无真实节点）。原版预期：exe 目录即数据目录；固定 Light 主题；设置窗口 5 分组（Core 基础设置/v2rayN 设置/系统代理设置/Tun 模式设置/Core 类型设置）；路由窗口 `预定义规则集列表` 含 3 条内置规则集。

本次必须通过的命令/真实场景：
- 复制冻结源到 `$env:TEMP\v2rayn-wpf-build`（work/ 只读）。
- `dotnet build <temp>\src\v2rayN\v2rayN\v2rayN.csproj -c Debug`。
- 原版隔离启动 + 三窗口截图：`docs/evidence/recheck-fixes/R3-WPF-COMPARE/capture_original.ps1`、`capture_original_windows.ps1`。
- RC 隔离启动 + 三窗口截图：`docs/evidence/recheck-fixes/R3-WPF-COMPARE/capture_rc.ps1`。
- 用 `read` 直接查看两侧 PNG 逐项比对。

证据文件位置：`docs/evidence/recheck-fixes/R3-WPF-COMPARE/`（`README.md`、`original-main.png`、`original-option.png`、`original-routing.png`、`rc-main.png`、`rc-option.png`、`rc-routing.png`、`original-main-uia.txt`、`original-probe.json`、`original-windows-probe.json`、`rc-windows-probe.json`、`capture_original.ps1`、`capture_original_windows.ps1`、`capture_rc.ps1`）。

完成条件：原版构建成功；原版与 RC 三窗口均在该隔离方式下真实启动并截图；差异按「一致/差异/无法判定」列出并附截图；安全边界满足。未做逐事件与真实平台效果，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：RC 设置/路由窗口是 `showDialog` 内嵌对话框，没有独立 OS 窗口标题，无法像原版那样按独立窗口做像素级窗口尺寸对照；本轮用整窗截图对照内容结构。
- 接口缺口（登记）：`V2RAYN_R_THEME` 证据钩子与异步 settings/主题加载存在竞态，本次 `light` 覆盖被持久化 `system` 覆盖（系统为深色 → RC 呈深色）；需固定证据运行的主题来源或让覆盖晚于加载。
- 接口缺口（登记）：RC 路由窗口缺少原版顶部的「添加规则集 / 一键导入规则集」，是否属于有意改版需产品确认。

本轮实际结果：构建与截图结果见证据 `README.md`。高价值后续卡建议：
1. `R3-WPF-Option-Labels`：设置窗口标题/分组名/字段名对齐原版（标题「设置」、tab「Core: 基础设置」等）、去掉 `settings.saved` 裸 key、补齐「认证用户名/密码」「默认 TLS 指纹」「Pac/Xray/mihomo 端口」提示，并按原版把 Mux/Hysteria2/Fragment/KCP 归位或明确标注。
2. `R3-WPF-Routing-Structure`：路由窗口补「预定义规则集列表」标题与顶部「添加规则集/一键导入规则集」，列头按原版（别名/数量/可选地址 (Url)/自定义图标），确认「状态」列的取舍。
3. `R3-WPF-Main-Chrome`：主窗口「#」索引列、状态栏文案（入站--/LAN--/TUN/系统代理:清除系统代理(未启用)）与「重载」vs「重启服务」命名核对；确认是否保留 RC 的深色主题跟随。
