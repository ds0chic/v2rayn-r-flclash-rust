# UX-PARITY-FIX-01 — 右键生成全部策略组与不可变命令上下文

状态：`implemented`。修复对象：`FIX-01`（审查台账 `F-PROFILE-009` / `ACT-PROF-007` / `ACT-PROF-008` / `ACT-PROF-013`，PR-19/21/18/10）。基线应用 HEAD `1251cbc6821276e35d082f7739b8b9c15b6f93dd`，冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。本目录只放本卡证据；未改 `dist/`、`work/`、`outputs/`，未 commit。

## 1. 根因

`ProfilesTable` 原先用 `ContextMenuSession _menuSession`：

1. `MenuItemButton` 先关闭菜单 → `onClose` 执行 `_onMenuClosed` 把 `_menuSession` 置空；
2. `onPressed` 随后执行 `_onContextAction(entry, session)`，但 `_moveToGroup`/`_genGroup` 仍读 **live** `_menuSession`（此时已 null），目标丢失；生成因此提示「请先选择节点以确定订阅分组」。
3. `_genGroup` 还用**选中 primary 节点的 `subid`** 反推分组并要求必须选中节点；原版 `GenGroupAllServer`/`GenGroupRegionServer` 的对象是当前订阅组 `SelectedSub`，与是否选中节点无关。
4. 移动分组按 label `startsWith` 找订阅，重名/前缀/无分组前缀可能移错稳定 ID。

## 2. 修复

- 新增 `apps/desktop/lib/features/profiles/command_context.dart`：不可变 `CommandContext{targetIds(不可变), primaryId, groupSubId, viewContext, menuOpenPosition, focusRestore}`。`targetIds` 用 `List.unmodifiable`。
- `profiles_table.dart`：菜单打开时捕获 `CommandContext`；`onClose` 仍清 UI 会话，但每个 `MenuItemButton` 闭包持有自己的上下文引用，命令只读该上下文。`_genGroup` 用 `context.groupSubId`（当前组稳定 ID），不再读选中节点，也无需选中节点；`_moveToGroup` 用 `context.targetIds` + 菜单项携带的稳定 `targetSubId`（`context_menu.dart` 新增字段），不再解析显示文字。
- `profiles_controller.dart`：`genGroupRegion` 回传 `GroupGenResult`（含新组），新增 `selectGenerated` 仅选中当前视图中真实存在的新组（原版 `_pendingSelectIndexId` 语义；合成摘要不含新组时不伪造选中）。
- Rust 最小缺陷修复：`crates/application/src/groups.rs` 的 `new_group_all`/`new_group_region` 补 `core_type = Some(CoreType::Xray)`。原版两个生成函数都设 `ECoreType.Xray`，当前漏设使生成 DTO `coreType=null`。无签名变化，未跑 FRB 生成。

## 3. 证据

真实 Windows 窗口 + 真实 FRB/Rust/SQLite，隔离 `V2RAYN_R_DATA_DIR`；合成订阅（URL 为 loopback，从未请求）+ 两个 `vless://` 合成节点（`127.0.0.1:11998`，HK/US 备注）。不启动内核、不监听端口、不写系统代理/TUN。

| 文件 | 内容 |
|---|---|
| `observations.json` | generate run：8/8 check 通过，`recordingComplete=true`，`failures=[]` |
| `reopen-observations.json` | reopen run（新进程、同 data dir、新 engine/controller）：2/2 通过 |
| `generate-run.log` / `reopen-run.log` | `flutter test ... -d windows --reporter expanded` 原始日志 |
| `generate-all.png` | 生成「全部配置项」后：顶部分组选中，节点表 3 行（2 节点 + 1 policyGroup，新组被选中） |
| `generate-region.png` | 生成「按地区分组」后：节点表 5 行（HK/US 地区组 + 全部组 + 2 节点） |
| `reopen-window.png` | 重开进程后同 5 行仍在，证明落库/重开一致 |

generate run 连接（`observations.json`）：`subscription-created` → `nodes-imported-into-group`(2) → `group-selected`(visible=2) → `no-selection`(selected=0) → `generate-all-persisted`(1) → `generate-all-fields-match-upstream` → `generated-group-in-node-table`(inAll/inVisible) → `generate-region-persisted`(HK/US)。

生成对象字段（`generate-all-fields-match-upstream`，对照 `ConfigHandler.AddGroupAllServer`）：

```json
{
  "coreType": "xray",
  "groupType": "PolicyGroup",
  "subChildItems": "<subId>",
  "childItems": null,
  "filter": "^(?!.*(?:剩余|过期|到期|重置|[Rr]emaining|[Ee]xpir|[Rr]eset)).*$",
  "multipleLoad": 0,
  "isSub": false
}
```

reopen run 读取 generate run 的 `observations.json` 中 `generatedGroupId`，在新进程中按同一 ID 命中（`reopen-generated-group-exists`），并确认地区组仍在（`reopen-region-groups-exist`，count=3）。

复现审查 `ui-run-04` 的同一场景：在隔离 data dir 重跑 `integration_test/parity_review_smoke_test.dart`，`parity-smoke/observations.json` 记录 `context-generate-group-keeps-menu-target: passed=true`（原为 `nodeCount 1→1`、提示先选节点）。其余 3 项失败（完整配置粘贴导入、屏幕/图片二维码）属其它工作包，与本卡无关。

## 4. 命令与结果

```
# 真实窗口生成（新 data dir）
$env:V2RAYN_R_DATA_DIR=<temp>\fix01-data
$env:V2RAYN_R_FIX01_EVIDENCE=docs\evidence\UX-PARITY-FIX-01
$env:V2RAYN_R_FIX01_MODE=generate
flutter test integration_test/ux_parity_fix01_gen_group_test.dart -d windows --reporter expanded   # +1 All tests passed

# 重开（同 data dir，新进程）
$env:V2RAYN_R_FIX01_MODE=reopen
flutter test integration_test/ux_parity_fix01_gen_group_test.dart -d windows --reporter expanded   # +1 All tests passed

# widget/单元
flutter test test/ux_parity_fix01_command_context_test.dart        # +3
flutter test test/ux_parity_fix01_gen_group_ui_test.dart           # +1
flutter test test/ux_parity_fix01_gen_group_guard_test.dart        # +1

# Rust 最小修复
cargo fmt -p application -- --check                                # 无差异
cargo test -p application --test t10_groups_templates --locked     # 10 passed
```

真实窗口记录：`generate` run 约 7s、`reopen` run 约 1s（远低于 120s）；仅本进程，无 10808 / 系统代理 / TUN 操作。首次 generate 运行遇到一次已记录的 flutter_tester/Windows 原生间歇崩溃（`did not complete`，无 Dart 栈），清理 data dir 重跑即通过；与本修复无关，保留说明。

### 门禁结果

| 门禁 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed lib test integration_test` | 174 文件，0 改变，exit 0 |
| `flutter analyze` | No issues found |
| `flutter test` 逐文件（`tools/flutter_test_retry.ps1 -PerFile`） | 仅 `ux_space03_vless_editor_test.dart` 10/10 失败（`Expected 14..18 Actual 44.0` 的编辑器间距断言，属并行编辑 `profile_editor_dialog.dart`/`profile_fields.dart` 的另卡在改代码，非 FIX-01）；FIX-01 三个测试文件全绿 |
| `flutter build windows --release` | Built `build\windows\x64\runner\Release\v2rayn_desktop.exe`，exit 0 |
| `cargo fmt -p application -- --check` | 无差异 |
| `cargo clippy -p application --all-targets --locked -- -D warnings` | Finished，exit 0 |
| `cargo test -p application --test t10_groups_templates --locked` | 10 passed |

说明：本轮在共享工作树运行，另一实现线同时在改 `profile_editor_dialog.dart`/`profile_fields.dart` 与 subs wire；`ux_space03_vless_editor_test.dart` 的失败在 FIX-01 改动前即由该并行编辑引入，与本卡无关，未越界修改。

### 证据哈希（SHA256）

```
d4350996080ebdd4791a856acdb9cf90381e548bdcc48e3f5ccf0216608f23bc  generate-all.png
20c42e0b55b2aac7a26db060e26ee1d114ee7a474828b7c06caa3c53f7ffd46d  generate-region.png
f6b7175bfe7aa00bbb640bd68022d7494eb3c53a8953c8c587f7493b74bf4926  reopen-window.png
af514f99bb40acbe86054eebbf704028b28098042e8d0d80e4fa102c66b17148  observations.json
a66f5f71f3803b4bb28d9e18501594f303749f4c8cd513a6662d261003f0dc7d  reopen-observations.json
de2acfb13e71b1c23a8198c81d8f6b0ad5f341c4e398d54a32b1fc8911aa8781  generate-run.log
1ab9f507bd6aebf068a0224cd6e868db14a79bec024e95331d0a9bac9ca33ea5  reopen-run.log
5fd4bcc9116f2be718093f41ac097b39f4c4927a178fc7c99e71e1d4f2b2bc0a  build-windows-release.log
```

## 5. 限制与未验证

- 未做原版实机双窗口、同 DPI 逐事件像素对照；状态为 `implemented`，不写 `verified`。
- 区域生成按冻结 `PolicyGroupRegionFilters` 现有 17 地区；本次只实测 HK/US，其余地区未逐一实机。
- widget 层用合成 `BridgePort`；其 `fetchSummaries` 为索引合成、不反射新生成组，故 widget 断言用保存路径（`state.profiles`）+ id 差集，节点表出现由真实窗口 integration 证明。
- 真实窗口仅 Windows；macOS/Linux/ARM64/Avalonia 未验证。
