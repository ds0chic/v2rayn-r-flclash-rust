# SP-23.FLD-CFG-141 — SystemProxyItem.CustomSystemProxyPacPath

状态：implemented（实例登记完成；正式入口/OS 事实验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-141（主 owner SP-23；消费者归属 SP-15/SP-32）。

本次唯一用户流程：系统代理页设自定义 PAC 路径→apply→隔离机实际 PAC
文件/hash 与该路径一致；路径变化（其余键不变）仍触发重下发；
不存在路径按上游 fallback 语义处理（非静默创建）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; actual Endpoint/PlatformReceipt/ownership ledger; 受控OS PC`。

对应 ID：FLD-CFG-141；leaf；platform_scope=windows；original_type=string；original_default=null。
关联：FLD-CFG-138/139/140（同组）、SD-05。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SystemProxyItem.CustomSystemProxyPacPath`；
只读核对原版 PAC 自定义文件/默认 fallback 语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法路径/null（默认 fallback）/
不存在路径（按冻结 fallback，不静默创建）；持久化 save；生效 immediate
（`settings_timing.rs:245-249`）。失败注入保留旧 PAC。

允许修改的模块（SP-15/SP-32）：`platform_controller.dart:182-187,310`
（路径装配）、`platform.dart:96-99,159-161`（`validateCustomProxyScript`）、
`proxy_settings_view.dart:66,88,210-213`、`platform_service.rs`。
本卡未改生产代码。

禁止改变的已有行为：同 138；不存在路径不得静默创建（CSV 缺口即此）。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:705,719`（默认 None）。
- DTO：`bridge_api/src/api/settings.rs:731,743,757` + FRB wire
  （`frb_generated.dart:8549,12477,15718`；Dart `frb_generated.dart:314,325,650`）。
- 缺口：去重键忽略自定义路径；不存在路径被创建而非上游 fallback
  （CSV current_gap）。

测试夹具和原版预期：合成 PAC 文件；正向 设路径→隔离机文件/hash 一致；
负向 不存在路径→fallback、失败注入保留旧 PAC。

本次必须通过的命令/真实场景（SP-15/SP-32，未运行）：正式代理页设路径→
apply→隔离机查询→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-141.md`。

完成条件：路径变化触发重下发 + fallback 语义一致 + 重开；仅落盘不算。

发现接口缺口时的处理：去重键/fallback 双缺口已登记；归属 SP-15/SP-32。
