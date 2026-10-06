# SP-23.FLD-CFG-115 — RoutingBasicItem.DomainStrategy4Singbox

状态：implemented（实例登记完成；真实 sing-box 解析/分流验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-115（主 owner SP-23；消费者归属 SP-30）。

本次唯一用户流程：路由设置窗选择 sing-box 侧 DomainStrategy→保存→重启 core→
sing-box routing domain strategy/default resolver 映射；按冻结 core 版本语义，
不支持 wire 不发旧字段；真实解析/分流生效。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-115；leaf；platform_scope=all；original_type=string；original_default=null。
关联：FLD-CFG-114（xray 侧），SD-07。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: RoutingBasicItem.DomainStrategy4Singbox`；
只读核对原版 sing-box 策略语义与版本能力。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法 sing-box 策略/null；
UI 草稿 `routing_windows.dart:309/327`、动作 `routing_actions.dart:81/199/264/311`、
默认 `settings_defaults.dart:94 null`。持久化 save；生效 restart_core。
版本能力诊断；非法值→拒绝且旧计划保留。平台写入仅授权隔离机；
10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-30）：`apps/desktop/lib/features/routing/routing_windows.dart`
（`:309/327`）、`apps/desktop/lib/features/routing/routing_actions.dart`
（`:81/199/264/311`）、`crates/application/src/routing.rs`（`:49` 落盘）、
`crates/config_codegen/src/util.rs`（`:427 DomainStrategy4Sbox`）。
本卡未改生产代码。

禁止改变的已有行为：原版 null 缺省；版本语义；不支持 wire 不发旧字段；
其它路由项不动。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs:380`（`DomainStrategy4Singbox` 缺省）、
  `persistence/src/schema.rs:181`、`mapping.rs:190`。
- DTO：`bridge_api` settings RoutingBasicItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：路由窗编辑→动作→保存→同 revision plan→sing-box 配置→真实会话。
- 单测：Dart `r4_13_s18_contract_test.dart:18`（prefer_ipv4）。
- 缺口：冻结 core 版本语义真实解析/分流验收未跑（CSV current_gap）。

测试夹具和原版预期：合成 sing-box 策略；正向 prefer_ipv4→plan 落值→版本校验→
合成解析/分流；负向非法值→拒绝，不发旧字段。

本次必须通过的命令/真实场景（SP-30，未运行）：`cargo test -p config_codegen --locked`
（sing-box 路由）+ `flutter test`（`r4_13_s18_contract`）；
补正式路由窗→FRB→保存→同 revision plan→真实解析/分流→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-115.md`。

完成条件：保存、重开和真实 sing-box 解析/分流三者一致；仅序列化不算。

发现接口缺口时的处理：真实分流联动缺口已登记；归属 SP-30。
