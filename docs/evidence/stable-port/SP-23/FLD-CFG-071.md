# SP-23.FLD-CFG-071 — UiItem.ColorPrimaryName

状态：implemented（实例登记完成；全窗口 accent 联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-071（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：主题对话框选择主色→保存→全窗口 accent 即时解析；
合法/未知色名按冻结回退，所有独立窗一致，菜单选中色对比度正常。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`。

对应 ID：FLD-CFG-071；leaf；platform_scope=['windows','macos','linux']；
original_type=string；original_default=null。
关联：FLD-CFG-072/073（主题/语言同对话框），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.ColorPrimaryName`；
只读核对原版 Material 色名语义与 null 缺省。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 Material swatch 名/null；
UI 草稿 `theme_setting_dialog.dart:126/157`、透传 `ui_shell_controller.dart:229 accentRaw`、
解析 `app_theme.dart:365`。持久化 save；生效 immediate（`settings_timing.rs:291`）。
未知色名→冻结回退。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`apps/desktop/lib/features/settings/theme_setting_dialog.dart`
（`:101-102/125-157`）、`apps/desktop/lib/app/shell/ui_shell_controller.dart`（`:229`）、
`apps/desktop/lib/shared/theme/app_theme.dart`（`:365` resolver）。
本卡未改生产代码。

禁止改变的已有行为：原版 null 缺省；未知色名回退；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` UiItem 段（null 缺省）。
- DTO：`bridge_api` settings UiItem 段 + FRB wire；Dart 默认 `settings_defaults.dart:114 null`。
- 调用链：主题对话框 `_set('ColorPrimaryName')`→保存→重开→shell/accent resolver。
- 缺口：全窗口 + 独立窗 accent 一致性验收未跑；CSV 记独立设置窗硬编码浅色主题
  问题，主窗消费者存在但独立窗联动待 SP-28（CSV current_gap）。

测试夹具和原版预期：合成色名；正向合法色→accent 生效、null→缺省；
负向未知色名→冻结回退。

本次必须通过的命令/真实场景（SP-28，未运行）：`flutter test`（主题/settings 合同）；
补正式对话框→FRB→保存→重开→主/独立窗真实 accent 观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-071.md`。

完成条件：保存、重开和全窗口真实 accent 三者一致；仅落盘不算。

发现接口缺口时的处理：独立窗联动缺口已登记；归属 SP-28。
