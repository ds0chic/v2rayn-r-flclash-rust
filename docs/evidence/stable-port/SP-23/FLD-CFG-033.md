# SP-23.FLD-CFG-033 — CoreBasicItem.EnableFinalFragment

状态：implemented（实例登记完成；正式入口/真实 final-fragment 路径验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-033（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：设置窗改 CoreBasic EnableFinalFragment 开/关→保存→
同 revision 生成 runtime plan→适用 core final fragment 路径及路由收尾生成；
与 EnableFragment 的真值组合保留冻结覆盖顺序。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-033；leaf；platform_scope=all；original_type=bool；original_default=false。
关联：FLD-CFG-032（EnableFragment 组合）/030/031/034、FLD-CFG-026..029、SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs:20 :: CoreBasicItem.EnableFinalFragment`；
只读核对原版 final fragment 开关语义。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `2ea50bd`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 true/false；
EnableFragment×EnableFinalFragment×TLS 场景真值组合按冻结语义输出。
持久化 save；生效 restart_core（`settings_timing.rs:97`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:379`（投影）、
xray `config.rs:102` final-fragment 门控。
本卡未改生产代码。

禁止改变的已有行为：与 EnableFragment 的冻结组合顺序；
关不删用户参数；未知键保留；stale revision 拒绝；保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:238`（默认 false，`:256`）。
- DTO：`bridge_api/src/api/settings.rs:61,76,93` + FRB wire；
  Dart 侧 `api/settings.dart:240` + wire。
- 正式入口：`option_setting_window.dart:782-783`（core tab `:499`）。
- 投影：`codegen.rs:379 base.core_basic.enable_final_fragment`。
- 单测：`codegen.rs:995 core_basic_item_reaches_generated_config`
 （`:1003 enable_final_fragment=true`、`:1010` 断言；`:1432` 同组 final-fragment 组合）；
  `config_codegen/tests/singbox_global.rs:271 singbox_bind_interface_and_final_fragment`
 （`:273-275` 开组合）；
  `config_codegen/tests/xray_global.rs:137-138` fragment 真值组合。
- 缺口：正式入口→FRB→持久化→重开→真实 final-fragment 路径完整验收未跑（CSV current_gap）。

测试夹具和原版预期：合成设置修订；正向 开×开×TLS→plan 含收尾段→core 校验；
负向 关→无收尾段；组合顺序与冻结一致。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`core_basic_item_reaches_generated_config`）+ `cargo test -p config_codegen --locked`
（`singbox_bind_interface_and_final_fragment`）；
正式设置窗→FRB→保存→同 revision plan→真实路径→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-033.md`。

完成条件：plan→真实 final-fragment 路径闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：真实路径验收缺口已登记；归属 SP-24。
