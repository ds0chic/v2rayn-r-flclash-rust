# SR-05 / SR-06 证据

状态：`implemented`。基线 HEAD `256c3dd`（工作树当时干净；`group_editor_dialog.dart` 在本轮期间被并行任务改动，与本卡无关，未触碰）。冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`（WPF 为准）。

## 结论

- SR-05：打开「全局热键」编辑窗口时暂停原生分派/注销旧绑定（对齐上游 `HotkeyManager.IsPause`），录制期间旧组合只被 Flutter 录制器捕获、不触发窗口/代理动作；取消/关闭恢复旧注册；保存按新草稿重注册并真实 dispatch。用可注入 `_FakeOsRegistrar` 建模 OS 注册表覆盖完整生命周期，**未注册任何真实 OS 全局热键**。
- SR-06：`groupHotkeyBindings` 按 `HotkeyCombo`（modifiers+持久化 WPF Key）分组，同组合只注册一次、回调按动作列表顺序分派（对齐上游 `Dictionary<int,List<EGlobalHotkey>>`）；同组内重复动作去重、未绑定跳过。注册失败/冲突如实报告，`HotkeyController.save` 仅在“持久化成功且无冲突”时返回 true，窗口保持打开。

## 改动文件

- `apps/desktop/lib/features/settings/hotkeys.dart`：新增 `HotkeyCombo`/`HotkeyRegistration`/`groupHotkeyBindings`；`HotkeyRegistrar.register` 改为接收分组；`PluginHotkeyRegistrar` 每组合一次 `manager.register` 并遍历动作回调；`HotkeyController` 新增 `beginEdit`/`cancelEdit`/`isPaused`，`save` 返回成功且无冲突；删除仅按 action 编码的标识函数。
- `apps/desktop/lib/features/settings/global_hotkey_window.dart`：`initState` 调 `beginEdit`；`dispose` 在未保存时 `cancelEdit`；保存成功置 `_saved` 后关窗，冲突时保持打开并提示。
- `apps/desktop/test/sr05_sr06_hotkey_test.dart`（新增，6 项）：分组纯函数、单次注册双动作分派、打开暂停+仅录制+取消恢复、保存新注册 dispatch、注册失败不关窗。
- `apps/desktop/test/fix15_hotkey_test.dart`、`apps/desktop/test/t13_hotkey_test.dart`：假 registrar 适配分组接口，语义不变。
- `docs/tasks/SR-05.md`、`docs/tasks/SR-06.md`（新增）；`compat/features.yaml`、`compat/actions.yaml`（仅追加 evidence 引用）。

## 实际命令与结果

在 `apps/desktop`：

```powershell
& 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat' analyze
# -> No issues found! (3.0s)  （见 flutter-analyze.log）

& 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat' test --concurrency=1 `
  test\sr05_sr06_hotkey_test.dart test\fix15_hotkey_test.dart `
  test\t13_hotkey_test.dart test\t12a_hotkey_test.dart
# -> +21: All tests passed!  （见 flutter-hotkey-tests.log）

& 'C:\Users\Colby\toolchains\flutter\bin\flutter.bat' test test\fix15b_startup_test.dart
# -> +3: All tests passed!
```

格式化：`dart format` 对上述改动文件执行，仅新增测试文件需格式化，其余“0 changed”。

注：默认并行 `flutter test` 跑本组文件时偶发把 `sr05_sr06_hotkey_test.dart` 的 widget 用例报成 `did not complete`（本仓已知 `flutter test` 并发偶发；单独或 `--concurrency=1` 稳定全绿），故证据用 `--concurrency=1`。

## 上游对照

- SR-05：`GlobalHotkeySettingWindow.xaml.cs:16-17` 打开置 `IsPause=true`、关闭置 false；`HotkeyManager.cs:145-158` 暂停时把 `WM_HOTKEY` 转成控件 `KeyDown`、`:159-165` 非暂停才逐动作触发。本项目在暂停时直接 `unregisterAll` 丢弃现网注册，使按键由 OS 正常投递到 Flutter 编辑器录制；行为等价（编辑期间旧组合不执行动作、只被录制；关闭恢复），差异仅在“重新注入 KeyDown”改为“不拦截按键”，因 Flutter 录制入口是窗口 Focus 而非 WPF TextBox。
- SR-06：`HotkeyManager.cs:8/25-59` 用 `Dictionary<int,List<EGlobalHotkey>>` 同组合仅注册一次、`:161-165` 逐动作分派。本项目 `groupHotkeyBindings`+`HotkeyCombo` 一一对应；持久化仍按动作逐条存 `GlobalHotkeys[]`，与上游一致。

## 边界与未运行

- 未注册真实 OS 全局热键、未进行真机编辑窗口录制验证（硬约束：热键测试用假 registrar）。
- 未验证真实 `hotkey_manager` 在同一组合多动作下的底层行为（登记为接口缺口）。
- 未改托盘/关闭/`main_shell`/`app.dart`；未跑全量 workspace 测试与 `flutter build windows`。
