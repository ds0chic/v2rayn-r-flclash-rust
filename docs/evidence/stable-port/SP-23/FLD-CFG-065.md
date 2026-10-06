# SP-23.FLD-CFG-065 — MsgUIItem.MainMsgFilter

状态：implemented（实例登记完成；正式入口/重开/最终消费者实测未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-065（主 owner SP-23；消费者归属 SP-17）。

本次唯一用户流程：日志页输入关键字过滤→保存→重开→日志页初始过滤即该值；
非法 regex→可诊断拒绝且旧过滤保留。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 日志输出与正式语义对照表`。

对应 ID：FLD-CFG-065；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-066（AutoRefresh 同页）、SD-13。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: MsgUIItem.MainMsgFilter`；
只读核对 `MsgViewModel` 过滤显示语义（冻结源码为准）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法字符串/空（不过滤）/
非法 regex（拒绝并诊断）；持久化 save；生效 immediate
（`settings_timing.rs:173`）。取消提交前不写；保存失败不更新已应用过滤。

允许修改的模块（SP-17）：`apps/desktop/lib/features/monitor/logs_view.dart`
（当前仅本地展示态 `:13,103-115`，`mainMsgFilter` 未从 canonical 播种）、
`bridge_api/src/api/settings.rs:312,320,330`（DTO）。
本卡未改生产代码。

禁止改变的已有行为：暂停/滚动/复制语义不动；过滤仅影响展示，不丢已收集日志。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:447`（`main_msg_filter: Option<String>`）。
- DTO：`bridge_api/src/api/settings.rs:312,320,330` + FRB wire
 （`frb_generated.rs:7922,12004,15473`）。
- 缺口：本地 keyword 未从 canonical 播种/持久化（CSV current_gap）——
  canonical 有值但日志页初始仍空。

测试夹具和原版预期：合成日志流；正向 设 `error`→重开初始即过滤、
清空→不过滤；负向 非法 regex 拒绝且旧值保留。

本次必须通过的命令/真实场景（SP-17，未运行）：正式日志页设过滤→FRB→保存→
独立重开→初始过滤一致 + 实时/暂停一致。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-065.md`。

完成条件：canonical→UI 初始播种 + 重开一致；仅 DTO 落盘不算。

发现接口缺口时的处理：播种缺失已登记；归属 SP-17。
