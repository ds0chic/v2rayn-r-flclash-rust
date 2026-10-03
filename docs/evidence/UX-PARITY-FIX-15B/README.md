# UX-PARITY-FIX-15B 证据 — 启动隐藏（AutoHideStartup）/ 单实例真实双进程 / 上游启动参数

任务卡：`docs/tasks/FIX-15B.md`
开始 HEAD：`fdedda2`（工作树干净）
上游基准：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`

## 本轮实际改动

- `apps/desktop/lib/app/shell/desktop_integration.dart`
  - 新增 `static bool shouldHideOnStartup(Map document)`（读 `UiItem.AutoHideStartup`，默认 false）。
  - `start()`：托盘就绪后，若 `AutoHideStartup` 为真，`_hideOnStartup()`——`windowManager.hide()` 后在下一帧补一次 hide，赢过 Win32 embedder 首帧 `Show()`。
  - 语义对照上游：`MainWindow` 构造函数 `WindowState = Minimized`、`OnLoaded` 调 `ShowHideWindow(false)`（`AppManager.ShowInTaskbar=false` + `Hide()`）；普通启动路径不变。
- `apps/desktop/windows/runner/main.cpp`
  - 解析命令行参数（`GetCommandLineArguments()`），识别上游唯一 `e.Args` 值 `rebootas`（`Global.RebootAs`）；命中时跳过单实例退出分支，并继续将参数透传给 Dart 入口。
  - 未改 FIX-15 已提交的命名互斥量单实例与 `PostMessage` 唤回逻辑。
- `apps/desktop/test/fix15b_startup_test.dart`：AutoHideStartup 启动决策单元测试。
- `apps/desktop/integration_test/ux_parity_fix15b_startup_test.dart`：真实窗口 seed/hidden 两阶段测试（已写，未运行）。
- `compat/actions.yaml`：ACT-WIN-010/011/012 各追加一行 `notes`（未删行）。

## 实际运行命令与结果

| 命令 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed <changed>` | 0 changed, exit 0（`format.log`） |
| `flutter analyze <changed 3 files>` | No issues found（`analyze.log`） |
| `flutter test test/fix15b_startup_test.dart` | 3/3 通过（`test.log`） |
| `dual-process.ps1`（真实 exe x2，隔离 `V2RAYN_R_DATA_DIR`） | pass=True（`dual-process.log`） |

## 真实双进程结果（`dual-process.log`）

- 首实例启动，主窗口 `visible=True`。
- 脚本用 `ShowWindow(SW_HIDE)` 隐藏该窗口，确认 `visible=False`。
- 启动第二个实例（同 exe、同隔离 data dir）：第二进程 **0.31s 退出，code=0**。
- 首窗口被 `PostMessage(kShowWindowMessage)` 唤回：`first_visible=True`，`foreground_is_first=True`。
- 脚本在 `finally` 中只 `Stop-Process` 自己启动的两个 PID。

说明：该实测针对 `apps/desktop/build/windows/x64/runner/Release/v2rayn_desktop.exe`（构建于 FIX-15 commit `04fcd93`）。FIX-15B 对 `main.cpp` 的 `rebootas` 改动在下次构建前仅存在于源码。

## 上游启动参数结论

`App.xaml.cs:28` 只解析一个启动参数：`e.Args.Any(t => t == Global.RebootAs)`，其中 `Global.RebootAs = "rebootas"`（`ServiceLib/Global.cs:88`）。用途：`ProcUtils.RebootAsAdmin`（`ProcUtils.cs:49`）以 `Arguments = "rebootas"` 重新启动（常为 `runas` 提权），随后 `AppManager.RebootAsAdmin` 退出当前进程。`rebootas` 命中时跳过 `ProgramStarted` 已存在的退出分支，即「自我重启的新进程照常启动」。

克隆现状：runner 已识别 `rebootas`（本卡）；但 app 侧「以管理员重启」动作与 runner「请求退出」消息尚未实现，且 Dart 入口尚未消费透传参数——登记为接口缺口，不伪造。

## 未完成 / 未验证

- `flutter build windows --release` 与 `flutter test integration_test/... -d windows` 未运行（本卡禁止构建），故 `AutoHideStartup` 的「真实窗口重开隐藏」仍未 `verified`。
- `rebootas` 分支未编译验证（同上）；未做提权重启端到端。
- 系统热键 OS 实触发、托盘叶子共享用例仍依赖 FIX-15 的后续门禁，不在本卡。
