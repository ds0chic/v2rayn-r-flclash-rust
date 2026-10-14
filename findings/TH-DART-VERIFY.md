# TH-DART 阶段 1：合并后 Dart 验证（定稿，2026-10-13）

> 状态：**阶段 1 完成，进入阶段 2**（全程低负载：一次一文件、Idle 优先级、`-j 1`；禁 cargo/build/全量 analyze）。
> 集成分支 HEAD：`e45c6c7`（B01…B17 全部合入）。worktree：`C:\Users\Colby\work\v2rayn-audit\worktrees\TH-DART`，分支 `fix/TH-DART`。

## 1. 门禁（阶段 1 只验证，不修）

| 命令（`apps/desktop`） | 结果 | 说明 |
|---|---|---|
| `flutter pub get --enforce-lockfile` | PASS（exit 0） | lockfile 未改动；附带的 `windows/flutter/generated_plugin*` 零内容差异已 `checkout --` 还原 |
| `dart format --output=none --set-exit-if-changed lib test` | **FAIL（exit 1）** | 473 文件中 1 个待改：`lib/app/app.dart`。差异仅一处：`final themeEnv = evidenceStringHook(Platform.environment, 'V2RAYN_R_THEME');` 长行需折行。归属：**B16**（最后触碰该文件的是 `1110a63 fix(app): armed-only evidence env guards [D3b-01]`）。阶段 2 用 `dart format` 修 |
| `flutter analyze` | PASS | `No issues found!`（9.4s） |

## 2. 全量 test/ 串行跑（已完成）

- 清单：`test/` 下共 352 个 `*_test.dart`，排除 3 个 scan（`scan_image_qr`、`scan_screen_qr`、`ux_parity_fix05_scan`）后 **349 个待跑**（基线时 327 个，B13–B16 新增约 22 个）。
- Runner：`scratchpad/run_chunk.ps1`（逐文件 `flutter test <file>`，单文件 300s 超时记 HANG 并 `taskkill /T` 只杀自有子进程树，CSV 记录 exit/耗时/尾部输出）。注意：必须用 **pwsh** 调用，`powershell` 5.1 下 `$p.ExitCode` 取不到（已踩坑，冒烟时 exit 列为空，改 pwsh 后正常）。
- 运行记录文件（scratchpad）：`run_list.txt`（349 行）、`results_1-2.csv`。
- 已完成：冒烟 2 文件全绿——`test\audit_r2_flows_test.dart`（exit 0，10/10）、`test\b07b_bridge_seams_test.dart`（exit 0，9/9）；`run_chunk.ps1 -Start 3 -End 3`（`b13_d102_picker_order_test`，exit 0）。
- chunk `4-52`（49 文件，Idle + `-j 1` 串行）：44 exit 0；4 exit 1（均为 did not complete：`fix02_tuic_editor`、`fix03c_proxy_chain_editor`、`fix08c_routing_draft`、`fix15c_autostart_timing`——Idle 下疑似饿死，待 solo 复判）；1 exit 79（`fix08_option_apply_test`，`No tests were found`——疑似 D 组同类 loader，待 solo 复判）。build 已删，generated_plugin* 噪声已还原。
- chunk `53-102`（50 文件，Idle + `-j 1` 串行）：38 exit 0；3 exit 1（`r3_wpf_option_window`、`r3_wpf_routing_window` did not complete，Idle 下疑似饿死待 solo；**`r4_11_contract_test` +4 -1，`edit-save-reopen` 失败——与基线 C1 一致，确定性复现**）；9 exit 79（`fix16b_settings_source`、`profiles_drag_scroll_down/up`、`profiles_drag_scroll_up`、`profiles_keyboard`、`r3_visual_dpi_shell_125/150`、`r3_visual_dpi_windows`、`r4_07_contract`、`r4_08_contract`——did-not-complete + `No tests were found`；含基线 D 组 r3-150/r4_07，余下待 solo 复判是否同类 loader）。build 已删，generated_plugin* 噪声已还原。
- chunk `103-152`（50 文件，Idle + `-j 1` 串行）：44 exit 0；4 exit 1（`r4_18_p01`、`r4_19`、`r4_22` did not complete 待 solo；**`r4_23_contract_test` +9 -1，`log filtering…` 失败——与基线 C2 一致，确定性复现**）；2 exit 79（`r4_14_contract`、`r4_18_p11_contract` loader 待 solo）。build 已删，generated_plugin* 噪声已还原。
- chunk `153-202`（50 文件，Idle + `-j 1` 串行）：42 exit 0；2 exit 1（`repair/r4_11_repro_test` did not complete 待 solo；**`r4_31_contract_test` +4 -1，`log read model stays bounded` 失败——与基线 C3 一致，确定性复现**）；6 exit 79（`re_prof_13_active_marker`、`re_prof_14_drag_disabled/enabled`、`recheck_r3_prof_menu_target_a`、`recheck_r3_prof08_picker_esc`、`repair/r4_08_repro` loader 待 solo）。build 已删，generated_plugin* 噪声已还原。
- chunk `203-252`（50 文件，Idle + `-j 1` 串行）：47 exit 0；2 exit 1（`repair/r4_13_s23_repro`、`repair/sp_16_sub_keyboard_select` did not complete 待 solo）；1 exit 79（`repair/r4_19_repro` loader 待 solo）。本 chunk 无确定性断言失败。build 已删，generated_plugin* 噪声已还原。
- chunk `253-302`（50 文件，Idle + `-j 1` 串行）：39 exit 0；7 exit 1（`wave_b_g12_log_seed`、`wave_b_grpc_052`、`wave_b_settings_059_079`、`t10_custom_editor` did-not-complete/部分断言待 solo——含基线 E 组 flaky；**`runtime_controller_test`、`sp21_async_page_test`、`t06a_editor_save_test` 系 loading 期 `Connection closed before test suite loaded`（tester 加载崩溃，非断言失败，待 solo）**）；4 exit 79（`sp_24_settings_entry`、`wave_b_rollback_131_135`、`wave_b_settings_110_112`、`t05_shell_chrome` loader 待 solo）。build 已删，generated_plugin* 噪声已还原。
- chunk `303-349`（47 文件，Idle + `-j 1` 串行）：35 exit 0；6 exit 1（`t11_dns`、`t17_disabled_entry`、`t21e_responsive` did-not-complete 待 solo；**`t11_menu_test` +1 -1 与基线 C10 一致**；`t18b_runtime_ui_test` 本轮为 did-not-complete（Idle 饿死貌）——需 solo 看真实失败形态；**`t21e_import_forms_test` +0 ~1 -5，setUpAll 的 markTestSkipped 未生效、5 测试体照跑照挂——与基线 C4–C9 一致**）；6 exit 79（`t12a_option_window`、`ux_parity_fix01_gen_group_ui`、`ux_space01_entries`、`ux_space01_group_flow`、`ux_test01_speedtest_message_ui` loader 待 solo；含基线 D 组 ux_parity_fix01/ux_space01）。build 已删，generated_plugin* 噪声已还原。
- **solo 复判 round 1（57 文件，Idle + `-j 1`）：38 转绿（归 flaky/负载敏感，含全部基线 E 组中除 4 个外的其余项，见对比表）→ 19 个仍红**（5 确定性断言：`r4_11` +4 -1、`r4_23` +9 -1、`r4_31` +4 -1、`t11_menu` +1 -1、`t21e_import_forms` +0 ~1 -5；3 did-not-complete：`sp_24`、`wave_b_059_079`、`t21e_responsive`；11 loader 崩溃 exit 79）。19 个进 solo round 2 确认是否翻转。
- **solo 复判 round 2（19 文件）：4 转绿**（`r4_19_contract`、`recheck_r3_prof_menu_target_a`、`wave_b_110_112`、`ux_parity_fix01_gen_group_guard`——归 flaky/loader 抖动）→ **15 个稳定红**。关键：`t18b` round 2 露出真实形态 `+2 -1`（`failed apply…` 断言失败，与基线 C4 一致；首轮的 did-not-complete 系 Idle 噪声）；`wave_b_059_079` 在 exit1/exit79 间翻转（重窗体 flaky）。
- **全量汇总：349 文件 → 首轮 292 绿；经两轮 solo，334 文件至少一次绿，15 文件稳定红，0 HANG。**

### 与 BASELINE.md Flutter 清单的逐项对比

#### C 组：确定性断言失败（基线 10 个测试 / 6 文件 → 现状 6 文件仍红，0 新增）

| 基线项 | 现状 | 结论 |
|---|---|---|
| r4_11 `edit-save-reopen`（窗口未关闭） | +4 -1，仍红 | 基线本来就红；真 bug 嫌疑最大，阶段 2 诊断（`SyntheticRuntimeBridge.applyActive` 默认成功，阻塞在保存链别处；B14 D2-01 改过同一路径但基线时已红） |
| r4_23 `log filtering…` Expected 2 Actual 0 | +9 -1，仍红 | 基线本来就红；**非 B16 引入**：`logCoalesceWindow`（150ms，`monitor_incremental.dart:14`）早在基线前提交 `467607e` 即存在，基线 `6617f47` 源码已含该 Timer；测试在 `emitLogs` 后同步读 state，flush 尚未发生。判**陈旧测试**，阶段 2 测试侧等待 flush 窗口 |
| r4_31 `log read model stays bounded` Expected 2000 Actual 0 | +4 -1，仍红 | 同上，同一根因（`_onLogs` 缓冲 + `Timer` 异步落盘），判**陈旧测试** |
| t18b `failed apply…`（缺 E_NO_ACTIVE_PROFILE 文案） | +2 -1，仍红 | 基线本来就红；R4-30（基线前 `393fafd`）已把状态栏错误改为本地化文案、code 只进"查看失败"弹窗（`status_bar_view.dart:435-438`），测试仍断言裸 code。判**陈旧测试** |
| t21e_import_forms 5 个（bridge 未初始化） | +0 ~1 -5，仍红 | 基线本来就红（环境缺失类）；根因是测试 harness 写法：`setUpAll` 里的 `markTestSkipped` 只记一个 `~1`，5 个测试体照跑照挂；同目录 `t21e_import_real_bridge_test` 把守卫放进每个 test 体内则正确全 skip。判** harness 缺陷（测试侧修）** |
| t11_menu `路由模式:` 0 命中 | +1 -1，仍红 | 基线本来就红；`路由模式:` 字串在 `lib/` 零命中，SP-19（基线前）已把状态栏改为只显示值（`status_bar_view.dart:151-154` 显示 `Rule/Global/Direct`）。判**陈旧测试** |

#### D 组：确定性加载失败（基线 5 文件 → 现状：2 仍红、1 转 flaky、1 已修好、1 转 solo 绿）

| 基线项 | 现状 | 结论 |
|---|---|---|
| r3_visual_dpi_shell_150 | 首轮 exit 79，solo 转绿 | 由确定性转为 flaky-loader（改善；疑前批窗口/DPI 相关修复见效，待观察） |
| r4_07_contract | 两轮 solo 仍 exit 79 | 仍红（loader 崩溃；重窗体类，与 D 组同类） |
| ux_parity_fix01_gen_group_guard | solo2 转绿 | 转 flaky（改善） |
| ux_space01_group_flow | 两轮 solo 仍 exit 79 | 仍红（loader 崩溃） |
| sp_16_group_chip_menu | 首轮即绿 | **已被前批修好**（profiles 域，疑 B13 D1 相关；首轮 exit 0 为证） |

#### E 组：flaky（基线 9 文件 → 现状：5 确认 flaky、4 转稳定红）

| 基线项 | 现状 | 结论 |
|---|---|---|
| fix03c_proxy_chain_editor | solo 转绿 | flaky 确认 |
| fix08_option_cancel | 首轮即绿 | flaky（本轮未发作） |
| profiles_drag_scroll_up | 两轮 solo 仍 exit 79 | 转稳定红（loader；重窗体拖拽类） |
| t09_sub_edit | solo 转绿 | flaky 确认 |
| t21e_dialogs_responsive | 首轮即绿 | flaky（本轮未发作） |
| wave_b_grpc_052 | solo 转绿 | flaky 确认 |
| wave_b_settings_059_079 | exit1/exit79 翻转，仍红 | 转稳定红（重设置窗体；两形态间抖动） |
| wave_b_settings_110_112 | solo2 转绿 | flaky 确认 |
| sp_24_settings_entry | 两轮 solo 仍 exit 1（did-not-complete） | 转稳定红 |

#### 新增红（基线清单无，现稳定红；均为崩溃型 exit 79/1-did-not-complete，无新增断言失败）

| 文件 | 形态 | 归属判断 |
|---|---|---|
| profiles_keyboard_test | exit 79 loader ×2 | 无法归属到逻辑批次：崩溃型（tester 加载期崩溃），基线已记录同类"flutter_tester 并发脆弱/原生资源泄漏"；Idle 下稳定复现。阶段 2 按 loader 根因修（测试卫生），不算某批次回归 |
| r3_visual_dpi_windows_test | exit 79 loader ×2 | 同上（DPI 矩阵重窗体，D 组同类） |
| repair/r4_11_repro_test | exit 79 loader ×2 | 同上（R4-11 设置域；与 r4_11_contract 真 bug 是否同源待阶段 2 看——loader vs 断言，形态不同，暂列两项） |
| t21e_responsive_test | exit 1 did-not-complete ×2 | 同上（shell 多尺寸重构建，崩溃型） |

**新增红结论：无新增确定性断言失败；4 个新增均为崩溃型（Idle 下 tester 加载/构建期崩溃），不能归属 B13/B14/B15/B16/B07b 任一批的逻辑改动。**
**已被前批修好的**：`sp_16_group_chip_menu`（D5，全绿）；`r3_visual_dpi_shell_150` 与 `ux_parity_fix01` 由确定性转为偶发（改善）。

## 3. 阶段 1 结论与阶段 2 计划

- 阶段 1 完成：门禁（format 1 处 B16 遗留 / analyze 绿）、全量 349 串行、两轮 solo、对比表（见上）。15 稳定红：6 确定性断言 + 9 崩溃型；42 flaky 无需修。
- 阶段 2 修复计划（test-first，小步 commit；范围仅 `apps/desktop/lib/features/**` 与 `test/**`）：
  1. `lib/app/app.dart`：跑 `dart format`（B16 长行）。
  2. r4_23 + r4_31（陈旧测试）：`emitLogs` 后等待 flush（`logCoalesceWindow` 150ms，取 300–500ms 裕量）再读 state；先复现红→改→绿。
  3. t11_menu（陈旧测试）：`路由模式:` 断言改为 SP-19 值显示（`Rule` 等）+ 保留 key 断言。
  4. t18b（陈旧测试）：断言本地化文案（`errorKeyText('error.no_active_profile')`），code 改去"查看失败"弹窗断言。
  5. t21e_import_forms（harness 缺陷）：守卫移入每个 test 体（仿 `t21e_import_real_bridge_test`），无 DLL 时干净 skip。
  6. r4_11 `edit-save-reopen`（真 bug 嫌疑）：实跑诊断保存链阻塞点→最小修复。
  7. 9 个崩溃型 loader：逐个找根因（重窗体构建、定时器、全局状态），能确定性修则修；Idle 下不可复现根因的如实记录 + 给 UX/后续批次建议，不硬改。
  8. UX/行为取舍一律不动，记入报告。

## 4. 实际运行过的命令

- `git -C C:\Users\Colby\work\v2rayn-audit\repo worktree add C:\Users\Colby\work\v2rayn-audit\worktrees\TH-DART -b fix/TH-DART audit/2026-10-13`
- `flutter pub get --enforce-lockfile`（`apps/desktop`，exit 0）
- `dart format --output=none --set-exit-if-changed lib test`（exit 1，见 §1）
- `flutter analyze`（No issues found）
- 全量串行：`run_chunk.ps1 -Start {1-2,3-3,4-52,53-102,103-152,153-202,203-252,253-302,303-349}`（Idle + `-j 1`，后台逐 chunk；每 chunk 后删 `apps/desktop/build` + `checkout --` 还原 generated_plugin*）
- solo 复判：`run_chunk.ps1 -Start 1 -End 57 -List solo_list.txt -Tag solo-`；`run_chunk.ps1 -Start 1 -End 19 -List solo2_list.txt -Tag solo2-`
- 清理：`git checkout --` 还原 generated_plugin* 3 文件（多次）；删除 `apps/desktop/build`（多次）
- 未运行：cargo 全系、`flutter build`、全量 analyze 之外的任何重活；阶段 2 修复（接下做）
