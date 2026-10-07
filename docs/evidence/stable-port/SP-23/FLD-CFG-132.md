# SP-23.FLD-CFG-132 — ClashUIItem.ProxiesAutoRefresh

状态：implemented（实例登记完成；真实自动刷新轮询联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-132（主 owner SP-23；消费者归属 SP-17）。

本次唯一用户流程：Clash 代理页开关自动刷新→保存→真实 core API 按
canonical ProxiesAutoRefresh 启停轮询，可见页面生命周期正确；
保存失败可见且回滚用户选择。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-13、SD-14`。

对应 ID：FLD-CFG-132；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-131/133（同 proxies 轮询组），SD-13/SD-14。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ClashUIItem.ProxiesAutoRefresh`；
只读核对原版自动刷新语义（默认 false）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:1123-1124` + `proxies_view.dart:85` 开关持久化、
配置 `clash_ui_config.dart:65` 读取、默认 `settings_defaults.dart:163 false`。
持久化 save；生效 immediate。保存异步失败传播缺口同 131，已登记。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-17）：`apps/desktop/lib/features/monitor/proxies_view.dart`
（`:85`）、`apps/desktop/lib/features/monitor/clash_ui_config.dart`（`:65`）、
`apps/desktop/lib/features/settings/option_setting_window.dart`（`:1123-1124`）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；轮询可见页面生命周期；其它 ClashUI 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` ClashUIItem 段（默认 false）。
- DTO：`bridge_api` settings ClashUIItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：代理页开关→`_persistClash({'ProxiesAutoRefresh': value})`→保存→重开→
  轮询启停。
- 单测：Dart `r4_13_s02_contract_test.dart:21/30`、`fix16c_clash_ui_config_test.dart:34/187`
 （autoRefresh + interval 视图）。
- 缺口：真实 core API 轮询启停 + 隐藏暂停/取消 + core 切换验收未跑；
  保存失败回滚缺口未验证（CSV current_gap）。

测试夹具和原版预期：合成开关值；正向 true→轮询启、false→停；
负向保存失败→可见且回滚（待验证），坏类型→拒绝。

本次必须通过的命令/真实场景（SP-17，未运行）：`flutter test`
（`r4_13_s02_contract`、`fix16c_clash_ui_config`）；
补正式页→FRB→保存→真实轮询启停→隐藏/切换→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-132.md`。

完成条件：保存、重开和真实轮询启停三者一致 + 失败回滚；仅 UI 值变化不算。

发现接口缺口时的处理：轮询联动 + 保存失败回滚缺口已登记；归属 SP-17。
