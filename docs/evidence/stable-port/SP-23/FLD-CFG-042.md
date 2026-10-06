# SP-23.FLD-CFG-042 — Inbound[0].NewPort4LAN

状态：implemented（实例登记完成；正式入口/真实 LAN 入站验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-042（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改入站 NewPort4LAN 开/关→保存→
同 revision 生成 runtime plan→按冻结 AllowLANConn 组合生成 LAN 专用第二入站/端口分配；
实际分配冲突→回滚，停止清理残留监听。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-042；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-043/044（User/Pass 认证）/045、FLD-CFG-035..041（同入站组）、SD-07/SD-05。

必读上游文件、符号和固定 commit：`ConfigItems.cs:35 :: InItem.NewPort4LAN`；
只读核对原版 LAN 第二入站语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
AllowLANConn×NewPort4LAN×第二端口组合按冻结语义；端口冲突/停止失败→清理回滚。
持久化 save；生效 restart_core（`settings_timing.rs:154`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:424`（投影）、
xray `inbound.rs:147` LAN 分支、sing-box `inbound.rs:90` LAN 分支。
本卡未改生产代码。

禁止改变的已有行为：10808 现网端口不动；关不断言删用户认证值；
stale revision 拒绝；保存/运行分离；未知入站键保留。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/entities.rs:363-364`（serde `NewPort4LAN`）；
  默认 false（`domain/src/settings.rs:170`）。
- DTO：`bridge_api/src/api/settings.rs:880,897,916` + FRB wire；
  Dart 侧 `api/settings.dart:561` + wire。
- 正式入口：`option_setting_window.dart:595-600`（`TbSettingsNewPort4LAN`，入站区 `:499` 起 core tab 内）。
- 投影：`codegen.rs:424 base.inbound.new_port4_lan`。
- 单测：`codegen.rs:1211 r4_13_s13_inbound_reaches_codegen`（`:1216 开`、`:1227` 断言）；
  `config_codegen/tests/xray_global.rs:9 xray_inbound_ports_lan_log_stat`（`:16 开`）；
  `config_codegen/tests/singbox_global.rs:76 singbox_log_and_inbound_ports`（`:82 开`）；
  `application/tests/t18b_runtime_plan.rs:506`、`t18b_runtime_e2e.rs:605` 同字段 plan 覆盖。
- 缺口：正式入口→FRB→持久化→重开→真实 LAN 监听完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 开→plan 含 LAN 入站→core 校验→真实监听；
负向 关→无 LAN 入站；冲突→回滚清理。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s13_inbound_reaches_codegen`）+ `cargo test -p config_codegen --locked`
（`xray_inbound_ports_lan_log_stat`、`singbox_log_and_inbound_ports`）；
正式设置窗→FRB→保存→同 revision plan→≥11808 真实 LAN 监听→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-042.md`。

完成条件：plan→真实 LAN 监听闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实监听验收缺口已登记；归属 SP-24。
