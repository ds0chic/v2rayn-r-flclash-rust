# UX-PARITY-FIX-08C — 路由草稿余量证据

状态：`implemented`（widget + Rust 单测通过；未做原版实机双窗口逐事件对照，
故不写 `verified`）。

对应任务卡：`docs/tasks/FIX-08C.md`；上游冻结 commit
`7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 本次范围（唯一用户流程）

路由规则窗口：外部模板导入 → 编辑草稿 → 导出 → 重开。

- 外部模板导入：文件（原生 `file_selector`）/剪贴板/URL → 当前草稿；解析
  失败或取消不碰草稿。
- Rust 侧导入/导出 casing 对齐上游 `RulesItem`（camelCase、去 Id）。
- 出站选择器排除 `ConfigType.custom`（对齐 `SelectProfileAsync`）。
- 规则删除确认（`RemoveServer`），取消不删。
- 不改变 FIX-08 已提交的草稿 ID 稳定 / 删空可存 / 未知键保留语义。

## 实际命令与结果

Rust（`C:\Users\Colby\.cargo\bin\cargo.exe`，workspace 根）：

- `cargo fmt -p application -p bridge_api -- --check` → exit 0
- `cargo clippy -p application -p bridge_api --all-targets --locked -- -D warnings` → 0 warning
- `cargo test -p application --lib routing` → 5 passed
  （新增：`import_rejects_invalid_and_empty`、
  `import_accepts_upstream_camel_case_and_keeps_extras`、
  `export_is_camel_case_without_id_and_round_trips`）
- `cargo test -p application --test t11_routing_dns`（包含在
  `cargo test -p application` 中）→ `routing_import_export_round_trip` 等 20 passed
- `cargo test -p bridge_api --locked` → 43 passed
- 备注：同次 `cargo test -p application` 中 `t15_monitor.rs::clash_api_service_reads_selects_and_closes`
  FAILED，属并行子代理正在改的 monitor 区域，与本卡无关（本卡未触碰 monitor/
  engine）。

Flutter（`apps/desktop`，`C:\Users\Colby\toolchains\flutter\bin\flutter.bat`）：

- `dart format lib\features\routing test\fix08c_routing_draft_test.dart test\fix08_routing_draft_test.dart`
  → 2 changed（routing_windows.dart、fix08c 测试）
- `flutter test test\fix08c_routing_draft_test.dart` → 3 passed
  （失败导入保草稿 / 删除确认取消保草稿 / 选择器排除 Custom）
- `flutter test test\fix08_routing_draft_test.dart test\fix08_draft_unit_test.dart`
  → 7 passed（FIX-08 回归）
- `flutter analyze` → 本卡改动文件 0 issue；仓库其余报错集中在
  `features/monitor/**`、`test/support/fake_monitor_bridge.dart`、
  `test/fix10b_profile_order_test.dart`（其它子代理在改），未计入本卡。

`flutter build windows --release`、真实窗口集成测试由根代理统一跑，本卡未跑。

## 上游对照结论

| 动作 | 上游（冻结） | 本卡实现 |
|---|---|---|
| 文件导入 | `ImportRulesFromFileCmd` → `BrowseRulesFileInteraction` 原生选择器 | `routing_actions.dart:pickRulesFromFile` 用 `file_selector.openFile`（json/txt） |
| 剪贴板导入 | `ImportRulesFromClipboardAsync` | `pickRulesFromClipboard`（已有）+ Rust `parse_imported_rules_compat` |
| URL 导入 | 空 Url 提示 `MsgNeedUrl`；超时/失败保草稿 | `pickRulesFromUrl`（已有） |
| 追加/替换 | `AddBatchRoutingRulesYesNo`：否→替换、是→追加，新 GUID | 导入对话框「追加/替换」，`parseImportedRuleDtos` 赋新 id |
| 导出选中 | `RuleExportSelectedAsync`：camelCase、去 Id、`IgnoreNull` | Dart `exportDraftRulesJson`；Rust `export_rules_camel`（camelCase、无 id、空值省略） |
| 删除 | `RuleRemoveAsync` 先问 `RemoveServer`，取消不删 | `_confirmRemove` 确认对话框；取消保草稿 |
| 出站选择器 | `SelectProfileAsync`：`SetConfigTypeFilter([Custom], exclude: true)` | `_selectProfile` 过滤 `ConfigType.custom` |

Rust 导入/导出 casing：FRB 面（`bridge_api::export_routing_rules`）现对
全量/选中都输出上游 camelCase，并复用 `application::export_rules_camel`；
导入经 `application::parse_imported_rules_compat`，camelCase 与存储 snake_case
均接受，`ruleType` 接受序号或名称，未知键进 `extra`。

## 未完成 / 接口缺口

- `crates/domain/src/routing.rs::export_rules` 仍为 snake_case。该文件不在本卡
  允许修改清单内，故未改；FRB 面已在 bridge/application 层对齐。若需 domain
  层也统一，请由根代理确认后修改。
- 多选/键盘 parity 仅完成可测部分（点击多选、删除确认）；Ctrl/Shift/Ctrl+A、
  T/U/D/B 快捷键绑定未实现，已在任务卡登记。
- 真实 Windows 窗口 取消/导入失败/重开 场景未跑（根代理统一）。

## 关键文件

- `apps/desktop/lib/features/routing/routing_actions.dart`
- `apps/desktop/lib/features/routing/routing_windows.dart`
- `crates/application/src/routing.rs`
- `crates/bridge_api/src/api/routing.rs`
- `apps/desktop/test/fix08c_routing_draft_test.dart`
- `apps/desktop/test/fix08_routing_draft_test.dart`
