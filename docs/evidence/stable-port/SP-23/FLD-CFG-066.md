# SP-23.FLD-CFG-066 — MsgUIItem.AutoRefresh

状态：implemented（实例登记完成；正式入口/重开/最终消费者实测未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-066（主 owner SP-23；消费者归属 SP-17）。

本次唯一用户流程：日志页关闭自动刷新→保存→重开→初始即冻结态（仍可手动滚）；
打开→实时跟随；运行中切换不丢已收日志。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 日志输出与正式语义对照表`。

对应 ID：FLD-CFG-066；leaf；platform_scope=all；original_type=bool?；original_default=true。
关联：FLD-CFG-065（同页过滤）、SD-13。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: MsgUIItem.AutoRefresh`；
只读核对原版日志自动刷新/暂停语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false/null（null 回默认开）；
持久化 save；生效 immediate（`settings_timing.rs:172`）。
取消提交前不写；保存失败保持旧刷新态并反馈。

允许修改的模块（SP-17）：`logs_view.dart:103-115`（本地 `autoRefresh` 展示态）、
`bridge_api/src/api/settings.rs:313,321,331`（DTO）。本卡未改生产代码。

禁止改变的已有行为：关闭刷新 ≠ 停止收集（Rust 侧继续）；滚动/剪贴板行为不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:449`（`auto_refresh: Option<bool>`）。
- DTO：`bridge_api/src/api/settings.rs:313,321,331` + FRB wire
  （`frb_generated.rs:7923,12005,15474`）。
- UI：`logs_view.dart:109-110` 本地开关（`state.autoRefresh`，非 canonical 播种）。
- 缺口：本地 auto-refresh 与 canonical 未接通（CSV current_gap）。

测试夹具和原版预期：合成日志流；正向 关→重开初始冻结、开→实时跟随；
负向 保存失败→旧态保持+可见回滚。

本次必须通过的命令/真实场景（SP-17，未运行）：正式日志页切换→FRB→保存→
独立重开→初始态一致。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-066.md`。

完成条件：canonical→UI 初始播种 + 重开一致；仅本地开关变化不算。

发现接口缺口时的处理：播种缺失已登记；归属 SP-17。
