# R4-13.S16 evidence (Mux4RayItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-120..122（Concurrency、XudpConcurrency、XudpProxyUDP443）。
- Rust 消费者断言：`codegen::tests::r4_13_s16_mux4_ray_reaches_codegen`，含 null→8/16/`reject` 回退；生成器侧 xray mux 使用 `settings.mux4_ray`。
- Dart 合同：`test/r4_13_s16_contract_test.dart`（2 例，pass）：保存→重开；缺省回退。
- 命令：见 `../R4-13/commands.log`。
