# SP-23.FLD-CFG-100 — TunModeItem.EnableIPv6Address

状态：implemented（实例登记完成；正式入口/真实会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-100（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗开 IPv6 并填合法 IPv6 地址→保存→同 revision 生成
runtime plan→core 校验通过→真实会话无 IPv6 泄漏/误配；关→plan 不含 IPv6 段。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`受控TUN实现session/route lease/退出清理; SD-04/05; 受控VM真实环境`。

对应 ID：FLD-CFG-100；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-105（IPv6Address 地址值）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: TunModeItem.EnableIPv6Address`；
只读核对原版 TUN IPv6 地址/路由语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `393fafd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
需与 105 联动（开但无地址→诊断失败，不静默纯 IPv4）。持久化 save；
生效 restart_core（`settings_timing.rs:272`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:441`
（投影）、`crates/application/src/tun_plan.rs:185-208`（plan 合成）、
`services/net_host/src/session.rs`（helper 生效）。本卡未改生产代码。

禁止改变的已有行为：未知 TUN 键保留；关闭不删用户地址值；IPv4 路径不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:278,301`（默认 false）。
- DTO：`bridge_api/src/api/settings.rs:108,125,144` + FRB wire（`:9798,13810,16683`）。
- 投影：`codegen.rs:441`；plan：`tun_plan.rs:185`；单测 `:1049-1064`（投影级）。
- 缺口：生产全局 IPv6 上下文恒 false——投影/plan 存在，真实 TUN 会话/启停/
  OS 效果仅 mock（CSV current_gap）。

测试夹具和原版预期：合成 TUN 修订；正向 开+地址→plan 含 IPv6 段→core 校验；
负向 开无地址/非法 CIDR→拒绝、关→不输出。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`；
正式 TUN 窗→FRB→保存→同 revision plan→core 校验/真实会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-100.md`。

完成条件：plan→core→真实会话闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实会话缺口已登记；归属 SP-24。
