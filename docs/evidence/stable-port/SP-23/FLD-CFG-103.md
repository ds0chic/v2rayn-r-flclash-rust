# SP-23.FLD-CFG-103 — TunModeItem.RouteExcludeAddress

状态：implemented（实例登记完成；正式入口/真实路由验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-103（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗填合法 CIDR 排除列表→保存→plan/实际 core 排除该段
（真实目标可达性验证）；非法 CIDR→拒绝；尾逗号按冻结语义处理。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`受控TUN实现session/route lease/退出清理; SD-04/05; 受控VM真实环境`。

对应 ID：FLD-CFG-103；leaf；platform_scope=all；original_type=list<string>；original_default=null。
关联：SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: TunModeItem.RouteExcludeAddress`；
只读核对原版排除段（含尾逗号接受语义）解析。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 CIDR 列表/空（不排除）/
非法条目（整单拒绝并定位条目）；UI 切分尾空项语义以冻结为准（见缺口）。
持久化 save；生效 restart_core（`settings_timing.rs:283-287`）。

允许修改的模块（SP-24）：`codegen.rs:443`（投影，空→`unwrap_or_default`）、
`tun_plan.rs:208`（plan 合成，`:483-485` 非法拒绝单测）。本卡未改生产代码。

禁止改变的已有行为：未知 TUN 键保留；非法输入不得静默截断生效。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:284,304`（`Option<Vec<String>>`，默认 None）。
- DTO：TunMode DTO 段 + FRB wire（以生成物为准）。
- 缺口：UI 切分保留尾空项，而 CIDR 校验拒绝原版接受的尾逗号——
  切分与校验语义分裂（CSV current_gap）；真实会话仅 mock。

测试夹具和原版预期：合成排除段；正向 `10.0.0.0/8`→plan 含该段（`:1051,1061`）；
负向 `nope` 拒绝、尾逗号按冻结语义、空尾项处理。

本次必须通过的命令/真实场景（SP-24，未运行）：正式 TUN 窗→FRB→保存→
plan→core 校验/真实目标可达→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-103.md`。

完成条件：切分/校验语义统一 + plan→真实路由 + 重开；仅单测不算。

发现接口缺口时的处理：语义分裂已登记；归属 SP-24。
