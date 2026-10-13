# TH-DART-FINAL3：U-FIX2 定向复验（audit/2026-10-13 @ d4cb8d7，2026-10-14）

worktree `worktrees\VERIFY-FINAL3`（分支 `verify/final3`，基点 `d4cb8d7`，rev-parse 已确认；完工后删除）。
U-FIX2（`status_bar_view.dart` 约 94+/50-，与 U-FIX 同区）merge message 自述“status strip flexes instead of overflowing at narrow edge viewports; r4_07”。
方法：30 文件定向集逐文件串行（Normal、`-j 1`，300s 超时记 HANG——0 HANG）。产物未运行。

## 1. 门禁表

| 命令（`apps/desktop`） | 结果 |
|---|---|
| `flutter pub get --enforce-lockfile` | PASS exit 0（lock 未动） |
| `dart format --output=none --set-exit-if-changed lib test` | PASS（515 文件，0 changed） |
| `flutter analyze` | PASS（No issues found, 5.8s） |
| 定向 30 文件串行 | 首轮 19 绿 / 9 exit1 / 2 exit79；solo 后见 §2 |
| `flutter build windows --release` | 见 §5 |

## 2. 定向结果表

- 首轮绿（19）：r4_07（U-FIX2 目标，首轮即绿——上一轮 4 轮 0 绿）、r3_picker、sp_19_status_viewport、t13、t15a_statusbar、u_b1 ×5、t11 系 3、r3_wpf_main_chrome、recheck_rr02_03、r4_06、u_b1_error_actions、sp_17 ×3、audit_r2_flows。
- solo 转绿（3）：t05_shell_chrome、u_b1_danger_confirm、u_fix2_statusbar_narrow（U-FIX2 自带回归测试，700px 无溢出目标达成）。
- 确定性红（8，§3）：r3_shell_100/125/150/200、r4_13_s23、r4_13_s23_repro、sp_19_shell_chrome、u_fix_status_viewport——全部同一断言：`expectChromeNotTruncated` 抓到脸文案 `不改变系统代理 (未启用)` 被省略（两轮一致，非抖动）。

## 3. 核心发现：U-FIX2 与 U-FIX 的契约冲突（未动产品代码，请总控裁决）

- U-FIX 的测试头文档（`u_fix_status_viewport_test.dart:14-17`）白纸黑字：“At 800 the captioned selectors shrink (captions ellipsize, **faces keep full text** under tooltips); **chrome text never truncates**.”
- U-FIX2 的实现让脸（`status-sysproxy` 值+状态）随 flex 收缩省略，正面违反上述契约；U-FIX2 自带窄测只断言无溢出、不查截断，所以它自己是绿的。
- 旧断言不是“陈旧”：它们正在忠实执行 U-FIX 的已批准契约。若把测试改成接受省略，等于单方面废掉 U-FIX 条款并降低覆盖（禁令明确不降覆盖）→ **测试一律未动**。
- 备选解（都需 UX/总控拍）：脸保全文案另找出路（徽章/caption 再压、800 下隐藏状态后缀、横滚等）。U-FIX2 作者与 UX 定夺后，若采纳“脸可省略”新契约，我可再按新契约 test-first 更新这 8 文件。
- 附带：u_fix_status_viewport 同轮另有一条 `UnmountedRefException（MonitorController）`，疑为截断失败后的 teardown 级联（仅见一次）；若新契约下仍现，需单独立项查 use-after-dispose。

## 4. 其余残留（均已绿 ≥1 次或历史 flaky，无需修）

- r4_07：本轮首轮绿（U-FIX2 的 55/77px 菜单溢出修法有效）；上一轮的溢出断言（U-A 字号 12→12.5）在本基线未复现。
- t05_shell_chrome（历史崩溃户）solo 绿；u_b1_danger_confirm、u_fix2_narrow solo 绿。
- 上一轮点名物：ux_space01、t05（上已述）、recheck_menu_b 上一轮末已自证 flaky。

## 5. 构建结果

`flutter build windows --release` **exit 0**（129.4s）。产物 `build\windows\x64\runner\Release\`：`v2rayn_desktop.exe` + 7 dll（`bridge_api.dll`、`flutter_windows.dll` + 5 插件）+ `data/{app.so, icudtl.dat, flutter_assets/}`。齐全。**产物未运行**（硬约束）。

## 6. 实际运行命令

- `git worktree add .../VERIFY-FINAL3 -b verify/final3 audit/2026-10-13`；rev-parse 确认 `d4cb8d7`。
- `flutter pub get --enforce-lockfile`；`dart format ...`（515/0）；`flutter analyze`。
- 定向集解析（显式 21 + 通配 6 组 + grep import 状 `status_bar_view|StatusBarView` 9 文件 → 去重 30）→ `run_chunk.ps1 -Start 1 -End 30 -List h_list.txt -Tag h-`；solo `-List hsolo_list.txt -Tag hs1-`（3）。
- 取证读：各 logout 尾部；`u_fix_status_viewport_test.dart` 头文档；`u_fix2_statusbar_narrow_test.dart`；`status_bar_view.dart:916-936`（danger gate 无关，复核）；`git log e45c6c7..d4cb8d7`。
- 未运行：cargo 全系、产物 exe、3 scan；产品代码零改动（本分支仅将提交本报告）。
