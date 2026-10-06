# SP-23.FLD-CFG-058 — GuiItem.DisplayRealTimeSpeed

状态：implemented（实例登记完成；真实流量→速率显示链路验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-058（主 owner SP-23；消费者归属 SP-25）。

本次唯一用户流程：设置窗勾选显示实时网速→保存→重启应用→实际流量→
delta/rate→托盘/主窗速率展示开启；关闭后对应展示关闭，不只改 Dart bool。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-058；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-057（同 monitor 门控），SD-13/SD-07/06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: GUIItem.DisplayRealTimeSpeed`；
只读核对原版实时速率展示语义与启动时机。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:839-840`、默认 `settings_defaults.dart:100 false`。
持久化 save；生效 restart_app（`settings_timing.rs:121`）。关后速率展示关闭，
不伪造显示。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-25）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:839-840`）、`crates/application/src/engine.rs`（`:3772-3773` 显示门）、
`crates/application/src/monitor.rs`（`:175` 采集激活判定）、
`apps/desktop/lib/bridge/bridge_port.dart`（`:1192` stats 门注释）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；冻结启动时机；其它 GuiItem 不动；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` GuiItem 段（默认 false）。
- DTO：`bridge_api` settings GuiItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('GuiItem','DisplayRealTimeSpeed')`→controller→FRB→保存→
  重启应用→monitor/engine 速率门→托盘/主窗展示。
- 单测：Dart `r4_13_s10_contract_test.dart:18/25`（save→重开与冻结默认）。
- 缺口：实际流量→delta/rate→显示端到端验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 true→重启应用→合成流量下速率展示更新、
false→展示关闭；负向坏类型→拒绝。

本次必须通过的命令/真实场景（SP-25，未运行）：`flutter test`
（`r4_13_s10_contract`）+ `cargo test -p application --locked`（monitor 门控）；
补正式窗→FRB→保存→重启应用→真实流量速率展示→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-058.md`。

完成条件：保存、重开和真实速率展示三者一致；仅改 Dart bool 不算。

发现接口缺口时的处理：真实显示链路缺口已登记；归属 SP-25。
