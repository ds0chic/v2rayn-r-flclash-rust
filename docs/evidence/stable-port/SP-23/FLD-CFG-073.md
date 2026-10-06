# SP-23.FLD-CFG-073 — UiItem.CurrentLanguage

状态：implemented（实例登记完成；全窗口语言重启应用联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-073（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：主题对话框选择语言→保存→重启应用→菜单/设置/对话框/
独立窗真实文字切换，资源 fallback 正确；重开保持。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`。

对应 ID：FLD-CFG-073；leaf；platform_scope=all；original_type=string；
original_default="zh-Hans"。
关联：FLD-CFG-071/072（同对话框），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.CurrentLanguage`；
只读核对原版语言码语义（`zh-Hans` 缺省）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法语言码；
UI 草稿 `theme_setting_dialog.dart:127/184` + `option_setting_window.dart:944-956`、
透传 `ui_shell_controller.dart:240 language`、映射 `locale_config.dart:5`、
l10n `l10n.dart:6/14`。持久化 save；生效 restart_app（`settings_timing.rs:294`）。
未知码→fallback。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`apps/desktop/lib/features/settings/theme_setting_dialog.dart`
（`:127/184`）、`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:944-956`）、`apps/desktop/lib/app/shell/ui_shell_controller.dart`（`:240`）、
`apps/desktop/lib/app/locale_config.dart`、l10n 资源。
本卡未改生产代码。

禁止改变的已有行为：原版 `zh-Hans` 缺省；fallback 语义；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` UiItem 段（`:1289/1378/1394` 语言往返单测）；
  `persistence/tests/sp01_config_text.rs:31/36`、`edge_cases.rs:45/60-61`、
  `application/tests/sp01_corrupt_config.rs:26/105/139/193` 覆盖。
- DTO：`bridge_api` settings UiItem 段 + FRB wire；Dart 默认
  `settings_defaults.dart:116 zh-Hans`；`make_fixtures.rs:38` 上游模板对照。
- 调用链：对话框 `_set('UiItem','CurrentLanguage')`→保存→重启应用→locale/l10n。
- 单测：Dart `r4_11_contract_test.dart:92`、`fix16_settings_field_test.dart:64/119/133`。
- 缺口：restart_app 后菜单/设置/对话框/独立窗真实文字端到端验收未跑；
  独立窗硬编码语言问题待 SP-28（CSV current_gap）。

测试夹具和原版预期：合成语言码；正向 en/zh-Hans→重启应用→全窗文字切换；
负向未知码→fallback。

本次必须通过的命令/真实场景（SP-28，未运行）：`flutter test`
（`r4_11_contract`、`fix16_settings_field`）；
补正式对话框→FRB→保存→重启应用→全窗真实文字观察→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-073.md`。

完成条件：保存、重启应用、重开和全窗真实文字四者一致；仅主窗重绘不算。

发现接口缺口时的处理：独立窗语言联动缺口已登记；归属 SP-28。
