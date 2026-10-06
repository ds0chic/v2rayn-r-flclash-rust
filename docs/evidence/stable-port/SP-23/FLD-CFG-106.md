# SP-23.FLD-CFG-106 — SpeedTestItem.SpeedTestTimeout

状态：implemented（实例登记完成；真实测速超时/取消验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-106（主 owner SP-23；消费者归属 SP-29）。

本次唯一用户流程：设置窗填写测速超时（秒）→保存→Rust speedtest worker
超时/cancel deadline 按新值执行；选值后重开并正式测速不悬挂。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-106；leaf；platform_scope=all；original_type=int；original_default=10。
关联：FLD-CFG-107/108/109（同测速组），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SpeedTestItem.SpeedTestTimeout`；
只读核对原版超时语义（上游归一化下限 10 秒，`speedtest.rs:37`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法正 int；
UI 草稿 `option_setting_window.dart:965-966`、默认 `settings_defaults.dart:135 10`、
调用 `profiles_controller.dart:650 timeoutSecs`。持久化 save；生效 immediate
（`settings_timing.rs:242`）。合成慢响应/超时/取消走真实 worker；失败不悬挂。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-29）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:965-966`）、`apps/desktop/lib/features/profiles/profiles_controller.dart`
（`:650`）、`crates/application/src/speedtest.rs`（`:37/43` 归一化语义）、
`crates/application/src/settings.rs`（`:104` 变更扇出）。
本卡未改生产代码。

禁止改变的已有行为：原版下限归一化（至少 10 秒）；取消释放语义；其它测速项不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` SpeedTestItem 段（`:1100` 修正-非拒绝合同注释）。
- DTO：`bridge_api` settings SpeedTestItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('SpeedTestItem','SpeedTestTimeout')`→保存→重开→
  worker deadline。
- 单测：Rust `persistence/tests/edge_cases.rs:101` 默认 10；
  Dart `ux_speedtest_diag_test.dart:248`、集成 `ux_test02_https_speedtest_test.dart:352` 置值。
- 缺口：合成慢响应/超时/取消真实 worker 端到端验收未跑（CSV current_gap）。

测试夹具和原版预期：合成超时值；正向 10/15→worker deadline 生效；
负向小值→归一化、坏类型→拒绝。

本次必须通过的命令/真实场景（SP-29，未运行）：`cargo test -p application --locked`
（speedtest worker）+ `flutter test`（测速诊断）；
补正式窗→FRB→保存→重开→合成慢响应真实测速→取消。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-106.md`。

完成条件：保存、重开和真实 worker 超时/取消三者一致；仅落盘不算。

发现接口缺口时的处理：真实 worker 验收缺口已登记；归属 SP-29。
