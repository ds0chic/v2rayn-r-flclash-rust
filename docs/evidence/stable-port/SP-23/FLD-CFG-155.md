# SP-23.FLD-CFG-155 — Fragment4RayItem.Interval（legacy 迁移源）

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-155（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：含旧 `Interval` 值的配置/备份导入→迁移填充 `Delays`→一次保存→
restart_core→wire delays 生效→重开新字段保留（旧键按冻结处理，不造新开关）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies 同 150；apply_timing=next_launch（冻结台账）。

对应 ID：FLD-CFG-155；leaf（legacy 内部迁移项）；platform_scope=all；
original_type=string；original_default=null。关联：152（目标），SD-09。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Fragment4RayItem.Interval`；
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `7466bee`。
原版无独立控件，不得造控件。

输入、输出、错误、取消、权限、持久化及生效语义：同 154（旧值→Delays 迁移；
仅缺省填充；save；next_launch）。

允许修改的模块（SP-24）：`domain/src/settings.rs:1114-1120`、备份导入 remap（SP-03 协同）。

禁止改变的已有行为：同 154。

当前 provider / caller / DTO（HEAD 实测）：
- 回填：`domain/src/settings.rs:1114-1120`。
- 本项无独立 UI/DTO（审计行）；`clash_ui_config.dart` 候选引用为字符串巧合，
  不当证据（已在此纠正）。

测试夹具和原版预期：合成旧文档（含 Interval 不含 Delays）→迁移后 Delays 生效；
显式新值优先；备份恢复往返。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p domain --locked`；
旧文档导入→保存→restart_core→wire→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-155.md`。

完成条件：迁移闭环；不替子字段生效、不造开关。

发现接口缺口时的处理：迁移优先级随 SP-24 核对。Fragment 组 6 叶登记完毕。
