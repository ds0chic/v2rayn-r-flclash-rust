# SP-23.FLD-CFG-101 — TunModeItem.IcmpRouting

状态：implemented（实例登记完成；正式入口/真实 ICMP 行为验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-101（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗改 IcmpRouting→保存→同 revision 生成 runtime plan→core 对应版本
ICMP 动作/路由生效；隔离 IPv4/IPv6 合成 ICMP 与 UDP/TCP 互不损坏；版本能力可诊断。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`主方案TUN实际session/route lease/退出清理; SD-04/05; 隔离VM真实验收`。

对应 ID：FLD-CFG-101；leaf；platform_scope=all；original_type=string；original_default="rule"。
关联：FLD-CFG-095（总开关）/096/097/098/099/104（同 TUN 组）、SD-07、SD-05/18。

必读上游文件、符号和固定 commit：`ConfigItems.cs:149 :: TunModeItem.IcmpRouting`；
只读核对原版 ICMP 路由语义（默认 "rule"）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 冻结枚举值/null（null=缺省 "rule"）；
未知值按冻结处理，不静默映射。持久化 save；生效 restart_core（`settings_timing.rs:281`）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:447`（投影）、
`crates/application/src/tun_plan.rs`（ICMP 进 plan）、`services/net_host` helper 生效。
本卡未改生产代码。

禁止改变的已有行为：未知 TUN 键保留；未知值不静默映射；UDP/TCP 路径不受 ICMP 改动损坏；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:280`（默认 `Some(TUN_ICMP_ROUTING)`=`"rule"`，`:126,:302`）。
- DTO：`bridge_api/src/api/settings.rs:109,126,145` + FRB wire；
  Dart 侧 `api/settings.dart:1193` + wire。
- 正式入口：`option_setting_window.dart:1232`（`_str(tun,'IcmpRouting')`，TUN tab `:1196`）。
- 投影：`codegen.rs:447 base.tun.icmp_routing`。
- 单测：`codegen.rs:1037 r4_13_s22_tun_fields_reach_generated_config`
 （`:1046 icmp="direct"`、`:1059` 断言）。
- 缺口：隔离 IPv4/IPv6 合成 ICMP、版本能力诊断未验收（CSV current_gap）。

测试夹具和原版预期：合成 TUN 修订；正向 direct/rule 等有效值→plan→core 校验；
负向 未知值按冻结处理；null→缺省 "rule"。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s22_tun_fields_reach_generated_config`）；
正式 TUN 窗→FRB→保存→同 revision plan→隔离合成 ICMP→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-101.md`。

完成条件：plan→真实 ICMP 行为闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实 ICMP 效果缺口已登记；归属 SP-24。
