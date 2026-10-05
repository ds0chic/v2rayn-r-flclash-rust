# R4-13.S18 evidence (RoutingBasicItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-114..116（DomainStrategy、DomainStrategy4Singbox、RoutingIndexId）。
- Rust 消费者断言：`codegen::tests::r4_13_s18_routing_basic_reaches_codegen`，含空→`AsIs` 回退；生成器侧 xray/sing-box routing 使用 `settings.routing_basic`。`RoutingIndexId` 由 routing 引擎选活动路由组消费。
- Dart 合同：`test/r4_13_s18_contract_test.dart`（2 例，pass）：保存→重开；缺省 `AsIs`。
- 命令：见 `../R4-13/commands.log`。
