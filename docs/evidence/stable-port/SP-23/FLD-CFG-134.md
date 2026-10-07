# SP-23.FLD-CFG-134 — ClashUIItem.ConnectionsAutoRefresh

状态：implemented（实例登记完成；实际 core API 查询/保存失败回滚验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-134（主 owner SP-23；消费者归属 SP-17 方向）。

本次唯一用户流程：ClashUI 连接页改自动刷新开关
（ClashUIItem.ConnectionsAutoRefresh，bool，默认 false）→即时生效→
实际 core API 查询/排序/可见页刷新，保存失败可见且回滚用户选择。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-04; native monitor 端点来自实际 session; timer lifecycle`。

对应 ID：FLD-CFG-134；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-135（同 connections 组，本批已登记）、131/132/133
（同 ClashUI 组，已有证据），SD-13/SD-14。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ClashUIItem.ConnectionsAutoRefresh`；
只读核对原版自动刷新语义。`work/` 仅只读核对
（`work/research-v2rayn/source-7.25.4.zip` 在位、未解压未改）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `36e7472`（全 `36e7472b3b75936d2e9297c53755ce7f7e803abf`）。
审计基线 `a7aa0a5b1ecd523bc60d201e0696ff6051cddf26`。

输入、输出、错误、取消、权限、持久化及生效语义：true/false；
保存失败→可见报错且回滚（当前缺口：saveGroup 前本地已变）。
生效 immediate（`settings_timing.rs:21-25`）。隐藏暂停/取消、core 切换
走 timer lifecycle。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-17 方向）：`crates/bridge_api/src/api/settings.rs`
（`:677` DTO auto_refresh、`:691/711` 往返）、
`crates/domain/src/settings.rs`（`:667` 存储、`:684` 默认 false）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 false；未知键保留；
stale revision 拒绝；其它 ClashUI 项不动。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain` ClashUIItem.connections_auto_refresh（`settings.rs:667/684`）。
- DTO：`ClashUiItemDto.connections_auto_refresh`
  （`api/settings.rs:677/691/711` 往返在位）+ FRB wire
  （`frb_generated.rs:6668/11171/14793` 编解码在位）。
- 单测/调用：Dart 侧 connections 视图消费本批未定位，不断言；
  现有证据仅到 DTO/wire 层。
- 缺口：saveGroup 前本地已变、持久化失败不展示/不回滚
  （CSV current_gap）；实际 core API 查询/排序/可见页验收未跑。

测试夹具和原版预期：合成 true/false；正向开→实际查询频率变化；
负向保存失败→可见报错且回滚（待验）。

本次必须通过的命令/真实场景（SP-17 方向，未运行）：正式入口→真实
Rust 提交→实际 Clash API 数据/顺序/请求频率→隐藏暂停/取消→core 切换→
保存失败可见回滚→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-134.md`。

完成条件：开关、实际查询行为和失败回滚三者一致；仅 UI 值变化不算。

发现接口缺口时的处理：保存/回滚缺口已登记；归属 SP-17 方向。
