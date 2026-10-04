# RE-PROF-11 — 恢复选择节点窗口合同与子项批量移除

状态：`implemented`（选择器分组/搜索/列/排序/自动列宽/单选多选/取消/类型 include-exclude 与子项批量移除的组件测试通过；未运行原版 WPF 窗口，故不写 `verified`）。

任务 ID：RE-PROF-11

本次唯一用户流程：从组编辑器点“选择节点...”打开选择器，按当前分组、搜索、表头排序、类型约束（调用方 include/exclude）找到目标节点，单选或多选后确定返回、取消不改草稿；组编辑器已有子项可多选并批量移除。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `cbf4fa7`。复核依据：`docs/evidence/parity-recheck-2026-10-04/profiles.md` RE-PROF-11（`group_editor_dialog.dart:527-637` 缩成复选框列表；`:312-355` 逐行移除）。已提交的 `FIX-03`（专用编辑器分流/多选添加/嵌套/五模式）、`FIX-03C`（链）、`FIX-01`（命令上下文）语义必须保持。

对应 feature / field / action / layout ID：`F-PROFILE-0xx`（选择器）、`ACT-PROF-007/008`（生成）、`ACT-PROF-016`（多选测试口径）、`LAY-PROFILES-004`、`FLG-ENT-005`（Subid）、`ENUM-007`。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/v2rayN/Views/ProfilesSelectWindow.xaml:49-199`（组 ListBox、自动列宽按钮、搜索框、DataGrid 列 类型/备注/地址/端口/传输/TLS/订阅/延迟/速度）、`ProfilesSelectWindow.xaml.cs:31-151`（`AllowMultiSelect`、`SelectionChanged`→`SelectedProfiles`、表头点击→`SortServer`、自动列宽、Enter 提交搜索/确定）、`ServiceLib/ViewModels/ProfilesSelectViewModel.cs:23-50`（`ProfileItems`/`SubItems`/`SelectedProfile(s)`/`ServerFilter`/`FilterConfigTypes`/`FilterExclude`/`MultiSelect`）、`:137-220`（`ServerFilterChanged` 仅清空时刷新、`RefreshServersBiz` 生成项/默认项/首项、`GetProfileItemsEx` 组+过滤+按 Sort）、`:257-325`（`SortServer` 双向、`SetConfigTypeFilter`）、`ServiceLib/ViewModels/AddGroupServerViewModel.cs:111-140`（`AddChildAsync` 设 `[Custom] exclude:true`+`MultiSelect`；`ChildRemoveAsync` 遍历 `SelectedChildren`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：调用方传入 `candidates`（合成/存储 `ProfileDto`）、可选 `subItems`（分组标签）、`multiSelect`、`filterConfigTypes` + `filterExclude`。
- 输出：确定的 `indexId` 列表（多选按候选顺序；单选为唯一项），或取消 `null`。
- 错误：候选为空显示“没有可选节点”；单选未选择时“确定”禁用（对应 `CanOk`）。
- 取消：取消返回 `null`，不改调用方草稿；组编辑器批量移除只在内存草稿，保存前不落库。
- 权限：仅本机 UI；不启动内核、不写系统代理/TUN、不监听端口、不读凭据。
- 持久化：选择器本身不落库；组编辑器保存经既有 `onSave`（`saveProfile`）路径。
- 生效：组编辑器 `_pickNodes` 追加去重；`_selectedChildren` 批量移除更新 `_childIds`，保存时写入 `ChildItems`。

允许修改的模块：`apps/desktop/lib/features/profiles/group_editor_dialog.dart`、`apps/desktop/lib/features/profiles/profiles_controller.dart`、`apps/desktop/test/**`、`docs/tasks/RE-PROF-11.md`、`docs/evidence/recheck-fixes/RR-08-REPROF11/**`、`compat/actions.yaml`（仅追加）。

禁止改变的已有行为：`FIX-03/03C` 专用编辑器分派与五模式/链/草稿预览语义、`FIX-01` 命令上下文、`showGroupEditor` 既有调用契约（`previewChildren` 测试 seam）、组编辑器备注/内核/模式/订阅子项/Filter/预览分区结构。

测试夹具和原版预期：合成 `ProfileDto`（两分组、多 ConfigType、重复备注不同地址），不下载、不连接、不启动核。原版预期：分组切换/搜索（Enter 提交、清空即时）/表头双向排序/自动列宽/单选多选返回/取消不改草稿；`ChildRemoveAsync` 遍历 `SelectedChildren` 批量移除。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`（仅本回合文件）
- `flutter analyze`
- 逐文件：`flutter test test/recheck_reprof11_*.dart`（选择器渲染/分组/搜索/排序/类型 include-exclude/单选/多选/取消、子项批量移除）
- 回归：`t10_group_editor_test`、`fix03_special_editors_test`、`reprof10_group_preview_test`、`fix03c_proxy_chain_editor_test`
- 真实 WPF `ProfilesSelectWindow` 实机对照：未运行。

证据文件位置：`docs/evidence/recheck-fixes/RR-08-REPROF11/`（`README.md`、`observations.json`）。

完成条件：选择器按上游恢复分组/搜索/列（地址/端口/传输/延迟/速度）/表头排序/自动列宽/单选多选返回/取消/类型 include-exclude 合同；组编辑器已有子项支持多选批量移除；既有编辑器回归通过。均通过。未运行原版窗口，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：“延迟/速度”列已渲染，但候选为 `ProfileDto`，不含 `ProfileExItem` 的 delay/speed，当前显示 `-`；需把测速/ProfileEx 叠加层接入选择器（与 RE-PROF-09 节点统计同源）。
- 接口缺口（登记）：未提供 `subItems` 时分组标签退化为 `subid` 文本；组编辑器调用已传 `subItems`。
- 台账遗漏（登记，仅追加）：`FilterConfigTypes/FilterExclude/SetConfigTypeFilter` 的 include/exclude 合同现由选择器 `filterConfigTypes`/`filterExclude` 承接，调用方（组编辑器）传 `[Custom] exclude:true`；WPF 上部只显示组/搜索/自动列宽，未新增不存在的类型筛选控件。

本轮实际结果：重建 `showNodePicker`/`_NodePickerDialog`（分组 chips、搜索、9 列表格、表头双向排序、自动列宽、单选/多选、取消、类型 include/exclude）；`_pickNodes` 传 `subItems`+`multiSelect`+`[Custom] exclude:true`；组编辑器加 `_selectedChildren` 与“移除选中”批量移除按钮，`_childTile` 加选择复选框。新增 7 个单次 `pumpWidget` 小测试文件全绿；回归 `t10` 6/6、`fix03_special` 7/7、`reprof10` 8/8、`fix03c` 7/7（`t10`/`fix03_special`/`reprof10` 首次因锁定 Flutter 构建多 `pumpWidget` 原生泄漏 `did not complete`，单独/重试全绿）；`flutter analyze` No issues。
