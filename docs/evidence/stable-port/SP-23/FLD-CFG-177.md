# SP-23.FLD-CFG-177 — HappyEyeballs4RayItem.TryDelayMs

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-177（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：DNS 窗开 Happy Eyeballs 且设 TryDelayMs=300→一次保存→
restart_core→xray 出站 `happyEyeballs.tryDelayMs=300`；再关总开关→同参数不再输出。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（均未 verified，跨领域前置）。
CSV dependencies：`SD-07 DNS策略/IPv6; 冻结V2rayDnsService enabled/skip分支`。

对应 ID：FLD-CFG-177；leaf；platform_scope=all；original_type=int?；original_default=250。
关联：FLD-CFG-176（门控），SD-08。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: HappyEyeballs4RayItem.TryDelayMs`；
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `7466bee`（全 SHA 见 README）。

输入、输出、错误、取消、权限、持久化及生效语义：合法非负整数毫秒/null（null 用默认）；
UI 草稿 `option_setting_window.dart:1062-1063`、`settings_defaults.dart:237`。
SP-02 提交语义同 176（expectedRevision/datasetEpoch/mutationId）。
持久化 save；生效 restart_core。取消/错误语义同 176。

允许修改的模块（SP-24）：`config_codegen/src/xray/dns.rs:374-381`（门控+参数发射），
`application/src/codegen.rs:399`（参数透传已存在，核对 enable 上下文）。

禁止改变的已有行为：同 176；参数默认值（250）与原版语义。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:877-890`（`HappyEyeballs4RayItem`，`try_delay_ms`）；
  透传：`application/src/codegen.rs:399`；单测透传 `codegen.rs:1147-1157`（参数到达 codegen 层，
  非发射门控证明）。
- DTO：`bridge_api/src/api/settings.rs:1062-1083`（`HappyEyeballs4RayItemDto`）。
- 发射：`config_codegen/src/xray/dns.rs:377 put_opt_i32(tryDelayMs)`，受 G-01 同一缺失门控影响；
  现有 `config_codegen/tests/xray_routing_dns.rs:247-309` 只证开路径。

测试夹具和原版预期：合成 dualstack；正向 on+300（含块且值为 300）/ off+默认（无块）；
负向 off+300（修后应无块）、非法类型/负值按冻结处理。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`；
正式窗→FRB→保存→restart_core→JSON diff→xray 校验→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-177.md`。

完成条件：开关×参数真值表全绿 + xray 校验 + 重开；只 round-trip 不算。

发现接口缺口时的处理：随 G-01 在 SP-24 统一修门控；本 ID 参数语义本身有透传+单测，
缺的是门控与端到端。
