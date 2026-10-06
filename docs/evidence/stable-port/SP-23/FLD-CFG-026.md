# SP-23.FLD-CFG-026 — CoreBasicItem.LogEnabled

状态：implemented（实例登记完成；正式入口/真实 core 日志文件验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-026（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 CoreBasic LogEnabled 关/开→保存→同 revision 生成
runtime plan→真实 core 日志文件与 level 行为变化；它不等同应用 EnableLog（063）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-026；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-027（Loglevel 联动）/028/029、FLD-CFG-063（应用日志，区分）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:6 :: CoreBasicItem.LogEnabled`；
只读核对原版 core 日志总开关语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
开→core 落日志文件；关→core 不落日志（应用日志 063 独立）。
持久化 save；生效 restart_core（`settings_timing.rs:99`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:373`（投影）、
config_codegen 各 core `log` 段生成。
本卡未改生产代码。

禁止改变的已有行为：与应用 EnableLog 解耦；关不删已有日志文件；未知键保留；
stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:224`（默认 false，`:249`）。
- DTO：`bridge_api/src/api/settings.rs:54,69,86` + FRB wire（`:6621,11068,14575`）；
  Dart 侧 `api/settings.dart:210` + wire。
- 正式入口：`option_setting_window.dart:625-626`（core tab `:499`）。
- 投影：`codegen.rs:373 base.core_basic.log_enabled`。
- 单测：`codegen.rs:995 core_basic_item_reaches_generated_config`
 （`:1001 log_enabled=true`、`:1008` 断言）。
- 缺口：真实 core 日志文件落盘/level 行为验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 关/开→plan→core 校验→真实日志文件；
负向 关→无 core 日志但应用日志独立。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`core_basic_item_reaches_generated_config`）；
正式设置窗→FRB→保存→同 revision plan→真实 core 日志→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-026.md`。

完成条件：plan→真实 core 日志闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实日志验收缺口已登记；归属 SP-24。
