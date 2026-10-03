# FIX-15B — 启动隐藏（AutoHideStartup）落链；单实例真实双进程实测；上游启动参数明确

状态：`implemented`（AutoHideStartup 已在 Dart 侧落链并有单元测试；`rebootas` 启动参数已在 runner 识别；真实双进程已在构建产物实测通过。真实窗口重开验证需 `flutter build windows`，本卡禁止构建，故不写 `verified`）。

任务 ID：FIX-15B

来源：`docs/evidence/parity-review-2026-10-03/repair-queue.md:40` 将 FIX-15 限定为“点 X → 托盘恢复 / 单实例唤回 / 热键录制 / 自启、PAC 另卡”，其中“启动隐藏（AutoHideStartup）按任务卡属 FIX-15B”（见 FIX-15 证据 README「未完成」）。

本次唯一用户流程：在设置中把「启动时自动隐藏 (AutoHideStartup)」存为开 → 退出 → 重开，应用启动时窗口不出现，只留托盘图标；点托盘或在窗口隐藏时再次启动该 exe，首实例窗口被唤回可见并置前。普通启动（未开隐藏）不回归，仍显示窗口。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `fdedda2`（工作树干净）。FIX-15 已提交 runner 命名互斥量单实例 + `PostMessage` 唤回、`CloseBehavior` 解析 `AutoHideStartup`，但未落启动隐藏、未跑真实双进程。

对应 feature / field / action / layout ID：`FLD-CFG-078`（`UiItem.AutoHideStartup`）、`FLD-CFG-079`（`UiItem.Hide2TrayWhenClose`）、`ACT-WIN-010`（启动参数 RebootAs 与单实例）、`ACT-WIN-011`（启动隐藏并恢复隐藏状态）、`ACT-WIN-012`（第二实例启动时显示主窗口）、`INV-WPF-021`/`ROOT-04`/`ROOT-06`/`RT-13`。

必读上游文件、符号和固定 commit：
- `v2rayN/v2rayN/App.xaml.cs:11/24-35`：`ProgramStarted` 命名 `EventWaitHandle`（名字=`Utils.GetMd5(Utils.GetExePath())`）；`rebootas`（`Global.RebootAs`）时不做单实例退出；普通第二实例 `ProgramStarted.Set()` 后 `Environment.Exit(0)`。
- `v2rayN/v2rayN/Views/MainWindow.xaml.cs:21/146-149/162-168/298-316/318-326`：`ThreadPool.RegisterWaitForSingleObject(App.ProgramStarted, OnProgramStarted)` → `OnProgramStarted` → `ShowHideWindow(true)`；构造函数 `WindowState = WindowState.Minimized`；`OnLoaded` 若 `AutoHideStartup` 调 `ShowHideWindow(false)`；`ShowHideWindow` 用 `AppManager.ShowInTaskbar` 决定显隐。
- `v2rayN/ServiceLib/Common/ProcUtils.cs:49-68` + `AppManager.cs:155-159`：`RebootAsAdmin` 以 `Arguments = Global.RebootAs`（`"rebootas"`）重启并退出当前进程。
- `ServiceLib/Models/ConfigItems` 的 `UIItem.AutoHideStartup` / `AppManager.ShowInTaskbar`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`UiItem.AutoHideStartup`（bool，默认 false）；命令行参数 `rebootas`。
- 输出：启动隐藏——窗口不出现、托盘图标存在；唤回——首窗口 `SW_RESTORE` 可见并尽量置前。
- 错误：`windowManager.hide()` 失败只 `debugPrint`，不伪造隐藏状态；参数不识别时按普通启动。
- 取消：无（启动决策在构造/启动一次读取，非实时切换）。
- 权限：仅本机 UI + runner；不启内核、不写宿主代理/注册表/路由/TUN。
- 持久化：`guiNConfig.json` 的 `UiItem.AutoHideStartup`；重开保持（沿用设置引擎既有路径，无新 IPC）。
- 生效：`AutoHideStartup` 在 `DesktopIntegration.start()` 读一次；`rebootas` 在 runner 启动时读一次。

允许修改：`apps/desktop/lib/app/shell/desktop_integration.dart`、`apps/desktop/windows/runner/**`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`docs/evidence/UX-PARITY-FIX-15B/**`、本卡、`compat/actions.yaml`（仅 notes 追加）。

禁止改变：FIX-15 已提交的托盘委派/关闭语义/热键编解码、`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`options_setting_window/settings_controller/settings_fields/settings_defaults`、profiles/subs/runtime/monitor/update/backup/routing、`crates/**`。不改热键录制与自启/PAC（属 FIX-15/15C）。

测试夹具和原版预期：隔离 data dir（`V2RAYN_R_DATA_DIR`）；真实双进程用 PowerShell 启动两次 Release exe，隐藏首窗口（`ShowWindow(SW_HIDE)`）后启动第二实例。原版预期：第二进程退出、首窗口被唤回可见/置前；隔离目录不碰用户配置。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed <changed>`
- `flutter analyze lib/app/shell/desktop_integration.dart test/fix15b_startup_test.dart integration_test/ux_parity_fix15b_startup_test.dart`
- `flutter test test/fix15b_startup_test.dart`
- 真实双进程：`docs/evidence/UX-PARITY-FIX-15B/dual-process.ps1`（隔离 data dir，只终止本脚本启动的 PID）
- 未跑：`flutter build windows --release`、`flutter test integration_test/... -d windows`（本卡禁止构建）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-15B/`（`README.md`、`observations.json`、`dual-process.ps1`、`dual-process.log`、`analyze.log`、`format.log`、`test.log`）。

完成条件：AutoHideStartup 决策可单测且普通启动不回归；runner 识别 `rebootas`；真实双进程第二实例退出且首窗口被唤回可见；门禁命令通过。真实窗口重开未跑，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：Dart 入口参数解析属 `main.dart`（`main(List<String> args)`），本卡禁止修改；runner 已把 `rebootas` 透传给 Dart 入口参数，但 Dart 侧尚无消费方。补丁见下。
- 接口缺口（登记）：`rebootas` 完整语义（提权重启 + 退出旧进程）需要 app 侧「以管理员重启」动作与 runner「请求退出」消息，本卡只保证被 `rebootas` 启动的新进程不再被单实例逻辑吞掉。
- 接口缺口（登记）：真实窗口「启动隐藏 + 重开」验证需 `flutter build windows`（本卡禁止），已写 `integration_test/ux_parity_fix15b_startup_test.dart`，待门禁执行。

## bootstrap 接线补丁（由根代理落地；未改 `app.dart`/`main.dart`）

1. 托盘委派（RT-11）已由根代理在 `main_shell.dart:53-58` 落地，无需重复。

2. `rebootas` 透传消费（可选、若要做「以管理员重启」完整链）：把 `main.dart` 的入口改为 `Future<void> main(List<String> args)`，对 `args.contains('rebootas')` 记录本次为提权重启（可注入一个 `rebootAsRestartProvider`），供后续 `ACT-MAIN-*` 重启动作在退出旧进程前使用。否则 `rebootas` 仅用于「新进程照常启动」，行为已由 runner 保证。

## 本轮实际结果

- `desktop_integration.dart`：新增 `DesktopIntegration.shouldHideOnStartup(document)`；`start()` 在托盘就绪后，若 `UiItem.AutoHideStartup` 为真则 `_hideOnStartup()`——先 `windowManager.hide()`，再在下一帧补一次 `hide()` 以赢过 Win32 embedder 首帧 `Show()`；普通启动路径不变。
- `windows/runner/main.cpp`：解析命令行 `rebootas`，命中时跳过单实例退出分支（对齐上游 `App.xaml.cs:30`），并继续把参数透传给 Dart 入口；未改 FIX-15 的互斥量/`PostMessage` 唤回逻辑。
- `test/fix15b_startup_test.dart`：3/3 通过（true/false、缺省/畸形/非 bool、close 两字段独立）。
- 真实双进程（`dual-process.ps1`）：首实例窗口出现 → `SW_HIDE` → 启动第二实例 → 第二进程 0.31s 退出（code=0）→ 首窗口恢复 `visible=true` 且 `foreground=true`；脚本只终止自己启动的 PID。
- `compat/actions.yaml`：ACT-WIN-010/011/012 各追加一行 `notes`（未删行、未降分母）。
- 未运行：`flutter build windows --release`、`flutter test -d windows`、Rust workspace 门禁。
