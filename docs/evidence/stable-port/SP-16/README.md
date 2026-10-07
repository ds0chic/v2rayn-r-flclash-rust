# SP-16 准备与正确合同（可独立部分）— 证据

状态：identified（准备范围已实现；完整卡未完成，不标 verified）。
基线：`92d46dd`。工作树为多卡并行共享脏树，本卡仅动文件级写锁内文件。
合成数据专用；无宿主网络/10808/系统代理/路由/TUN/DNS/Run-key 操作；
不读用户秘密。不 commit。

## 1. 准备范围（本次做 / 不做）

做（可独立）：原版键鼠/右键行为对照；选择集合与当前组身份纯合同；
故障夹具与最小实现；定向检查。
不做（等前置）：SubIndexId 持久化读写与重开恢复接线（profiles_controller
A07 独占 + SP-12 A04 在途）；FRB/桥/引擎/设置-运行时重试收尾；
真实 FRB/SQLite 重开、正式包入口、DPI 真机对照。不伪造完整卡完成。

## 2. 原版对照（冻结 7d6a967，只读 work/）

- `ServiceLib/ViewModels/ProfilesViewModel.cs:862-883` EditSubAsync：
  blNew=true 新建空 SubItem；false 取 GetSubItem(_config.SubIndexId)，
  null 直接 return（All/已删门控）。修后 RefreshSubscriptions +
  SubSelectedChangedAsync。当时本项目 `profiles_page.dart:118-129`
  两按钮同调 openSubSettings（总列表），违反直达（UI-07）。
- `ProfilesViewModel.cs:334-341` SubSelectedChangedAsync 写
  `_config.SubIndexId`；`:381-391` RefreshSubscriptions 按 SubIndexId
  命中恢复否则 All；`:361-375` RefreshServersBiz pending > IndexId > 首行；
  单击只改内存 SelectedProfile，从不写默认；SetDefaultServer 显式才写。
- 键鼠/右键：单击即时选择、pointer-down 捕获 Ctrl/Shift、右键已选保留多选、
  target 冻结、滚动/失焦关闭已有实现保留；子菜单 Esc 逐级语义未真机对照，
  当前实现为整链关闭，本卡如实锁定该行为（见 sub_entry.dart 注记）。

## 3. 改动文件（写锁内）

- `crates/application/src/selection.rs`（+2 纯合同 +2 单测）：
  SubEditTarget/resolve_sub_edit_target（G 命中直达，否则 GatedAll）；
  FrozenCommandTargets/restore_command_targets/is_primary_target_live
  （组变更/不可见即失效，不回落首行）。groups.rs 未动（订阅组 G 与
  PolicyGroup 无关，不硬凑改动）。
- `apps/desktop/lib/features/profiles/sub_entry.dart`（新增，纯）：
  SubEntryKind/resolveSubEntry/resolveCreateEntry + frozen 命令恢复 +
  menuEscClosesWholeChain。如实复刻 All/缺失门控。
- `apps/desktop/lib/features/subs/sub_direct_edit.dart`（新增）：
  openEditCurrentSub（直达 G；门控/取消/保存拒绝均提示且不开总列表）/
  openAddSub（空白新建，取消零写）。保存走已有 subs_controller.save；
  跨窗 revision 重试对账待 SP-12。
- `apps/desktop/lib/features/profiles/profiles_page.dart`：
  工具栏编辑→openEditCurrentSub，新增→openAddSub（修 UI-07 同入口错误）。
- `apps/desktop/test/repair/sp_16_sub_entry_test.dart`（新增，9 项）。

## 4. 先红后修

红：新测试先引用不存在的 sub_entry.dart，flutter test 报读取/方法缺失
（exit!=0）。Rust 新函数先缺失，standalone 编译红。
修：实现后 Dart 9/9 绿；selection.rs standalone 12/12 绿（含既有 10）；
table_actions 既有 6/6 绿。

## 5. 定向检查（实际结果，未跑全量门禁）

- `rustfmt --check crates/application/src/selection.rs`：exit 0（自有文件）。
  `cargo fmt --all -- --check`：全树仍报 net_host_client.rs 等他卡格式差，
  非本卡文件，不动。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：
  exit!=0；错误全在 net_host_client.rs（E0615 as_u64 缺调用括号 ×3，
  基线既有）+ routing.rs unused_mut；无 selection.rs/groups.rs 相关。
  本卡文件 clippy 干净（按文件过滤零命中）。
- `cargo test -p application --locked`（selection/groups 过滤）：整包 lib
  被上述基线错误阻塞编译，exit!=0；改用 selection.rs 独立
  `rustc --test`：12 passed / 0 failed（合成数据）。
- `flutter analyze`：7 issues，均为 routing/settings 他卡既有；
  sub_entry/sub_direct_edit/profiles_page/sp_16 零命中。
- `flutter test test/repair/sp_16_sub_entry_test.dart`：9/9 passed。
  `flutter test test/table_actions_test.dart`：6/6 passed。
  `test/r4_07_contract_test.dart` / `r4_08_contract_test.dart`：
  均 did not complete（已知 flutter_tester 进程异常，审计已登记），
  不计通过，不装绿。
- `dart format --set-exit-if-changed`（4 自有文件）：修后 exit 0。

## 6. 等待接口 / 阻塞

- SP-12（A04 在途）：SubIndexId 持久化读写、保存 core/platform 分阶段
  收据与重试、独立重开恢复验证。需提供方/DTO/版本/生效点由 SP-00 整合。
- profiles_controller.dart（A07 独占）：setGroupSubId 持久化 + build 恢复
  + group_reopen 真实重开，本卡未碰该文件。
- FRB/桥/引擎真实接线、正式包/DPI 真机对照：未运行，标未验证。

## 7. 卡状态

SP-16：identified（准备实现完成，完整接线 blocked by SP-12 + A07 文件锁）。
SP-18：identified（冻结目标/Enter/Esc 纯合同已备，原生逐场景 trace/截图、
DPI 跨屏、全 ACT-PROF 矩阵未验）。

## 8. 本轮接线（基线 3635392，SP-16+SP-18 并行）

状态：implemented（独立接线完成；完整卡 blocked，不标 verified）。
合成数据专用；无宿主网络/10808/系统代理/路由/TUN/DNS/Run-key 操作；
不读用户秘密。不 commit。

做（可独立）：`restoreCommandTargets`（sub_entry.dart）与
`restore_command_targets`（selection.rs）组归一（空白拼写≡All 视图，
双端一致，见 SP-18/README）；`_onContextAction` 改走纯门控
`isCommandContextLive` + 既有重绑（行为等价，profiles_table.dart）；
工具栏编辑/新增直达入口保持（沿用准备实现，未改 profiles_page）。
不做（等前置/他锁）：SubIndexId 持久化读写与重开恢复
（profiles_controller A07 独占 + SP-12 A04 在途，未碰该文件）；
FRB/桥/引擎真实接线、正式包入口、DPI 真机对照（未运行，标未验证）；
groups.rs 未动（订阅组 G 与 PolicyGroup 无关）。

定向检查（实际结果）：`rustfmt --check selection.rs` exit 0；
`cargo test -p application --locked --lib` 330/330（含 selection 13/13）；
`cargo test -p application --locked`（全 target）被他卡
t11_codegen_matrix 缺字段阻塞，exit!=0；clippy --lib exit 0，
--all-targets 同被 t11 阻塞；`flutter analyze` 本卡文件零命中；
`flutter test` 7 文件 51/51 passed（sp_16 9/9、sp_18 8/8、
table_actions 6/6 + 既有 profiles 键鼠/右键回归 28 项）；
`dart format` 本卡 4 文件 exit 0。

阻塞：A07 profiles_controller（setGroupSubId 持久化 + build 恢复 +
group_reopen 真实重开）；SP-12 跨窗 revision 重试对账；真机 trace/DPI。
group_reopen 绿、真实 G 保存重开、同名/普通组、取消/失败：未运行，
保持 blocked，不装绿。

## 9. 接线轮（profiles_controller 已解 Karen，A07 独占解除）

状态：implemented（接线完成；verified 不标：真机 group_reopen trace/DPI、
真实 FRB/SQLite 重开未运行）。
合成数据专用；无宿主网络/10808/系统代理/路由/TUN/DNS/Run-key 操作；
不读用户秘密。不 commit。

### 9.1 做（本轮）

- 持久化写：`setGroupSubId` 每次切换经已有
  `saveSettingsGroup('SubIndexId', patch, groupRevision)` 写规范
  `Config.SubIndexId`（显式 SubIndexId 组写即 engine 组切换；
  整树保存会 pin 回该值，故不用 `saveSettingsJson`）。
  写为上游 `SubSelectedChangedAsync` 对等（`_config` 写本身不校验存在性，
  存在性在读侧强制）。返回 bool：失败留原组（无假切换、无半状态）。
- 重开恢复：`build()` 读持久化组对现存订阅解析（`resolveReopenGroup`），
  悬空/空白回 All，不回落首组，不持久化修复。
- 统一刷新与回落：`subs_controller.save/delete` 成功后
  `profiles.reload() + resyncGroupFromSubs()`（上游 `RefreshSubscriptions`
  对等：命中保持、新组不自动切换、删当前组内存回 All 且不写修复）。
- 工具栏直达入口（`openEditCurrentSub`/`openAddSub`）与订阅设置窗
  add/edit/delete 均走上述控制器路径，无需改 UI 文件；
  主窗直接删 G 入口（上游 `ProfilesViewModel.DeleteSubAsync`  counterpart）
  未新增按钮，删组经订阅设置窗（已有入口）覆盖。
- 纯帮助（`sub_entry.dart` 新增）：`resolveReopenGroup`（对
  `selection::resolve_current_group`）、`readPersistedSubIndexId`、
  `subIndexIdPatch`、`decodeGroupRevisions`、`subIndexIdGroup`。
-  collateral：`ux_space01_entries_test.dart`  stale 断言已按 SP-16 合同修正
  （上一轮把 `toolbar-sub-edit` 改为直达 G 后，该测试仍期望弹出总列表；
  与本轮无关的既有失败，现改为：选 G 后点编辑弹出 `sub-edit-window`，
  点新增弹出空白 `sub-edit-window`，备注行前切回 All）。

### 9.2 改动文件（写锁内）

- `apps/desktop/lib/features/profiles/sub_entry.dart`：+5 纯帮助。
- `apps/desktop/lib/features/profiles/profiles_controller.dart`：
  `build` 恢复、`setGroupSubId -> bool` 持久化、`resyncGroupFromSubs`。
- `apps/desktop/lib/features/subs/subs_controller.dart`：
  save/delete 成功后 profiles `reload + resync`。
- `apps/desktop/test/repair/sp_16_group_persistence_test.dart`：新增 15 项
  （纯合同 4 + 切换持久化 3 + 合成重开 3 + 真实路径 create/edit/delete 5，
  含 persist 失败、非法保存、删他组保持、不写修复断言）。
- `apps/desktop/test/ux_space01_entries_test.dart`：stale 断言修正（见 9.1）。
- `docs/repair/stable-port-2026-10-06/execution-manifest.json`：SP-16 note。

未动：`crates/bridge_api`、FRB 文件、Rust（本轮无 Rust 改动，未跑 cargo）；
`profiles_page.dart`、`sub_direct_edit.dart`、`sub_setting_window.dart`
（入口行为已符合合同，统一刷新由控制器保证）。

### 9.3 定向检查（实际结果）

- `dart format --output=none --set-exit-if-changed lib test`：0 changed。
- `flutter analyze`：No issues found。
- `flutter test test/repair/sp_16_group_persistence_test.dart`：15/15。
- 回归（`setGroupSubId`/subs 存取触及面）：
  sp_16 9/9、sp_18 8/8、table_actions 6/6、recheck01×2、recheck05、
  recheck_r3_prof10、r4_09、r4_17、r4_22、re_prof_06、r4_16/r4_17 repro、
  t15b、ux_parity_fix01、t09×2、fix09×2、r4_34、sr01、t21e、ux_parity_fix06、
  ux_space01——全部绿（含本轮修正后的 ux_space01）。
- 未运行：cargo（无 Rust 改动，按任务卡免跑）；全量 `flutter test`
  （按任务卡只跑受影响文件）；真机/正式包/DPI。

### 9.4 登记缺口（不发明接口）

- `engine.set_current_group/resolve_current_group` 无 FRB 暴露：本轮经
  settings 独立组缝达到同等持久化效果；专用桥接由 SP-00 整合。
- engine `save_settings_group('SubIndexId')` 不校验订阅存在性（与
  `set_current_group` 不对称）：存在性门控只在 Dart 读侧
  （build/resync/edit 入口门控），写侧保持上游 `_config` 写对等。
- 上游订阅设置窗删除有确认框（`ShowYesNoInteraction`），本项目
  `sub_setting_window._delete` 直接删除：行为差，未改（他卡 UX 范畴）。
- 真实 FRB/SQLite 重开、正式包入口、DPI 真机对照：未运行，标未验证。

## 10. 删除 parity 轮（主窗直接删 G + 订阅窗删除确认）

状态：implemented（独立接线完成；verified 不标：真机删除 trace/DPI、
真实 FRB/SQLite 重开未运行）。
合成数据专用；无宿主网络/10808/系统代理/路由/TUN/DNS/Run-key 操作；
不读用户秘密。不 commit。未动 `profiles_controller.dart`（他锁文件，
本轮只用其已有 `reload`/`resyncGroupFromSubs`/`setGroupSubId` API）。

### 10.1 做（本轮）

- 主窗直接删 G 入口（上游 `ProfilesViewModel.DeleteSubAsync:885-900`
  counterpart）：`sub_direct_edit.dart` 新增 `deleteCurrentSub`
 （取当前组 G，All/缺失按原版门控直接 return 且不弹确认框；
  确认框取消直接 return；删除走已有 `subs_controller.delete`，
  成功后控制器统一刷新 + profiles 回 All，失败报出错误并保持当前组）。
  `profiles_page.dart` 工具栏新增 `toolbar-sub-delete`（删除当前订阅）
  调用该入口。
- 订阅窗删除确认（上游 `SubSettingViewModel.DeleteSubAsync:84-96`
  counterpart，`ResUI.RemoveServer` 对等文案）：`sub_setting_window.dart`
  新增 `confirmAndDeleteSub`（[confirmDelete] 可注入覆盖，便于测试）；
  删除按钮与右键菜单删除项均经此确认路径（菜单项先 select 再确认，
  与按钮行为一致）。取消零写；失败在窗内状态行报出错误并保留该组。
- 确认框统一用共享 `showAppConfirmDialog`（取消左/删除右，Enter 确认/
  Esc 取消），键为 `sub-delete-confirm` / `sub-delete-confirm-ok` /
  `sub-delete-cancel`。

### 10.2 改动文件（写锁内）

- `apps/desktop/lib/features/subs/sub_direct_edit.dart`：+`deleteCurrentSub`。
- `apps/desktop/lib/features/profiles/profiles_page.dart`：工具栏
  +`toolbar-sub-delete`。
- `apps/desktop/lib/features/subs/sub_setting_window.dart`：
  +`confirmAndDeleteSub`；`_delete` 与右键菜单删除项改走确认路径。
- `apps/desktop/test/repair/sp_16_sub_delete_test.dart`：新增 8 项
  （纯门控 3 + 主窗删除 All 门控/取消保留/确认回 All/失败保组报错 2 +
  订阅窗确认取消/确认删除/失败保组报错 2 + 订阅窗真实对话框取消→确认 1）。
- `apps/desktop/test/repair/sp_16_sub_delete_toolbar_test.dart`：新增 1 项
  （整壳 MainShell 工具栏真实对话框：取消保留组与订阅，确认删当前组回
  All；独立文件、单次整壳构建，避开单进程多次整页构建的 tester 崩溃）。
- `docs/repair/stable-port-2026-10-06/execution-manifest.json`：SP-16 note。

未动：`profiles_controller.dart`、`crates/**`、桥/FRB 文件、其他 feature。

### 10.3 定向检查（实际结果）

- `dart format --output=none --set-exit-if-changed lib test`：0 changed。
- `flutter analyze`：本卡文件零命中；全树 2 issues 均为他卡在途文件
  `test/sp21_paged_load_test.dart`（`ProfileSummary` 未定义 + 类型错，
  非本卡文件，不动）。
- `flutter test test/repair/sp_16_sub_delete_test.dart`：8/8 passed。
- `flutter test test/repair/sp_16_sub_delete_toolbar_test.dart`：1/1。
- 回归：`t09_sub_setting_test` 5/5、`ux_space01_entries_test` 1/1、
  `sp_16_group_persistence` 15/15、`sp_16_sub_entry` 9/9 +
  `t09_sub_edit_test` 5/5。
- 过程记录：整壳工具栏测试与 8 项同文件时 tester 进程崩溃
 （did not complete，无 Dart 栈；单测隔离均绿，属已知
  flutter_tester 单进程多次整页构建崩溃，见 profiles_harness 注记），
  按该注记拆为独立文件后各自绿；另有一次加载期偶发崩溃，重试即绿。
- 未运行：cargo（无 Rust 改动）；全量 `flutter test`（按任务卡只跑受影响
  文件）；真机/正式包/DPI。

### 10.4 登记缺口（不发明接口）

- §9.4 既有缺口延续：engine `set_current_group` 无 FRB 暴露（SP-00 整合）；
  engine 组写不校验存在性（Dart 读侧门控）；真机 trace/DPI 未验证。
- 新增按钮沿用工具栏图标按钮样式（30x30），上游主窗删除为菜单命令，
  本项目主窗菜单/右键暂无删 G 入口——工具栏按钮覆盖该行为，菜单镜像缺口
  仍登记（需菜单 owner 确认 id/文案后再补，不在本轮发明）。
- 多选批量删除（上游订阅窗 `SelectedSources`）：本项目订阅窗为单选模型，
  仍单删；批量语义缺口登记，不强行引入多选。

## 11. 菜单镜像 + 多选批量删除轮（收敛 §10.4 两项缺口）

状态：implemented（独立接线完成；verified 不标：真机删除 trace/DPI、
真实 FRB/SQLite 重开未运行）。
合成数据专用；无宿主网络/10808/系统代理/路由/TUN/DNS/Run-key 操作；
不读用户秘密。不 commit。未动 `profiles_controller.dart`、`crates/**`、
桥/FRB 文件、其他 feature。

### 11.1 做（本轮）

- 主窗分组右键菜单（上游 `v2rayN.Desktop/Views/ProfilesView.axaml` 组
  `ListBox.ContextMenu`：`menuSubEdit`/`menuSubAdd`/`menuSubDelete` 对等）：
  `profiles_page.dart` 新增 `showGroupChipMenu`（键 `group-chip-menu-edit`/
  `add`/`delete`），组 chip 包 `GestureDetector.onSecondaryTapUp` 弹出；
  三项分别委托 `sub_direct_edit.dart` 既有入口
 （`openEditCurrentSub`/`openAddSub`/`deleteCurrentSub`），门控/确认框/
  失败语义与工具栏按钮同一。节点右键菜单模型（`context_menu.dart`）与
  `profiles_controller.dart` 均未动——新菜单挂在组 chip 上，不需要改
  菜单 owner 的 id/文案，故无 §10.4 所述那类改动。
- 订阅窗多选批量删除（上游 `SubSettingViewModel.SelectedSources`/
  `SelectedSource` + `DeleteSubAsync:84-96` 一次确认删全部对等）：
  `subs_controller.SubsState` 新增 `selectedIds`（普通单击收拢单选，
  单选行为不变；Ctrl+单击 `toggleMultiSelected`；Shift+单击 `selectRange`
  以 primary 为锚区间选择）+ `deleteIds`
 （`SelectedSources ?? [SelectedSource]` 对等）；`delete` 成功经
  `reload` 裁剪已删选中、失败保留选中集；`reload` 顺带裁剪悬空选中
  （只收缩、不发明选择）。`sub_setting_window.dart` 新增
  `confirmAndDeleteSubs`（一次确认删全部选中行；原单行
  `confirmAndDeleteSub` 转调，语义不变）；删除按钮与右键菜单删除项均走
  批量路径（右键落在已选集内保留多选，与主窗 `handleRightTap` 一致；
  编辑/分享/更新仍单目标，与上游 `EditSubAsync` 取 `SelectedSource` 一致）；
  右键菜单项加 key（`sub-menu-edit/share/enable/update/delete`）。
- 测试合成数据专用（见新文件头注记）。

### 11.2 改动文件（写锁内）

- `apps/desktop/lib/features/profiles/profiles_page.dart`：
  +`showGroupChipMenu`；组 chip 包右键手势。
- `apps/desktop/lib/features/subs/subs_controller.dart`：
  `SubsState` +`selectedIds`/`deleteIds`；+`toggleMultiSelected`/
  `selectRange`/`_pruneSelection`；`select` 收拢单选；`reload` 裁剪悬空。
- `apps/desktop/lib/features/subs/sub_setting_window.dart`：
  +`confirmAndDeleteSubs`（`confirmAndDeleteSub` 转调）；行点选 Ctrl/Shift
  语义；`_delete` 与右键菜单删除项走批量路径；菜单项 key。
- `apps/desktop/test/repair/sp_16_sub_batch_delete_test.dart`：新增 11 项
  （模型 5：单选收拢/toggle 增删清空/Shift 区间/空集门控/成功仅裁剪已删；
  批量入口 4：取消全留 + 确认只删批量 + 单行路径不碰他行 + 未知 id 免确认 +
  失败保数据保选中报错；真窗 2：Ctrl+点多选 + 批量确认框计数与取消/确认、
  右键已选行保留多选删批量）。
- `apps/desktop/test/repair/sp_16_group_chip_menu_test.dart`：新增 1 项
  （整壳 MainShell 独立文件、单次整壳构建：右键三项齐备；菜单删除取消保留/
  确认删当前组回 All；All 视图右键删除直接门控免确认）。
- `docs/repair/stable-port-2026-10-06/execution-manifest.json`：SP-16 note。

未动：`profiles_controller.dart`、`context_menu.dart`、`crates/**`、
桥/FRB 文件、其他 feature。

### 11.3 定向检查（实际结果）

- `dart format --output=none --set-exit-if-changed lib test`：0 changed。
- `flutter analyze`：No issues found。
- 新测：`sp_16_sub_batch_delete_test` 11/11、
  `sp_16_group_chip_menu_test` 1/1。
- 回归：`sp_16_sub_delete` 8/8、`sp_16_sub_delete_toolbar` 1/1、
  `sp_16_group_persistence` 15/15、`sp_16_sub_entry` 9/9、
  `t09_sub_setting` 5/5、`t09_sub_edit`（同跑文件内）、
  `ux_space01_entries`、`r4_17_contract`、`recheck_rr02_03`、
  `table_actions` 6/6、`t11_routing`、`r4_17_repro`——全部绿。
- 过程记录：本轮无 flutter_tester 崩溃，未启用 per-file retry；
  新整壳测试按注记独占一文件。
- 未运行：cargo（无 Rust 改动）；全量 `flutter test`（按任务卡只跑受影响
  文件）；真机/正式包/DPI。

### 11.4 登记缺口（不发明接口）

- §9.4/§10.4 既有缺口延续：engine `set_current_group` 无 FRB 暴露
  （SP-00 整合）；engine 组写不校验存在性（Dart 读侧门控）；
  真机 trace/DPI 未验证。
- 订阅窗多选目前仅键鼠点选（Ctrl/Shift+单击）：无全选快捷键/按钮，
  无 Shift+方向键键盘扩展（上游 DataGrid Extended 选择含键盘扩展语义，
  本项目未补）；选择完整性缺口登记，不属删除 parity，不强行补。
- 订阅窗更新（`update`）仍单目标（右键行/按钮语义未动）：上游批量更新
  语义（多选 update）是否在 SubSetting 覆盖内未核对，登记待查，
  不在本轮删除 parity 范围内伪造。
