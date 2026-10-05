# R4-13.S17 evidence (Mux4SboxItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-123..125（Protocol、MaxConnections、Padding）。
- Rust 消费者断言：`codegen::tests::r4_13_s17_mux4_sbox_reaches_codegen`，含 Protocol null→`h2mux` 回退；生成器侧 sing-box mux 使用 `settings.mux4_sbox`。
- Dart 合同：`test/r4_13_s17_contract_test.dart`（2 例，pass）：保存→重开；缺省回退。
- 命令：见 `../R4-13/commands.log`。
