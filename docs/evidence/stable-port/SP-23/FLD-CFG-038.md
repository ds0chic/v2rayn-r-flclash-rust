# SP-23.FLD-CFG-038 — Inbound[0].SniffingEnabled

状态：implemented（实例登记完成；正式入口/真实嗅探路由验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-038（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 SniffingEnabled→保存→同 revision 生成 runtime plan→
xray `sniffing.enabled`/sing-box 入站或路由嗅探动作生效；
与 039（destOverride）×040（routeOnly）联动配置，合成 DNS/HTTP/SNI 实际路由对照。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-038；leaf；platform_scope=all；original_type=bool；original_default=true。
关联：FLD-CFG-039/040（嗅探联动组）/035/036/037/041、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:31 :: InItem.SniffingEnabled`（默认 true）；
只读核对原版流量探测语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
开→嗅探联动 039/040 生效；关→直通路由。
持久化 save；生效 restart_core（`settings_timing.rs:163`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:428`（投影）、
config_codegen xray/sing-box 嗅探段生成。
本卡未改生产代码。

禁止改变的已有行为：关嗅探不改 039/040 存量值；未知入站键保留；stale revision 拒绝；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:357`（默认 true，`domain/src/settings.rs:166`）。
- DTO：`bridge_api/src/api/settings.rs:876,894` + FRB wire（`:7192,11779,14944`）；
  Dart 侧 `api/settings.dart:534` + wire。
- 正式入口：`option_setting_window.dart:536`（“开启流量探测”，core tab `:499`）。
- 投影：`codegen.rs:428 base.inbound.sniffing_enabled`。
- 单测：`codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`
 （`:1220 sniffing=false`、`:1231` 断言）。
- 缺口：sniff×routeOnly×destOverride 联动、合成 DNS/HTTP/SNI 真实路由未验收
 （CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 开/关→plan→core 校验→真实路由；
负向 关→嗅探段缺席但 039/040 值保留。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s13_inbound_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→合成嗅探路由对照→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-038.md`。

完成条件：plan→真实嗅探路由闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：联动验收缺口已登记；归属 SP-24。
