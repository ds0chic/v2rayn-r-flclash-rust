# R4-13.S06 evidence (CoreTypeItem)

- 状态：verified（配置链）。
- 字段：FLD-CFG-093/094（ConfigType、CoreType，绑定行列表）。
- 消费者（已存在）：`crates/domain/src/settings.rs::AppSettings::core_for` → `crates/application/src/engine.rs` 选核决策（含 `ConfigType::Custom` 的 CoreTypeItem 绑定，engine.rs:2597..2701）。
- Rust 消费者断言：`codegen::tests::r4_13_s06_core_type_item_reaches_core_selection`：默认 Vless→Xray；绑定 Vless→SingBox 后 `core_for` 返回 SingBox，未绑定型仍回退 Xray。
- Dart 合同：`test/r4_13_s06_contract_test.dart`（2 例，pass）：行保存→重开；缺省为 null 由窗口 init 补齐。
- 命令：见 `../R4-13/commands.log`。
