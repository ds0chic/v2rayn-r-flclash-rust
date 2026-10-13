# TH-DART 阶段 1：合并后 Dart 验证（进度版，2026-10-13，已暂停）

> 状态：**PAUSED**（用户在游戏，高负载任务暂停）。本文记录已完成部分与未完成清单，恢复后继续。
> 集成分支 HEAD：`e45c6c7`（B01…B17 全部合入）。worktree：`C:\Users\Colby\work\v2rayn-audit\worktrees\TH-DART`，分支 `fix/TH-DART`。

## 1. 门禁（阶段 1 只验证，不修）

| 命令（`apps/desktop`） | 结果 | 说明 |
|---|---|---|
| `flutter pub get --enforce-lockfile` | PASS（exit 0） | lockfile 未改动；附带的 `windows/flutter/generated_plugin*` 零内容差异已 `checkout --` 还原 |
| `dart format --output=none --set-exit-if-changed lib test` | **FAIL（exit 1）** | 473 文件中 1 个待改：`lib/app/app.dart`。差异仅一处：`final themeEnv = evidenceStringHook(Platform.environment, 'V2RAYN_R_THEME');` 长行需折行。归属：**B16**（最后触碰该文件的是 `1110a63 fix(app): armed-only evidence env guards [D3b-01]`）。阶段 2 用 `dart format` 修 |
| `flutter analyze` | PASS | `No issues found!`（9.4s） |

## 2. 全量 test/ 串行跑（进行中，已暂停）

- 清单：`test/` 下共 352 个 `*_test.dart`，排除 3 个 scan（`scan_image_qr`、`scan_screen_qr`、`ux_parity_fix05_scan`）后 **349 个待跑**（基线时 327 个，B13–B16 新增约 22 个）。
- Runner：`scratchpad/run_chunk.ps1`（逐文件 `flutter test <file>`，单文件 300s 超时记 HANG 并 `taskkill /T` 只杀自有子进程树，CSV 记录 exit/耗时/尾部输出）。注意：必须用 **pwsh** 调用，`powershell` 5.1 下 `$p.ExitCode` 取不到（已踩坑，冒烟时 exit 列为空，改 pwsh 后正常）。
- 运行记录文件（scratchpad）：`run_list.txt`（349 行）、`results_1-2.csv`。
- 已完成：冒烟 2 文件全绿——`test\audit_r2_flows_test.dart`（exit 0，10/10）、`test\b07b_bridge_seams_test.dart`（exit 0，9/9）；`run_chunk.ps1 -Start 3 -End 3`（`b13_d102_picker_order_test`，exit 0）。
- chunk `4-52`（49 文件，Idle + `-j 1` 串行）：44 exit 0；4 exit 1（均为 did not complete：`fix02_tuic_editor`、`fix03c_proxy_chain_editor`、`fix08c_routing_draft`、`fix15c_autostart_timing`——Idle 下疑似饿死，待 solo 复判）；1 exit 79（`fix08_option_apply_test`，`No tests were found`——疑似 D 组同类 loader，待 solo 复判）。build 已删，generated_plugin* 噪声已还原。
- 未完成（低负载恢复后按序跑）：chunk `53-102`、`103-152`、`153-202`、`203-252`、`253-302`、`303-349`，共 297 文件（第 1–52 行已跑：52/349）。跑完每 chunk 后删 `apps/desktop/build` + 还原 generated_plugin* 噪声。
- 与 `BASELINE.md` Flutter 清单（10 deterministic / 5 load failures / 9 flaky）的逐项对比：**待跑完后补**。

## 3. 未完成清单（恢复后继续）

1. 跑完 7 个 chunk（逐文件串行，绝不并行）。
2. 失败文件立即 solo 再跑一次区分 flaky；hang（>300s）记录并继续。
3. 汇总通过/失败/加载失败/flaky，与 BASELINE 逐项对比：已修好 / 仍红 / 新增红（新增红给归属批次判断：B13/B14/B15/B16/B07b）。
4. 定稿本文件（去掉"进度版"标记），进入阶段 2（测试健康修复）。

## 4. 实际运行过的命令

- `git -C C:\Users\Colby\work\v2rayn-audit\repo worktree add C:\Users\Colby\work\v2rayn-audit\worktrees\TH-DART -b fix/TH-DART audit/2026-10-13`
- `flutter pub get --enforce-lockfile`（`apps/desktop`，exit 0）
- `dart format --output=none --set-exit-if-changed lib test`（exit 1，见 §1）
- `flutter analyze`（No issues found）
- runner 冒烟：`run_chunk.ps1 -Start 1 -End 2`（2/2 绿）
- 清理：`git checkout --` 还原 generated_plugin* 3 文件；删除 `apps/desktop/build`
- 未运行：全量 chunk 串行（已暂停）、阶段 2 任何修复
