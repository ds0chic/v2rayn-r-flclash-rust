# SP-23.FLD-CFG-098 — TunModeItem.Stack

状态：implemented（实例登记完成；正式入口/跨 stack 真实流量验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-098（主 owner SP-23；消费者归属 SP-24）。

本次唯一用户流程：TUN 窗选 Stack（system/gvisor/mixed 等冻结值）→保存→同 revision 生成
runtime plan→对应 core/平台实现承载流量；未知值按冻结能力矩阵拒绝，不静默回退。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`主方案TUN实际session/route lease/退出清理; SD-04/05; 隔离VM真实验收`。

对应 ID：FLD-CFG-098；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-095（总开关）/096/097/099/101/104（同 TUN 组）、SD-07、SD-05/18。

必读上游文件、符号和固定 commit：`ConfigItems.cs:146 :: TunModeItem.Stack`；
只读核对原版 stack 枚举语义（system/gvisor/mixed 等）。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `467607e`。

输入、输出、错误、取消、权限、持久化及生效语义：合法 冻结枚举值/null（null=缺省）；
非法值→拒绝并保留旧 plan（错误能力按冻结矩阵）。持久化 save；
生效 restart_core（`settings_timing.rs:288`）。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-24）：`crates/application/src/codegen.rs:446`（投影）、
`crates/application/src/tun_plan.rs`（stack 进 plan）、`services/net_host` helper 生效。
本卡未改生产代码。

禁止改变的已有行为：未知 TUN 键保留；未知 stack 值不静默映射；stale revision 拒绝；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:274`（`Option<String>`，默认 None，`:299`）。
- DTO：`bridge_api/src/api/settings.rs:106,123,142` + FRB wire；
  Dart 侧 `api/settings.dart` TunModeItemDto + wire。
- 正式入口：`option_setting_window.dart:1217`（`_str(tun,'Stack')`，TUN tab `:1196`）。
- 投影：`codegen.rs:446 base.tun.stack`。
- 单测：`codegen.rs:1037 r4_13_s22_tun_fields_reach_generated_config`
 （`:1045 stack="system"`、`:1058/:1086` 断言）。
- 缺口：各 stack 真实流量/错误能力、隔离 VM 对照未验收（CSV current_gap）。

测试夹具和原版预期：合成 TUN 修订；正向 system/gvisor 等有效值→plan→core 校验；
负向 未知值→拒绝、旧 plan 保留；null→缺省行为。

本次必须通过的命令/真实场景（SP-24，未运行）：`cargo test -p application --locked`
（`r4_13_s22_tun_fields_reach_generated_config`）；
正式 TUN 窗→FRB→保存→同 revision plan→隔离真实流量→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-098.md`。

完成条件：plan→各 stack 真实流量闭环 + 重开；仅投影单测不算。

发现接口缺口时的处理：跨 stack 真实流量缺口已登记；归属 SP-24。
