# SP-23.FLD-CFG-140 — SystemProxyItem.SystemProxyAdvancedProtocol

状态：implemented（实例登记完成；正式入口/OS 事实验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-140（主 owner SP-23；消费者归属 SP-15/SP-32）。

本次唯一用户流程：系统代理页改高级协议映射模板→apply→隔离机实际
protocol-map 落值与模板一致；模板变化（其余键不变）仍触发重下发。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; actual Endpoint/PlatformReceipt/ownership ledger; 受控OS PC`。

对应 ID：FLD-CFG-140；leaf；platform_scope=windows；original_type=string；original_default=null。
关联：FLD-CFG-138/139/141（同组）、SD-05。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SystemProxyItem.SystemProxyAdvancedProtocol`；
只读核对原版 protocol-map 语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法模板/null（不用高级协议）/
坏模板（拒绝并诊断）；持久化 save；生效 immediate
（`settings_timing.rs:261-265`）。失败注入保留旧映射。

允许修改的模块（SP-15/SP-32）：`proxy_settings_view.dart:65,85,104-132`
（模板装配）、`platform_controller.dart:295`、`platform_service.rs`。
本卡未改生产代码。

禁止改变的已有行为：同 138；坏模板不得静默回退默认。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:703,718`（默认 None）。
- DTO：`bridge_api/src/api/settings.rs:730,742,756` + FRB wire（`:9677,13671,16615`）。
- 缺口：去重键忽略高级协议变化（CSV current_gap）。

测试夹具和原版预期：合成模板；正向 改模板→隔离机落值一致；
负向 坏模板拒绝、失败注入保留旧值。

本次必须通过的命令/真实场景（SP-15/SP-32，未运行）：正式代理页改模板→
apply→隔离机查询→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-140.md`。

完成条件：模板变化触发重下发 + OS 事实一致 + 重开；仅落盘不算。

发现接口缺口时的处理：去重键缺口已登记；归属 SP-15/SP-32。
