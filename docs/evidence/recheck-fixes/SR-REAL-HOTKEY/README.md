# SR-REAL-HOTKEY 证据 — 真实 OS 全局热键派发验证

任务卡：`docs/tasks/SR-REAL-HOTKEY.md`
开始 HEAD：`baf2299f26a044e6a2e2e0282106e550e19681f9`（工作树干净；`docs/evidence/recheck-fixes/real-os-2/` 为并行任务未跟踪产物，未触碰）
上游基准：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`（WPF 为准）

## 结论

在 Windows 真机上验证了此前只由 `_FakeOsRegistrar` 覆盖的真实注册/派发链路：

`真实设置 bridge 写绑` → `HotkeyController.save` → `PluginHotkeyRegistrar` → `hotkey_manager`(system scope) → Win32 `RegisterHotKey(MOD_CONTROL|MOD_ALT, VK_F12)` → `WM_HOTKEY` → 插件 event channel → `onTriggered` → `DesktopIntegration._onHotkey` → 窗口可见→隐藏。

`Ctrl+Alt+F12 → 显示/隐藏窗口` 唯一无害动作，测试结束已注销：插件注册表为空，外部 `RegisterHotKey` 探测组合重新可用，二次按键无动作。默认（无 opt-in）运行跳过，不注册任何真实热键。

## 改动文件

- `apps/desktop/integration_test/sr_real_hotkey_test.dart`（新增，opt-in，默认 skip）。
- `tools/real_hotkey_sendinput.ps1`（新增，user32 `SendInput` 注入 + `RegisterHotKey` 探针）。
- `docs/tasks/SR-REAL-HOTKEY.md`（新增）；`compat/features.yaml`、`compat/actions.yaml`（仅追加 evidence 引用）。
- **未改任何 `lib/**`**：真机注册/触发/注销全链通过，未发现需最小修复的真实注册路径缺陷。

## 实际命令与结果

在 `apps/desktop`（Flutter `C:\Users\Colby\toolchains\flutter\bin\flutter.bat`）：

| 命令 | 结果 |
|---|---|
| `flutter analyze integration_test/sr_real_hotkey_test.dart` | No issues found |
| `dart format --output=none --set-exit-if-changed integration_test/sr_real_hotkey_test.dart` | 0 changed, exit 0 |
| 默认跳过：`flutter test integration_test/sr_real_hotkey_test.dart -d windows` | `+0 ~1 All tests skipped.` exit 0（`flutter-real-hotkey-skip.log`） |
| 真机 opt-in：`V2RAYN_R_REAL_HOTKEY=1` + 隔离 `V2RAYN_R_DATA_DIR` + `V2RAYN_R_REAL_HOTKEY_EVIDENCE` + `AUTOSTART=0/AUTO_SMOKE=0`，同上 `-d windows` | `+1 All tests passed!` exit 0（`flutter-real-hotkey.log`） |

真机运行的判定证据（`observations.json`，`failures=[]`）：

| step | 结果 |
|---|---|
| `os-combo-free-before` | 探测 `free=true, lastError=0`（modFlags=3, VK=123） |
| `bridge-write-and-save` | `ok=true, saved=true` |
| `registered-showform` | `registered=[showForm], conflicts=[]` |
| `window-visible-before` | `visible=true` |
| `sendinput-1` | `exit=0`，`{"mode":"send","request":6,"sent":6,"lastError":0}` |
| `callback-fired-showform` | `fired=[showForm]` |
| `window-hidden-after-press` | `hidden=true` |
| `plugin-registry-empty` | `registeredList=0` |
| `os-combo-free-after` | `free=true, lastError=0`（注销后 OS 无残留） |
| `no-dispatch-after-unregister` | `firedBefore=1, firedAfter=1`，`visibleBefore=visibleAfter=true` |

注：真机运行日志出现 `[desktop] tray init failed`（集成测试绑定下系统托盘不可用，与本卡无关，热键链路不受影响）。

## 上游对照

- `MainWindow.xaml.cs:180`：`OnHotkeyHandler(EGlobalHotkey.ShowForm)` → `ShowHideWindow(null)` 按可见性切换。本项目同名动作经共享 `DesktopIntegration._onHotkey` → `_toggleWindow()`（`windowManager.isVisible()` 切换）实现，真机按键后窗口由可见变隐藏，行为一致。
- `EGlobalHotkey.cs:5`：`ShowForm=0`；绑定 `EGlobalHotkey=0` 经真实设置 bridge 落 `GlobalHotkeys[]`，与原版 `KeyEventItem` 形状一致。
- 注册标识/分组语义仍为 SR-05/06 已提交实现（`HotkeyCombo` / `groupHotkeyBindings`），本卡不改。

## 边界与未运行

- 同组合多动作的真实 OS 派发未运行：`EGlobalHotkey` 其余四动作都会写系统代理，受硬约束（只绑定显示窗口类无害动作）无法在同组合上验证；SR-06 分组语义仍由 `sr05_sr06_hotkey_test.dart` 单测覆盖。
- `hotkey_manager_windows` 忽略 `RegisterHotKey` 返回值（始终 `Success(true)`），真实冲突不可由 `PluginHotkeyRegistrar` 观测；登记为接口缺口，未改 pub 缓存。
- 未跑 `flutter build windows --release`、全量 `flutter test`、Rust workspace 门禁（按任务卡约束）。
