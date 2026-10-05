# R4-13.S04 evidence (ConstItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-084..087（SubConvertUrl、GeoSourceUrl、SrsSourceUrl、RouteRulesTemplateSourceUrl）。
- 消费者（已存在）：`crates/application/src/dns.rs::effective_sub_convert_url/effective_geo_source_url/effective_srs_source_url/effective_route_rules_template_source_url`；`subs.rs`（订阅转换模板）、`engine.rs:3288..`(geo/srs 下载) 使用。
- Dart 合同：`test/r4_13_s04_contract_test.dart`（2 例，pass）：4 URL 保存→重开；空串/`null` 按原样留存（消费者决定回退）。
- 语义：空白/缺省时消费者回退上游内置源，不落库改写。
- 命令：见 `../R4-13/commands.log`。
