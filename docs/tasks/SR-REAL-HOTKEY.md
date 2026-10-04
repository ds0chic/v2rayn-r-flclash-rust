# SR-REAL-HOTKEY — 真实 OS 全局热键派发验证（opt-in 集成测试）

状态：`verified`（Windows 真机注册 `Ctrl+Alt+F12` → “显示/隐藏窗口” 单绑定，经 PowerShell `SendInput` 真实按键触发回调、窗口隐藏，注销后 OS 组合重新可注册且无插件残留。同组合多动作的 OS 级验证因安全约束未运行，见边界）。

任务 ID：SR-REAL-HOTKEY

本次唯一用户流程：隔离数据目录启动应用 → 经真实设置 bridge 写入 `Ctrl+Alt+F12 → 显示/隐藏窗口` → 应用按共享 `HotkeyController.save` 路径注册真实 `RegisterHotKey` → 测试用 PowerShell `SendInput` 注入真实组合 → 断言动作回调被触发（窗口由可见变隐藏）→ 注销热键 → 再次注入确认无动作，并用外部 `RegisterHotKey` 探测确认 OS 组合已释放。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；`docs/tasks/SR-05.md`、`docs/tasks/SR-06.md`（此前仅用 `_FakeOsRegistrar` 覆盖，未注册真实 OS 热键，状态保持 `implemented`）；`docs/evidence/recheck-fixes/SR-05-06/README.md`（登记“真实 `hotkey_manager` 组合行为需真机确认”的接口缺口）；`docs/tasks/FIX-15.md`（登记“系统热键 OS 实触发需同夹具实机”的接口缺口）。本卡以真机测试补齐这两条缺口中的“注册/触发/注销”部分。

对应 feature / field / action / layout ID：`F-DESKTOP-002`、`HKR-007`、`FLD-CFG-022`、`FLD-CFG-088`、`LAY-HOTKEY-001`、`ACT-HOTKEY-001`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `v2rayN/v2rayN/Views/MainWindow.xaml.cs:180`（`OnHotkeyHandler(EGlobalHotkey.ShowForm)` → `ShowHideWindow`）、`ServiceLib/Enums/EGlobalHotkey.cs:5`（`ShowForm=0`）、`ServiceLib/Manager/HotkeyManager.cs`（`RegisterHotKey` 注册 + `WM_HOTKEY` 分派）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：隔离目录 + 预置 `SystemProxyItem.SysProxyType=2`（`Unchanged`，保证启动恢复只读、不写宿主代理）；测试经真实 `SettingsController.saveGroup('GlobalHotkeys', …)` 写入 `KeyEventItem{EGlobalHotkey:0,Alt:true,Control:true,Shift:false,KeyCode:101}`（WPF `Key.F12`）。
- 输出：`HotkeyController.save` 走 `groupHotkeyBindings` → `PluginHotkeyRegistrar.register` → `hotkey_manager` system scope → Win32 `RegisterHotKey(MOD_CONTROL|MOD_ALT, VK_F12)`；`onTriggered` 经 `hotkeyDispatchProvider` 分派到 `DesktopIntegration._onHotkey` → `_toggleWindow()`。
- 错误：真机 `hotkey_manager_windows` 忽略 `RegisterHotKey` 返回值，冲突不会被 `PluginHotkeyRegistrar` 观测到（接口缺口，见下）；本测试在注册前后用外部 `RegisterHotKey` 探测组合可用性，避免把“被占用”误判为成功。
- 取消：不涉及编辑窗口；测试结束无条件注销并恢复原始 dispatcher。
- 权限：仅本机 UI + FRB/Rust/SQLite + 一次性真实热键；不写系统代理/TUN/路由/注册表，不占用 10808。
- 持久化：`guiNConfig.json` 的 `GlobalHotkeys[]`（真实 bridge 写）。
- 生效：注销后 `HotKeyManager.instance.registeredHotKeyList` 为空，且外部 `RegisterHotKey` 探测组合 `free=true`。

允许修改的模块：`apps/desktop/integration_test/**`、`apps/desktop/test/**`、`apps/desktop/lib/features/settings/{hotkeys.dart,global_hotkey_window.dart}`（仅缺陷最小修复）、`apps/desktop/lib/app/shell/desktop_integration.dart`（仅缺陷最小修复）、`tools/**`、本卡、`docs/evidence/recheck-fixes/SR-REAL-HOTKEY/**`、`compat` 台账（仅追加）。**本轮未改任何 lib 文件**（未发现需最小修复的真实注册路径缺陷）。

禁止改变的已有行为：SR-05/06 已提交的分组/暂停/保存语义、`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/{profiles,subs,runtime,monitor,update,backup,routing}/**`、`crates/**`、`services/**`；不删入口、不降分母、不伪造注册/触发结果。

测试夹具和原版预期：隔离 `V2RAYN_R_DATA_DIR`；合成绑定（`Ctrl+Alt+F12`，无节点/无订阅/无内核）；`tools/real_hotkey_sendinput.ps1`（Add-Type user32）提供 `SendInput` 注入与 `RegisterHotKey` 探针。原版预期：保存后真实按键触发 `EGlobalHotkey.ShowForm` 的 `ShowHideWindow`；注销后 OS 不再投递该组合。

本次必须通过的命令/真实场景：
- `flutter analyze integration_test/sr_real_hotkey_test.dart`
- 默认跳过：`flutter test integration_test/sr_real_hotkey_test.dart -d windows`（无 opt-in）→ `All tests skipped.`
- 真机（opt-in）：`$env:V2RAYN_R_REAL_HOTKEY='1'` + 隔离 `V2RAYN_R_DATA_DIR` + `V2RAYN_R_REAL_HOTKEY_EVIDENCE` + `V2RAYN_R_AUTOSTART=0` / `V2RAYN_R_AUTO_SMOKE=0`，`flutter test integration_test/sr_real_hotkey_test.dart -d windows`。
- 回归：既有 hotkey 单测（`sr05_sr06_hotkey_test.dart`、`fix15_hotkey_test.dart`、`t13_hotkey_test.dart`、`t12a_hotkey_test.dart`）保持全绿。

证据文件位置：`docs/evidence/recheck-fixes/SR-REAL-HOTKEY/`（`observations.json`、`flutter-real-hotkey.log`、`flutter-real-hotkey-skip.log`、`README.md`）。

完成条件：显式 opt-in（无 env 默认跳过）；隔离 data dir；经真实 bridge/设置写入并注册；PowerShell `SendInput` 真实按键；断言回调触发且窗口可见→隐藏；注销后插件注册表为空、外部探测组合可注册、再次按键无动作；测试不常驻注册、不触系统代理/TUN/10808；可重复运行。全部满足，故记 `verified`（限单绑定 happy path）。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`hotkey_manager_windows` 的 `Register` 不检查 `RegisterHotKey` 返回值、始终 `Success(true)`，因此本项目的 `PluginHotkeyRegistrar` 无法从插件观测真实冲突；SR-06 的“注册失败不关窗”在真机 OS 冲突下不会被触发。建议后续在允许范围内自持 `RegisterHotKey` 探测或修复插件，本卡不改 pub 缓存、不新增 IPC。
- 接口缺口（登记）：`EGlobalHotkey` 五个动作中仅 `ShowForm` 无害，其余四个（清除/设置/不变/PAC）都会写系统代理；受硬约束“只允许绑定显示窗口类无害动作”，**同组合多动作**的真实 OS 派发无法在不触发代理写的前提下验证，SR-06 的分组语义仍由单测覆盖（`groupHotkeyBindings`）。

本轮实际结果：新增 `apps/desktop/integration_test/sr_real_hotkey_test.dart`（opt-in，默认 skip）与 `tools/real_hotkey_sendinput.ps1`（`SendInput` 注入 + `RegisterHotKey` 探针）。真机运行 1/1 通过，`observations.json` 记录：注册前组合可用、真实 bridge 写+保存成功、`registered=[showForm]` 无冲突、按键 `sent=6` 后回调 `fired=[showForm]` 且窗口 `hidden=true`、注销后 `registeredList=0` 且外部探测 `free=true`、二次按键 `firedBefore=firedAfter=1` 且可见性不变。默认（无 opt-in）运行输出 `All tests skipped.`。未改任何 lib 文件。

## 边界与未运行

- 未验证同组合多动作的真实 OS 派发（安全约束，见接口缺口）。
- `hotkey_manager_windows` 忽略注册失败的问题未修（pub 缓存不可改；需另行决策）。
- 未跑 `flutter build windows --release`、全量测试与 Rust workspace 门禁（按任务卡约束）。
