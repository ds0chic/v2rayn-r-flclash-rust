# R4-13.S12 evidence (HysteriaItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-126..128（UpMbps、DownMbps、HopInterval）。
- Rust 消费者断言：`codegen::tests::r4_13_s12_hysteria_reaches_codegen`；生成器侧 xray/sing-box hysteria2 使用 `hysteria.up_mbps/down_mbps/hop_interval`。
- Dart 合同：`test/r4_13_s12_contract_test.dart`（2 例，pass）：保存→重开；缺省 100/100/30。
- 命令：见 `../R4-13/commands.log`。
