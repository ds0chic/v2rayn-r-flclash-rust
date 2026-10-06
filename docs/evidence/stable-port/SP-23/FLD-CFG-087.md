# SP-23.FLD-CFG-087 — ConstItem.RouteRulesTemplateSourceUrl（外部路由模板）

状态：implemented（实例登记完成；正式入口/最终效果验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-087（主 owner SP-23；消费者归属 SP-25）。

本次唯一用户流程：配置外部路由模板 URL→显式“下载模板”动作→异步 job 下载→
schema 校验→路由候选提交→UI 刷新为新规则；坏内容/中断/取消→保留旧规则并可见错误。

前置任务及已验证证据：SP-00（新异步 fetch/job API 核定，必需）；
SP-01/02、SP-12、SP-13（路由提交，未 verified）。
CSV dependencies：`SD-01/02; SD-10client; routing/resource source schema; 正式codegen context`。
关联 SD-07/10/17。

对应 ID：FLD-CFG-087；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-086（同源组），SD-15。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ConstItem.RouteRulesTemplateSourceUrl`；
`ConfigHandler.InitRouting` 外部来源分支（原版功能依据）；固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
当前 HEAD `7466bee`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 HTTPS URL/空（空走内置三 scheme）；
UI 草稿 `option_setting_window.dart:1010,1013`、`resource_auto_update.dart:42`、
`settings_defaults.dart:132`。SP-02 提交语义；持久化 save；生效 save（冻结）。
取消：下载取消不写规则；校验失败不提交；提交走 SP-13 权威快照语义
（部分失败不复活、loadFailed 只读）。

允许修改的模块（SP-25，新 API 由 SP-00 整合者定归属）：
`crates/bridge_api/src/api/routing.rs:403-439`（`import_builtin_routing` 现状点）、
新异步 fetch job + 事务化导入（模块待定，不预设）。本卡未改生产代码。

禁止改变的已有行为：内置三 scheme 分支（`:430-439` 后续）不动；
配 URL 时的显式拒绝不得悄悄改成静默内置导入；未知字段保留。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:532`（`route_rules_template_source_url`）；
  DTO `settings.rs:502,512,524`；FRB wire（`:6587,11023,14558`）。
- 模板读取：`application/src/dns.rs:259 effective_routing_template_source`；
  `bridge_api/src/api/dns.rs:317-318` 源回填。
- 现状（缺口）：`routing.rs:411-428` 配 URL 即返回
  `ok:false + UNAVAILABLE/error.routing_external_template`（`:632-652` 两单测锁定该行为）——
  无另一个异步 fetch/导入入口。区域 Russia/Iran 仅存 URL（`dns.rs`），≠ 落地。

测试夹具和原版预期：本地合成模板源（合法/坏 schema/超大/慢流/中断）；
正向 下载→校验→候选提交→UI 刷新；负向各失败保留旧规则；取消边界；
事务化（半写不可见）。

本次必须通过的命令/真实场景（SP-25，未运行）：新 job 单测 + 
`cargo test -p bridge_api --locked`；正式配置 URL→下载动作→候选提交→UI→重开。
最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-087.md`。

完成条件：异步下载→校验→提交→刷新→重开闭环；保持显式错误直到新链落地，
不得静默内置。

发现接口缺口时的处理：新异步 API 缺口已登记（provider=SP-25 待定/caller=路由窗/
DTO=job+候选/版本与取消语义/存储=暂存+事务发布/生效=提交时），由 SP-00 整合者排期。
