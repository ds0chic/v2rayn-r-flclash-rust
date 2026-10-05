# R4-12 窗口和三布局 — 证据

- HEAD：`cbce523`（卡内基线）；应用基线 `77c74ed`。`armed=false`。
- 范围：三主布局核对与重开保持；53 窗口库存逐条核对；子窗读取失败不提交空草稿；原版子编辑/即时提交不被全窗确定/取消覆盖。
- 夹具：`MemoryUiStateStore` + 内存 host（`_FailingSettingsHost` / `_RecordingRoutingHost`）。
- 约束遵守：不加载原生库、不启核、不监听端口（未触碰 10808）、不写宿主代理/TUN/路由/注册表、不读用户数据。
- 上游：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`（冻结 `7d6a967`）。
- 本轮未截图：3 个 Scenarios 需要第二 Flutter engine 与真实窗口，`flutter test` 无法覆盖；结论以结构与源码核对为准并如实标注 `未验证`。

## 三布局（main_layouts 3 条）

| ID | 模式 | 结论 | 证据 |
|---|---|---|---|
| LAY-MAIN-001 | horizontal | implemented | `main_shell.dart:192-220`（Row: ProfilesPage + GridSplitter + SideTabs(top)）；`r4_12_contract_test.dart` 断言 `split-horizontal`/`main-tab-content-info` |
| LAY-MAIN-002 | vertical（默认） | implemented | `main_shell.dart:221-247`（Column，TabStripPlacement left）；测试断言 `split-vertical` |
| LAY-MAIN-003 | tab | implemented | `main_shell.dart:248-257`；`side_tabs.dart:63-74` profiles 为首个 tab，无分栏；测试断言无 splitter |

- 状态源：`UiItem.MainGirdOrientation`（0→水平 / 1→垂直 / 2→标签）经 `applySettingsDocument` 应用；`setLayout` 写 `ui_state_store` layout section，重开由 `build()` 恢复。测试覆盖三值映射与逐模式重开保持。
- 未验证：真实三布局的表格/菜单/底栏渲染与 100–200% DPI 实测。

## 53 窗口库存核对

分类计数：**已实现 = 3**（独立原生窗 OptionSettingWindow / RoutingSettingWindow / RoutingRuleSettingWindow-internal），**独立窗或对话框（嵌入布局，app-dialog）= 12**，**未实现 = 0**，**not_applicable = 38**（application/resourcedictionary/styles/usercontrol_view 与 Avalonia 独有 / 非 Windows 条目）。详见「窗口库存核对表.md」。

## 改动

- `apps/desktop/lib/features/settings/settings_window_host.dart`
  - 新增 `SettingsEditorLoadException`；`NativeSettingsEditorHost` 50 次 `ready` 重试耗尽或快照为 `{}`/空 → `loadSnapshot()` 抛错，不再返回空 Map（D34）。
- `apps/desktop/lib/features/settings/option_setting_window.dart`
  - `_loadHostSnapshot` 捕获失败/空文档 → `_loadFailed` 错误态；新增重试 `settings-load-retry`；错误态不渲染「确定」，`_save` 在 `_loadFailed||!_draftInit` 时拒绝保存。
- `apps/desktop/lib/features/routing/routing_windows.dart`
  - 新增 `RoutingEditorLoadException`；`NativeRoutingEditorHost.loadSnapshot()` 失败/空抛错。
  - `RoutingEditorWindow`：`_loadFailed` 错误态 + 重试；失败态隐藏「确定」，`_ok` 拒绝。
  - **D31 即时提交 seam**：新增能力接口 `RoutingCommitHost.commit(actionJson)`；`NativeRoutingEditorHost` 复用既有 `saveDraft/saveOutcome` 通道（无原生协议改动）发送带 `kind` 的动作。子编辑器确定→`saveScheme`、删除→`deleteScheme`、设为活动→`setDefault`、策略变化→`strategy` 各自即时提交。子编辑器确定不再等待全窗确定；全窗取消不再撤销已提交子编辑；「确定」仍按前后端一致的全窗 draft 语义。
- `apps/desktop/lib/features/routing/routing_actions.dart`
  - `_applyRoutingDraft` 识别带 `kind` 的载荷并分派到新增 `_applyRoutingAction`，逐动作事务化（`saveScheme`/`deleteScheme`/`setDefault`/`strategy`），失败返回错误不谎报成功。
- `apps/desktop/test/r4_12_contract_test.dart`（新增）。
- `compat/layouts.yaml`：仅对 LAY-MAIN-001/002/003 追加 `r4_12_status/_evidence`，未删行、未改分母、未动 53 窗口条目的 status。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter analyze` | exit 0，No issues found |
| `flutter test test/r4_12_contract_test.dart` | exit 0，8/8 通过 |
| `flutter test r3_wpf_routing_window/r3_wpf_option_window/r4_01/r4_02/r4_11` | exit 0，31/31 通过（整窗确定/取消语义无回归） |
| `flutter build windows --release` | 未运行：原生 runner 无改动（提交复用既有通道），且 build 会占用 exe 锁 |

## 状态

`implemented`。三布局结构/持久化、读失败保护与即时提交语义均有合成 widget 合同测试；未做真实 Windows 双 engine 窗口、真实 FRB/SQLite、真实 DPI/主题矩阵、真实原生失败注入 → 未 `verified`。

## 未完成 / 接口缺口

1. 第二 engine 的 DPI/主题/字体/locale 传递（UFS-13）未接：`option_setting_window_entry.dart:21` 与 `routing_windows.dart:1756` 仍 `buildAppTheme(Brightness.light)`。属 R4-11/R4-30 范围，本卡未改。
2. 原生 `option_window_host.cpp`/`routing_window_host.cpp` 在主 channel 缺失时仍回 Success（UFS-09）；本卡在 Dart 侧以超时/异常兜底，原生 pending 治理归 R4-27。
3. 全窗确定失败后的部分提交回滚 / 事务化（D31 残留）与策略/删除的真实持久化闭环需整合者解锁并做真实 FRB 验收。
4. 53 窗口剩余 `未实现` 为 0：其余均为 not_applicable 或已嵌入对话框；无静默遗漏。
