# RR-08 / RE-PROF-11 修复证据（2026-10-04）

开始 HEAD `cbf4fa7`，工作树干净。本回合只改允许清单内文件；未占用/修改 127.0.0.1:10808，未改宿主系统代理/注册表/路由/TUN，未读用户凭据（测试仅用合成节点），未运行原版 WPF 双窗口，未 `git add/commit`。

## 范围与结论

- **RR-08（implemented）**：托盘菜单从共享 `TrayReadModel` 构建，`DesktopIntegration` 通过 `ref.listenManual` 订阅 profiles/routing/platform/runtime/settings 五个 provider，任何业务/运行/配置变化都会重建菜单与图标；节点勾选与路由勾选来自同一 active/default 状态；图标状态由 proxy/core 派生；`TrayMenuSync` 去重且异步 apply 后由监听再次刷新。未做真实系统托盘点击，故不写 `verified`。
- **RE-PROF-11（implemented）**：`showNodePicker` 恢复上游 `ProfilesSelectWindow` 合同（当前分组切换、备注/地址搜索、列 类型/备注/地址/端口/传输/TLS/订阅/延迟/速度、表头排序、自动列宽、单选/多选返回、取消、调用方 ConfigType include/exclude 约束）；组编辑器已有子项支持多选批量移除（对应 `ChildRemoveAsync` 遍历 `SelectedChildren`）。未运行原版 WPF 窗口。

## 改动文件

- `apps/desktop/lib/app/shell/tray_menu_model.dart`（新增 `TrayIconStatus`/`trayIconStatus`/`TrayReadModel`/`TraySurface`/`TrayMenuSync`，`TraySubEntry` 值相等）
- `apps/desktop/lib/app/shell/desktop_integration.dart`（`_SystemTraySurface`、`_bindTraySync`、`_syncTray`、订阅关闭；替换一次性 `_refreshTrayMenu`）
- `apps/desktop/lib/features/profiles/group_editor_dialog.dart`（重建 `showNodePicker`/`_NodePickerDialog`；`_pickNodes` 传类型排除合同；子项多选批量移除）
- `apps/desktop/test/recheck_rr08_tray_sync_test.dart`
- `apps/desktop/test/support/node_picker_harness.dart`
- `apps/desktop/test/recheck_reprof11_node_picker_test.dart`
- `apps/desktop/test/recheck_reprof11_picker_type_exclude_test.dart`
- `apps/desktop/test/recheck_reprof11_picker_type_include_test.dart`
- `apps/desktop/test/recheck_reprof11_picker_multi_test.dart`
- `apps/desktop/test/recheck_reprof11_picker_single_test.dart`
- `apps/desktop/test/recheck_reprof11_picker_cancel_test.dart`
- `apps/desktop/test/recheck_reprof11_child_batch_remove_test.dart`
- `docs/tasks/RR-08.md`、`docs/tasks/RE-PROF-11.md`（新增）
- `compat/actions.yaml`（仅追加注释条）

## 实际命令与结果

| 命令 | 结果 | 说明 |
|---|---|---|
| `flutter analyze` | No issues found (3.3s) | 全应用静态分析 |
| `flutter test test/recheck_rr08_tray_sync_test.dart` | exit 0，7 passed | 图标矩阵/去重/勾选/订阅刷新，纯单元 |
| `flutter test test/recheck_reprof11_node_picker_test.dart` | exit 0，1 passed | 渲染+分组+搜索+排序+自动列宽 |
| `flutter test test/recheck_reprof11_picker_type_exclude_test.dart` | exit 0，1 passed | exclude 合同 |
| `flutter test test/recheck_reprof11_picker_type_include_test.dart` | exit 0，1 passed | include 合同 |
| `flutter test test/recheck_reprof11_picker_multi_test.dart` | exit 0，1 passed | 多选返回顺序 |
| `flutter test test/recheck_reprof11_picker_single_test.dart` | exit 0，1 passed | 单选 gate/返回 |
| `flutter test test/recheck_reprof11_picker_cancel_test.dart` | exit 0，1 passed | 取消 null |
| `flutter test test/recheck_reprof11_child_batch_remove_test.dart` | exit 0，1 passed | 子项批量移除 |
| `flutter test test/t10_group_editor_test.dart`（重试） | exit 0，6 passed | 既有组编辑器/选择器回归 |
| `flutter test test/fix03_special_editors_test.dart`（重试） | exit 0，7 passed | 既有分派/组编辑器回归 |
| `flutter test test/reprof10_group_preview_test.dart`（重试） | exit 0，8 passed | 草稿预览回归 |
| `flutter test test/fix03c_proxy_chain_editor_test.dart` | exit 0，7 passed | 链编辑器回归 |
| `flutter test test/fix15_tray_pac_test.dart` | exit 0，8 passed | 托盘模型/PAC 回归 |
| `flutter test test/t13_platform_models_test.dart` | exit 0，10 passed | 托盘模型顺序/PAC Windows-only |
| `flutter test test/recheck_rr02_03_test.dart` | exit 0，5 passed | RR-02/03 语义保持 |

`t10`/`fix03_special`/`reprof10` 首次运行出现锁定 Flutter 构建的多 `pumpWidget` 原生资源泄漏（`did not complete [E]`）；每个失败用例单独运行通过，重试整文件全绿，与本卡改动无关（仓库 `test/support/profiles_harness.dart:18-21` 已记录该历史）。新 widget 测试按仓库惯例拆成单次 `pumpWidget` 的小文件。

## 未完成 / 接口缺口

1. 未实现真实系统托盘点击与真实图标资源切换；`_SystemTraySurface.applyIcon` 已接 `system_tray.setImage`，但当前仅打包单一图标路径，proxy/core 多图标资源未提供（登记）。
2. 选择器“延迟/速度”列已渲染，但调用方传入的候选是 `ProfileDto`（不含 `ProfileExItem` 的 delay/speed），故显示 `-`；需要把测速叠加层接入选择器（登记，RE-PROF-09 同源）。
3. 选择器分组标签在未提供 `subItems` 时退化为 `subid` 文本（无订阅备注）；组编辑器调用已传 `subItems`。
4. 未运行原版 WPF `ProfilesSelectWindow` 实机对照，也未做真实托盘/图标视觉实测。
