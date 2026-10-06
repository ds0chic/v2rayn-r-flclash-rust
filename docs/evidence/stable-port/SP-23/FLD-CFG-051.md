# SP-23.FLD-CFG-051 — KcpItem.MaxSendingWindow

状态：implemented（实例登记完成；正式入口/真实 mKCP 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-051（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 KCP MaxSendingWindow→保存→
同 revision plan→xray mKCP 发送窗口适用 wire 能力映射；
冻结 core 参数名/版本，合法数值输出与 actual validation，不支持版本显式诊断。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-051；leaf；platform_scope=all；original_type=int；original_default=2097152。
关联：FLD-CFG-050（同窗口组）/046/047/048/049、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:54 :: KcpItem.MaxSendingWindow`；
只读核对原版最大发送窗口语义（2*1024*1024）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正整数；
`<=0`→回填 `2*1024*1024`（`settings.rs:1049-1050`），不写脏 plan。
持久化 save；生效 restart_core（`settings_timing.rs:168`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs` kcp 投影区（同 `:392-396`）、
xray `outbound.rs:763` 发送窗口装配。
本卡未改生产代码。

禁止改变的已有行为：原版默认 2097152（`domain/src/settings.rs:340`）；
回填语义；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:327`（默认 `2*1024*1024` `:340`；回填 `:1049-1050`）。
- DTO：`bridge_api/src/api/settings.rs:163,175,189` + FRB wire；
  Dart 侧 `api/settings.dart:623` + wire。
- 正式入口：`option_setting_window.dart:817-818`（KCP 区）。
- 投影：kcp 整体投影 `codegen.rs:392-396`。
- 单测：`codegen.rs:1239 r4_13_s14_kcp_item_reaches_codegen`（KCP 组覆盖）；
  Dart `r4_13_s14_contract_test.dart:32` 冻结默认覆盖。
- 缺口：正式入口→FRB→持久化→重开→真实 mKCP 会话完整验收未跑（CSV current_gap）；
  不支持版本显式诊断未验证。

测试夹具和原版预期：合成设置修订；正向 合法值→plan 发送窗口→core 校验；
负向 <=0→回填默认；版本能力按冻结参数名。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s14_kcp_item_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→真实 mKCP 会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-051.md`。

完成条件：plan→真实 mKCP 会话闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实会话/版本诊断缺口已登记；归属 SP-24。
