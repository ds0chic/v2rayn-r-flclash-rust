# SP-23.FLD-CFG-050 — KcpItem.CwndMultiplier

状态：implemented（实例登记完成；正式入口/真实 mKCP 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-050（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 KCP CwndMultiplier→保存→
同 revision plan→xray mKCP `cwndMultiplier` 适用 wire 能力映射；
冻结 core 参数名/版本，合法数值输出与 actual validation，不虚称新版都支持。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-050；leaf；platform_scope=all；original_type=int；original_default=1。
关联：FLD-CFG-051（同窗口组）/046/047/048/049、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:52 :: KcpItem.CwndMultiplier`；
只读核对原版拥塞窗口倍数语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正整数；
`<=0`→钳制为 1（`settings.rs:1046-1047`），不写脏 plan。
持久化 save；生效 restart_core（`settings_timing.rs:166`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:396`
（`.max(1)` 钳制投影）、xray `outbound.rs:760` cwndMultiplier 装配。
本卡未改生产代码。

禁止改变的已有行为：原版默认 1（`domain/src/settings.rs:339`）；
钳制语义（非法不报错写 1）；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:325`（默认 1 `:339`；钳制 `:1046-1047`）。
- DTO：`bridge_api/src/api/settings.rs:162,174,188` + FRB wire；
  Dart 侧 `api/settings.dart:622` + wire。
- 正式入口：`option_setting_window.dart:812-813`（KCP 区）。
- 投影：`codegen.rs:396 base.kcp.cwnd_multiplier`。
- 单测：`codegen.rs:1239 r4_13_s14_kcp_item_reaches_codegen`（KCP 组覆盖）；
  Dart `r4_13_s14_contract_test.dart:32` 冻结默认覆盖。
- 缺口：正式入口→FRB→持久化→重开→真实 mKCP 会话完整验收未跑（CSV current_gap）；
  不支持版本须显式诊断（未验证）。

测试夹具和原版预期：合成设置修订；正向 合法值→plan cwndMultiplier→core 校验；
负向 <=0→钳制 1；版本能力按冻结参数名。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s14_kcp_item_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→真实 mKCP 会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-050.md`。

完成条件：plan→真实 mKCP 会话闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实会话/版本诊断缺口已登记；归属 SP-24。
