# SP-23.FLD-CFG-130 — ClashUIItem.EnableMixinContent（Mihomo merge）

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-130（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：开 EnableMixinContent + 受控 mixin 文件→保存→Mihomo custom 启动→
mixin 按 `prepend-/append-/removed-` 语义合并生效；关→原文件保留且不 merge。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）；SP-28 协同。
CSV dependencies：`SD-01/02; SD-10可选资源; 冻结Clash merge顺序`。关联 SD-07。

对应 ID：FLD-CFG-130；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-129（同 merge），SD-14。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ClashUIItem.EnableMixinContent`；
`CoreConfigClashService.GenerateClientCustomConfig`；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:1096-1097`、`settings_defaults.dart:161`。
SP-02 提交语义。持久化 save；生效 restart_core。
关不断言删文件（关闭保留原文件）；不存在/坏 YAML/未知键按冻结处理并可见错误；
mixin 文件读写失败归属与重试按 SP-02。

允许修改的模块（SP-24）：同 129（`codegen.rs:461-479`、`mixin.rs:75`、`engine.rs:4325-4339`
writer 预约）。本卡未改生产代码。

禁止改变的已有行为：同 129；关闭不删原文件；`mixin.rs:1-40` 既有语义不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:659`（`enable_mixin_content`，默认 false `:680`）。
- DTO：`bridge_api/src/api/settings.rs:673,687,707`；FRB wire `enable_mixin_content`
  （`frb_generated.rs:6504,10910,14488`）。
- 投影/Helper：`codegen.rs:466-477`（`mixin_enabled`）、`:1086-1093` 开/关投影单测；
  `mixin.rs:212-271` merge 语义单测。
- 缺口：同 G-06（无正式 custom 计划调用）。

测试夹具和原版预期：合成 base + 受控 mixin（prepend/append/removed 各一）；
正向 开（合并生效）/关（原文 + 文件保留）；负向 不存在文件/坏 YAML/未知键；
真实 Mihomo API/配置效果（SP-24）。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`；
正式 custom 计划 merge diff（开/关/坏文件三形）→真实会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-130.md`。

完成条件：正式 merge 接入 + 三形 + 会话 + 重开；helper 单测不算。

发现接口缺口时的处理：随 G-06。Mihomo 组 2 叶登记完毕。
