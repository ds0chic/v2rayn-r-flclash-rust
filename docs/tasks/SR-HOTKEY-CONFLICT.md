# SR-HOTKEY-CONFLICT — 全局热键冲突如实上报（自持 Win32 探针）

状态：`verified`（Windows 真机：外部进程占用 `Ctrl+Alt+F11` → 应用自持 user32 探针检出占用 → 保存如实报冲突且不关窗、不注册 → 释放后同组合注册成功并经真实 `SendInput` 派发显示/隐藏 → 注销后双方均释放、组合空闲。1409 由独立探测进程证得；应用侧 `GetLastError` 受 dart:ffi 引擎边界限制可能为 0，按“有效组合注册失败=占用”如实归类，见边界）。

任务 ID：SR-HOTKEY-CONFLICT

本次唯一用户流程：隔离数据目录启动应用 → 测试自有进程用 `RegisterHotKey` 占用 `Ctrl+Alt+F11` → 应用按共享 `HotkeyController.save` 路径保存该绑定：注册前用自持 `user32!RegisterHotKey` 探针检查组合 → 检出占用 → 该组合不交插件、标记冲突、窗口保持打开并明确提示冲突组合 → 释放占用进程 → 再次保存：探针通过、插件注册成功、真实按键触发显示/隐藏窗口 → 注销后插件注册表为空、OS 组合空闲。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；`docs/tasks/SR-REAL-HOTKEY.md`、`docs/evidence/recheck-fixes/SR-REAL-HOTKEY/README.md`（注册/触发/注销单绑定全链 `verified`，并登记 `hotkey_manager_windows` 忽略 `RegisterHotKey` 返回值、冲突不可观测的接口缺口）；`docs/evidence/recheck-fixes/remaining-boundaries-2026-10-05.md` 第 5 项（明确把该缺陷列为独立代码缺口，要求应用侧原生返回值探测/替换注册）。

对应 feature / field / action / layout ID：`F-DESKTOP-002`、`HKR-007`、`FLD-CFG-022`、`FLD-CFG-088`、`LAY-HOTKEY-001`、`ACT-HOTKEY-001`、`ACT-MAIN-028`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `ServiceLib/Manager/HotkeyManager.cs`（`RegisterHotKey` 注册、按 `(vKey<<16)|modifiers` 分组、`IsPause` 暂停）、`v2rayN/v2rayN/Views/GlobalHotkeySettingWindow.xaml.cs:16/17`（编辑期暂停注册）、`ServiceLib/Enums/EGlobalHotkey.cs:5`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：隔离目录 + 预置 `SystemProxyItem.SysProxyType=2`（`Unchanged`，启动恢复只读）；测试经真实 `SettingsController.saveGroup('GlobalHotkeys', …)` 写 `KeyEventItem{EGlobalHotkey:0,Alt:true,Control:true,Shift:false,KeyCode:100}`（WPF `Key.F11`）。
- 输出：`PluginHotkeyRegistrar.register` 对每个 `HotkeyCombo` 先跑 `Win32HotkeyProbe.probe`（`WPF Key → VK`，`RegisterHotKey(NULL,0x9A02,mods,vk)`；成功立即 `UnregisterHotKey`），仅当探针报告空闲才交 `hotkey_manager` 注册；冲突组合进入 `HotkeyController.conflicts`。
- 错误：探针失败即标记该组合冲突，不注册、不谎报成功；`HotkeyController.save` 返回 `false`，`GlobalHotkeySettingWindow` 保持打开并列出冲突组合（`已保存；以下组合注册冲突：…`）；成功分支原样保留。
- 取消：编辑期 `beginEdit/cancelEdit`、SR-05/06 分组/暂停语义不变。
- 权限：仅本机 UI + FRB/Rust/SQLite + 一次性真实热键；不写系统代理/TUN/路由/注册表，不占用 10808。
- 持久化：`guiNConfig.json` 的 `GlobalHotkeys[]`（真实 bridge 写）不变。
- 生效：冲突分支不注册；释放后重存成功注册；注销后 `HotKeyManager.instance.registeredHotKeyList` 为空，外部探测组合 `free=true`。

允许修改的模块：`apps/desktop/lib/features/settings/{hotkeys.dart,global_hotkey_window.dart}`、`apps/desktop/integration_test/**`、`apps/desktop/test/**`、`tools/**`、本卡、`docs/evidence/recheck-fixes/SR-HOTKEY-CONFLICT/**`、`compat` 台账（仅追加）。未新增 platform/IPC 接口，未改 `frb_generated`、`main_shell.dart`、`app.dart`、`lib/bridge/api/**`、禁止目录与 `crates/**`。

禁止改变的已有行为：SR-05/06 已提交的分组/暂停/保存语义、`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/{profiles,subs,runtime,monitor,update,backup,routing}/**`、`crates/**`、`services/**`；插件本身（pub 缓存）不可改；不删入口、不降分母、不伪造注册/冲突结果。

测试夹具和原版预期：隔离 `V2RAYN_R_DATA_DIR`；合成绑定（`Ctrl+Alt+F11`，无节点/无订阅/无内核）；扩展 `tools/real_hotkey_sendinput.ps1` 新增 `-Hold` 模式（`RegisterHotKey` 占用并在父进程关闭 stdin 时释放），保留 `-Probe`/`Send`。原版预期：冲突组合不应被 `HotkeyManager` 注册成功；应用应在用户可见处如实反馈。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed`（改动文件）
- `flutter analyze`（改动文件）
- `flutter test test/sr_hotkey_conflict_test.dart test/sr05_sr06_hotkey_test.dart test/t13_hotkey_test.dart test/fix15_hotkey_test.dart test/t12a_hotkey_test.dart`
- 真机（opt-in）：`$env:V2RAYN_R_REAL_HOTKEY='1'` + 隔离 `V2RAYN_R_DATA_DIR` + `V2RAYN_R_HOTKEY_CONFLICT_EVIDENCE` + `V2RAYN_R_AUTOSTART=0` / `V2RAYN_R_AUTO_SMOKE=0`，`flutter test integration_test/sr_hotkey_conflict_test.dart -d windows`。

证据文件位置：`docs/evidence/recheck-fixes/SR-HOTKEY-CONFLICT/`（`observations.json`、`flutter-real-conflict.log`、`README.md`）。

完成条件：冲突分支（占用组合不入插件、保存 `ok=false`、不关窗、列出冲突组合）、成功分支、1409/占用映射、真实 OS 占用→冲突→释放→成功→注销→组合空闲全链通过；既有 hotkey 单测全绿。全部满足，故记 `verified`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记，已在本卡定位并处理）：`hotkey_manager_windows` 的 `Register` 忽略 `RegisterHotKey` 返回值、恒 `Success(true)`，故冲突不可由插件观测。本卡以应用侧自持 `user32!RegisterHotKey` 探针补齐，未改 pub 缓存。后续若要彻底消除探针与插件注册之间的极短竞态，建议在 Rust `crates/platform` 增加原生探针并经 FRB 暴露（本卡未做，因修改桥接需重生成 `frb_generated`）。
- `dart:ffi` 的 `GetLastError` 在 Flutter 引擎边界后可能读到 0（独立 `dart run` 脚本可读到 1409）；本卡据 `RegisterHotKey` 返回 FALSE 判定占用，`GetLastError` 非 0 时保留原码、为 0 时归类为占用，不伪造具体错误码。独立探测进程已在真实 OS 上直接证得 1409。

本轮实际结果：`hotkeys.dart` 新增 `errorHotkeyAlreadyRegistered`/`HotkeyProbeResult`/`hotkeyConflictNote`/`HotkeyProbe`/`Win32HotkeyProbe`/`HotkeyPlugin`/`HotkeyManagerPlugin`，`PluginHotkeyRegistrar` 在每次插件注册前探针并只注册空闲组合，`unregisterAll` 改用注入的插件（修正此前未走注入实现的问题）；`global_hotkey_window.dart` 状态行改为列出具体冲突组合；`tools/real_hotkey_sendinput.ps1` 新增 `-Hold`。单测/widget 4/4 通过，既有 hotkey 5 文件 26/26 全绿；真机 1/1 通过，`observations.json` `failures=[]`：占用前 `free=true`，外部占用后独立探测 `free=false,lastError=1409`，冲突保存 `ok=false/registered=[]/conflicts=[显示/隐藏窗口: 组合已被其它程序占用]`，释放后 `free=true`，重存 `ok=true/registered=[showForm]`，真实按键后 `fired=[showForm]` 且 `hidden=true`，注销后 `registeredList=0`、组合 `free=true`。

## 边界与未运行

- 探针成功 `UnregisterHotKey` 到插件 `RegisterHotKey` 之间存在极短竞态窗口；窗口内被第三方抢占仍可能误报一次成功（已在代码注释与本卡登记）。
- 未跑 `flutter build windows --release`、全量 `flutter test`、Rust workspace 门禁（按任务卡约束）。
- 未做原版 WPF 真机逐事件对照；不做系统代理/TUN/路由/注册表改动。
