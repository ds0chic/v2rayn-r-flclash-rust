# SP-23.FLD-CFG-154 — Fragment4RayItem.Length（legacy 迁移源）

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-154（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：含旧 `Length` 值的配置/备份导入→迁移填充 `Lengths`→一次保存→
restart_core→wire lengths 生效→重开新字段保留（旧键按冻结处理，不造新开关）。

前置任务及已验证证据：SP-00；SP-01/02（迁移备份语义）、SP-12（未 verified）。
CSV dependencies 同 150；apply_timing=next_launch（冻结台账）。

对应 ID：FLD-CFG-154；leaf（legacy 内部迁移项）；platform_scope=all；
original_type=string；original_default=null。关联：151（目标），SD-09。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Fragment4RayItem.Length`；
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `7466bee`。
原版无独立 OptionSettingWindow 控件（审计行已注明），不得造控件。

输入、输出、错误、取消、权限、持久化及生效语义：输入为旧文档/备份中的遗留值；
`domain/src/settings.rs:1114-1120` 回填语义（SP-24 核对：仅缺省时填，不覆盖显式值）。
提交/持久化 save；生效 next_launch（冻结）。取消/错误语义同 150。

允许修改的模块（SP-24）：`domain/src/settings.rs:1114-1120`（迁移优先级核对）、
备份/ZIP 导入 remap（SP-03 协同）。

禁止改变的已有行为：不把 legacy 项变成新可见开关；不覆盖用户显式新值；
未知键保留。

当前 provider / caller / DTO（HEAD 实测）：
- 回填：`domain/src/settings.rs:1114-1120`（`MaxSplit/Lengths/Delays` 缺省填充，
  含 legacy 来源注释行）。
- 本项无独立 UI/DTO（审计：`None; no original dedicated control`），
  typed setting 经 config/相关窗保留与编辑。

测试夹具和原版预期：合成旧文档（含 Length 不含 Lengths）→迁移后 Lengths 生效；
显式新值优先；坏旧值冻结处理；备份恢复往返（SP-03 协同）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p domain --locked`（迁移用例）；
旧文档导入→保存→restart_core→wire→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-154.md`。

完成条件：迁移闭环（旧→新→wire→重开）；容器式“结构保留”不能替本叶生效。

发现接口缺口时的处理：迁移优先级疑问随 SP-24 核对，不自造覆盖语义。
