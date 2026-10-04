# R4-00 冻结证据与完整库存 — 基线

日期：2026-10-05。本卡不修改生产实现，只锁定基线、核对库存、登记验证口径。

## 锁定

| 项 | 值 |
|---|---|
| 原版基线 | v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336` |
| 应用基线（审查基线） | `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`（HEAD，本卡提交前） |
| 当前 Windows 包构建提交 | `73da06eb52a5989aeb31f0071b7ef03a72c4bd19`（`git_dirty=false`、`smoke_armed=false`） |
| 包哈希 | zip `45a702b7527843fe4344c11260d6ae2e2510700f1654880f09dfd9dceb01484c`；setup `a8fd08e29e0331ff8fb9cb15601cc5f0b1c32a7b5f52e172ad947b7e22608b64`（与 `dist/SHA256SUMS` 一致） |
| 工具链 | Flutter 3.47.5 / Dart 3.13.4；Rust 1.98.1；FRB 2.13.0；Windows 11 25H2 x64；宿主 DPI 96 |

## 库存核对

- `coverage.csv`：844 行；`owner_task` 无空值；`repair_status` 全部 `identified`（本卡不提升任何行）。
- 集合与 `execution-manifest.json` 一致：features 110、items 559（actions 185 + fields.settings 180 + fields.entities 158 + layouts 36）、window_inventory 53、fmt_formats 17、config_types 15、core_capability_matrix 15、enum_scan 15、existing_tests 14、hotkey_scope_rules 8、migration_paths 8、storage_tables 8、scheduled_tasks 7、scheduled_and_background 6、reference_semantics 6、main_layouts 3；合计 844。
- 工作包 35、参数卡 41（R4-13.S01–S24、R4-18.P01–P11、R4-33.O01–O06），共 76 张执行卡。
- 本轮 37 项缺陷证据：`docs/evidence/user-flow-audit-2026-10-05/`（D01–D37 在方案 §2 表）。

## 验证口径（本计划全程）

- 状态只用 `identified / implemented / verified / preserved_only / blocked / not_applicable`；未运行写未运行、未实测写未验证。
- 每卡：先读冻结源码并建立会在现版失败的行为断言（断言不得按现有错误实现改预期），再最小修改；走 UI→真实 FRB→Rust→持久化→重开→配置/核心/平台效果。
- 测试端口先探测且 ≥11808；不碰 10808、宿主代理/路由/TUN、自启注册表、用户秘密；OS 副作用须授权隔离环境。
- 假桥只作故障注入；不得用 AUTO_SMOKE、预置 active、开发 Xray 环境变量、直接调用控制器替代被测 UI 入口。
- `work/`、`outputs/` 只读；`compat/` 只追加不降分母。

## 待办与边界

- 生产编辑：0（本卡仅文档）。`execution-manifest.json` 的 `production_edits=false` 保持。
- 未验证：全部 844 条维持 `identified`；真实 TUN/发行源/六平台等按卡内前置登记 blocked。
