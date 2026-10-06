# SP-23.FLD-CFG-114 — RoutingBasicItem.DomainStrategy

状态：implemented（实例登记完成；真实 DNS/分流联动验收未跑，不写 verified）。
任务 ID：SP-23.FLD-CFG-114（主 owner SP-23；消费者归属 SP-30）。

本次唯一用户流程：路由设置窗选择 DomainStrategy→保存→重启 core→xray
`routing.domainStrategy` 默认与路由实体显式值按冻结优先级生成；真实 DNS 和分流生效。

前置任务及已验证证据：SP-00；SP-01/02、SP-12（未 verified）。
CSV dependencies：`SD-01/02/04; 主方案正式窗口与命令队列`。

对应 ID：FLD-CFG-114；leaf；platform_scope=all；original_type=string；
original_default="AsIs"。
关联：FLD-CFG-115（sing-box 侧），SD-07；旧 RoutingIndexId 见 SD-15。

必读上游文件、符号和固定 commit：`ConfigItems.cs :: RoutingBasicItem.DomainStrategy`；
只读核对原版 xray 策略默认与实体显式优先级。
固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。当前 HEAD `0b80627`
（全 `0b80627661536f6bb76b4b05c897400646076048`）。

输入、输出、错误、取消、权限、持久化及生效语义：合法策略枚举；
UI 草稿 `routing_windows.dart:301/318`、动作 `routing_actions.dart:80/198/263/310`、
默认 `settings_defaults.dart:93 AsIs`。持久化 save；生效 restart_core。
设置值/活动路由值组合按冻结优先级；非法值→拒绝且旧计划保留。
平台写入仅授权隔离机；10808 禁占，测试端口 ≥11808 预探测。

允许修改的模块（SP-30）：`apps/desktop/lib/features/routing/routing_windows.dart`
（`:301/318`）、`apps/desktop/lib/features/routing/routing_actions.dart`
（`:80-81/198-199/263-264/310-311`）、`crates/application/src/routing.rs`
（`:47` 落盘）、`crates/config_codegen/src/util.rs`（`:427` Sbox 映射对照）、
`crates/domain/src/routing.rs`（`:20` Xray 候选）。
本卡未改生产代码。

禁止改变的已有行为：原版默认 AsIs；冻结优先级；其它路由项不动；
保存/运行分离。

当前 provider / caller / DTO（HEAD 实测）：
- 存储：`domain/src/settings.rs`（`:380` 附近）、`persistence/src/schema.rs:180`、
  `mapping.rs:189` 行级映射。
- DTO：`bridge_api` settings RoutingBasicItem 段 + FRB wire；Dart 侧 wire + 默认。
- 调用链：路由窗编辑→动作→保存→同 revision plan→core 配置→真实会话。
- 单测：Dart `r4_13_s18_contract_test.dart:17`（IPIfNonMatch）、
  `fix08_routing_draft_test.dart:209`；Rust `t11_codegen_matrix.rs:474` 双 core 差异、
  `t18_settings_chain.rs:139` 链路覆盖 DomainStrategy、`edge_cases.rs:96` 保留。
- 缺口：设置值/活动路由值组合真实 DNS 和分流验收未跑（CSV current_gap）。

测试夹具和原版预期：合成策略值；正向 AsIs/IPIfNonMatch→plan 落值→core 校验→
合成 DNS/分流；负向非法值→拒绝。

本次必须通过的命令/真实场景（SP-30，未运行）：`cargo test -p application --locked`
（`t11_codegen_matrix`、`t18_settings_chain`）+ `flutter test`（`r4_13_s18_contract`）；
补正式路由窗→FRB→保存→同 revision plan→真实 DNS/分流→重开。最终门禁 SP-34。

证据文件位置：本目录 `FLD-CFG-114.md`。

完成条件：保存、重开和真实 DNS/分流三者一致；仅序列化不算。

发现接口缺口时的处理：真实分流联动缺口已登记；归属 SP-30。
