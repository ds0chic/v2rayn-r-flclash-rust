# SP-23.FLD-CFG-048 — KcpItem.UplinkCapacity

状态：implemented（实例登记完成；正式入口/真实 mKCP 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-048（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 KCP UplinkCapacity（如 12→15）→保存→
同 revision plan→xray mKCP `uplinkCapacity` 生成；字段边界/缺省、方向单位正确，
真实支持版本会话验证。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-048；leaf；platform_scope=all；original_type=int；original_default=12。
关联：FLD-CFG-049（下行成对）/046/047/050/051、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:48 :: KcpItem.UplinkCapacity`；
只读核对原版上行容量默认 12。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正整数；
上/下行方向不混淆；非法→拒绝且旧值不变。
持久化 save；生效 restart_core（`settings_timing.rs:171`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:394`（投影）、
xray `outbound.rs` kcp 容量装配。
本卡未改生产代码。

禁止改变的已有行为：原版默认 12（`domain/src/settings.rs:337`）；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:321`（默认 12 `:337`）。
- DTO：`bridge_api/src/api/settings.rs:160,172,186` + FRB wire；
  Dart 侧 `api/settings.dart:620` + wire。
- 正式入口：`option_setting_window.dart:802-803`（KCP 区）。
- 投影：`codegen.rs:394 base.kcp.uplink_capacity`。
- 单测：`codegen.rs:1239 r4_13_s14_kcp_item_reaches_codegen`
 （`:1243 UplinkCapacity=15`、`:1250` 断言）；
  Dart `r4_13_s14_contract_test.dart:12`（`:18 UplinkCapacity=15`→`:25` 断言）。
- 缺口：正式入口→FRB→持久化→重开→真实 mKCP 会话完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 15→plan uplinkCapacity=15→core 校验；
负向 坏类型→拒绝；方向单位正确。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s14_kcp_item_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→真实 mKCP 会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-048.md`。

完成条件：plan→真实 mKCP 会话闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实会话验收缺口已登记；归属 SP-24。
