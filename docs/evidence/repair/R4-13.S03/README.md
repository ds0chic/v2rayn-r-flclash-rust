# R4-13.S03 evidence (ConfigRoot)

- 状态：verified（配置链）。
- 字段：FLD-CFG-001..025（IndexId/SubIndexId + 23 个 ConfigItems 顶层组）。
- Dart 合同：`test/r4_13_s03_contract_test.dart`（2 例，pass）：根文档含全部 25 组；跨 4 组编辑保存→重开后可见。
- 消费者：整个根文档即持久化数据链；未知键经 `ExtraMap` 保留（domain `settings.rs` 既有测试）。
- 命令：见 `../R4-13/commands.log`。
