# SP-23 追加说明（SP-28 只读复核，2026-10-06/基线 92d46dd）

本文件为追加批次说明，不改 SP-23 已有 18 个 FLD 条目与 README 结论。

- SP-28 只读复核确认：18 个 FLD 文件仍在；G-01..G-09 所引符号在本基线无漂移
 （`config_codegen/src/input.rs:562/469`、`xray/dns.rs:381`、`application/src/settings.rs:93`、
  `application/src/codegen.rs:461` 及测试内调用 1076/1092、`engine.rs:4840`）。
- 未登记缺口 34 个（2 internal + 32 leaf）已全部记入
  `docs/evidence/stable-port/SP-28/field-gap-matrix.csv`，建议 owner 见该表与 SP-28 README。
- 冲突口径重申：`settings-current-180.csv` 180 行 `actual_effect_verified` 全为 false；
  implemented 仅表示静态可达，不代表任一字段生效。每叶仍须逐实例完成正式入口/重开/最终消费者证据。
