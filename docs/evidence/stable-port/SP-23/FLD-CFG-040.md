# SP-23.FLD-CFG-040 — Inbound[0].RouteOnly

状态：implemented（实例登记完成；正式入口/真实分流对照验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-040（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 RouteOnly→保存→同 revision 生成 runtime plan→
xray `sniffing.routeOnly` 与适用 sing-box 等价路由语义生效；
开启仅路由不改目的地址；合成对照，关时按冻结 core 语义。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-040；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-038/039（嗅探联动组）/035/036/037/041、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:33 :: InItem.RouteOnly`；
只读核对原版仅路由语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
开→仅路由（目的地址不变）；关→按冻结 core 语义改写。
持久化 save；生效 restart_core（`settings_timing.rs:157`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:430`（投影）、
config_codegen xray/sing-box 嗅探/路由段生成。
本卡未改生产代码。

禁止改变的已有行为：开不改目的地址；未知入站键保留；stale revision 拒绝；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:360`（默认 false，`domain/src/settings.rs:168`）。
- DTO：`bridge_api/src/api/settings.rs:878,895` + FRB wire（`:7194,11781,14946`）；
  Dart 侧 `api/settings.dart:536` + wire。
- 正式入口：`option_setting_window.dart:586`（“仅限路由 (routeOnly)”，core tab `:499`）。
- 投影：`codegen.rs:430 base.inbound.route_only`。
- 单测：`codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`
 （`:1222 route_only=true`、`:1233` 断言）。
- 缺口：开/关合成路由对照、sing-box 等价语义验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 开/关→plan→core 校验→真实分流；
负向 需嗅探联动语义一致（038/039 同测）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s13_inbound_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→合成路由对照→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-040.md`。

完成条件：plan→真实分流闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：联动验收缺口已登记；归属 SP-24。
