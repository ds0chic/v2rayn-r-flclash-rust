# SP-23.FLD-CFG-037 — Inbound[0].UdpEnabled

状态：implemented（实例登记完成；正式入口/真实 UDP 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-037（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 UdpEnabled→保存→同 revision 生成 runtime plan→支持的 core
上 UDP 能力生效；合成 UDP 往返/拒绝/关闭对照，TCP 不受损。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-037；leaf；platform_scope=all；original_type=bool；original_default=true。
关联：FLD-CFG-035（端口）/036（协议）/038/039/040/041（同入站组）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:30 :: InItem.UdpEnabled`；
只读核对原版 UDP 开关语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
开→支持的 core 放行 UDP 会话（`engine.rs:4984` 门控）；关→UDP 拒绝/关闭，TCP 照常。
持久化 save；生效 restart_core（`settings_timing.rs:164`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:427`（投影）、
`crates/application/src/engine.rs:4984`（UDP 门控）。
本卡未改生产代码。

禁止改变的已有行为：关 UDP 不影响 TCP；未知入站键保留；stale revision 拒绝；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:355`（默认 true，`domain/src/settings.rs:165`）。
- DTO：`bridge_api/src/api/settings.rs:875,893` + FRB wire（`:7191,11778,14943`）；
  Dart 侧 `api/settings.dart:533` + wire。
- 正式入口：`option_setting_window.dart:530`（“开启 UDP”，core tab `:499`）。
- 投影/门控：`codegen.rs:427 base.inbound.udp_enabled`；`engine.rs:4984`。
- 单测：`codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`
 （`:1219 udp=false`、`:1230` 断言）。
- 缺口：真实 UDP 往返/拒绝/关闭对照、关时 TCP 无损验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 开/关→plan→core 校验→真实 UDP 会话；
负向 关→UDP 拒绝且 TCP 照常。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s13_inbound_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→合成 UDP 对照→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-037.md`。

完成条件：plan→真实 UDP 行为闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实 UDP 会话缺口已登记；归属 SP-24。
