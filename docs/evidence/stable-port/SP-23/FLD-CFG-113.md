# SP-23.FLD-CFG-113 — SpeedTestItem.SpeedTestDelayInterval

状态：implemented（实例登记完成；实际任务间隔/取消与 UI 线程非阻塞验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-113（主 owner SP-23；消费者归属 SP-29）。

本次唯一用户流程：设置窗改测速间隔（SpeedTestItem.SpeedTestDelayInterval，
int?，台账默认 1）→保存→immediate→测速调度按间隔 deadline 执行，
实际任务间隔准确，取消及时，不阻塞 UI 线程。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-113；leaf；platform_scope=all；original_type=int?；original_default=1。
关联：FLD-CFG-109/112（同调度组，本批已登记），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SpeedTestItem.SpeedTestDelayInterval`；
只读核对原版间隔语义（可空）。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `be408da`（全 `be408da40487e0a8e3bbbe812ee9431e9484c4bf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：合法非负 int / null（缺省）；
取消→调度停止。持久化 save；生效 immediate。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-29）：`crates/application/src/speedtest.rs`
（`:109` `delay = item.speed_test_delay_interval.unwrap_or(1).max(0)`）、
`crates/bridge_api/src/api/settings.rs`（`:540/:554/:570`）。
本卡未改生产代码。

禁止改变的已有行为：缺省 1 语义（Rust 侧 `unwrap_or(1)`）；
取消停止调度；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测，rg 只读）：
- 存储：`settings_defaults.dart:142 'SpeedTestDelayInterval': null`
  （Dart 缺省 null，Rust 侧归一为 1；与台账默认 1 的对应关系已如实记录，
  非等价宣称）。
- DTO：`bridge_api` `speed_test_delay_interval`（`settings.rs:540/554/570`）+
  FRB wire（`frb_generated.rs:9619/13629/16793` 在位）。
- 调用链：保存→`speedtest.rs:109` 归一化→调度 deadline。
- 单测/集成：`codegen.rs:1356`（`Some(2)` 透传在位）；
  `ux_test02:353=0`、`ux_speedtest_diag_test.dart:249=0` 置值。
- 缺口：虚拟 clock 约束+A 下实际任务间隔/取消及界面响应性验收未跑
  （CSV current_gap）。

测试夹具和原版预期：合成间隔；正向多值→任务间隔准确；
负向取消→调度停止且 UI 不卡（待验）。

本次必须通过的命令/真实场景（SP-29，未运行）：`cargo test -p application --locked`
（speedtest 调度）+ `flutter test`（测速诊断）；补正式窗→FRB→保存→
真实调度间隔→取消→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-113.md`。

完成条件：保存、真实间隔和取消响应性三者一致；仅落盘不算。

发现接口缺口时的处理：调度验收 + 缺省口径说明缺口已登记；归属 SP-29。
