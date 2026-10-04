# SR-HOTKEY-CONFLICT 证据 — 占用组合如实报冲突

任务卡：`docs/tasks/SR-HOTKEY-CONFLICT.md`
开始 HEAD：`55e6d10`（工作树干净）
上游基准：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`（WPF 为准）

## 结论

修复了 `docs/evidence/recheck-fixes/remaining-boundaries-2026-10-05.md` 第 5 项登记的独立代码缺口：`hotkey_manager_windows` 忽略 `RegisterHotKey` 返回值导致真机冲突被误报成功。现在应用在每次交给插件注册前，用自持 `user32!RegisterHotKey` 探针检查组合占用；占用组合不注册、保存如实失败并列出冲突组合，窗口保持打开。

真机全链（`Ctrl+Alt+F11`，唯一无害动作“显示/隐藏窗口”）：

`外部占用进程` → `应用保存`：探针检出占用 → `ok=false`、`registered=[]`、`conflicts=[显示/隐藏窗口: 组合已被其它程序占用]`、`isPaused=true`（窗口保持打开）→ `释放占用` → `组合空闲` → `应用重存`：`ok=true`、`registered=[showForm]` → `真实 SendInput` → 回调触发、窗口可见→隐藏 → `注销` → 插件注册表为空、OS 组合空闲。

## 改动文件

- `apps/desktop/lib/features/settings/hotkeys.dart`：新增 `errorHotkeyAlreadyRegistered`(1409)、`HotkeyProbeResult`、`hotkeyConflictNote`、`HotkeyProbe`/`Win32HotkeyProbe`（dart:ffi user32/kernel32）、`HotkeyPlugin`/`HotkeyManagerPlugin`；`PluginHotkeyRegistrar` 注册前探针、只注册空闲组合、`unregisterAll` 改用注入插件。
- `apps/desktop/lib/features/settings/global_hotkey_window.dart`：冲突状态行列出具体冲突组合（`已保存；以下组合注册冲突：…`）。
- `tools/real_hotkey_sendinput.ps1`：新增 `-Hold` 模式（`RegisterHotKey` 占用，父进程关闭 stdin 时释放）；`-Probe`/`Send` 保留。
- `apps/desktop/test/sr_hotkey_conflict_test.dart`（新增）：1409/空码→占用映射、占用组合不入插件、空闲组合探针后注册、冲突保存不关窗并列出组合。
- `apps/desktop/integration_test/sr_hotkey_conflict_test.dart`（新增，opt-in，默认 skip）。
- `docs/tasks/SR-HOTKEY-CONFLICT.md`（新增）；`docs/evidence/recheck-fixes/SR-HOTKEY-CONFLICT/**`（新增）；`compat/features.yaml`、`compat/actions.yaml`（仅追加证据引用）。
- **未改** `frb_generated`、`main_shell.dart`、`app.dart`、`lib/bridge/api/**`、`crates/**`、`services/**`、插件 pub 缓存。

## 实际命令与结果

在 `apps/desktop`（Flutter `C:\Users\Colby\toolchains\flutter\bin\flutter.bat`，Dart `…\bin\dart.bat`）：

| 命令 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed hotkeys.dart global_hotkey_window.dart sr_hotkey_conflict_test.dart` | 0 changed |
| `flutter analyze`（上述 4 文件 + integration test） | No issues found |
| `flutter test test/sr_hotkey_conflict_test.dart test/sr05_sr06_hotkey_test.dart test/t13_hotkey_test.dart test/fix15_hotkey_test.dart test/t12a_hotkey_test.dart` | +26 All tests passed! |
| 真机 opt-in：`V2RAYN_R_REAL_HOTKEY=1` + 隔离 `V2RAYN_R_DATA_DIR` + `V2RAYN_R_HOTKEY_CONFLICT_EVIDENCE` + `AUTOSTART=0`/`AUTO_SMOKE=0`，`flutter test integration_test/sr_hotkey_conflict_test.dart -d windows` | `+1 All tests passed!`（`flutter-real-conflict.log`） |

真机判定证据（`observations.json`，`recordingComplete=true`，`failures=[]`）：

| step | 结果 |
|---|---|
| `os-combo-free-before` | `free=true, lastError=0`（modFlags=3, VK=122） |
| `os-combo-occupied-externally` | 外部 `-Hold` 进程占用后，独立探测 `exit=2, free=false, lastError=1409` |
| `conflict-save-reports-occupied` | `ok=false, registered=[], conflicts=[显示/隐藏窗口: 组合已被其它程序占用], isPaused=true` |
| `os-combo-free-after-release` | `exit=0, free=true` |
| `release-then-save-registers` | `ok=true, registered=[showForm], conflicts=[]` |
| `real-key-dispatches-showform` | `sendExit=0, fired=[showForm], visibleBefore=true, hidden=true` |
| `plugin-registry-empty` | `registeredList=0` |
| `os-combo-free-after` | `exit=0, free=true`（注销后无残留） |

辅助工具 `-Hold` 独立冒烟（`tools/real_hotkey_sendinput.ps1`，命令见下）：`FREE_BEFORE free=true` → `HOLD_STARTED held=true` → `PROBE_WHILE_HELD free=false,lastError=1409` → 释放 `HOLDER_EXIT=0` → `FREE_AFTER free=true`。测试结束后 `Ctrl+Alt+F11`/`Ctrl+Alt+F12` 独立探测均 `free=true`（无残留）。

## 上游对照

- 上游 `ServiceLib/Manager/HotkeyManager.cs` 用 Win32 `RegisterHotKey` 注册；本项目的 `PluginHotkeyRegistrar` 仍走 `hotkey_manager`，但注册前增加等价的自持 `RegisterHotKey` 探针，冲突语义对齐“注册失败”而非“注册成功”。
- 动作/编辑期暂停仍为 SR-05/06 已提交实现（`HotkeyCombo`/`groupHotkeyBindings`/`IsPause`），本卡不改分组/暂停/保存语义；只把“注册结果”从插件恒真改为探针可证。
- `docs/tasks/SR-REAL-HOTKEY.md` 已实测注册/触发/注销 happy path；本卡补其登记的冲突缺口。

## 边界与未运行

- 探针 `UnregisterHotKey` 与插件 `RegisterHotKey` 间有极短竞态窗口；窗口内被第三方抢占仍可能误报一次成功。已在代码注释与本卡登记。
- `dart:ffi` 的 `GetLastError` 在 Flutter 引擎边界后实测读到 0；本卡据 `RegisterHotKey` 返回 FALSE 判定占用，非 0 时保留原码，不伪造。1409 已由独立探测进程在真实 OS 上直接证得。
- 未跑 `flutter build windows --release`、全量 `flutter test`、Rust workspace 门禁（按任务卡约束）。
