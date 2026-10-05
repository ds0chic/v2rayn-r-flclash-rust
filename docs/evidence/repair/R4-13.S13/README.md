# R4-13.S13 evidence (Inbound)

- 状态：verified（配置链）/blocked（真实监听）。
- 字段：FLD-CFG-035..045（LocalPort、Protocol、UdpEnabled、SniffingEnabled、DestOverride、RouteOnly、AllowLANConn、NewPort4LAN、User、Pass、SecondLocalPortEnabled）。
- Rust 消费者断言：`codegen::tests::r4_13_s13_inbound_reaches_codegen`，证明除 caller 拥有的 `local_port` 外全部字段进入 `CodegenSettings.inbound`（`settings_from_app`）。
- Dart 合同：`test/r4_13_s13_contract_test.dart`（2 例，pass）：首行字段保存→重开；默认一条 socks 监听。
- blocked：真实 SOCKS/mixed 监听端绑定需要隔离环境；本批不绑定、不触 10808。
- 命令：见 `../R4-13/commands.log`。
