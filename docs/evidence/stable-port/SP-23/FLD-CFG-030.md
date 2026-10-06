# SP-23.FLD-CFG-030 — CoreBasicItem.SendThrough

状态：implemented（实例登记完成；正式入口/真实出站绑定验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-030（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 CoreBasic SendThrough（本地出站源地址）→保存→
同 revision 生成 runtime plan→对应 core 出站绑定配置校验/真实会话；
空/空白回 null（与 BindInterface 同一 trim 语义）。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-030；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-031（BindInterface 同组绑定）/032/033/034、FLD-CFG-026..029（同 CoreBasic 组）、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:14 :: CoreBasicItem.SendThrough`；
只读核对原版出站源地址语义（可空）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法源地址字符串 / 空白→trim 后 null
（`settings_controller.dart:815-821`）；非法地址→core 校验失败，已保存值不伪装已应用。
持久化 save；生效 restart_core（`settings_timing.rs:101`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:376`（投影）、
config_codegen 各 core `send_through` 出站装配（sing-box `config.rs:118-127`、
xray `config.rs:363-371`）。
本卡未改生产代码。

禁止改变的已有行为：空值回退语义；trim 规则与 BindInterface 一致；
未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:232`（`Option<String>`，默认 None `:253`）。
- DTO：`bridge_api/src/api/settings.rs:58,73,90` + FRB wire；
  Dart 侧 `api/settings.dart:237` + wire。
- 正式入口：`option_setting_window.dart:663-665`（core tab `:499`）。
- 投影：`codegen.rs:376 base.core_basic.send_through`。
- 单测：`codegen.rs:995 core_basic_item_reaches_generated_config`；
  Dart `r4_13_s05_contract_test.dart:136 SendThrough/BindInterface trim and empty->null on save`；
  `config_codegen/tests/xray_global.rs:188 xray_bind_interface_and_send_through`。
- 缺口：正式入口→FRB→持久化→重开→真实出站绑定完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 合法地址→plan→core 校验；
负向 空白→null、不可用地址→失败不宣称已应用。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`core_basic_item_reaches_generated_config`）；
正式设置窗→FRB→保存→同 revision plan→core 校验/真实会话→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-030.md`。

完成条件：plan→core 出站绑定闭环 + 重开；仅投影/trim 单测不算。

发现接口缺口时的处理：真实会话验收缺口已登记；归属 SP-24。
