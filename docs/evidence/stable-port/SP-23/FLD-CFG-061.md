# SP-23.FLD-CFG-061 — GuiItem.TrayMenuServersLimit

状态：implemented（实例登记完成；真实 native tray 菜单上限验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-061（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗填写托盘节点菜单数量上限→保存→DesktopIntegration
托盘节点菜单按上限截断并标记当前节点；0/1/上限/大列表均按冻结语义展示，快捷切换可用。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-061；leaf；platform_scope=all；original_type=int；original_default=20。
关联：SD-07；托盘模型 `tray_menu_model.dart:69/199`。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.TrayMenuServersLimit`；
只读核对原版托盘上限语义（默认 20）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法非负 int；
UI 草稿 `option_setting_window.dart:908-909`、默认 `settings_defaults.dart:103 20`。
持久化 save；生效 save（`settings_timing.rs:127`）。
0/1/上限/大列表真实 native tray 菜单顺序/当前标记与快捷切换。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:908-909`）、`apps/desktop/lib/app/shell/desktop_integration.dart`（`:461` 读取）、
`apps/desktop/lib/app/shell/ui_shell_controller.dart`（`:245` 透传）、
`apps/desktop/lib/app/shell/tray_menu_model.dart`（`:69/199` 上限语义）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 20；菜单顺序与当前标记语义；其它 GuiItem 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` GuiItem 段（默认 20；`:1377` 类型错误单测覆盖
  `TrayMenuServersLimit=oops-not-a-number`）。
- DTO：`bridge_api` settings GuiItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('GuiItem','TrayMenuServersLimit')`→保存→重开→
  `desktop_integration:461`→tray 模型截断与当前标记。
- 单测：`persistence/tests/sp01_config_text.rs:23/46` 类型错误结构化；
  `application/tests/sp01_corrupt_config.rs:27/86/140/193` 腐蚀/恢复；
  Dart `r4_11_contract_test.dart:79/89`、`r4_13_s10_contract_test.dart:20/27/50`、
  `t13_platform_models_test.dart:55` 上限隐藏。
- 缺口：真实 native tray 大列表顺序/当前标记/快捷切换验收未跑（CSV current_gap）。

测试夹具和原版预期：合成节点列表；正向 0/1/20/大列表→菜单截断与当前标记正确；
负向坏类型→拒绝且旧值保留。

本次必须通过的命令/真实场景（SP-24，未运行）：`flutter test`
（`r4_13_s10_contract`、`t13_platform_models`）+ `cargo test -p persistence --locked`
（`sp01_config_text`）；补正式窗→FRB→保存→重开→隔离机真实 tray 观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-061.md`。

完成条件：保存、重开和真实 tray 菜单三者一致；仅落盘/仅单测不算。

发现接口缺口时的处理：真实 tray 验收缺口已登记；归属 SP-24。
