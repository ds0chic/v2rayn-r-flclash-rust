# SP-23.FLD-CFG-138 — SystemProxyItem.SystemProxyExceptions

状态：implemented（实例登记完成；正式入口/OS 事实验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-138（主 owner SP-23；消费者归属 SP-15/SP-32）。

本次唯一用户流程：系统代理页改 bypass 列表→apply→Windows 查询实际
proxy/PAC 值含新列表；同 mode/session/port 二次 apply 去重（内容变仍重下发）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; actual Endpoint/PlatformReceipt/ownership ledger; 受控OS PC`。

对应 ID：FLD-CFG-138；leaf；platform_scope=all；original_type=string；
original_default=`localhost;127.*;10.*;...;192.168.*`（Windows）/`localhost,127.0.0.0/8,::1`（Linux）。
关联：FLD-CFG-139/140/141（同组去重键）、SD-05。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: SystemProxyItem.SystemProxyExceptions`；
只读核对原版 bypass 语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法分号/逗号列表/空（回默认）/
超长（截断并诊断）；持久化 save；生效 immediate
（`settings_timing.rs:266-270`）。fake/失败注入保留旧值；平台写入仅授权隔离机。

允许修改的模块（SP-15/SP-32）：`platform_controller.dart:182-310`
（apply 装配）、`crates/application/src/platform_service.rs`（OS 下发）、
去重键（含 PAC 实际 hash，归属 SP-00 核定）。本卡未改生产代码。

禁止改变的已有行为：冻结去重语义在修键前不动；未知代理键保留；
10808 禁占，宿主代理禁改。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:699,716`（默认见 `:133` 平台默认；`:1108-1110` 空回默认）。
- DTO：`bridge_api` SystemProxy DTO 段 + FRB wire（`frb_generated.dart:8546,12474,15715`）。
- 调用：`platform_controller` apply 路径（`:182-310`）；OS consumer 存在
  （SP-28 矩阵：`platform_service.rs`，native OS 结果本卡未验证）。
- 缺口：OS consumer 存在但同步去重键忽略 bypass 内容变化——同组键仅
  mode/session/port，改列表不重下发（CSV current_gap）。

测试夹具和原版预期：合成 bypass 列表；正向 改列表→apply→隔离机查询值一致；
负向 同键二次 apply 去重验证、失败注入保留旧值。

本次必须通过的命令/真实场景（SP-15/SP-32，未运行）：正式代理页改列表→
apply/retry→隔离 Windows 查询实际值→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-138.md`。

完成条件：内容变化触发重下发 + OS 事实一致 + 重开；仅落盘不算。

发现接口缺口时的处理：去重键缺口已登记；归属 SP-15/SP-32。
