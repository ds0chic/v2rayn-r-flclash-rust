# FIX-16D 窗口尺寸/列宽统一状态源（按 TypeName 恢复）

状态：`implemented`（状态源 + 迁移 + 尺寸/列宽保存重开 + 桌面字体 1.5x widget 可达已在本机 widget/单元测试验证；真实 Windows 窗口绑定未接线，见「未完成」）。

任务 ID：FIX-16D（来源 `docs/tasks/FIX-16.md` 后续卡表；`docs/evidence/parity-review-2026-10-03/repair-queue.md` 第 41/43 行「布局和列宽用一致可迁移/备份状态源，按窗口 TypeName 恢复尺寸」；关联 FIX-16C-2 / LAY-CLASHCN-002）。

本次唯一用户流程：任意窗口/表格的尺寸与列宽经同一可迁移状态源（`ui_state.json`）持久化——按上游 `GetType().Name` 写入 `WindowGeometry`，按表名（节点表 `profiles` / Clash 连接表 `clash_connections`）写入同一 `ColumnLayout` 形状；关闭重开或从备份文档恢复后尺寸/列宽不丢；在 1.5x 桌面字体下设置窗口不溢出且主按钮可点击。

前置任务及已验证证据：FIX-16（五页分组 + 字段消费矩阵）、FIX-16B（源解析消费者）、FIX-16C（ClashUIItem 刷新/排序消费者）均不回退。上游基准：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

对应 feature / field / action / layout ID：`FLD-CFG-068`（MainGirdHeight1）、`FLD-CFG-069`（MainGirdHeight2）、`FLD-CFG-082`（UiItem.WindowSizeItem）、`FLD-CFG-156/157/158`（WindowSizeItem.TypeName/Width/Height）、`FLD-CFG-080`（UiItem.MainColumnItem）、`FLD-CFG-136`（ClashUIItem.ConnectionsColumnItem）、`LAY-MAIN-007`（主窗口布局恢复与持久化）、`LAY-PROFILES-003`（节点表列状态持久化）、`LAY-CLASHCN-002`（连接表列状态持久化）。

必读上游文件、符号和固定 commit（`7d6a967`）：
- `v2rayN/ServiceLib/Models/Configs/ConfigItems.cs:266`（`WindowSizeItem { TypeName, Width, Height }`）、`:91/92`（`UiItem.MainGirdHeight1/2`）、`:105`（`WindowSizeItem` 列表）、`:178`（`MainColumnItem`）、`:219`（`ConnectionsColumnItem`）
- `v2rayN/ServiceLib/Handler/ConfigHandler.cs:2963`（`GetWindowSizeItem`：按 `TypeName` 匹配且 `Width/Height > 0`）、`:2974`（`SaveWindowSizeItem` upsert）、`:2989`（`SaveMainGirdHeight`：`(int)(h + 0.1)`）
- `v2rayN/v2rayN/Base/WindowBase.cs:14/35` 与 `v2rayN.Desktop/Base/WindowBase.cs:26/58`（Loaded 恢复、Closed 保存，`GetType().Name`）
- `v2rayN/v2rayN/Views/MainWindow.xaml.cs:347/351/355`（保存窗口尺寸与两个星值）、`v2rayN.Desktop/Views/MainWindow.axaml.cs:374-399`
- `v2rayN/v2rayN/Views/ProfilesView.xaml.cs:328-381`（`MainColumnItem` 按 Index 恢复 Width/DisplayIndex）、`ClashConnectionsView.xaml.cs:74-125`（`ConnectionsColumnItem`）

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`ui_state.json` 文档 + 各窗口/表格当前尺寸/列宽。
- 输出：`UiStateStore` 的新原语 `saveWindowGeometry`/`loadWindowGeometry`（按 `TypeName`）与 `saveColumnLayout`/`loadColumnLayout`（按 `ColumnTable.profiles`/`clashConnections`）；Rust `application::settings::{get_window_size,save_window_size,save_main_grid_height}` 提供 `guiNConfig` 树的权威 upsert。
- 错误：I/O 失败静默保留旧文档（沿用既有 best-effort 语义）；退化尺寸（`Width<=0 || Height<=0`）视为未保存，回退默认。
- 取消：窗口未写尺寸即不产生新行。
- 权限：仅本机 UI + 文件/FRB/SQLite；不启动内核、不写系统代理/TUN、不监听端口。
- 持久化：`ui_state.json`（`window_geometry` / `column_layout` / `clash_connections_column_layout` / `meta`），旧 `window`、`column_widths` 扁平键由 `migrateLegacyUiState` 幂等折叠，键保留不丢。
- 生效：尺寸/列宽 `immediate`；窗口几何需在窗口创建/关闭处调用（`desktop_integration`/runner，非本卡所有权）。

允许修改的模块：`apps/desktop/lib/features/profiles/{ui_state_store.dart,column_settings_dialog.dart}`、`apps/desktop/test/**`、`crates/application/src/settings.rs`（settings 树纯逻辑，不动 engine.rs）、`docs/evidence/UX-PARITY-FIX-16D/**`、`docs/evidence/UX-PARITY-FIX-16/field-matrix.md`、本卡、`compat/`（仅追加）。未改 `main_shell.dart`/`app.dart`/生成文件/`profiles_controller.dart`/`profiles_table.dart`/`profile_*.dart`/`features/{settings 被并行占用文件,subs,runtime,monitor,update,backup,routing}`/`crates/updater`。

禁止改变的已有行为：FIX-16/16B/16C 的字段消费与草稿保存语义、`settings-*` 键、`ui_state.json` 既有 `layout`/`theme`/`status_ui`/`column_layout` 键与 `column_widths` 兼容读取、节点表列键（稳定 ExName，不用本地化标签）。

测试夹具和原版预期：`MemoryUiStateStore`（内存文档）；原版预期：按 `TypeName` 命中且 `Width/Height>0` 才恢复；upsert 不产生重复行；退化尺寸回退默认；节点表/连接表列各存各的段互不覆盖；1.5x 字体下设置窗口无 overflow 且「保存/应用」可点击。

本次必须通过的命令（实际运行见「本轮实际结果」）：
- `dart format`（改动文件）
- `flutter analyze`（改动文件）
- 针对性 `flutter test`（3 个新文件 + `ux_space01_column_persistence`、`t21e_dialogs_responsive`、`fix16c_clash_ui_config`、`fix16_settings_field`、`t05_profiles_ui`）
- Rust：`cargo fmt -p application -- --check`、`cargo test -p application settings --locked`
- 真实窗口：未运行（见 FIX-16F）

证据文件位置：`docs/evidence/UX-PARITY-FIX-16D/README.md`、`observations.json`；字段状态并入 `docs/evidence/UX-PARITY-FIX-16/field-matrix.md`。

完成条件：窗口尺寸与两类表格列宽使用一致、可迁移且备份不丢的状态源；迁移幂等；节点列宽在 UI 可编辑并重开恢复；1.5x 字体下不溢出/按钮可达；既有测试不回退；门禁通过。已达成（真实 Windows 窗口绑定/集成除外）。

接口缺口处理（登记，不自行削减）：
- 主窗口与各子窗口的 `window_manager` 绑定（Loaded 恢复 / Closed 保存，按 `GetType().Name`）在 `features/{settings/desktop_integration,platform_*}`, `main_shell.dart` 与 `windows/runner`，非本卡所有权 → 交回根代理/FIX-16F 接线；本卡只交付状态源。
- `connections_view.dart` 消费者未接线：`ColumnLayout`/`saveColumnLayout(ColumnTable.clashConnections)` 已就绪，但 `features/monitor/**` 本卡禁止修改 → FIX-16C-2 后续接线。
- 若需 Flutter 直接读写 `guiNConfig` 的 `UiItem.WindowSizeItem`（而非本地 `ui_state.json` 草稿），需新 FRB 函数；本卡未改生成文件，登记后由根代理重生成。

## 本轮实际结果

- `ui_state_store.dart`：新增 `WindowGeometry`（`TypeName/Width/Height/MainGirdHeight1/2/Orientation`）、`ColumnLayout`、`ColumnTable`、`WindowTypeNames`；`UiStateStore` 提供 `loadColumnLayout/saveColumnLayout/loadWindowGeometries/loadWindowGeometry/saveWindowGeometry/migrateLegacyUiState`；两个实现改为 `extends` 以继承具体方法；旧 `window`/`column_widths` 扁平键幂等迁移且不删。
- `column_settings_dialog.dart`：补列宽编辑（上游 `MainColumnItem.Width`，±10 步进，40..600 与 `ProfilesController.resizeColumn` 一致），稳定键持久化；提示统一状态源。
- `crates/application/src/settings.rs`：新增 `get_window_size`/`save_window_size`/`save_main_grid_height`（对照 `ConfigHandler`）+ 3 个单元测试。
- 测试：`test/fix16d_ui_state_store_test.dart` 9/9、`test/fix16d_column_dialog_test.dart` 1/1、`test/fix16d_layout_reachability_test.dart` 1/1；回归 5 文件全过（`t05_profiles_ui` 合跑偶发 `did not complete`，单跑通过）。
- Rust：`cargo fmt -p application -- --check` 0 差异；`cargo test -p application settings --locked` 20 通过 / 0 失败。
- 未改 Rust engine、生成文件、被并行占用的 `features/monitor|settings 其它`；未跑全量 workspace/`flutter build windows`。

## 后续（登记，未在本卡完成）

| 项 | 范围 | 未完成原因 |
|---|---|---|
| FIX-16D-2 | 各窗口 `window_manager` Loaded/Closed 绑定（按 TypeName 恢复尺寸） | 需改 `desktop_integration`/runner，非本卡所有权 |
| FIX-16C-2 | `connections_view.dart` 接入 `clash_connections_column_layout`（Index/Width 恢复 + 写回） | `features/monitor/**` 并行占用 |
| FIX-16F | 真实 Windows 窗口集成测试（尺寸/列宽重开 + 1.5x 字体可达） | 环境抖动，留给根代理 |
| FRB | 读写 `UiItem.WindowSizeItem`/`MainColumnItem` 的桥接函数 | 需重生成 `frb_generated`，本卡不动生成文件 |
