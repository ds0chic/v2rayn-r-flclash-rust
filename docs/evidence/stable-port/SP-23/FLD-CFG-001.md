# SP-23.FLD-CFG-001 — IndexId（internal）

状态：implemented（实例登记完成；正式入口/重开/最终消费者实测未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-001（主 owner SP-23；消费者归属 SP-03）。

本次唯一用户流程：节点列表选中节点 B→保存→独立重开→选中仍为 B；
原版 ZIP 导入→IndexId remap→激活 B→原生 ZIP 往返恢复 B（非焦点/非 applied_target_id 冒充）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; 节点详情与协同; SD-17`。

对应 ID：FLD-CFG-001；internal；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-002（SubIndexId 组过滤）、SD-03/SD-04/06/17。

必读上游文件、符号和固定 commit：`Config.cs :: Config.IndexId`；
只读核对 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/ViewModels/`
`MainWindowViewModel.cs:364-376`（切换/reload）、`ProfilesViewModel.cs:374,423`
（选中与 `IsActive = t.IndexId == _config.IndexId`）、`StatusBarViewModel.cs:295-301`。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法非空 ID / 空（回默认身份）/
未知 ID（拒绝激活，不静默切首节点）；调用方 payload 不携带身份（`engine.rs:1922-1984`
显式 IndexId 组写才校验并同步 mirror）。持久化 save；生效 save（冻结台账来源，
`settings_timing.rs:64`）。stale revision 拒绝；未知结果先 query，不重放。

允许修改的模块（SP-03）：`crates/application/src/engine.rs`
（`set_active:1222`、`canonical_active_from_config:442,630`、
`save_settings_group` IndexId 分支 `:1995-2099`、`set_default:1256`）。
本卡未改生产代码。

禁止改变的已有行为：ZIP 激活语义用 IndexId（非 active_index_id 元概念混淆）；
未知 ID 不自动选中；其它身份/分组行为不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：settings tree（`engine.rs:1927 save_settings` / `:1985 save_settings_group` /
  `:3026-3038` 提交前 canonical 重 pin）。
- 镜像：engine mirror `active_index_id`（`:90,1256-1356`，SP-03 愈合预 SP-03 写漂移）。
- DTO：`bridge_api` contract `index_id/sub_index_id`（`frb_generated.rs:9104,9143-9144,13071-13072`）；
  调用 `engine.rs:687 set_active_profile` / `:708 get_profile`。
- 缺口：`active_index_id` 与 canonical IndexId 分歧愈合路径缺正式 UI→重开证据（CSV current_gap）。

测试夹具和原版预期：合成两节点 A/B；正向 选 B→落盘 JSON/SQLite 一致→重开仍 B；
ZIP 导入 remap→激活→往返；负向 未知 ID 激活拒绝、空 ID 回默认。

本次必须通过的命令/真实场景（SP-03，未运行）：正式节点列表选 B→FRB→保存→
独立重开→仍 B；原版 ZIP→native ZIP 往返。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-001.md`。

完成条件：保存、重开、激活三处一致 + ZIP remap 往返；仅函数存在不算。

发现接口缺口时的处理：分歧愈合缺证据已登记；归属 SP-03，不私定模块。
