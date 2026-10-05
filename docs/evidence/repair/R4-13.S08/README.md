# R4-13.S08 evidence (GlobalHotkeys)

- 状态：implemented（存储/模型）；blocked（OS 注册）。
- 字段：FLD-CFG-088..092（EGlobalHotkey、Alt、Control、Shift、KeyCode）。
- Dart 合同：`test/r4_13_s08_contract_test.dart`（2 例，pass）：热键行保存→重开；默认空列表。
- 消费链现状：`settings_controller`/`global_hotkey_window.dart`/`hotkeys.dart`/`hotkey_keycodec.dart` 承载模型与编解码；注册到 Windows 的 `RegisterHotKey` 属于 OS 副作用。
- blocked：真实 OS 热键注册/触发需要隔离授权环境（S23/R4-26 前置），本批不实测、不伪造成功。
- 命令：见 `../R4-13/commands.log`。
