# SP-23.FLD-CFG-117 — UiItem.MainColumnItem[].Name

状态：implemented（实例登记完成；正式入口/表格往返验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-117（主 owner SP-23；消费者归属 SP-16）。

本次唯一用户流程：节点表格改列（含未知列名）→保存→重开→内部列快照往返一致、
未知列保留；正式表格不断言该 canonical 生效（UI 用独立缓存）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原型表/独立列宽重排者；单测legacy迁移`。

对应 ID：FLD-CFG-117；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-118/119（同行宽/序）、SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ColumnItem.Name`；
只读核对原版列定义语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法列名/未知列名（保留并回写，
不丢）/空（拒绝）；持久化 save；生效 save（`settings_timing.rs:50`）。
列结构只做存储/迁移闭环，不当开关。

允许修改的模块（SP-16）：`profiles_controller.dart:440`（`column_layout` UI 缓存段）、
`ui_state_store.dart:29,33,39`（各表独立布局键）、DTO `bridge_api/src/api/settings.rs:427,450,480`。
本卡未改生产代码。

禁止改变的已有行为：不得把 container 当开关；未知列不得丢弃；
正式表格当前读 UI 缓存的行为不动（改接线属 SP-16 另卡）。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:487,513`（`main_column_item: Vec<ColumnDefinition>`）。
- DTO：`bridge_api/src/api/settings.rs:427,450,480`（`ColumnItemDto`）+
  FRB wire（`frb_generated.rs:9889,13874,16747`）。
- UI 现实：`profiles_controller` 用 `column_layout` 独立缓存，
  canonical MainColumnItem 无 UI 消费者（SP-28 矩阵同口径）。
- 缺口：canonical 存了但实际表格不用；未知列回写路径未验证。

测试夹具和原版预期：合成列集（含未知列）；正向 改名/增未知列→落盘→重开往返一致；
负向 空名拒绝、坏类型拒绝。

本次必须通过的命令/真实场景（SP-16，未运行）：正式表格调列→FRB→保存→
独立重开→内部快照 diff 一致。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-117.md`。

完成条件：存储/未知键/迁移闭环 + 重开；表格视觉生效属 SP-16 另验。

发现接口缺口时的处理：UI 消费者悬空已登记；归属 SP-16。
