# SP-23.FLD-CFG-097 — TunModeItem.StrictRoute

状态：implemented（实例登记完成；正式入口/真实路由效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-097（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗改 StrictRoute→保存→同 revision 生成 runtime plan→对应 core
TUN wire 与真实路由生效；隔离 VM 验证 strict 开/关泄漏与例外地址；停止恢复。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`主方案TUN实际session/route lease/退出清理; SD-04/05; 隔离VM真实验收`。

对应 ID：FLD-CFG-097；leaf；platform_scope=all；original_type=bool；original_default=true。
关联：FLD-CFG-095（总开关）/096/098/099/101/104（同 TUN 组）、SD-07、SD-05/18。

必读上游文件、符号和固定 commit：`ConfigItems.cs:145 :: TunModeItem.StrictRoute`（默认 true）；
只读核对原版严格路由语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
开→严格路由（防泄漏）→core wire；关→宽松路由。
持久化 save；生效 restart_core（`settings_timing.rs:289`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:445`（投影）、
`crates/application/src/tun_plan.rs`（路由 plan 合成）、`services/net_host` helper 生效。
本卡未改生产代码。

禁止改变的已有行为：未知 TUN 键保留；例外地址（103）独立存在，不因 strict 关被删；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:272`（默认 true，`:298`；serde `default_true`）。
- DTO：`bridge_api/src/api/settings.rs:105,122,141` + FRB wire；
  Dart 侧 `api/settings.dart:1189` + wire。
- 正式入口：`option_setting_window.dart:1212`（TUN tab `:1196`）。
- 投影：`codegen.rs:445 base.tun.strict_route`。
- 单测：`codegen.rs:1037 r4_13_s22_tun_fields_reach_generated_config`
 （`:1057 assert!(strict_route)`、`:1085` JSON 断言）。
- 缺口：隔离 VM strict 开/关泄漏与例外地址对照、停止恢复未验收（CSV current_gap）。

测试夹具和原版预期：合成 TUN 修订；正向 开/关→plan 路由段对应变化→core 校验；
负向 需 EnableTun 开才有意义；stale 按冻结语义拒绝。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s22_tun_fields_reach_generated_config`）；
正式 TUN 窗→FRB→保存→同 revision plan→隔离 VM 泄漏对照→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-097.md`。

完成条件：plan→隔离 VM 真实路由效果闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实路由效果缺口已登记；归属 SP-24。
