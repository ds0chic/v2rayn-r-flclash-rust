# SP-23.FLD-CFG-116 — RoutingBasicItem.RoutingIndexId

状态：implemented（实例登记完成；正式入口/迁移往返验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-116（主 owner SP-23；消费者归属 SP-13）。

本次唯一用户流程：旧配置载入含 legacy RoutingIndexId→迁移为 IsActive 目标→
路由页选中同一路由→保存→重开→load/native/原版 ZIP 往返仍选中该路由。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; SD-10client; routing/resource source schema; 正式codegen context`。

对应 ID：FLD-CFG-116；leaf；platform_scope=all；original_type=string；original_default=null。
关联：SD-15/SD-07/10/17。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: RoutingBasicItem.RoutingIndexId`；
只读核对原版路由选中/IsActive 语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法路由 ID/空（回默认）/
未知 ID（拒绝并回默认，不空选）；迁移仅 legacy→IsActive 单向，不回写旧字段。
持久化 save；生效 save（`settings_timing.rs:190`）。

允许修改的模块（SP-13）：`crates/application/src/codegen.rs:436`（codegen 上下文读取）、
`config_codegen/src/{xray,singbox}/routing.rs`（路由生成）、迁移器（SP-00 核定归属）。
本卡未改生产代码。

禁止改变的已有行为：普通 `set_default` 不得冒充迁移；未知路由键保留；
非路由设置不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:383,393`（`routing_index_id: Option<String>`）。
- DTO：`bridge_api` contract `routing_index_id`（`frb_generated.rs:8802,12693,16097`）。
- 单测：`domain/src/settings.rs:1559`（字段读写级）。
- 缺口：legacy RoutingIndexId→IsActive 迁移缺失，普通 setter 不能替代迁移
  （CSV current_gap；SP-28 矩阵同口径）。

测试夹具和原版预期：合成两路由 + 旧 ID；正向 旧 ID→迁移→选中→往返一致；
负向 未知 ID→回默认并诊断。

本次必须通过的命令/真实场景（SP-13，未运行）：旧配置载入→迁移→路由页→
保存→重开→ZIP 往返。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-116.md`。

完成条件：迁移→选中→往返闭环；仅字段读写单测不算。

发现接口缺口时的处理：迁移缺失已登记；归属 SP-13。
