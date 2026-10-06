# SP-23.FLD-CFG-099 — TunModeItem.Mtu

状态：implemented（实例登记完成；正式入口/真实设备 MTU 验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-099（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗改 Mtu→保存→同 revision 生成 runtime plan→core TUN 设备真实
MTU 生效；边界/越界值按冻结规则回退或拒绝，错误保留旧 session。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`主方案TUN实际session/route lease/退出清理; SD-04/05; 隔离VM真实验收`。

对应 ID：FLD-CFG-099；leaf；platform_scope=all；original_type=int；original_default=9000。
关联：FLD-CFG-095（总开关）/096/097/098/101/104（同 TUN 组）、SD-07、SD-05/18。

必读上游文件、符号和固定 commit：`ConfigItems.cs:147 :: TunModeItem.Mtu`；
只读核对原版 MTU 语义（默认 9000）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法正整数；
非正→回退 1280（`tun_plan.rs:194-206`）；超 u16→拒绝（`:196-197`），错误保留旧 session。
持久化 save；生效 restart_core（`settings_timing.rs:282`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:439`（投影）、
`crates/application/src/tun_plan.rs:194-206`（MTU 归一/拒绝）、`services/net_host` helper 生效。
本卡未改生产代码。

禁止改变的已有行为：未知 TUN 键保留；非法 MTU 不写脏 plan；stale revision 拒绝；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:276`（默认 9000，`:300`）。
- DTO：`bridge_api/src/api/settings.rs:107,124,143` + FRB wire；
  Dart 侧 `api/settings.dart` TunModeItemDto + wire。
- 正式入口：`option_setting_window.dart:1227`（`_int(tun,'Mtu')`，TUN tab `:1196`）。
- 投影/plan：`codegen.rs:439 base.tun.mtu`；`tun_plan.rs:194-206`。
- 单测：`codegen.rs:1037 r4_13_s22_tun_fields_reach_generated_config`
 （`:1047 mtu=1400`、`:1055/:1083` 断言）；
  `tun_plan.rs:485 explicit_ipv4_cidr_is_used`（mtu 9000）、
  `:517 nonpositive_mtu_falls_back_to_1280`、`:529 out_of_range_mtu_is_rejected`。
- 缺口：真实设备 MTU/大包 fragment/正常流量、隔离 VM 对照未验收（CSV current_gap）。

测试夹具和原版预期：合成 TUN 修订；正向 1400/9000→plan mtu→core 校验；
负向 0/-5→1280，超 u16→拒绝且旧 session 保留。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s22_tun_fields_reach_generated_config`、`nonpositive_mtu_falls_back_to_1280`、
`out_of_range_mtu_is_rejected`）；
正式 TUN 窗→FRB→保存→同 revision plan→隔离真实设备/大包→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-099.md`。

完成条件：plan→真实设备 MTU 闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实设备效果缺口已登记；归属 SP-24。
