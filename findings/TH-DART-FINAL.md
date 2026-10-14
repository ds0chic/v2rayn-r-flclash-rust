# TH-DART-FINAL：最终集成 Dart 验证（audit/2026-10-13 @ a2f7cf8，2026-10-14）

worktree `worktrees/VERIFY-DART`（分支 `verify/dart-a2f7cf8`，基点 `a2f7cf8`，rev-parse 已确认；完工后删除）。
方法沿用 TH-DART：逐文件串行（Normal 优先级、`-j 1`，单文件 300s 超时记 HANG——实际 0 HANG；scan 3 文件排除）。产物 exe 未运行。

## 1. 门禁表

| 命令（`apps/desktop`） | 结果 |
|---|---|
| `flutter pub get --enforce-lockfile` | PASS exit 0（lock 未动；本次无 generated_plugin 噪声，EOL 已统一 LF） |
| `dart format --output=none --set-exit-if-changed lib test` | PASS（513 文件，0 changed） |
| `flutter analyze` | PASS（No issues found, 5.9s） |
| `flutter test` 全量串行 382 文件 | 首轮 339 绿 / 25 exit1 / 18 exit79（§2）；两轮 solo 后见 §3 |
| `dart analyze`（改动 3 文件作用域） | 无问题（format 0 changed 一并确认） |
| `flutter build windows --release` | **exit 0**，136.5s（§5） |

## 2. 全量结果（382 文件，含 UX 新增 u_b1_/ux_ub2_/ux_u_c1_/u_c2_/t17 等 35 个）

- 首轮：339 exit 0 / 25 exit 1 / 18 exit 79 / 0 HANG。43 非绿进 solo1。
- solo1：23 转绿 → 20 仍红。solo2：5 转绿（r3_windows_150、r4_06、t13、ub2_02、ub2_11）→ 15 仍红。
- TH-DART 阶段 6 确定性修复（r4_23/r4_31/t11/t18b/t21e/r4_11）与 6 拆分文件（059/060/079、r3_100_125/150/200）在本基线首轮全绿，仅 060/150/060 系偶发崩溃抖动（solo 即绿）；t18b 首轮/solo 连红 3 次后前台单跑 3/3 绿——断言内容有效，红为 tester 崩溃。
- r3_windows_150（TH-DART 拆分）在本基线 solo1 转绿，拆分继续有效。

## 3. 稳定红对比表（15 项）与归属

### A. 54px 溢出家族（8 文件，确定性断言，全新，归属 U-B1）

`r3_visual_dpi_shell_{100,125,150,200}`、`r4_13_s23_contract`、`repair/r4_13_s23_repro`、`repair/sp_19_shell_chrome`、`repair/sp_19_status_viewport`——solo 两轮一致文本 8 次：`RenderFlex overflowed by 54 pixels on the right`，首迭代（1280×800 @100% 首 layout）即爆。与 scale 无关的 54 逻辑像素 → UX 新增固定宽度内容。区间 `e45c6c7..a2f7cf8` 内状态栏加宽提交：UX1-10（selector group captions“模式/方案”）、UX1-09（常驻 speedtest 进度）、UX2-02（连接徽章）、UX1-04（可操作错误）。**判定：真 bug 嫌疑（违 SP-19 800px 无溢出契约），但修属产品布局取舍 → 未动产品代码，交总控/UX 裁决。**

### B. r3_picker "vless" 截断（1 文件，全新，归属 U-A/U-B1 存疑 → UX）

首迭代即 `didExceedMaxLines`（`truncated text "vless"`）；测试文件区间内未动。5 字符徽章被截断疑字体/列宽变更（UX2-05 字体栈 / UX2-04 字阶 / 密度）。修属 UX 取舍 → 未动，交裁决。solo1 断言失败、solo2 崩溃，叠加 tester 抖动。

### C. U-B1 danger-gate 陈旧断言（3 文件，已修，归属 U-B1 UX1-03）

`t13_statusbar`、`r4_06_contract`、`recheck_rr02_03`（2 widget 测）——UX1-03 给 forcedChange/Pac 选择加 `showDangerConfirm`，旧测试不断言确认框直接断 bridge。测试侧补确认步骤（`sysproxy-apply-confirm` + `danger-confirm-ok`），覆盖不降：
`t13` +1 -1 → 2/2；`r4_06` +0 -1 → 绿；`rr02_03` +3 -2 → 5/5。3 commits（bbdb23d、e2608d0、8ccb960）。

### D. tester 崩溃残留（3 文件，基线即有， flaky/环境）

`r4_07`、`t05_profiles_ui`、`t05_shell_chrome`（另 `ux_parity_fix01_gen_group_ui` solo1 红、solo2 红——计入则 4）：多轮 exit79/did-not-complete，零断言失败；TH-DART 已证此类为 flutter_tester  segfault 竞争（r4_07 有全绿记录）。未动。建议集成机空闲 normal 复测。
对照 TH-DART 15 稳定红清单：ux_space01_group_flow、profiles_keyboard、r4_11_repro、sp_24、t21e_responsive、wave_b_059_079（group）已在本基线转绿或被拆分覆盖；`profiles_filter_test` 本轮全绿（协调人括号清单与实测略有出入，以实测为准）；t05_shell_chrome 两边皆红（一致）。

## 4. 修复清单（本批，test-only，共 3 commits）

| commit | 内容 | 验证 |
|---|---|---|
| bbdb23d | t13 确认 danger gate 后断言 | 2/2 绿 |
| e2608d0 | r4_06 同上（Pac + E_NO_RUNNING_SESSION） | 绿 |
| 8ccb960 | rr02_03 两 Pac 流程确认（controller restore 测未动） | 5/5 绿 |

范围：仅 `test/t13_statusbar_test.dart`、`test/r4_06_contract_test.dart`、`test/recheck_rr02_03_test.dart`。format 0 changed。未动产品代码、无新依赖。

## 5. 构建结果

`flutter build windows --release` exit 0（136.5s）。产物 `build\windows\x64\runner\Release\`：`v2rayn_desktop.exe`（0.2MB launcher）、`flutter_windows.dll`（20.3MB）、`bridge_api.dll`（21.7MB）、5 插件 dll、`data/{app.so, icudtl.dat, flutter_assets/(7项)}`。齐全。**产物未运行**（硬约束）。

## 6. 实际运行命令

- `git worktree add .../VERIFY-DART -b verify/dart-a2f7cf8 a2f7cf8`；`rev-parse` 确认两端。
- `flutter pub get --enforce-lockfile`；`dart format --output=none --set-exit-if-changed lib test`；`flutter analyze`。
- `run_chunk.ps1 -Start {1-55,…,331-382} -List frun_list.txt -Tag f- -Prio Normal -Root .../VERIFY-DART/apps/desktop`（7 chunk 后台串行；每 chunk 后删 build）。
- solo：`-List fsolo_list.txt -Tag fs1-`（43）、`-List fsolo2_list.txt -Tag fs2-`（20）； gate 验证 `-List gate3_list.txt -Tag gate3-`（3）；t18b 前台单跑 1 次。
- 诊断性读取：各 logout/logerr 尾部；`git log e45c6c7..a2f7cf8`（状态栏/测试文件/UX 批次）；因取证写过 1 个临时 probe（`scratch_dpi_probe_test.dart`，跑后即删，未提交）。
- `flutter build windows --release`（后台 1 次）。
- 未运行：cargo 全系、产物 exe、3 个 scan 测试。
