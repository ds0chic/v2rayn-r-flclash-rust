# FIX-15 — 点 X → 托盘恢复；单实例唤回；热键编码/录制/重注册/dispatch；托盘动态同步；四模式/PAC

状态：`implemented`（Flutter 改动已过 analyze/format 与针对性 widget/单元测试；真实窗口集成测试与双进程单实例未运行，故不写 `verified`）。

任务 ID：FIX-15

本次唯一用户流程：点击窗口关闭按钮 X → 窗口隐藏到托盘 → 从托盘恢复窗口。围绕该流程补齐热键编码链、托盘菜单动态同步与系统代理四模式/PAC 生命周期。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `066d8d385fb1052388a2b295e3500eca3ce4543a`（工作树干净）。来源：`docs/evidence/parity-review-2026-10-03/repair-queue.md:40`，`runtime-report.md` RT-11/12/13/14/20，`settings-report.md` SET-15，`root-report.md` ROOT-04/06/10。

对应 feature / field / action / layout ID：`ACT-WIN-001/002`、`ACT-TRAY-001..013`、`ACT-MAIN-016/017/020/021`、`ACT-HOTKEY-001..008`、`ACT-WIN-012`、`FLD-CFG-088..092`、`ENUM-004`、`LAY-HOTKEY-001`、`INV-WPF-001/002/021`、`INV-AVA-026`。

必读上游文件、符号和固定 commit：
- `v2rayN/v2rayN/Views/MainWindow.xaml.cs:146/162/180/193-203`（`MainWindow_Closing` 恒 `e.Cancel=true` + `ShowHideWindow(false)`；`OnHotkeyHandler`；`Current_SessionEnding`）。
- `v2rayN/v2rayN/Views/GlobalHotkeySettingWindow.xaml.cs:49/64`（`TxtGlobalHotkey_PreviewKeyDown`：`e.Handled=true`，修饰键写 `Key.None`，非修饰键写 `(int)e.Key`，不因修饰键结束录制）。
- `ServiceLib/ViewModels/GlobalHotkeySettingViewModel.cs`（`GetKeyEventItem`/`ResetKeyEventItem`/`SaveSettingAsync`）。
- `ServiceLib/Enums/EGlobalHotkey.cs`（0 ShowForm / 1 Clear / 2 Set / 3 Unchanged / 4 Pac）。
- `v2rayN.Desktop/Manager/HotkeyManager.cs:40-56`（`KeyInterop.VirtualKeyFromKey((Key)item.KeyCode)` + `Modifiers` + `HotKeyPressed` 分发）。
- `ServiceLib/ViewModels/MainWindowViewModel.cs` / `StatusBarViewModel.cs` 托盘菜单构建；`SysProxyHandler` 四模式/PAC。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`GlobalHotkeys` 的 `KeyEventItem{EGlobalHotkey,Alt,Control,Shift,KeyCode}`；`KeyCode` 为 WPF `System.Windows.Input.Key` 枚举整数值。托盘节点/路由来自当前 `profilesController` / `routingController`。
- 输出：录制→`KeyCode` 转换；保存→设置落库 + 重注册 + dispatch；托盘模式→系统代理/PAC 变更并持久化 `SystemProxyItem.SysProxyType`。
- 错误：不受支持按键不落值、如实提示；注册冲突逐项上报，不伪造成功；PAC 启动失败透传 `ErrorDto`。
- 取消：热键窗口取消丢弃草稿；菜单外点/Esc 关闭。
- 权限：仅本机 UI + FRB/Rust；不启内核、不写宿主代理（真机写入只在显式用户动作且 `initPlatformBackend('windows')` 后）。
- 持久化：`guiNConfig.json` 的 `GlobalHotkeys`（WPF Key 编码）与 `SystemProxyItem.SysProxyType`；重开保持。
- 生效：`HotkeyManager.ReLoad` 语义；托盘菜单每轮重建；PAC 端口 ≥11808（禁止 10808）。

允许修改的模块：`apps/desktop/lib/features/settings/{hotkeys.dart,global_hotkey_window.dart,platform_bridge.dart,platform_controller.dart,proxy_settings_view.dart,settings_actions.dart}` 与新增文件、`apps/desktop/lib/features/settings/hotkey_keycodec.dart`、`apps/desktop/lib/app/shell/tray_menu_model.dart`、`apps/desktop/lib/app/shell/desktop_integration.dart`（桌面线；RT 缺陷所在，非禁止文件）、`apps/desktop/windows/runner/**`、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、本卡、`docs/evidence/UX-PARITY-FIX-15/**`。

禁止改变的已有行为：`main_shell.dart`、两处 `frb_generated`、`features/profiles/**`、`features/subs/**`、`features/runtime/**`、`features/monitor/**`、`features/update/**`、`features/backup/**`、`crates/**`、`option_setting_window.dart`/`settings_controller.dart`/`settings_fields.dart`/`settings_defaults.dart`/`theme_setting_dialog.dart`；不删入口、不降分母、不伪造注册/落库/平台结果。

测试夹具和原版预期：录制键事件为合成 `LogicalKeyboardKey`；WPF Key↔VK 由本机 .NET Framework `KeyInterop` 实测（`wpf-key-vk-dump.txt`）；PAC/代理用内存假桥，不写宿主。原版预期：`KeyCode` 存 WPF Key 枚举；修饰键不结束录制；保存后按键可 dispatch；关闭 X 恒隐藏；模式选择写 settings 且重开保持。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed <changed>`
- `flutter analyze lib test` / `flutter analyze integration_test/...
- `flutter test test/fix15_hotkey_test.dart`、`test/fix15_tray_pac_test.dart` 及 T12a/T13 回归
- 真实窗口：`flutter test integration_test/ux_parity_fix15_hotkey_tray_test.dart -d windows`（record/reopen）
- 未跑 `flutter build windows --release`、全量测试与 Rust workspace 门禁（按任务卡约束）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-15/`（`README.md`、`observations.json`、`wpf-key-vk-dump.txt`、`fix15-hotkey.log`、`fix15-tray-pac.log`、`fix15-regression.log`、`analyze.log`、`format.log`）。

完成条件：编码三向转换显式且实测；录制不被修饰键终止；保存后重注册并 dispatch；托盘菜单节点/路由动态同步且叶子映射共享 `ACT-MAIN-*`；四模式/PAC 生命周期与模式保存齐全；Windows 关闭恒隐藏、非 Windows 按字段；单实例判定与唤回接入口存在；门禁通过。真实窗口与双进程未运行，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：托盘叶子执行“共享用例”需要一个从 shell 注入的 `TrayCommandDelegateHolder`（见下方 bootstrap 补丁）；未注入时托盘如实提示“尚未接入后端”，不重复实现业务。
- 接口缺口（登记）：PAC 使用内联最小脚本 `__PROXY__` 占位；读取完整 `data/pac.txt` / 自定义脚本内容属后续数据源接入。
- 接口缺口（登记）：系统热键的真机 OS 触发（WM_HOTKEY）无法用 widget 事件验证，需同夹具实机（另卡/FIX-15B 一并验收）。

## bootstrap 接线补丁（由根代理落地；未改 `main_shell.dart`）

1. 在 `main_shell.dart` 初始化（拿到 `context` 与 `ref` 后）安装托盘共享用例委派，复用现有同款函数，勿重写：

```dart
ref.read(trayCommandDelegateProvider).value = TrayCommandDelegate(
  onSharedCommand: (command) => _onMenuAction(command), // 复用主菜单同一 switch
  onSelectNode: (id) => profiles_actions.activateProfile(context, ref, id),
  onSelectRouting: (id) => ref.read(routingControllerProvider.notifier).select(id),
);
```

其中 `onSelectNode` 应调用主窗口“设为活动/双击”所用的同一函数（核实 `features/profiles/profile_actions.dart` 的激活入口名后填入），`onSelectRouting` 使用 `RoutingController.select`。

2. 可选：`app.dart` bootstrap 无需改动即可运行；`DesktopIntegration.start()` 已安装热键 dispatcher 并在托盘/退出路径管理 PAC。

## 本轮实际结果

- 新增 `hotkey_keycodec.dart`：`wpfKeyForEvent`、`virtualKeyFromWpf`、`wpfFromVirtualKey`、`physicalFromWpf`、`labelForWpf`、`isModifierKey`；WPF↔VK 表由本机实测。
- `hotkeys.dart`：`HotkeyBinding.keyCode` 语义为 WPF Key；`HotkeyRegistrar.register` 增 `onTriggered`；`PluginHotkeyRegistrar` 传 `keyDownHandler` 分发；新增 `hotkeyDispatchProvider`。
- `global_hotkey_window.dart`：修饰键不结束录制；保存经 `HotkeyController.save` 重注册并 dispatch；标签走 codec。
- `tray_menu_model.dart`：`children`、`sharedCommandForTrayAction`、`TrayAction`、`TrayCommandDelegateHolder`。
- `platform_bridge.dart`/`platform_controller.dart`/`proxy_settings_view.dart`：PAC 生命周期、mode 持久化、PAC 清除位。
- `desktop_integration.dart`：Windows 恒隐藏；dispatcher 安装；托盘动态节点/路由；共享命令路由；PAC 启停 + 模式持久化；退出停 PAC。
- `windows/runner/main.cpp`/`flutter_window.cpp`/`runner_messages.h`：命名互斥量单实例 + 第二实例 `PostMessage` 唤回。
- 测试：`fix15_hotkey_test.dart` 8/8、`fix15_tray_pac_test.dart` 8/8、T12a/T13 回归 19/19；analyze 0 issue；format 0 changed。
- 未运行：真实窗口集成测试、双进程单实例、全量门禁。
