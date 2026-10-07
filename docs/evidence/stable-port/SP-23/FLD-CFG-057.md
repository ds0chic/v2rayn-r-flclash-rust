# SP-23.FLD-CFG-057 — GuiItem.EnableStatistics

状态：implemented（实例登记完成；真实统计采集/表格列联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-057（主 owner SP-23；消费者归属 SP-17）。

本次唯一用户流程：设置窗勾选启用统计→保存→重启应用→monitor/core API
真实统计采集开启，表格统计列按冻结时机显隐；关闭不伪造零值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-057；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-058（DisplayRealTimeSpeed，同 monitor 门控），SD-13/SD-07/06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.EnableStatistics`；
只读核对原版统计采集开关语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:834-835`、默认 `settings_defaults.dart:99 false`。
持久化 save；生效 restart_app（`settings_timing.rs:124`）。失败→旧行为保留，
保存失败不更新 applied。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-17）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:834-835`）、`apps/desktop/lib/app/shell/ui_shell_controller.dart`（`:242 showStatistics`）、
`crates/application/src/engine.rs`（`:3772-3773` 采集门、`settings.rs:254,268` 变更扇出）、
`crates/application/src/monitor.rs`（`:175` 采集激活判定）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；关闭不伪造零；其它 GuiItem 不动；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` GuiItem 段（默认 false；`by_path` 含
  `GuiItem.EnableStatistics`，`:1459` 附近单测覆盖）。
- DTO：`bridge_api` settings GuiItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('GuiItem','EnableStatistics')`→controller→FRB→保存→
  重启应用→engine/monitor 采集门 + `ui_shell_controller:242 showStatistics`。
- 单测：`engine.rs:6626/6647` 开/关扇出断言；Dart `r4_13_s10_contract_test.dart:17/24`
 （save→重开与冻结默认）；`t12a_settings_storage_test.dart:30/50` 落盘往返。
- 缺口：真实 API 采集流量/计数聚合与表格列显隐端到端验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 true→重启应用→真实采集开、
false→关；负向坏类型→拒绝，保存失败→回滚可见。

本次必须通过的命令/真实场景（SP-17，未运行）：`cargo test -p application --locked`
（settings 扇出/monitor 门控）+ `flutter test`（`r4_13_s10_contract`、`t12a`）；
补正式窗→FRB→保存→重启应用→真实采集/列显隐→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-057.md`。

完成条件：保存、重开和 monitor 真实采集/列显隐三者一致；仅落盘/仅单测不算。

发现接口缺口时的处理：真实采集联动缺口已登记；归属 SP-17。
