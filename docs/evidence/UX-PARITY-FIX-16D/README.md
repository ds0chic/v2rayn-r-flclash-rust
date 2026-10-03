# UX-PARITY-FIX-16D evidence

FIX-16D：窗口尺寸/列宽统一状态源（按窗口 TypeName 恢复）。

- `README.md`（本文件）：范围、上游对照结论、实际命令与结果、缺口。
- `observations.json`：改动文件、命令结果、状态统计、blockers、约束自证。

## 范围

1. **TypeName 窗口尺寸**：`ui_state.json` 的 `window_geometry` 段按键为上游 `GetType().Name`，形状对齐 `WindowSizeItem { TypeName, Width, Height }`，并扩展 `MainGirdHeight1/2`、`Orientation`（`EGirdOrientation` 0/1/2）。`WindowTypeNames` 登记 `WindowBase<...>` 全部 14 个窗口键。
2. **列宽统一状态源**：节点表（`LAY-PROFILES-003`）与 Clash 连接表（`LAY-CLASHCN-002`）共用同一 `ColumnLayout { order, visible, widths }`；节点表沿用现网段 `column_layout`，连接表用 `clash_connections_column_layout`，互不覆盖。
3. **迁移/备份不丢**：`migrateLegacyUiState()` 幂等把旧 `window`（扁平或 TypeName 键）折入 `window_geometry`、旧 `column_widths` 折入 `column_layout`，并写 `meta.schema_version=2`；旧键保留。`loadDocument()` 即为备份文档，可直接 `saveDocument()` 恢复。
4. **桌面字体/DPI**：设置窗口在 1000x700 + 1.5x 文本缩放下无 overflow，「保存/应用」可命中；列编辑对话框同条件可用。

## 上游对照结论

| 上游机制 | 本实现 | 结论 |
|---|---|---|
| `WindowSizeItem{TypeName,Width,Height}`（ConfigItems.cs:266） | `WindowGeometry` 按 TypeName 键 | 形状一致 + 扩展星值/方向 |
| `ConfigHandler.GetWindowSizeItem`（:2963，`Width/Height>0` 才返回） | `WindowGeometry.isValid` + `get_window_size` | 退化尺寸视为未保存 |
| `ConfigHandler.SaveWindowSizeItem`（:2974 upsert） | `save_window_geometry`/`save_window_size` | upsert，不重复行 |
| `ConfigHandler.SaveMainGirdHeight`（:2989） | `WindowGeometry.mainGridHeight1/2` + `save_main_grid_height` | 一致 |
| `WindowBase.cs:14/35` Loaded 恢复 / Closed 保存（`GetType().Name`） | 状态源就绪，绑定缺（缺口） | 绑定登记 FIX-16D-2 |
| `ProfilesView RestoreUI/StorageUI`（MainColumnItem 按 Index） | `ColumnLayout` + 列宽编辑器 | 状态源就绪；Index 排序由既有控制器 keyed 逻辑承担 |
| `ClashConnectionsView ConnectionsColumnItem`（:74-125） | `ColumnLayout(ColumnTable.clashConnections)` | 状态源就绪；消费者登记 FIX-16C-2 |
| WPF `App.xaml WindowGlobal` DPI/字体 | Flutter `MediaQuery.textScaler` | 1.5x widget 断言 |

## 实际命令与结果

| 命令 | 结果 |
|---|---|
| `dart format`（5 文件） | 5 changed（已格式化） |
| `flutter analyze`（5 文件） | No issues found |
| `flutter test test/fix16d_ui_state_store_test.dart` | 9 passed |
| `flutter test test/fix16d_column_dialog_test.dart` | 1 passed |
| `flutter test test/fix16d_layout_reachability_test.dart` | 1 passed |
| `flutter test`（回归 5 文件） | `ux_space01_column_persistence`/`t21e`/`fix16c`/`fix16_settings` 通过；`t05_profiles_ui` 合跑 `did not complete`，单跑通过（已知抖动，非回归） |
| `cargo fmt -p application -- --check` | 0 差异 |
| `cargo test -p application settings --locked` | 20 passed / 0 failed |

## 状态口径

- `implemented`：状态源、迁移、列宽编辑、Rust 助手、widget 断言。
- `verified`：以上命令在本机实际运行通过。
- 未接线（登记，`identified`）：`window_manager` 各窗口绑定（FIX-16D-2）、`connections_view` 消费者（FIX-16C-2）、FRB 读写 `guiNConfig` 树。

## 未完成（后续卡）

FIX-16D-2（窗口绑定）、FIX-16C-2（连接表消费者）、FIX-16F（真实 Windows 窗口集成）、FRB（`WindowSizeItem`/`MainColumnItem` 桥接）。
