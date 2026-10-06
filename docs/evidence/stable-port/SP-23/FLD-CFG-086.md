# SP-23.FLD-CFG-086 — ConstItem.SrsSourceUrl（含本地 SRS 相关）

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-086（主 owner SP-23；消费者归属 SP-25）。

本次唯一用户流程：SRS 源指向本地合成资源源→下载 srss 落地→`local_srs_files` 快照→
断网后正式 sing-box 计划仍选本地 srss（不暗访 remote）→重开保持。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02; SD-10client; routing/resource source schema; 正式codegen context`。
关联 SD-07/10/17。

对应 ID：FLD-CFG-086；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-085（GeoSourceUrl 同形）、085/086/087 资源源组，SD-15。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ConstItem.SrsSourceUrl`；
区域 Russia/Iran 模板（`application/src/dns.rs:189-205`）；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。
注意：区域预设“仅存待下载 URL ≠ 完整预设落地”（审计 CP-SET-08 原话）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 URL 模板（含 `{0}/{1}` 占位）/
空（空回内置，`dns.rs:241-242 effective_srs_source` + `:582-599` 单测）；
UI 草稿 `option_setting_window.dart:1003,1005`、`resource_auto_update.dart:40`、
`settings_defaults.dart:131`。SP-02 提交语义；持久化 save；生效 save（冻结）。
下载中断/取消保留旧文件；坏内容/离线回退冻结行为；secret 不入日志。

允许修改的模块（SP-25）：`crates/application/src/dns.rs`（源模板）、
`engine.rs:248-256,1715-1734,4054-4056`（资源请求与 codegen 输入 writer 预约）、
`config_codegen` 本地选择逻辑（`local_srs_files` 填充者，新写处由 SP-00 核定归属）。
本卡未改生产代码。

禁止改变的已有行为：内置源回退；`resource_requests` 现有 region 预设；
未知 YAML/字段保留（merge 侧）；stale/保存语义。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:530`（`srs_source_url`）；DTO `settings.rs:501,511,523`；
  FRB wire（`frb_generated.rs:6586,11022,14557`）；timing Save（`settings_timing.rs:83`）。
- 模板生效：`application/src/dns.rs:238-242 effective_srs_source`；
  `engine.rs:248-256` 资源请求装配；`:4054-4056` 进 codegen `ruleset_url`；
  `:1715-1734,5011-5328` 源持久化与单测。
- 本地选择消费者（悬空）：`config_codegen/src/singbox/ruleset.rs:82`
  读 `input.settings.local_srs_files`（`input.rs:469 BTreeSet<String>`）。
- 缺口 G-07：`local_srs_files` 无生产填充者——全仓库仅定义 + 消费者 +
  `config_codegen/tests/singbox_global.rs:254` 测试插入。下载了 srss ≠ 生成器选本地。

测试夹具和原版预期：本地合成 SRS 源（定长 srss + manifest）；正向 下载→快照→
断网生成选本地；负向 坏内容/中断/取消/离线；remote 零访问断言（抓包/计数层，
SP-25 定方法）。

本次必须通过的命令/真实场景（SP-25，未运行）：`cargo test -p application -p config_codegen --locked`；
正式源设置→FRB→保存→下载→快照→断网生成 diff→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-086.md`。

完成条件：下载→快照→离线本地选择闭环 + 重开；只落文件不算。

发现接口缺口时的处理：G-07 已登记；填充者归属由 SP-00 核定，不私定模块。
