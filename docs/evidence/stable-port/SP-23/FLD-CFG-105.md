# SP-23.FLD-CFG-105 — TunModeItem.IPv6Address

状态：implemented（实例登记完成；正式入口/真实设备路由验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-105（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗开 IPv6 并填合法 IPv6 CIDR（如 `fc00::172:18:0:1/126`）→
保存→plan 含该设备地址/路由→真实 core 校验→重开保持；非法/冲突值拒绝。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`受控TUN实现session/route lease/退出清理; SD-04/05; 受控VM真实环境`。

对应 ID：FLD-CFG-105；leaf；platform_scope=all；original_type=string；original_default=`fc00::172:18:0:1/126`。
关联：FLD-CFG-100（开关）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: TunModeItem.IPv6Address`；
只读核对原版默认地址与设备/路由语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 IPv6 CIDR/空（回默认或随开关）/
非法/冲突/跨平台不支持（拒绝并诊断）；开关关时地址值保留但不生效。
持久化 save；生效 restart_core（`settings_timing.rs:280`）。

允许修改的模块（SP-24）：`codegen.rs:442`（投影）、`tun_plan.rs:187`
（plan 地址合成；单测 `:462-475` 开/关/无地址三形）。本卡未改生产代码。

禁止改变的已有行为：原版默认地址不得改；关开关不删地址值；IPv4 地址不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:288,306`（`Option<String>`，默认 None；原版默认见 CSV）。
- DTO：`bridge_api/src/api/settings.rs:113,130,149` + FRB wire（`:9803,13815,16688`）。
- 缺口：生产全局 IPv6 上下文恒 false——plan/校验存在，真实设备/路由/
  启停/OS 效果仅 mock（CSV current_gap）。

测试夹具和原版预期：合成地址；正向 开+`fd00::1/64`→plan 含地址（`:1050`）、
关+地址→忽略；负向 非法 CIDR/冲突→拒绝。

本次必须通过的命令/真实场景（SP-24，未运行）：正式 TUN 窗→FRB→保存→
同 revision plan→core 校验/真实会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-105.md`。

完成条件：地址→plan→真实设备/路由 + 重开；仅三形单测不算。

发现接口缺口时的处理：真实环境缺口已登记；归属 SP-24。
