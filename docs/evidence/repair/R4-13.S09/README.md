# R4-13.S09 evidence (GrpcItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-052..055（IdleTimeout、HealthCheckTimeout、PermitWithoutStream、InitialWindowsSize）。
- UI：上游无控件（与冻结 `OptionSettingWindow.xaml` 一致），仅持久化编辑。
- Rust 消费者断言：`codegen::tests::r4_13_s09_grpc_item_reaches_codegen` → `settings_from_app` 投影；生成器侧由 `config_codegen` xray/sing-box gRPC 测试覆盖。
- Dart 合同：`test/r4_13_s09_contract_test.dart`（2 例，pass）：保存→重开；缺省回退 60/20/false/0。
- 命令：见 `../R4-13/commands.log`。
