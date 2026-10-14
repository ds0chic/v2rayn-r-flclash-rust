# TH-DART-FINAL4 — D-FINAL 全量核验（`verify/d-final` @ step-1 后）

- 基线：`3da0cce`（merge: D-LEFT2）＋ docs step-1（`2368af8`，8 项已决维持现状）。
- 日期：2026-10-14；Flutter 3.47.5 / Dart 3.13.4；测试端口 ≥11808；未碰 10808／系统代理／密钥；未 push。

## 门禁表

| 门 | 命令 | 结果 |
|---|---|---|
| pub get | `flutter pub get --enforce-lockfile` | OK（Got dependencies!） |
| format | `dart format --output=none --set-exit-if-changed lib test` | 0 changed（521 files），exit 0 |
| analyze | `flutter analyze` | No issues found!（5.4s），exit 0 |
| 全量测试 | 393 个 `*_test.dart`（递归）串行单文件，`-j 1`＋Idle 优先级＋`flutter.lock`＋300s/文件 | 见下（全部绿，无稳定回归） |
| release 构建 | `flutter build windows --release` | exit 0，132s；exe＋7 DLL＋`data/{flutter_assets,app.so,icudtl.dat}`；exe 未运行；`build/` 已删 |

## 计数

- 测试文件：393（`apps/desktop/test` 递归 `*_test.dart`）。
- 执行目标：391 行（7 个缺 `bridge_api.dll` 文件按令跳过不跑；2 个 scan 文件各按 `--plain-name` 拆 4＋3 个子跑）。
- sweep 首轮：358 PASS／1 FAIL／32 LOADFLAKE。
- solo 裁决（≤3 次＋记录，`dfinal_solo.csv`）：33 个标签中 30 个回绿；3 个顽固 loader-flake 第 4 次回绿（见分类）。
- 终态：391/391 运行目标绿；9 个 doctrine-skip（7 nodll＋2 scan-hang）；**0 稳定回归**。

## 非绿名单＋分类（全 baseline-flake，无 regression／stale）

| 文件 | 首轮 | solo 记录 | 分类＋证据 |
|---|---|---|---|
| `scan_screen_qr_test.dart [capture exception]` | FAIL exit 1 | solo 第 1 次 exit 0 綠 | flake：D-LEFT 改动文件，solo 即绿，无断言失败文本 |
| `d_left_error_keys_test.dart` 等 31 个 LOADFLAKE（含 r3-dpi×3、r4_0x、repair、ux_ub2_* 等） | exit 79／"did not complete"（loader 未起跑） | 28 个 ≤3 次内回绿 | flake：`flutter_tester` loader 脆弱性，历史多轮同类 |
| `profiles_drag_select_test.dart` | 79 | 79×3 后第 4 次 exit 0 綠 | flake：loader 连崩后回绿，文件本身无改动 |
| `profiles_keyboard_test.dart` | 79 | 79×3 后第 4 次 exit 0 綠 | 同上 |
| `repair/wave_b_settings_036_044_test.dart` | LOADFLAKE | exit 1／1／79；第 4 次 full-file 仍 exit 1（第 3 个 widget 起 "did not complete"，零断言失败）；第 3 个用例 `--plain-name` 单跑 exit 0 綠 | flake（order-dependent tester crash）：崩溃用例隔离绿；D-LEFT/D-LEFT2 未碰 `option_setting_window.dart` 及其保存链（31202c7 仅碰 settings 内 `theme_setting_dialog`）；5 次运行零 `Expected/Actual` 文本 |

- D-LEFT/D-LEFT2 触及文件（`d_left_*`、`d_left2_*`、`b13_d109`、`r3_root_03`、`t06a_editor_error`、`b15_routing_subs_api`、scan 两文件各子跑）终态全绿。
- 按令跳过（记 SKIP，不跑）：7 个 `bridge_api.dll` 缺失自跳文件（`r4_16`、`t06a_frb_bridge`、`t21e_import_forms`、`t21e_import_real_bridge`、`fix04b`、`fix04_inner`、`fix05_scan`；worktree 内确无此 dll）；2 个 scan 成功路径（`importShareText` 在 `runAsync` 下游既有挂起，TH-DART scan-skip doctrine）。

## 过程备忘（给后轮）

- 必须用 `pwsh`（PS7）跑 sweep harness：Windows PowerShell 5.1 下 `Start-Process -PassThru` 返回对象的 `.ExitCode` 读回为空（实测），导致 exit 列全空；PS7 下正常。本轮因此重跑一次，另耗 ~35min。
- `Start-Process -ArgumentList` 必须传**单个预拼好引号的字符串**；数组形式会把含空格的 `--plain-name` 值拆成目标路径。
- `flutter test` 串行持有仓根 `flutter.lock`（20s 重试／30min 上限／finally 释放）；子进程 Idle 优先级；超时 300s 后只 `taskkill /PID <own> /T /F`。
