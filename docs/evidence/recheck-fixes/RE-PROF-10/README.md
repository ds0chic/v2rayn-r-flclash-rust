# RE-PROF-10 evidence

状态：`implemented`。修复“策略组/代理链编辑器刷新预览看已落库存量、不看当前草稿”。
2026-10-04；冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。开始 HEAD `256c3dd`（工作树干净）。

## 变更点

- `apps/desktop/lib/features/profiles/group_editor_dialog.dart`：
  - 新增纯函数 `resolveGroupPreview`，Dart 移植 Rust 生成路径 `application::groups::resolve_children`/`resolve_sub_children`（亦即上游 `GroupProfileManager.GetChildProfileItemsByProtocolExtra`）：订阅匹配优先（`SubChildItems` → `self` 解析为组自身 `subid`，只保留 leaf/Outbound，备注正则过滤，按 `IndexId` 升序），再接显式 `ChildItems` 顺序并去重。
  - `_refreshPreview` 改为读取当前草稿（`_childIds`/`_selectedSubId`/`_draft.filter`/`_draft.subid`）解析；不再调用已落库 `group_children`。`previewChildren` 仅保留为测试可选覆盖。
  - 预览区常显（含新建组），标题改为“按当前草稿解析，订阅匹配优先”，按钮文案“刷新预览”；移除“新建节点保存后可预览”。
- `apps/desktop/lib/features/profiles/profiles_controller.dart`：删除旧存量解析入口 `groupChildPreview(indexId)`（生产不再引用）。
- `apps/desktop/lib/features/profiles/profile_actions.dart`：三处 `showGroupEditor` 不再传 `previewChildren: controller.groupChildPreview`，改由编辑器按草稿解析。
- `apps/desktop/test/reprof10_group_preview_test.dart`：新增 3 个纯函数用例 + 5 个 widget 用例。

## 上游对照

- `AddGroupServerViewModel.cs:199-222`：`GetUpdatedProtocolExtra` 取当前 `ChildItemsObs`/模式/`SelectedSubItem`/`Filter` → `UpdatePreviewList` → `GroupProfileManager`。现 `_refreshPreview` 取草稿同名字段，语义对齐。
- `GroupProfileManager.cs:78-125`：`GetSubChildProfileItems`（`ProfileItems(subid)` + `IsValid() && (!IsComplexType() || Outbound) && IsRegexMatch(Remarks, Filter)`）后接 `GetSelectedChildProfileItems`。现 `resolveGroupPreview` 顺序/资格/正则一致。
- 持久化一致性：Rust `groups.rs::resolve_children` 是桥接 `group_children` 的实现，预览以同一规则在 Dart 复算，保证“保存后重开与预览一致”。差异说明：上游用 .NET 正则且有 2s 超时；Dart 用 `RegExp`，非法模式按上游 `IsRegexMatch` catch 分支返回 `true`（匹配全部）。
- 备注：上游 `GetChildProfileItemsByProtocolExtra` 未在订阅子项与显式子项间去重，Rust 生成路径 `resolve_children` 去重；本实现按生成路径去重，以对齐“保存后重开”。

## 本次实际运行

见 `runs.txt`；场景见 `observations.json`。全部通过（Dart 8 + 回归 20；`t10_groups_panel_test` 首跑 `did not complete`，单独重跑 2 passed）。未运行真实 Windows 窗口/原版实机双窗口，未跑全量 workspace/`flutter build windows`，未改宿主代理/注册表/路由/TUN，未占用 10808。
