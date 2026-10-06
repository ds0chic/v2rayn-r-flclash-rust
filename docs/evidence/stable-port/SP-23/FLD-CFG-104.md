# SP-23.FLD-CFG-104 — TunModeItem.IPv4Address

状态：implemented（实例登记完成；正式入口/真实设备地址验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-104（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗填 IPv4Address 合法 CIDR→保存→同 revision 生成 runtime plan→
设备地址/路由生效；冲突/空/错误输入→拒绝或回退上游默认；隔离验证实际设备/监听/流量与清理。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`主方案TUN实际session/route lease/退出清理; SD-04/05; 隔离VM真实验收`。

对应 ID：FLD-CFG-104；leaf；platform_scope=all；original_type=string；
original_default="172.18.0.1/30"。
关联：FLD-CFG-095（总开关）/100（IPv6 开关）/105（IPv6 地址）、SD-07、SD-05/18。

必读上游文件、符号和固定 commit：`ConfigItems.cs:152 :: TunModeItem.IPv4Address`；
只读核对原版 TUN IPv4 地址/路由语义（默认 172.18.0.1/30）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 CIDR（如 172.18.0.1/30）；
裸 IP（无前缀）→INVALID_ARGUMENT（field_path=IPv4Address）；None→回退上游默认。
持久化 save；生效 restart_core（`settings_timing.rs:279`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:440`（投影）、
`crates/application/src/tun_plan.rs:180`（地址解析/回退）、`services/net_host` helper 生效。
本卡未改生产代码。

禁止改变的已有行为：未知 TUN 键保留；非法地址不写脏 plan；与 100/105 联动
（开 IPv6 但无地址→诊断失败，不静默纯 IPv4）；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:286`（`Option<String>`，默认 None，`:305`；
  缺省由 plan 回退上游默认）。
- DTO：`bridge_api/src/api/settings.rs:112,129,148` + FRB wire；
  Dart 侧 `api/settings.dart:1196` + wire。
- 正式入口：`option_setting_window.dart:1264`（`_str(tun,'IPv4Address')`，TUN tab `:1196`）。
- 投影/plan：`codegen.rs:440`；`tun_plan.rs:180` 地址解析。
- 单测：`codegen.rs:1037 r4_13_s22_tun_fields_reach_generated_config`
 （`:1048` 地址断言）；`tun_plan.rs:480 explicit_ipv4_cidr_is_used`、
  `:490 missing_ipv4_falls_back_to_upstream_default`、
  `:499 bare_ip_is_rejected_with_field`。
- 缺口：隔离真实设备/监听/流量与清理未验收（CSV current_gap）。

测试夹具和原版预期：合成 TUN 修订；正向 合法 CIDR→plan 地址/前缀→core 校验；
负向 裸 IP→拒绝并定位字段，None→上游默认。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s22_tun_fields_reach_generated_config`、`explicit_ipv4_cidr_is_used`、
`missing_ipv4_falls_back_to_upstream_default`、`bare_ip_is_rejected_with_field`）；
正式 TUN 窗→FRB→保存→同 revision plan→隔离真实设备→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-104.md`。

完成条件：plan→真实设备地址闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实设备效果缺口已登记；归属 SP-24。
