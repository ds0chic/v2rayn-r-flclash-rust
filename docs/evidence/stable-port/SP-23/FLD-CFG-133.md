# SP-23.FLD-CFG-133 — ClashUIItem.ProxiesRefreshInterval

状态：implemented（实例登记完成；真实轮询间隔联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-133（主 owner SP-23；消费者归属 SP-25）。

本次唯一用户流程：Clash 代理页/设置窗修改刷新间隔（秒，默认 2）→保存→
真实 core API 按 canonical ProxiesRefreshInterval 节流轮询；
隐藏暂停/取消、core 切换按 timer lifecycle 生效；保存失败可见且回滚。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; native monitor端点来自实际session; timer lifecycle`。

对应 ID：FLD-CFG-133；leaf；platform_scope=all；original_type=int；original_default=2。
关联：FLD-CFG-131（ProxiesSorting，本卡前批已登记）/132（同轮询组，本批）、
FLD-CFG-134/135（connections 侧），SD-13/SD-14。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ClashUIItem.ProxiesRefreshInterval`；
只读核对原版刷新间隔语义（默认 2 秒）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法正 int（秒）；
配置读取 `clash_ui_config.dart`（同组 `ProxiesAutoRefresh:65`/`ProxiesSorting:67`
附近的 interval 解析）、视图 `proxies_view.dart` 轮询 timer。
持久化 save；生效 immediate。保存异步失败传播缺口同 131/132，已登记。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-25）：`apps/desktop/lib/features/monitor/clash_ui_config.dart`
（interval 解析）、`apps/desktop/lib/features/monitor/proxies_view.dart`
（轮询 timer）、`apps/desktop/lib/features/settings/option_setting_window.dart`
（ClashUI 设置区）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 2；轮询可见页面生命周期；其它 ClashUI 不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` ClashUIItem 段（默认 2）。
- DTO：`bridge_api` settings ClashUIItem 段 + FRB wire；Dart 侧 wire。
- 调用链：设置/代理页改间隔→保存→重开→轮询 timer 按新间隔请求。
- 单测：Dart `fix16c_clash_ui_config_test.dart:187`
 （proxies view honors ProxiesAutoRefresh + interval）。
- 缺口：真实 core API 请求频率 + 隐藏暂停/取消 + core 切换验收未跑；
  保存失败回滚缺口未验证（CSV current_gap）。

测试夹具和原版预期：合成间隔值；正向 2/5→轮询周期对应变化；
负向 0/负数/坏类型→拒绝或冻结回退，保存失败→可见且回滚（待验证）。

本次必须通过的命令/真实场景（SP-25，未运行）：`flutter test`
（`fix16c_clash_ui_config`）；
补正式页→FRB→保存→真实 core API 频率观察→隐藏/切换→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-133.md`。

完成条件：保存、重开和真实轮询频率三者一致 + 失败回滚；仅 UI 值变化不算。

发现接口缺口时的处理：轮询联动 + 保存失败回滚缺口已登记；归属 SP-25。
