# SP-23.FLD-CFG-179 — HappyEyeballs4RayItem.Interleave

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-179（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：DNS 窗开 Happy Eyeballs 且 Interleave=2→一次保存→restart_core→
`happyEyeballs.interleave=2`；关总开关→同值不再输出。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。CSV dependencies 同 177。

对应 ID：FLD-CFG-179；leaf；platform_scope=all；original_type=int?；original_default=1。
关联：FLD-CFG-176（门控），SD-08。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: HappyEyeballs4RayItem.Interleave`；
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法非负整数/null；
UI 草稿 `option_setting_window.dart:1073-1074`、`settings_defaults.dart:239`。
提交/持久化 save、生效 restart_core、取消/错误语义同 176。

允许修改的模块（SP-24）：`config_codegen/src/xray/dns.rs:379` 及门控；
`application/src/codegen.rs:401` 透传核对。

禁止改变的已有行为：同 176；默认值 1 的冻结语义。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:877-890`（`interleave`）；透传 `codegen.rs:401`。
- DTO：`HappyEyeballs4RayItemDto`（`settings.rs:1062-1083`）。
- 发射：`dns.rs:379 put_opt_i32(interleave)`，G-01 门控缺失同样适用。

测试夹具和原版预期：合成 dualstack；正向 on+2（含块）、off+默认（无块）；
负向 off+2（修后无块）、非法类型按冻结处理。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`；
正式窗→FRB→保存→restart_core→JSON diff→xray 校验→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-179.md`。

完成条件：真值表 + 校验 + 重开；只 round-trip 不算。

发现接口缺口时的处理：随 G-01 修门控。
