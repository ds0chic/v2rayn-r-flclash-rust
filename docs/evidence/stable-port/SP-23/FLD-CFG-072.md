# SP-23.FLD-CFG-072 — UiItem.CurrentTheme

状态：implemented（实例登记完成；全窗口主题跟随验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-072（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：主题对话框选择 light/dark/system→保存→全窗口
Material/native 主题即时生效；主/独立窗跟随，系统变化与重开一致。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`。

对应 ID：FLD-CFG-072；leaf；platform_scope=['windows','macos','linux']；
original_type=string；original_default=null。
关联：FLD-CFG-071/073（同主题对话框），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.CurrentTheme`；
只读核对原版 light/dark/system 语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 light/dark/system（Dart
`FollowSystem` 缺省）；UI 草稿 `theme_setting_dialog.dart:125/150`、透传
`ui_shell_controller.dart:233 themeMode`。持久化 save；生效 immediate
（`settings_timing.rs:295`）。非法值→冻结回退。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`apps/desktop/lib/features/settings/theme_setting_dialog.dart`
（`:101/125/150`）、`apps/desktop/lib/app/shell/ui_shell_controller.dart`（`:233`）。
本卡未改生产代码。

禁止改变的已有行为：原版主题值语义；系统跟随；其它 UiItem 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` UiItem 段（`:1463` by_path 单测）；
  `persistence/src/upstream_config.rs:291`、`:343/396`、`candidate.rs:1116-1147`、
  `backup_service.rs:1015/1078`、`t16_backup.rs:317/468/493` 主题往返覆盖。
- DTO：`bridge_api` settings UiItem 段 + FRB wire；Dart 默认 `settings_defaults.dart:115 null`；
  `bridge_api/src/api/settings.rs:1460/1471` immediate 变更扇出覆盖 CurrentTheme。
- 调用链：主题对话框 `_set('CurrentTheme')`→保存→重开→shell themeMode。
- 缺口：主/独立窗跟随 + 系统变化 + 重开端到端验收未跑；独立窗硬编码浅色问题
  同 071 待 SP-28（CSV current_gap）。

测试夹具和原版预期：合成主题值；正向 light/dark/system→全窗生效；
负向非法值→回退。

本次必须通过的命令/真实场景（SP-28，未运行）：`flutter test` + 
`cargo test -p persistence --locked`（`upstream_import` 主题断言 `:83/239/283`）；
补正式对话框→FRB→保存→重开→主/独立窗真实主题观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-072.md`。

完成条件：保存、重开和全窗口真实主题三者一致；仅落盘不算。

发现接口缺口时的处理：独立窗跟随缺口已登记；归属 SP-28。
