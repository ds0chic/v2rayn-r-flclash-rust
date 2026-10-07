# SP-23.FLD-CFG-081 — UiItem.MainColumnItem

状态：implemented（实例登记完成；canonical→真实表列联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-081（主 owner SP-23；消费者归属 SP-28）。

本次唯一用户流程：列设置对话框改主表列（Name/Width/Index）→保存→重开→
profiles 真实表列按 canonical 恢复；未知列保留、缺列回默认。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; UI原版布局/窗口消费者；单次legacy迁移`；
另涉 SD-03/17（profiles）。

对应 ID：FLD-CFG-081；leaf；platform_scope=all；original_type=list<ColumnItem>；
original_default=[]。
关联：FLD-CFG-117/118/119（列 Name/Width/Index 子语义），SD-06。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: UIItem.MainColumnItem`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:104`
（`List<ColumnItem> MainColumnItem`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `176fd39`
（全 `176fd396ac052dc592efc1b55e2ad21ca88477d8`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法列行
（Name/Width/Index）/空（→默认列）；坏类型行→拒绝且旧组不变。
持久化 save；生效 immediate（`domain/src/settings_timing.rs:306`）。
取消提交前不写。平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-28）：`apps/desktop/lib/features/profiles/column_settings_dialog.dart`
（`:6` 列形状契约）、`apps/desktop/lib/features/profiles/ui_state_store.dart`
（`:17/:29-39/:176` `column_layout` 节与 canonical 互转）、
`crates/persistence/src/upstream_config.rs`（`:276/:400` 组读写）。
本卡未改生产代码。

禁止改变的已有行为：原版默认空（→默认列）；未知列保留；其它 UiItem 不动；
`column_layout` 缓存不得静默丢弃 canonical 未知行。

当前 provider / caller / DTO（HEAD 实测，grep 只读）：
- 存储：`domain/src/settings.rs:487`（`Vec<ColumnDefinition>`，默认空 `:513`）+
  `domain/src/entities.rs:296-302`。
- DTO：`bridge_api/src/api/settings.rs:427/:450/:480-481` + FRB wire。
- 调用链：列对话框→`column_layout` 节（`profiles_controller.dart:450`）→
  canonical 组；表实际读 `column_layout` 缓存而非直接读 canonical
  （CSV current_gap：`profile table uses column_layout cache rather than
  canonical MainColumnItem`）。
- 缺口：列改→保存→重开→真实表列（含未知列保留）端到端验收未跑。

测试夹具和原版预期：合成列组；正向 改宽/排序/增删→重开表列一致；
负向 坏行→拒绝、未知列→保留；与 067 自动调宽组合。

本次必须通过的命令/真实场景（SP-28，未运行）：`flutter test`
（profiles/列合同）；补列对话框→保存→重开→真实表列观察。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-081.md`。

完成条件：保存、重开和真实表列三者一致；仅落盘/仅缓存不算。

发现接口缺口时的处理：canonical→表列联动缺口已登记；归属 SP-28。
