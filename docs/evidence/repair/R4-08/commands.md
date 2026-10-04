# R4-08 命令记录

工作目录：`apps/desktop`；Flutter：`C:\Users\Colby\toolchains\flutter\bin\flutter.bat`。

| 命令 | 结果 |
|---|---|
| `flutter analyze` | No issues found! |
| `flutter test test/r4_08_contract_test.dart` | All tests passed! |
| `flutter test test/repair/r4_08_repro_test.dart` | 修复后 All tests passed!（基线经 `git stash push -- apps/desktop/lib/features/profiles/profiles_table.dart` 回退后失败于 line 54，`git stash pop` 恢复） |
| `flutter test test/profiles_select_basic_test.dart test/profiles_pointer_test.dart test/profiles_keyboard_test.dart test/profiles_drag_select_test.dart test/profiles_drag_select_up_test.dart test/profiles_drag_scroll_up_test.dart test/profiles_drag_scroll_down_test.dart` | All tests passed!（+7） |
| `flutter test test/recheck_r3_prof10_default_selection_test.dart test/table_actions_test.dart test/profiles_models_test.dart test/ux_parity_fix01_command_context_test.dart` | 实际用例 +16 通过；`table_actions_test` 首次为加载期 "Connection closed before test suite loaded"（已知资源泄漏/偶发），单独重跑 All tests passed! |
| `flutter build windows --release` | 未运行（本次仅 Dart UI 改动，无原生/Rust 改动；门禁要求仅原生改动需构建） |

说明：`flutter test` 偶发 exit 79，重试即通过；多文件同进程并发 MainShell 构建会触发 flutter_tester 已知泄漏崩溃，故按文件单独运行。
