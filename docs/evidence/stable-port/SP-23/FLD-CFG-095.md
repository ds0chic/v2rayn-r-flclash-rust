# SP-23.FLD-CFG-095 — TunModeItem.EnableTun

状态：implemented（实例登记完成；正式入口/真实 TUN 会话验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-095（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗开 EnableTun→保存→同 revision 生成 runtime plan→core 校验通过→
真实 TUN 会话建立/路由接管；关→plan 不含 TUN 段→停止清理；只改 bool 不算生效验证。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`主方案TUN实际session/route lease/退出清理; SD-04/05; 隔离VM真实验收`。

对应 ID：FLD-CFG-095；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-096/097/098/099/101/104（同 TUN 组）、SD-07、SD-05/18。

必读上游文件、符号和固定 commit：`ConfigItems.cs:143 :: TunModeItem.EnableTun`；
只读核对原版 TUN 总开关语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
开→plan 含 TUN 段并走 helper 真实会话（失败 retry、停止清理）；关→不建会话。
持久化 save；生效 restart_core（`settings_timing.rs:278`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。界面显示的是期望值（desired），非运行 fact。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:434,488`（投影）、
`crates/application/src/tun_plan.rs:222,248`（plan 门控）、
`crates/application/src/engine.rs:4674,4776,4797,5077`（会话/网关条件）。
本卡未改生产代码。

禁止改变的已有行为：未知 TUN 键保留；关闭不删用户 TUN 参数；非 TUN 路径不动；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:268`（默认 false，`:296`）。
- DTO：`bridge_api/src/api/settings.rs:103,120,139` + FRB wire（`:9793,13805,16678`）；
  Dart 侧 `api/settings.dart:1187` + wire。
- 正式入口：`option_setting_window.dart:1202`（TUN tab `:1196`）。
- 投影/plan：`codegen.rs:434` + `:488 tun_enabled`；`tun_plan.rs:222,248` 关即无 spec。
- 单测：`codegen.rs:1037 r4_13_s22_tun_fields_reach_generated_config`；
  `tun_plan.rs:473 disabled_tun_builds_nothing`；
  `engine.rs:6359 first_tun_without_interface_builds_a_deferred_plan`、
  `:7014 sp24_g06_mihomo_native_plan_tun_gate_off_skips_tun_section`。
- 缺口：真实 TUN 会话/启停/OS 效果未验收（CSV current_gap；TUN 总体有已确认缺陷见 runtime 报告）。

测试夹具和原版预期：合成 TUN 修订；正向 开→plan 含 TUN 段→core 校验；
负向 关→plan 无 TUN 段；并发 stale 按冻结语义拒绝。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s22_tun_fields_reach_generated_config`、`disabled_tun_builds_nothing`）；
正式 TUN 窗→FRB→保存→同 revision plan→core 校验/真实会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-095.md`。

完成条件：plan→core→真实会话闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实会话缺口已登记；归属 SP-24。
