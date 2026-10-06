# SP-23.FLD-CFG-151 — Fragment4RayItem.Lengths

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-151（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：`Lengths=50-100`→一次保存→restart_core→wire `lengths=["50-100"]`
且 `length=50-100`（首值）→xray 校验通过→重开往返。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。CSV dependencies 同 150。

对应 ID：FLD-CFG-151；leaf；platform_scope=all；original_type=list<string>；
original_default=["50-100"]。关联：032/033、150/152/153，SD-09。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Fragment4RayItem.Lengths`；
`V2rayOutboundService.cs:831-843`；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 `"a-b"` 列表/null/空；
UI 草稿 `option_setting_window.dart:353,741,745`、`settings_defaults.dart:193,293,296`。
提交/持久化 save、生效 restart_core、取消/错误语义同 150。

允许修改的模块（SP-24）：`config_codegen/src/xray/config.rs:248-252`、
`application/src/codegen.rs:443` 组。

禁止改变的已有行为：同 150；列表顺序冻结语义；legacy Length（154）迁移源地位。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` `Fragment4RayItem.lengths`（legacy 回填 `:1114-1120`）。
- 透传：`application/src/codegen.rs:443`；DTO 同 150（同结构）。
- 发射：`config.rs:248-252`（空回 `["10-20"]`？注：该默认属 delays 行语义，
  SP-24 须按原版核对 lengths 空回退，不沿用 delays 值——此处仅登记疑问，不下结论）。

测试夹具和原版预期：合成出站；正向单段 `50-100`/多段两组；负向空列表（冻结回退）/
坏格式（冻结处理）；legacy Length 共存时优先级按冻结（154 侧验收）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`；
正式窗→FRB→保存→restart_core→JSON diff→xray 校验→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-151.md`。

完成条件：正向×2 + 负向 + 校验 + 重开；只 round-trip 不算。

发现接口缺口时的处理：空回退默认值须 SP-24 核原版（疑问已登记，不自造语义）。
