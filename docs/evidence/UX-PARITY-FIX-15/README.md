# UX-PARITY-FIX-15 证据 — 点 X → 托盘恢复 / 热键编码与 dispatch / 托盘动态同步 / PAC

任务卡：`docs/tasks/FIX-15.md`
开始 HEAD：`066d8d385fb1052388a2b295e3500eca3ce4543a`（工作树干净）
上游基准：冻结 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`

## 本轮实际改动（Flutter/Windows runner；未跑全仓门禁）

- 新增 `apps/desktop/lib/features/settings/hotkey_keycodec.dart`：显式三编码转换
  （持久化 WPF `System.Windows.Input.Key` 枚举 ↔ Win32 VK ↔ Flutter 逻辑/物理键）。
  WPF↔VK 表用 `windows powershell` 的 `System.Windows.Input.KeyInterop` 实测生成（见
  `wpf-key-vk-dump.txt`）。
- `hotkeys.dart`：`HotkeyBinding.keyCode` 语义改为持久化 WPF Key；`HotkeyRegistrar.register`
  增加 `onTriggered` 回调；`PluginHotkeyRegistrar` 走 `WPF Key -> VK -> Flutter physical`
  并传 `keyDownHandler`（RT-12/SET-15）；新增 `hotkeyDispatchProvider` 与
  `hotkeyIdentifier`/`hotkeyActionFromIdentifier`；注册异常不再抛出而是如实上报。
- `global_hotkey_window.dart`：录制时裸修饰键不再终止录制（先更新修饰位，非修饰键才落
  `KeyCode`），标签/编码走 codec；保存改为经 `HotkeyController.save` 重注册并 dispatch。
- `app/shell/tray_menu_model.dart`：`TrayMenuItem.children` + `sharedCommandForTrayAction`
  （托盘叶子→主窗口 ACT-MAIN-* 共享用例）+ `TrayCommandDelegateHolder` 注入点。
- `platform_bridge.dart`：新增 `pacStart/pacStartFromFile/pacStop/pacState`、
  `PacHandleView`、`PlatformView.copyWith` 的 PAC 清除位。
- `platform_controller.dart`：`startPac/startPacFromFile/stopPac`，真实 handle 回填。
- `proxy_settings_view.dart`：`systemProxyItemWithMode` 保留其它字段地写 `SysProxyType`（RT-14）。
- `app/shell/desktop_integration.dart`（非禁止文件，桌面线）：Windows 关闭恒隐藏、
  非 Windows 按 `Hide2TrayWhenClose`；安装热键 dispatcher；托盘菜单动态节点/路由 child；
  共享命令/节点/路由走 delegate；PAC 模式启停 + 模式持久化；退出停 PAC。
- `windows/runner/main.cpp` + `flutter_window.cpp` + `runner_messages.h`：命名互斥量单实例判定，
  第二实例向首实例窗口 `PostMessage(kShowWindowMessage)` 唤回后退出（ROOT-06/ACT-WIN-012）。
- 测试：`test/fix15_hotkey_test.dart`、`test/fix15_tray_pac_test.dart`；
  `integration_test/ux_parity_fix15_hotkey_tray_test.dart`（真实窗口，未运行）。

## 实际运行命令与结果

| 命令 | 结果 |
|---|---|
| `flutter analyze lib test` | No issues found |
| `flutter analyze integration_test/ux_parity_fix15_hotkey_tray_test.dart` | No issues found |
| `dart format --output=none --set-exit-if-changed <changed>` | 0 changed, exit 0 |
| `flutter test test/fix15_hotkey_test.dart` | 8/8 通过（`fix15-hotkey.log`） |
| `flutter test test/fix15_tray_pac_test.dart` | 8/8 通过（`fix15-tray-pac.log`） |
| `flutter test test/t13_hotkey_test.dart test/t13_platform_models_test.dart test/t12a_hotkey_test.dart test/t13_statusbar_test.dart` | 19/19 通过（`fix15-regression.log`） |
| `flutter test integration_test/ux_parity_fix15_hotkey_tray_test.dart -d windows` | **未运行**（见未完成） |

未跑：`flutter build windows --release`、`flutter test`（全量）、Rust workspace 门禁——按任务卡约束。

## 编码对照实测（wpf → vk）

由 `powershell.exe -NoProfile`（.NET Framework WPF）`KeyInterop.VirtualKeyFromKey` 打印，
覆盖 A/Z、D0/D9、F1/F12/F24、小键盘、Oem、导航、修饰键；落库形如 `A wpf=44 vk=0x41`、
`F1 wpf=90 vk=0x70`、`OemMinus wpf=143 vk=0xBD`。文件见 `wpf-key-vk-dump.txt`。

## 未完成 / 未验证

- 真实窗口集成测试未运行：`flutter test -d windows` 需完整构建且会注册真实系统热键/托盘，
  本轮未执行（不伪造）。
- 系统热键的“OS 实触发”未实测：`hotkey_manager` system scope 由 OS 投递 WM_HOTKEY，
  无法用 widget 按键事件触发；本卡以 dispatcher 单测覆盖回调链，真机注册/触发留待
  同夹具实机验证。
- 托盘叶子的真实共享用例执行依赖 shell 注入 `TrayCommandDelegateHolder`（见任务卡 bootstrap 补丁）；
  未注入时如实提示“尚未接入后端”，不伪造。
- 第二实例唤回在 runner 层实现，未在本轮启动双进程实测。
- 启动隐藏（AutoHideStartup）按任务卡属 FIX-15B，未在本卡实现。
