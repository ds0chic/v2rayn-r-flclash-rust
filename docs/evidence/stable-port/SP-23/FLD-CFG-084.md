# SP-23.FLD-CFG-084 — ConstItem.SubConvertUrl

状态：implemented（实例登记完成；真实转换→解析→事务替换验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-084（主 owner SP-23；消费者归属 SP-26）。

本次唯一用户流程：设置窗填写订阅转换器 URL→保存→订阅导入时请求构造按
SubConvertUrl（`{0}` 编码）经 shared HttpPolicy 真实转换→解析→事务替换；
禁用/空/自定义/错误响应/取消均按冻结语义处理，不泄漏真实订阅。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-084；leaf；platform_scope=all；original_type=string；original_default=null。
关联：SD-07/SD-10；`subscriptions/src/convert.rs:27` 模板语义。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: ConstItem.SubConvertUrl`；
只读核对原版转换器 URL 组合与编码语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 HTTPS URL 模板/null/空（禁用）；
UI 草稿 `option_setting_window.dart:1005-1007`、默认 `settings_defaults.dart:129 null`、
计划引用 `resource_auto_update.dart:23/41`。持久化 save；生效 save
（`settings_timing.rs:84`）。错误响应/取消→旧组保留；secret 不入日志。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-26）：`apps/desktop/lib/features/settings/option_setting_window.dart`
（`:1005-1007`）、`crates/application/src/subs.rs`（`:304-313/515/772/1662` 解析与回退）、
`crates/subscriptions/src/convert.rs`（`:27` 模板）、`crates/bridge_api/src/api/subs.rs`
（`:260` 转换路由注释）。
本卡未改生产代码。

禁止改变的已有行为：原版 URL 组合与编码；空=禁用；失败不覆盖旧组；秘密保护合同。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs` ConstItem 段（null 缺省）。
- DTO：`bridge_api` settings ConstItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：正式窗 `_set('ConstItem','SubConvertUrl')`→保存→重开→订阅导入
  `effective SubConvertUrl` 解析→`convert`→事务替换。
- 单测：Dart `r4_13_s04_contract_test.dart:18/25/46`（save→重开与冻结默认）、
  `r4_13_s03_contract_test.dart:24/31`、`fix16b_settings_source_test.dart:78/97`；
  Rust `persistence/tests/edge_cases.rs:100` 空串保留。
- 缺口：合成输入真实转换→解析→事务替换（含错误/取消）端到端验收未跑
  （CSV current_gap）；mock HTTP/ZIP 解析不等同真实链路。

测试夹具和原版预期：合成转换器 URL；正向自定义 HTTPS→真实转换替换；
负向空（禁用）/错误响应/取消→旧组保留，不泄漏真实订阅。

本次必须通过的命令/真实场景（SP-26，未运行）：`flutter test`
（`r4_13_s04_contract`）+ `cargo test -p subscriptions --locked`；
补正式窗→FRB→保存→合成转换服务→真实解析替换→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-084.md`。

完成条件：保存、重开和真实转换替换三者一致；仅保存 URL 不算。

发现接口缺口时的处理：真实转换链路缺口已登记；归属 SP-26。
