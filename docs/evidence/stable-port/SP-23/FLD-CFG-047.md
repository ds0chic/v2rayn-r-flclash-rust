# SP-23.FLD-CFG-047 — KcpItem.Tti

状态：implemented（实例登记完成；正式入口/真实 mKCP 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-047（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 KCP Tti（如 50→40）→保存→
同 revision plan→xray mKCP `tti` 生成；冻结单位/范围及 core 版本实际接受度，
错误保留旧计划（saved≠applied）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-047；leaf；platform_scope=all；original_type=int；original_default=50。
关联：FLD-CFG-046/048/049/050/051（同 KCP 组）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:46 :: KcpItem.Tti`；
只读核对原版 mKCP tti 默认 50。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正整数；
非法→拒绝且旧计划不变；错误保留旧计划。
持久化 save；生效 restart_core（`settings_timing.rs:170`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:393`（投影）、
xray `outbound.rs:754` kcp tti 装配。
本卡未改生产代码。

禁止改变的已有行为：原版默认 50（`domain/src/settings.rs:336`）；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:319`（`tti: i32`，默认 50 `:336`）。
- DTO：`bridge_api/src/api/settings.rs:159,171,185` + FRB wire；
  Dart 侧 KcpItem `tti` + wire。
- 正式入口：`option_setting_window.dart:797-798`（KCP 区）。
- 投影：`codegen.rs:393 base.kcp.tti`。
- 单测：`codegen.rs:1239 r4_13_s14_kcp_item_reaches_codegen`；
  `config_codegen/tests/xray_transport_security.rs:160-172`（tti 落盘断言）；
  Dart `r4_13_s14_contract_test.dart:12`（`:17 Tti=40`→`:25` 断言）与 `:32` 冻结默认。
- 缺口：正式入口→FRB→持久化→重开→真实 mKCP 会话完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 40→plan kcpSettings.tti=40→core 校验；
负向 坏类型→拒绝且旧值不变。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s14_kcp_item_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→真实 mKCP 会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-047.md`。

完成条件：plan→真实 mKCP 会话闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实会话验收缺口已登记；归属 SP-24。
