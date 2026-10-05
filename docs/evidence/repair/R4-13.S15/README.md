# R4-13.S15 evidence (MsgUIItem)

- 状态：blocked（无消费者）。
- 字段：FLD-CFG-065/066（MainMsgFilter、AutoRefresh）。
- 发现：全仓库（排除 frb 生成）仅 `domain::MsgUiItem`、`SETTINGS_GROUPS` 与 `settings_timing` 引用；无消息窗/过滤器读取这两个字段，故无真实消费者可断言。
- Dart 合同：`test/r4_13_s15_contract_test.dart`（2 例，pass）：保存→重开；缺省保持 null。
- 处理：登记为 blocked（消费者/消息窗不在本交付范围），不伪造效果，不自行删字段。
- 命令：见 `../R4-13/commands.log`。
