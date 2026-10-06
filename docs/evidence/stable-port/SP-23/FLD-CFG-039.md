# SP-23.FLD-CFG-039 — Inbound[0].DestOverride

状态：implemented（实例登记完成；正式入口/真实协议识别验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-039（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 DestOverride 列表→保存→同 revision 生成 runtime plan→
适用 core（xray `sniffing.destOverride` 等）列表生效；冻结列表顺序/未知项策略；
实际协议识别对照，不只序列化。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-039；leaf；platform_scope=all；original_type=list<string>；
original_default=["http","tls"]。
关联：FLD-CFG-038/040（嗅探联动组）/035/036/037/041、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:32 :: InItem.DestOverride`
（默认 ["http","tls"]）；只读核对原版覆盖目标语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 字符串列表/null（null→缺省）；
未知项按冻结策略处理，顺序按冻结语义保留。
持久化 save；生效 restart_core（`settings_timing.rs:152`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:429`（投影，
None→`unwrap_or_default`）、config_codegen 嗅探段生成。
本卡未改生产代码。

禁止改变的已有行为：未知项不静默丢弃语义按冻结策略；列表顺序保留；未知入站键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:358-359`（`Option<Vec<String>>`，
  serde `default_dest_override`，`:259`）；默认 ["http","tls"]（`settings.rs:167`）。
- DTO：`bridge_api/src/api/settings.rs:877,894` + FRB wire（`:7193,11780,14945`）；
  Dart 侧 `api/settings.dart:535` + wire。
- 正式入口：`option_setting_window.dart:568`（DestOverride 列表编辑，core tab `:499`）。
- 投影：`codegen.rs:429 base.inbound.dest_override`。
- 单测：`codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`
 （`:1221 ["tls"]`、`:1232` 断言）。
- 缺口：冻结顺序/未知项策略的真实协议识别对照未验收（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 改列表→plan→core 校验→真实识别；
负向 未知项/空按冻结策略；顺序保留断言。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s13_inbound_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→真实协议识别→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-039.md`。

完成条件：plan→真实协议识别闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实识别验收缺口已登记；归属 SP-24。
