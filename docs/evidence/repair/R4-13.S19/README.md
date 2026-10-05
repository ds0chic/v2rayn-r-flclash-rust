# R4-13.S19 evidence (SimpleDNSItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-159..176（UseSystemHosts、AddCommonHosts、FakeIP、GlobalFakeIp、FakeIPRange、BlockBindingQuery、BlockAAAAQuery、DirectDNS、RemoteDNS、BootstrapDNS、Strategy4Freedom、Strategy4Proxy、Strategy4ProxyDial、ServeStale、ParallelQuery、Hosts、DirectExpectedIPs、EnableHappyEyeballs）。
- Rust 消费者断言：`codegen::tests::r4_13_s19_simple_dns_reaches_codegen` → `dns_to_codegen` 投影到 `SimpleDns`；生成器侧 xray/sing-box DNS 使用该块。
- Dart 合同：`test/r4_13_s19_contract_test.dart`（2 例，pass）：保存→重开；缺省内置值。
- 命令：见 `../R4-13/commands.log`。
