# SP-23.FLD-CFG-139 — SystemProxyItem.NotProxyLocalAddress

状态：implemented（实例登记完成；正式入口/OS 事实验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-139（主 owner SP-23；消费者归属 SP-15/SP-32）。

本次唯一用户流程：系统代理页开关“不代理本地地址”→apply→隔离机实际
bypass 合成含/不含本地段；开关翻转（其余键不变）仍触发重下发。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; actual Endpoint/PlatformReceipt/ownership ledger; 受控OS PC`。

对应 ID：FLD-CFG-139；leaf；platform_scope=windows；original_type=bool；original_default=true。
关联：FLD-CFG-138/140/141（同组）、SD-05。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SystemProxyItem.NotProxyLocalAddress`；
只读核对原版本地 bypass 合成语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
持久化 save；生效 immediate（`settings_timing.rs:255-259`）。
失败注入保留旧合成；平台写入仅授权隔离机。

允许修改的模块（SP-15/SP-32）：`proxy_settings_view.dart:64,84,160-167`
（开关与合成）、`platform_controller.dart:299`（下发装配）、
`platform_service.rs`（OS 生效）。本卡未改生产代码。

禁止改变的已有行为：同 138；Windows 专属语义不扩到全平台。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:701,717`（默认 true；`:1436` 单测）。
- DTO：`bridge_api/src/api/settings.rs:729,741,755` + FRB wire（`:9676,13670,16614`）。
- 缺口：去重键忽略本地 bypass 开关变化（CSV current_gap）。

测试夹具和原版预期：合成开关；正向 开/关→隔离机合成含/不含本地段；
负向 失败注入保留旧合成、去重键验证。

本次必须通过的命令/真实场景（SP-15/SP-32，未运行）：正式代理页翻转开关→
apply→隔离机查询→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-139.md`。

完成条件：开关变化触发重下发 + OS 事实一致 + 重开；仅落盘不算。

发现接口缺口时的处理：去重键缺口已登记；归属 SP-15/SP-32。
