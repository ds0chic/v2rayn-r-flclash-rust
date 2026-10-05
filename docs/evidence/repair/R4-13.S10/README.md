# R4-13.S10 evidence (GuiItem)

- 状态：implemented（配置链部分）。
- 字段：FLD-CFG-056..064（AutoRun、EnableStatistics、DisplayRealTimeSpeed、KeepOlderDedupl、AutoUpdateInterval、TrayMenuServersLimit、EnableHWA、EnableLog、RootCertProvider）。
- Rust 消费者断言：`codegen::tests::r4_13_s10_gui_item_reaches_codegen` → `EnableStatistics`/`DisplayRealTimeSpeed` 投影到生成器 `gui`。
- Dart 合同：`test/r4_13_s10_contract_test.dart`（3 例，pass）：flags 保存→重开；越界 RootCertProvider 归一为 `system`；缺省回退。
- 其它字段消费者：`AutoRun`→Dart 自启写入（`settings_controller._writeAutostart`）；`AutoUpdateInterval`/`TrayMenuServersLimit` 为应用级；`EnableHWA`/`RootCertProvider` 涉及 OS（HWA/证书安装）=blocked，未实测。
- 命令：见 `../R4-13/commands.log`。
