# ADR T21-E — 剪贴板导入的 Dart 侧兼容层与工具条横向滚动

状态：accepted（临时候选方案；待 Rust 侧批量落库补丁后回退）
日期：2026-10-02
关联：`crates/bridge_api/src/api/subs.rs::import_from_text`，
`apps/desktop/lib/features/subs/import_persistence.dart`，
`apps/desktop/lib/shared/widgets/horizontal_toolbar.dart`

## 背景

1. 真实桥 `import_from_text` 仅在 `subid` 非空时落库，导致无订阅的剪贴板导入
   表面成功、实际 0 行（用户实测“点了没法导入”）。
2. 1200 宽下节点工具条单行超宽，尾部按钮被裁切且无滚动提示。

## 决策

### D1 导入落库：Dart 兼容层（本轮不碰 crates）

在 `persistImportedProfiles` 中对 `importFromText` 返回的 `profiles` 逐条调用
`BridgePort.saveProfile`，每次重新读取 `profileRevision()`。理由：

- 任务边界明确禁止本轮修改 `crates/**`；`ImportResult.profiles` 已携带完整解析结果，
  UI 可用既有乐观保存 seam 落库。
- 不动 UI→bridge 契约，既有测试与调用方零破坏。
- 代价：每条一次 FFI + revision 往返；导入量受 `MAX_IMPORT_ITEMS` 限制，剪贴板
  场景通常很小。

回退条件：Rust 增加原子批量导入并持久化后，删除 `persistImportedProfiles`
调用改为直接 `reload()`。最小补丁见 `docs/evidence/T21-E.md` §6。

### D2 工具条：横向滚动 + 常显滚动条

`HorizontalToolbar` 用 `SingleChildScrollView(scrollDirection: horizontal)` 包住
Row，并叠加 `Scrollbar(thumbVisibility: true, trackVisibility: true)`。理由：

- 满足“所有按钮在 1000/1200 宽可达”，保留原按钮与快捷键 tooltip；
- 常显滚动条把“被裁切”变成“可滚动”的明确提示；
- 相比收进「更多」菜单，改动小、无按钮重排风险。

### D3 表格列自适应

`ProfileColumn` 增加 `flexible`/`minWidth`；`fittedColumnWidths` 在可用宽度内让
关键列吸收剩余宽度，不足时回退最小宽并由 `TableView` 横向滚动。用户拖动列宽仍写入
`width` 并持久化，语义不变。

## 备选

- 直接改 Rust 落库（被本轮边界否决，但为目标终态）。
- 工具条「更多」下拉（改动大，暂不采用）。
- 工具条 `Wrap` 换行（改变原版单行布局，暂不采用）。

## 影响

- 新增 `apps/desktop/pubspec.yaml` 的 `integration_test` dev 依赖。
- 新增纯函数/可复用 widget，便于单测。
- 无跨模块依赖倒置；未触碰 runtime/平台/内核路径。
