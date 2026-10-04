# RE-PROF-13 — 活动节点独立视觉标记

状态：`implemented`（Dart widget/纯函数覆盖已通过；未做原版实机双窗口逐事件对照，未跑真实 Windows 窗口，故不写 `verified`）。

任务 ID：RE-PROF-13

本次唯一用户流程：在节点表设节点 A 为活动节点，再点选另一行 B 查看；A 整行/单元格保留“活动”标记，B 保留多选高亮，二者互不覆盖；浅色与深色主题都能分辨当前实际活动节点。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核结论见 `docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-13。开始 HEAD `842520b`，工作树干净。本卡只读禁止清单内文件。

对应 feature / field / action / layout ID：`F-PROFILE-014`（节点表渲染）、`ACT-PROF-010`(设置活动节点)、`LAY-PROFILES-002`（14 列）。

必读上游文件、符号和固定 commit：
- `v2rayN/Views/ProfilesView.xaml:271-279`：`DataGridCell` 样式 `DataTrigger Binding="{Binding IsActive}" Value="True"` → `Background=MaterialDesign.Brush.Primary.Light`、`Foreground=Black`、`BorderBrush=Primary.Light`（整行单元格级活动色，独立于 DataGrid 选择高亮）。
- `ServiceLib/Models/Entities/ProfileItem.cs` 的 `IsActive`（当前活动节点标志）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`ProfilesState.activeId`（来自 `bridge.getActiveProfile()` / `setActive`）与 `ProfilesState.selected`（多选集合）。
- 输出：活动行数据单元格使用独立 `activeRowFill`；活动行 handle 列绘制独立实心前导标记 `activeRowMarkerColor`；选择行继续使用 `Semantics.selectedRow`（`primaryContainer`）。
- 错误：无活动 id 时不绘制活动标记（透明标记），不伪造。
- 取消：纯渲染，无副作用。
- 权限：仅本机 UI；不启动内核、不监听端口、不写系统代理/TUN、不碰 10808。
- 持久化：活动 id 仍由既有 `setActiveProfile` 落库；本卡不改写链。
- 生效：`activeId` 变化后表格重建即更新标记；重开读取同一 `activeId`。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_table.dart,profiles_models.dart}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/RE-PROF-13-14/**`、`compat/*.yaml`（仅追加）。未改 `main_shell.dart`、`app.dart`、`frb_generated.*`、`lib/bridge/**`、`features/*` 其它域、`crates/**`。

禁止改变的已有行为：FIX-01/05/10B 已提交的选择/拖选/排序语义；`context.semantics.selectedRow` 多选色；列可画出的完成条件。

测试夹具和原版预期：合成节点（`SyntheticBridgePort`，`syn-00000x`/`Synthetic-0000x`，不下载、不连接）。原版预期：活动行整行 `Primary.Light`、选中行保留选择色、两者互不覆盖。

本次必须通过的命令/真实场景（实际运行）：
- `dart format`（改动文件）：Passed。
- `flutter analyze`：No issues found。
- `flutter test test/re_prof_13_active_marker_test.dart`：1 passed。
- `flutter test test/re_prof_13_theme_colors_test.dart`：1 passed（浅/深主题色断言）。

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-13-14/`（`README.md`、`observations.json`、`runs.txt`）。

完成条件：widget 覆盖“选 A 设活动→选 B：A 保留活动填充、B 保留选择填充、两者不等”；活动行 handle 前导标记存在；纯函数覆盖浅/深主题下活动填充与选择填充不相等；`flutter analyze` 通过。未跑真实 Windows 窗口与原版实机，保持 `implemented`。

本轮实际结果：见 `docs/evidence/recheck-fixes/RE-PROF-13-14/README.md`。
