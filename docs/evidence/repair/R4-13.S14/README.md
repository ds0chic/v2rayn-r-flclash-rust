# R4-13.S14 evidence (KcpItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-046..051（Mtu、Tti、UplinkCapacity、DownlinkCapacity、CwndMultiplier、MaxSendingWindow）。
- Rust 消费者断言：`codegen::tests::r4_13_s14_kcp_item_reaches_codegen`，含 `CwndMultiplier` 0→1 修正；生成器侧 xray kcp 使用 `settings.kcp`。
- Dart 合同：`test/r4_13_s14_contract_test.dart`（2 例，pass）：保存→重开；缺省 1350/50/12/100/1/2097152。
- 命令：见 `../R4-13/commands.log`。
