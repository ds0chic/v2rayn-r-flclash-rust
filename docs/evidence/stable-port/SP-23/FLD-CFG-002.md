# SP-23.FLD-CFG-002 — SubIndexId（internal）

状态：implemented（实例登记完成；正式入口/重开/最终消费者实测未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-002（主 owner SP-23；消费者归属 SP-03）。

本次唯一用户流程：切换订阅组→列表按新组过滤→保存→独立重开→仍在该组；
删除选中组→回退默认组且不指向已删 ID。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; 分组管理/ID remap; SD-17`。

对应 ID：FLD-CFG-002；internal；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-001（IndexId 激活）、SD-03/SD-06/17。

必读上游文件、符号和固定 commit：`Config.cs :: Config.SubIndexId`；
只读核对 `ProfilesViewModel.cs:336`（`_config.SubIndexId = SelectedSub?.Id`）、
`:363,388-400`（按组取列表）、`MainWindowViewModel.cs:188-192`
（`UpdateSubscriptionProcess(_config.SubIndexId)`）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法组 ID / 空（回默认组）/
已删 ID（拒绝并回退，不空列表）；`SubIndexId` 写即组切换本身
（`engine.rs:2011-2013`，其余组写走普通分支）。持久化 save；生效 save
（`settings_timing.rs:72`）。stale 拒绝；保存失败不更新 applied。

允许修改的模块（SP-03）：`crates/application/src/engine.rs`
（`switch_current_group:1401`、`save_settings_group:1985-2013`、
提交重 pin `:3034-3035`）。本卡未改生产代码。

禁止改变的已有行为：SubIndexId 仅做视图过滤/订阅归属，不冒充激活；
已删组不残留选中；导入 remap 语义保留。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：settings tree + 提交快照 `sub_index_id`（`:3035`）。
- DTO：同 001（`frb_generated.rs:9104,9143-9144`）。
- 缺口：选中组当前用另一 UI 状态源；完整组往返未验证（CSV current_gap）。

测试夹具和原版预期：合成两组各两节点；正向 切组→过滤→落盘→重开仍该组；
负向 删选中组→回默认、未知组 ID 拒绝。

本次必须通过的命令/真实场景（SP-03，未运行）：正式分组切换→FRB→保存→
独立重开→过滤与选中一致；删组回退。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-002.md`。

完成条件：切换、过滤、重开三处一致 + 删组回退；仅存储存在不算。

发现接口缺口时的处理：组往返缺证据已登记；归属 SP-03。
