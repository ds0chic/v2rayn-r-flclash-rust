# SP-23.FLD-CFG-180 — HappyEyeballs4RayItem.MaxConcurrentTry

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-180（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：DNS 窗开 Happy Eyeballs 且 MaxConcurrentTry=3→一次保存→
restart_core→`happyEyeballs.maxConcurrentTry=3`；关总开关→同值不再输出。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。CSV dependencies 同 177。

对应 ID：FLD-CFG-180；leaf；platform_scope=all；original_type=int?；original_default=4。
关联：FLD-CFG-176（门控），SD-08。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: HappyEyeballs4RayItem.MaxConcurrentTry`；
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法非负整数/null；
UI 草稿 `option_setting_window.dart:1078,1080`、`settings_defaults.dart:240`。
提交/持久化 save、生效 restart_core、取消/错误语义同 176。

允许修改的模块（SP-24）：`config_codegen/src/xray/dns.rs:380` 及门控；
`application/src/codegen.rs:402-403` 透传核对。

禁止改变的已有行为：同 176；默认值 4 的冻结语义。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:877-890`（`max_concurrent_try`）；透传 `codegen.rs:402-403`。
- DTO：`HappyEyeballs4RayItemDto`（`settings.rs:1062-1083`）。
- 发射：`dns.rs:380 put_opt_i32(maxConcurrentTry)`，G-01 门控缺失同样适用。

测试夹具和原版预期：合成 dualstack；正向 on+3（含块）、off+默认（无块）；
负向 off+3（修后无块）、非法类型按冻结处理。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`；
正式窗→FRB→保存→restart_core→JSON diff→xray 校验→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-180.md`。

完成条件：真值表 + 校验 + 重开；只 round-trip 不算。

发现接口缺口时的处理：随 G-01 修门控。Happy 组 5 叶登记完毕。
