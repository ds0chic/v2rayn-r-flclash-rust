# R4-13.S20 evidence (SpeedTestItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-106..113（SpeedTestTimeout、SpeedTestUrl、SpeedPingTestUrl、MixedConcurrencyCount、IPAPIUrl、UdpTestTarget、SpeedTestPageSize、SpeedTestDelayInterval）。
- Rust 消费者断言：`codegen::tests::r4_13_s20_speed_test_reaches_engine` → `crates/application/src/speedtest.rs::SpeedTestSettings::from_item`，证明 timeout/concurrency `<10` 修正与空 URL 回退到内置；`engine.rs` 通过 `speed_ping_test_url` 消费。
- Dart 合同：`test/r4_13_s20_contract_test.dart`（2 例，pass）：8 字段保存→重开；缺省回退。
- 命令：见 `../R4-13/commands.log`。
