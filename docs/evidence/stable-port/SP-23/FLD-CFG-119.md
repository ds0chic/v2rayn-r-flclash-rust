# SP-23.FLD-CFG-119 — UiItem.MainColumnItem[].Index

状态：implemented（实例登记完成；正式入口/表格往返验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-119（主 owner SP-23；消费者归属 SP-16）。

本次唯一用户流程：节点表格拖拽重排列序→保存→重开→内部快照顺序一致；
ZIP 恢复/重复 Index→按冻结去重语义处理。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原型表/独立列宽重排者；单测legacy迁移`。

对应 ID：FLD-CFG-119；leaf；platform_scope=all；original_type=int；original_default=0。
关联：FLD-CFG-117/118（同行名/宽）、SD-06/SD-17。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ColumnItem.Index`；
只读核对原版列序语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法序号/重复序号（去重，
语义以冻结为准）/坏类型（拒绝）；持久化 save；生效 save
（`settings_timing.rs:49`）。

允许修改的模块（SP-16）：同 117（`column_layout` 缓存、DTO `ColumnItemDto`）。
本卡未改生产代码。

禁止改变的已有行为：同 117；重排不得丢列。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` `ColumnDefinition` 序字段（与 `:487,513` 同结构）。
- DTO：同 117（`settings.rs:427,450,480` + FRB wire）。
- UI 现实：canonical 序无实际表格消费者（SP-28 矩阵同口径）。
- 缺口：重排→重开→ZIP 恢复链未验证。

测试夹具和原版预期：合成列序；正向 重排→落盘→重开一致；
负向 重复 Index 去重、坏类型拒绝。

本次必须通过的命令/真实场景（SP-16，未运行）：正式表格重排→FRB→保存→
独立重开→快照一致。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-119.md`。

完成条件：存储/迁移闭环 + 重开；视觉生效属 SP-16 另验。

发现接口缺口时的处理：UI 消费者悬空已登记；归属 SP-16。
