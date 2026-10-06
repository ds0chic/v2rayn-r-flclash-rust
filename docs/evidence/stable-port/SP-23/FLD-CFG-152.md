# SP-23.FLD-CFG-152 — Fragment4RayItem.Delays

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-152（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：`Delays=10-20`→一次保存→restart_core→wire `delays=["10-20"]`
且 `delay=10-20`→xray 校验通过→重开往返。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。CSV dependencies 同 150。

对应 ID：FLD-CFG-152；leaf；platform_scope=all；original_type=list<string>；
original_default=["10-20"]。关联：032/033、150/151/153，SD-09。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: Fragment4RayItem.Delays`；
`V2rayOutboundService.cs:831-843`；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 `"a-b"` 列表/null/空；
UI 草稿见审计 `settings-current` 行（monitor_controller 引用为误配候选，
正式入口以 `option_setting_window.dart:751,755` 为准，SP-24 核对）。
提交/持久化 save、生效 restart_core、取消/错误语义同 150。

允许修改的模块（SP-24）：`config_codegen/src/xray/config.rs:248-252`、
`application/src/codegen.rs:443` 组。

禁止改变的已有行为：同 150；legacy Interval（155）迁移源地位。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` `Fragment4RayItem.delays`。
- 透传：`application/src/codegen.rs:443`；DTO 同 150。
- 发射：`config.rs:248-252`（空回 `["10-20"]`）。
- 注意：审计 `fresh_reference_candidates` 把 monitor_controller 列为候选，
  那是字符串巧合引用，不能当消费证据——已在此纠正，正式 UI 入口待 SP-24 核对。

测试夹具和原版预期：合成出站；正向单段/多段；负向空列表（回 `10-20`）/坏格式冻结处理。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p config_codegen --locked`；
正式窗→FRB→保存→restart_core→JSON diff→xray 校验→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-152.md`。

完成条件：正向×2 + 负向 + 校验 + 重开；只 round-trip 不算。

发现接口缺口时的处理：正式 UI 控件归属疑问已登记（SP-24 先核对 `option_setting_window`），
不把候选引用当证据。
