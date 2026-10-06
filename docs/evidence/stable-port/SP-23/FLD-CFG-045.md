# SP-23.FLD-CFG-045 — Inbound[0].SecondLocalPortEnabled

状态：implemented（实例登记完成；正式入口/真实第二端口验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-045（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改入站 SecondLocalPortEnabled 开/关→保存→
同 revision plan→各协议第二本地入站与实际 endpoint；
平台 endpoint 取真实端口（`derived_local_port`），不硬编码。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-045；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-042/043/044、FLD-CFG-035..041（同入站组）、SD-07/SD-05。

必读上游文件、符号和固定 commit：`ConfigItems.cs:38 :: InItem.SecondLocalPortEnabled`；
只读核对原版第二本地端口语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
开→各协议第二端口及偏移生效；关→单入站。端口偏移派生见
`platform_service.rs:33 derived_local_port`（FLD-CFG-035 同机制）。
持久化 save；生效 restart_core（`settings_timing.rs:162`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:422`（投影）、
`platform_service.rs:33` 偏移派生。
本卡未改生产代码。

禁止改变的已有行为：endpoint 取真实端口不硬编码；关不删用户其它入站参数；
stale revision 拒绝；保存/运行分离；未知入站键保留。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:373`；默认 false（`domain/src/settings.rs:173`）。
- DTO：`bridge_api/src/api/settings.rs:883,900,919` + FRB wire；
  Dart 侧 `api/settings.dart:564` + wire。
- 正式入口：`option_setting_window.dart:521-525`（`TbSettingsSecondLocalPortEnabled` 行）。
- 投影：`codegen.rs:422 base.inbound.second_local_port_enabled`。
- 单测：`codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`
 （`:1214 开`、`:1225` 断言）；
  `config_codegen/tests/xray_global.rs:14`（开组合）；
  `config_codegen/tests/singbox_global.rs:80`（开组合）；
  Dart `r4_13_s13_contract_test.dart:24` 同字段 save→reopen。
- 缺口：正式入口→FRB→持久化→重开→各协议第二端口真实监听完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 开→plan→core 校验→各协议第二端口真实监听；
负向 关→单入站；endpoint 为真实派生端口。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s13_inbound_reaches_codegen`）；
正式设置窗→FRB→保存→同 revision plan→≥11808 真实第二端口→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-045.md`。

完成条件：plan→真实第二端口闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实监听验收缺口已登记；归属 SP-24。
