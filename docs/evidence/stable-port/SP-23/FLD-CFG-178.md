# SP-23.FLD-CFG-178 — HappyEyeballs4RayItem.PrioritizeIPv6

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-178（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：DNS 窗开 Happy Eyeballs 且 PrioritizeIPv6=true→一次保存→
restart_core→`happyEyeballs.prioritizeIPv6=true` 落配置；关总开关→同值不再输出。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。CSV dependencies 同 177。

对应 ID：FLD-CFG-178；leaf；platform_scope=all；original_type=bool?；original_default=false。
关联：FLD-CFG-176（门控）、IPv6 策略组（Strategy4*），SD-08。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: HappyEyeballs4RayItem.PrioritizeIPv6`；
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false/null；
UI 草稿 `option_setting_window.dart:1067,1069`、`settings_defaults.dart:238`。
提交/持久化 save、生效 restart_core、取消/错误语义同 176。

允许修改的模块（SP-24）：`config_codegen/src/xray/dns.rs:378` 及门控；
`application/src/codegen.rs:400` 透传核对。

禁止改变的已有行为：同 176；不得把 IPv6 策略（Strategy4*）与本开关语义混同。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:877-890`（`prioritize_ipv6`）；透传 `codegen.rs:400`。
- DTO：`HappyEyeballs4RayItemDto`（`settings.rs:1062-1083`）。
- 发射：`dns.rs:378 put_opt_bool(prioritizeIPv6)`，G-01 门控缺失同样适用。

测试夹具和原版预期：合成 dualstack（含 IPv6 可达/不可达两形）；
正向 on+true（含块）、off+默认（无块）；负向 off+true（修后无块）；
与 Strategy4Proxy/IPv6 组合的实际解析行为对照（SP-24 做，不在此冒充）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`；
正式窗→FRB→保存→restart_core→JSON diff→xray 校验→重开；dualstack 合成行为观察。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-178.md`。

完成条件：真值表 + 校验 + 重开；只 round-trip 不算。

发现接口缺口时的处理：随 G-01 修门控；IPv6 实际能力诊断缺失另由 SP-24 登记，不削范围。
