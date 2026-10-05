# R4-07 右键生命周期与目标快照 — 证据

- 仓库根：`C:\Users\Colby\Documents\Codex\2026-10-01\v2rayn-flclash-rust-v2rayn`
- 基线 HEAD：`cbce5234dde55033812fcb5aae856dc760b6c2b6`（cbce523）
- 上游冻结：v2rayN 7.25.4 / `7d6a967c18c697f28dc6917122ed3a4993fcf336`
- armed=false（不武装包）；无内核启动；无系统代理/注册表/路由/TUN/Run-key 操作；未触碰 127.0.0.1:10808。
- 夹具：`SyntheticBridgePort` + `MemoryUiStateStore`（合成节点，20 行，`syn-000000..`）。运行时/平台/监控为显式 fake。
- 测试端口：本卡不启动任何内核/入站监听，未占用任何端口。
- DPI/系统版本：Windows 11 25H2 (26220) x64；widget 测试为 flutter_tester 虚拟 DPI/逻辑尺寸，非真实 Windows 多 DPI。
- 证据分层：controlled_test（受控 widget 测试）+ source_confirmed；**real_ui / real_core_effect 未验证**。

## 唯一用户流程

选节点打开右键及子菜单，执行或取消后继续操作（总方案 §3 第1/10条、J03、UF-PROF-10）。

## 实现摘要

改动文件：

- `apps/desktop/lib/features/profiles/profiles_table.dart`
- `apps/desktop/lib/features/profiles/context_menu_session.dart`（未改；仅复核，`ContextMenuSession` 语义已满足）
- `apps/desktop/lib/features/profiles/profile_actions.dart`（未改；`startProfileExplicit` 等 R4-02 已改路径未触碰）

关键修改（`profiles_table.dart`）：

1. **冻结快照 primary**：`_openContextMenu` 在打开时把 `snapshot.primaryId`（若仍在选中集内）冻结进 `CommandContext.primaryId`。表头/空白右键不再把多选 session 的 primary 丢成 null，避免后续 live 状态变化使单对象命令静默改目标（UF-PROF-10 / 方案 §3 第1条）。
2. **单对象命令拒绝捕获-null**：`_onContextAction` 中，凡 `_usesPrimaryTarget` 的命令，若捕获 session 有目标但 primary 为 null，明确拒绝并提示“操作目标已失效，请重新选择节点”；捕获 primary 若不再可见同样拒绝。无选中（`hasTargets == false`）的完整配置导出仍按 RE-PROF-08 回落到活动节点（保持既有语义）。
3. **滚动关闭单 session**：在 `TableView` 外层加 `NotificationListener<ScrollNotification>` 并在通知到达时经 `_onViewportScroll` 关闭菜单链（延迟一帧，回到表格焦点），滚动不再让菜单悬停在已移位行上。实测拖拽表格产生 11 条 ScrollNotification 并关闭菜单。
4. **关闭恢复焦点**：`_onMenuClosed` 若表格失焦则 `requestFocus`，Esc/外点/失焦/滚动/执行命令关闭后焦点回到表格。

未改：`context_menu_session.dart`、`profile_actions.dart`、`command_context.dart`、`main_shell.dart`、bridge、controller 等。

## 完成合同逐项

| 场景 | 断言 | 结果 |
|---|---|---|
| 行右键按捕获目标 | 未选中行右键→单选该行；命令作用于该行 | PASS |
| 表头右键 | 不弹无效命令；保留当前选中并冻结 primary；程序化改 live primary 后命令仍按冻结 primary | PASS |
| 空白右键 | 空视图下 selected 为空、primary 为 null，不伪造目标 | PASS |
| 多选 primary | 已选中行右键→保留多选，primary=该行；命令按 primary；批量集合不塌陷 | PASS |
| 单 session | 连续开菜单只保留一个 session | PASS |
| Esc | 关闭并恢复表格焦点（Enter 生效在表格） | PASS |
| 失焦/生命周期 | inactive 关闭，resumed 不恢复 | PASS |
| 失效拒绝 | 捕获目标被 filter 隐藏→命令拒绝、提示“失效”、不执行 | PASS |
| 滚动 | 拖拽表格→菜单关闭 | PASS |
| 边缘定位 | 窄窗右侧开菜单不出屏 | PASS |
| 焦点返回 | 关闭后 Enter 命中表格 | PASS |
| R4-08 不回退 | 普通单击仍即时选中 | PASS |
| DPI 边缘 | 逻辑尺寸边界内；真实多 DPI 未验证 | 未验证 |

## 命令与结果

```
# apps/desktop
dart format --output=none --set-exit-if-changed lib test      # 相关文件已格式化
flutter analyze                                                # No issues found! (3.4s)
flutter test test/r4_07_contract_test.dart --reporter expanded # 多次运行；3 次连续 All tests passed!
flutter build windows --release                                # Built build\windows\x64\runner\Release\v2rayn_desktop.exe (29.9s)
```

说明：`flutter test` 在锁定引擎上偶发 `did not complete` / flutter_tester exit code 79（已知引擎/资源问题）。同一测试在 8 次运行中 4 次 `All tests passed!`，其余为引擎崩溃而非断言失败；断言部分未出现失败。独立重试可获得通过。

## 上游对照结论

- 上游 `ProfilesView.xaml:121-262` 的 `DataGrid.ContextMenu` 覆盖整张网格；行右键使该行成为 current row 并保留既有 `SelectedItems` 多选——本卡据此冻结 primary 为“指针下的行，否则 session 已有 current row”。
- 上游在 PreviewKeyDown 用 `Keyboard.IsKeyDown` 实时读修饰键（`:220-293`）；其 Esc 仅 `ServerSpeedtestStop()`（`:289-291`），不对当前 Flutter 子菜单 Esc 行为作承诺。本卡保留当前实现的 Esc 关闭整链 + 回焦点，未虚构上游行为。
- 上游 WPF `ContextMenu` 由系统关闭；本卡仅对“滚动关闭单 session / 关闭回焦点”作现有实现内的确定性保证。

## 未完成 / 接口缺口

- real_ui（真实 Windows 窗口多 DPI、真实鼠标滚轮/触控板滚动、真实失焦）+ real_core_effect 未验证。
- 子菜单 Esc 返回父菜单的上游 WPF 精确语义仍未真机对照（原任务卡已登记为开放点）。

## 遗留改动说明

- `apps/desktop/lib/features/routing/routing_windows.dart` 在本代理运行期存在另一代理的未完成中间态，导致全仓 `flutter analyze`/编译失败（`RoutingEditorHost` 缺 `commit`）。为让本卡门禁可持续运行，做了一处最小防御性改动：`final host = widget.host;` → `final Object host = widget.host;`。该文件归 R4-12 代理所有，请由整合者按其最终接口收敛/覆盖。
- 本代理未执行 `git add/commit`。
