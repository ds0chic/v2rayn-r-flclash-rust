# SP-23.FLD-CFG-129 — ClashUIItem.EnableIPv6（Mihomo merge）

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-129（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：Mihomo custom 节点 + 开 EnableIPv6→保存→启动→生成的
mihomo YAML 含 `ipv6:true` 且真实会话按 IPv6 能力建信；关→不改自带 YAML 值。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）；SP-28（逐核矩阵）协同。
CSV dependencies：`SD-01/02; 冻结Clash merge顺序; native custom plan`。关联 SD-07。

对应 ID：FLD-CFG-129；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-130（同 merge），SD-14。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ClashUIItem.EnableIPv6`；
`CoreConfigClashService.GenerateClientCustomConfig`（ipv6/端口/TUN/Mixin 实际改写，
见审计 CP-SET-08）；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
UI 草稿 `option_setting_window.dart:1091-1092,1228-1229`、`settings_defaults.dart:71,160`。
SP-02 提交语义。持久化 save；生效 restart_core（冻结）。
自带 YAML 已有 ipv6 值时覆盖顺序按冻结 merge 顺序（SP-24 核对）；无 IPv6 能力准确诊断。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:461-479`
（`mixin_options_from_app` 接正式 custom 计划）、`engine.rs:4325-4339`
（native_custom 分支 merge 接入点，须预约 writer）。本卡未改生产代码。

禁止改变的已有行为：冻结 Clash merge 顺序；未知 YAML 顺序/字段保留；
关闭不删用户文件；非 custom 计划行为不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` `ClashUIItem`（`enable_ipv6`；`:680` 默认 `enable_mixin_content:false` 同结构）。
- DTO：`bridge_api/src/api/settings.rs:673,687,707`（`enable_mixin_content` 同结构；ipv6 同 DTO）。
- 投影（存在但悬空）：`codegen.rs:461-479 mixin_options_from_app`
  （`:475 ipv6`、`：477 mixin_enabled`）；唯一调用为同文件测试 `:1076-1093`
  （`r4_13_s02_clash_ui_item_reaches_mihomo_merge`，投影级证明）。
- 正式链缺口 G-06：`engine.rs:4325-4339` native_custom 分支原文直通
  （`custom_config` 非空即用，不经 merge）；`generate_mihomo`（`mixin.rs:75`）+
  `MixinOptions`（`mixin.rs:20-40`）仅被测试/`t10_core_matrix.rs:404-409` 调用。

测试夹具和原版预期：合成 Mihomo custom（自带 YAML 有/无 ipv6 两形）；
正向 开（ipv6:true 落盘且会话）、关（自带值原样）；负向 坏 YAML 拒绝且旧计划保留；
未知键保留断言。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`；
正式 custom 计划生成 diff（开/关/自带值三形）→真实 Mihomo 校验/会话→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-129.md`。

完成条件：正式 custom 计划 merge 接入 + 三形 diff + 会话 + 重开；helper 单测不算。

发现接口缺口时的处理：G-06 已登记；merge 接入点 writer 与 SP-00/引擎侧预约。
