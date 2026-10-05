# R4-13.S11 evidence (HappyEyeballs4RayItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-177..180（TryDelayMs、PrioritizeIPv6、Interleave、MaxConcurrentTry）。
- Rust 消费者断言：`codegen::tests::r4_13_s11_happy_eyeballs_reaches_codegen` → `settings_from_app`；生成器侧 `config_codegen` xray DNS 使用 `happy_eyeballs4_ray`。
- Dart 合同：`test/r4_13_s11_contract_test.dart`（2 例，pass）：保存→重开；缺省 250/false/1/4。
- 命令：见 `../R4-13/commands.log`。
