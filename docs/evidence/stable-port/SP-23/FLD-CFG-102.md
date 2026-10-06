# SP-23.FLD-CFG-102 — TunModeItem.EnableLegacyProtect

状态：implemented（实例登记完成；正式入口/真实 core 生效验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-102（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗开旧版保护→保存→plan 含保护上下文→真实 core 启动
受保护（版本/平台相关失败可诊断）；关→不保护且旧行为可恢复。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`受控TUN实现session/route lease/退出清理; SD-04/05; 受控VM真实环境`。

对应 ID：FLD-CFG-102；leaf；platform_scope=all；original_type=bool；original_default=true。
关联：SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: TunModeItem.EnableLegacyProtect`；
只读核对原版 legacy protect 语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
持久化 save；生效 restart_core（`settings_timing.rs:273-277`）。
core 可执行路径缺失→结构化失败（不静默无保护运行）。

允许修改的模块（SP-24）：`crates/application/src/engine.rs:4726`
（读 `tun.enable_legacy_protect`）、`tun_plan.rs`（plan 上下文）、
helper session（归属 SP-00 核定）。本卡未改生产代码。

禁止改变的已有行为：默认 true（原版默认）不得反转；非 TUN 计划行为不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:282,303`（默认 true）。
- DTO：TunMode DTO 段（`bridge_api` settings TunModeItem + FRB wire，以生成物为准）。
- 调用：`engine.rs:4726`；单测 `:6210,6335,6358`（内存级）。
- 缺口：保护上下文缺生产 core 可执行路径——投影存在，真实 TUN 会话/
  启停/OS 效果仅 mock（CSV current_gap）。

测试夹具和原版预期：合成 TUN 修订；正向 开/关→plan 上下文有/无→core 行为；
负向 缺 core 路径→结构化失败可诊断。

本次必须通过的命令/真实场景（SP-24，未运行）：正式 TUN 窗→FRB→保存→
plan→真实 core 启停→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-102.md`。

完成条件：plan→真实 core 生效 + 重开；仅内存单测不算。

发现接口缺口时的处理：生产路径缺口已登记；归属 SP-24。
