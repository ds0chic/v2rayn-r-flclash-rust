# SP-23.FLD-CFG-027 — CoreBasicItem.Loglevel

状态：implemented（实例登记完成；正式入口/真实 core 日志级别验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-027（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 Loglevel（各合法 level）→保存→同 revision 生成
runtime plan→xray/sing-box/core `log.level` 生效；错误 enum 与关闭 log 组合按冻结处理。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-027；leaf；platform_scope=all；original_type=string；
original_default="warning"。
关联：FLD-CFG-026（LogEnabled 联动）/028/029、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:8 :: CoreBasicItem.Loglevel`；
只读核对原版日志级别语义（默认 "warning"）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 冻结 level 枚举/null；
显式 null 保持 null（对象缺席→回退 "warning"，
`domain/src/settings.rs:1309-1315 null_loglevel_stays_none_but_missing_object_gets_warning`）；
错误 enum 按冻结处理。持久化 save；生效 restart_core（`settings_timing.rs:100`）。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:372`
（投影，None→"warning"）、config_codegen 各 core `log` 段生成。
本卡未改生产代码。

禁止改变的已有行为：显式 null 不转默认；未知键保留；stale revision 拒绝；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:226`（`Option<String>`，默认 Some("warning")，`:250`）。
- DTO：`bridge_api/src/api/settings.rs:55,70,87` + FRB wire（`:6611,11069,14576`）；
  Dart 侧 `api/settings.dart:211` + wire。
- 正式入口：`option_setting_window.dart:630,638`（core tab `:499`）。
- 投影：`codegen.rs:372`（`unwrap_or_else(|| "warning")`）。
- 单测：`codegen.rs:995 core_basic_item_reaches_generated_config`
 （`:1000 loglevel="debug"`、`:1007/:1025` xray log 断言）；
  `domain/src/settings.rs:1309 null_loglevel_stays_none_but_missing_object_gets_warning`、
  `:1448/:1555` roundtrip。
- 缺口：真实 core 日志级别行为验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 debug/warning 等→plan→core 校验→真实级别；
负向 错误 enum 按冻结处理；显式 null 保持 null。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application -p domain --locked`
（`core_basic_item_reaches_generated_config`、
`null_loglevel_stays_none_but_missing_object_gets_warning`）；
正式设置窗→FRB→保存→同 revision plan→真实 core 日志级别→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-027.md`。

完成条件：plan→真实 core 日志级别闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实日志验收缺口已登记；归属 SP-24。
