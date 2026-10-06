# SP-23.FLD-CFG-035 — Inbound[0].LocalPort

状态：implemented（实例登记完成；正式入口/真实监听验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-035（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改本地混合监听端口→保存→同 revision 生成 runtime plan→
对应 core 配置校验/真实会话监听生效；范围 1..65535，越界/占用→拒绝回滚；
测试监听只用 ≥11808 预探测端口，绝不占 10808。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-035；leaf；platform_scope=all；original_type=int；original_default=10808。
关联：FLD-CFG-036（协议）/037/038/039/040/041（同入站组）、042 双端口、SD-07、SD-05。

必读上游文件、符号和固定 commit：`ConfigItems.cs:28 :: InItem.LocalPort`；
只读核对原版入站监听端口语义（默认 10808）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 1..65535；
越界→FIELD_RANGE `error.local_port_range`（`settings.rs:84-86`、`engine.rs:4955-4962`），
旧值不变；端口占用→回滚。注意：持久化 LocalPort ≠ 已应用监听端口——
实际监听端口是调用方持有的 `opts.local_port`（测试默认 11808，`codegen.rs:38,54`），
Pac=+3、Xray API=+4、mihomo API=+5（UI `:517` 注释）。
持久化 save；生效 restart_core（`settings_timing.rs:153`）。平台写入仅授权隔离机。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:418-430,479`
（入站投影与监听端口装配）、`crates/application/src/settings.rs:84`（范围校验）、
`crates/application/src/engine.rs:4955-4962`（plan 端口门控）。
本卡未改生产代码。

禁止改变的已有行为：10808 现网端口不动；非法端口不写脏 plan；stale revision 拒绝；
保存/运行分离；未知入站键保留。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:353`（`local_port: i32`）；
  默认 10808（`domain/src/settings.rs:163`）。
- DTO：`bridge_api/src/api/settings.rs:872,890` + FRB wire（`:7189,11776,14941`）；
  Dart 侧 `api/settings.dart:529` + wire。
- 正式入口：`option_setting_window.dart:510`（`_int(inbound,'LocalPort')`，core tab `:499`）。
- 投影/门控：`codegen.rs:418-422`；`engine.rs:4955-4962` 范围门控；
  `platform_service.rs:33 derived_local_port` 偏移派生。
- 单测：`settings.rs:242 validate_rejects_out_of_range_port`；
  `engine.rs:6555 settings_invalid_port_does_not_change_old_value`；
  `engine.rs:6462 applied_inbound_facts_report_protocol_and_port_only_when_running`；
  `codegen.rs:1118` 断言测试端口 11808（非 10808）。
- 缺口：正式入口→FRB→持久化→重开→真实监听完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 合法端口→plan→core 校验→真实监听；
负向 0/越界→拒绝且旧值不变；占用→回滚。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`validate_rejects_out_of_range_port`、
`settings_invalid_port_does_not_change_old_value`、
`r4_13_s13_inbound_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→≥11808 真实监听→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-035.md`。

完成条件：plan→真实监听闭环 + 重开；仅投影/校验单测不算。

发现接口缺口时的处理：真实监听验收缺口已登记；归属 SP-24。
