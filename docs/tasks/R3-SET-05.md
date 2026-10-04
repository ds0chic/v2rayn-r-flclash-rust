# R3-SET-05 — 热键部分注册冲突后继续编辑不恢复实时动作

状态：`implemented`（fake registrar 断言 partial failure 后保持 paused、撤销成功组合、cancel 恢复；未运行真实 OS 热键）。

任务 ID：R3-SET-05

本次唯一用户流程：两个组合中一个注册成功、一个被占用；点保存后窗口提示冲突并保持打开；继续录制时不应触发任何已成功组合的动作；结束编辑（取消/关闭）后才恢复。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核 `docs/evidence/parity-recheck-2026-10-04/round3-settings.md` SR-05 / R3-SET-05。

上游对照：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/v2rayN/Views/GlobalHotkeySettingWindow.xaml.cs:16-17`（构造到 Closing 一直 `IsPause=true`）；`v2rayN/Manager/HotkeyManager.cs:137-166`（暂停时转录制，非暂停才 dispatch）。

对应 feature / field / action：`ACT-HOTKEY-006..008`、`LAY-HOTKEY-001`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：编辑后的 bindings、persist 回调。
- 输出：`save` 仅在持久化成功且全部注册成功时返回 true（窗口关闭）；否则窗口保持打开。
- 错误：冲突/注册失败写入 `state.conflicts`，UI 显示冲突文本。
- 取消：`cancelEdit`（窗口 dispose）恢复持久化 bindings 的注册并解除暂停。
- 权限：`None`；fake registrar，无真实插件。

允许修改的模块：`apps/desktop/lib/features/settings/{hotkeys.dart,global_hotkey_window.dart}`、`apps/desktop/test/**`、本卡、证据目录、compat 台账（仅追加）。

禁止改变的已有行为：同组合多动作分组（SR-06）；正常保存关闭窗口；录制仅捕获不派发。

测试夹具与原版预期：`sr05_sr06_hotkey_test.dart` 的 fake registrar 使 `showForm` 注册失败；断言 `save` 返回 false、`isPaused=true`、成功组合注册被撤销且不触发；`cancelEdit` 后恢复并触发。原版预期：编辑窗口存在期间不派发动作。

本次必须通过的命令/真实场景：
- `flutter analyze`
- `flutter test test/sr05_sr06_hotkey_test.dart`

证据文件位置：`docs/evidence/recheck-fixes/R3-MISC/README.md`。

完成条件：编辑期间（含保存冲突后窗口保持打开）一直阻止动作 dispatch；成功组合不在编辑结束前恢复；补 partial-failure→record/cancel/retry 测试；门禁通过。

接口缺口（登记）：native registrar 无“注册但不派发”模式，冲突检测期间存在极短注册窗口后立即 `unregisterAll`；如需严格原子，建议插件提供 dry-run/check API。

本轮实际结果：`HotkeyController.save` 在持久化成功但存在冲突时设 `_paused=true`、`unregisterAll` 并清空 `registered`，窗口保持打开；无冲突才 `_paused=false`。新增 partial-failure→cancel 用例；既有 SR-05/06 用例仍通过。
