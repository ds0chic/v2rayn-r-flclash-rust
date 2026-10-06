# SP-23.FLD-CFG-060 — GuiItem.AutoUpdateInterval

状态：implemented（实例登记完成；真实调度启停/改周期端到端验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-060（主 owner SP-23；消费者归属 SP-26）。

本次唯一用户流程：设置窗填写自动更新订阅时间间隔（小时，0=关闭）→保存→
application 订阅 scheduler 真实调度/取消/重新计划；退出后停止。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-060；leaf；platform_scope=all；original_type=int；original_default=0。
关联：SD-07；SubItem AutoUpdateInterval（`subs.rs` 行级），SD-03 订阅事务。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.AutoUpdateInterval`；
只读核对原版小时语义与 0=关闭。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法非负 int（0=关闭）；
UI 草稿 `option_setting_window.dart:913-914`、默认 `settings_defaults.dart:102 0`。
持久化 save；生效 immediate（`settings_timing.rs:120`）。
虚拟 clock 故障注入 + 正式打开/改周期/不重叠/退出停止；失败订阅不覆盖旧组。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-26）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:913-914`）、`apps/desktop/lib/features/settings/resource_auto_update.dart`
（`:19/47` 周期计划）、`crates/application/src/engine.rs`（`:323` 小时槽）、
`crates/application/src/subs.rs`（`:1416` 间隔判定）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 0=关闭；失败订阅不覆盖旧组；其它 GuiItem 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` GuiItem 段（默认 0）；`persistence/src/schema.rs:151`、
  `mapping.rs:142/164` 行级映射。
- DTO：`bridge_api` settings GuiItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('GuiItem','AutoUpdateInterval')`→保存→scheduler 计划/
  取消/重新计划→退出停止。
- 单测：Dart `r4_13_s10_contract_test.dart:21/28`、`r4_34_contract_test.dart:44`、
  `r4_34_repro_test.dart:10/30`（0 关闭）；`resource_auto_update.dart` 计划语义注释。
- 缺口：真实打开/改周期/不重叠/退出停止端到端验收未跑（CSV current_gap）。

测试夹具和原版预期：合成周期值；正向 0→停、N→按 N 小时计划、改周期→重新计划不重叠；
负向负数/坏类型→拒绝，失败订阅→旧组保留。

本次必须通过的命令/真实场景（SP-26，未运行）：`flutter test`
（`r4_13_s10_contract`、`r4_34_contract`、`r4_34_repro`）；
补正式窗→FRB→保存→虚拟 clock 调度断言→重开→退出停止。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-060.md`。

完成条件：保存、重开和真实调度/取消/重计划三者一致；仅落盘不算。

发现接口缺口时的处理：端到端调度验收缺口已登记；归属 SP-26。
