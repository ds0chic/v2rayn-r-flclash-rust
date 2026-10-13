# TH-DART-FINAL2：U-FIX 后最终复验（audit/2026-10-13 @ 5bbc070，2026-10-14）

worktree `worktrees\VERIFY-FINAL`（分支 `verify/final-5bbc070`，基点 `5bbc070`，rev-parse 已确认；完工后删除）。
方法同上一轮：逐文件串行（Normal 优先级、`-j 1`，300s 超时记 HANG——实际 0 HANG；3 个 scan 排除）。产物 exe 未运行。

## 1. 门禁表

| 命令（`apps/desktop`） | 结果 |
|---|---|
| `flutter pub get --enforce-lockfile` | PASS exit 0（lock 未动） |
| `dart format --output=none --set-exit-if-changed lib test` | PASS（514 文件，0 changed） |
| `flutter analyze` | PASS（No issues found, 5.8s） |
| `flutter test` 全量串行 383 文件 | 首轮 348 绿 / 18 exit1 / 17 exit79（§2）；solo 两轮见 §3 |
| `flutter build windows --release` | 见 §5 |

## 2. 全量结果（383 文件；比上一轮 +1）

- 首轮：348 exit 0 / 18 exit 1 / 17 exit 79 / 0 HANG。35 非绿进 solo1。
- solo1：25 转绿 → 10 仍红。solo2：7 转绿 → 3 仍红（r3_wpf_option、r4_07、recheck_menu_b）。
- 追加确认：r3_wpf_option 绿（5/5）；recheck_menu_b 第 5 次跑绿（flaky 实锤）；r4_07 两次 overflow 断言 + 两次崩溃（§4）。
- 上一轮 3 个测试修复（t13/r4_06/rr02_03）：t13、r4_06 本轮首轮即绿；rr02_03 首轮偶发崩溃、solo1 绿——修复有效。
- TH-DART 拆分文件（059/060/079、r3_100_125/150/200）本轮全绿（060/150 各抖动 1 次即绿）。

## 3. 与上一轮对比：U-FIX 9/9 目标全绿

| U-FIX 目标（上一轮 §3A/B） | 本轮 |
|---|---|
| r3_shell_100/125/200（54px 溢出） | 首轮即绿（7/7 行：`[0]`） |
| r4_13_s23 / r4_13_s23_repro | s23 solo1 绿；repro 首轮绿 |
| sp_19_shell_chrome / sp_19_status_viewport | 首轮即绿 |
| r3_picker "vless" 截断 | 首轮即绿（type 列加宽有效） |

上一轮 §3C（t13/r4_06/rr02_03 陈旧断言）已合入本基线，无需重修。上一轮 §3D 崩溃残留对照：t05_shell_chrome 本轮 solo2 仍红（一致）；ux_space01_group_flow、profiles_keyboard、r4_11_repro、sp_24、t21e_responsive 本轮全绿（改善）；`profiles_filter_test` 本轮全绿（上一轮报告已注明以实测为准）。

## 4. 残留清单（新增稳定红：仅 r4_07；其余 1 项偶发）

### 4.1 r4_07 context menu 溢出（新增稳定红，归属 U-A，真 bug，UX 范围，未动产品代码）

- 形态：tester 存活时必现断言（2 次同文：55px + 77px 横向溢出，creator `Row ← Column ← LayoutBuilder … Material` 菜单行）；另 2 次 tester segfault。4 轮 0 绿（上一轮：1 次 201px 溢出 + 崩溃；e45c6c7 时 2 次绿）。
- 测试与菜单模型（`context_menu.dart`）区间内零改动；菜单渲染器（`profiles_table.dart:1073/1204-1234`）被 U-A/U-B1 改过，关键行：UX2-04（`2fa9e54`）把菜单项字号 12 → `AppTokens.fontSize = 12.5`（`app_theme.dart:40`），叠加 UX2-05 CJK/mono 字体栈、UX2-08 焦点环复用 marker 槽 → 条目变宽 → 超出 overlay 约束。
- 结论：真 bug（回归），修属 UX 视觉取舍（overlay 宽度/字号/省略）→ **未动产品代码，请总控交 UX**。修好标准：r4_07 在本机连续 3 次绿（含崩溃轮次统计）。

### 4.2 recheck_r3_prof_menu_target_b（偶发崩溃，非稳定红）

- 4 连崩后第 5 次全绿。flaky 实锤（同 tester segfault 家族），未动。建议集成机空闲复测。
- 其余抖动（各绿过 ≥1 次）：t11、r4_11（他测）、060、r3_wpf_option、t09 系、fix02、drag 系、ub2 系等——环境崩溃抖动，非回归。

## 5. 构建结果

`flutter build windows --release` **exit 0**（131.2s）。产物 `build\windows\x64\runner\Release\`：`v2rayn_desktop.exe` + 7 dll（`bridge_api.dll`、`flutter_windows.dll` + 5 插件）+ `data/{app.so, icudtl.dat, flutter_assets/}`。齐全。**产物未运行**（硬约束）。

## 6. 实际运行命令

- `git worktree add .../VERIFY-FINAL -b verify/final-5bbc070 5bbc070`；两端 rev-parse 确认。
- `flutter pub get --enforce-lockfile`；`dart format ...`（514/0）；`flutter analyze`。
- `run_chunk.ps1 -Start {1-55,…,331-383} -List grun_list.txt -Tag g- -Prio Normal -Root .../VERIFY-FINAL`（7 chunk）；solo `-List gsolo_list.txt -Tag gs1-`（35）、`-List gsolo2_list.txt -Tag gs2-`（10）、`-List gsolo3_list.txt -Tag gs3-`（3）；recheck_menu_b 前台单跑 1 次。
- 取证读：各 logout 尾部；`git log e45c6c7..5bbc070`（菜单/测试/UX）；`git show 2fa9e54`（字号行）；AppTokens 取值。
- 未运行：cargo 全系、产物 exe、3 scan；产品代码零改动（本分支仅将提交本报告）。
