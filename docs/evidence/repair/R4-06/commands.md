# R4-06 命令记录

工作目录：`apps/desktop`；Flutter：`C:\Users\Colby\toolchains\flutter\bin\flutter.bat`。

| 命令 | 结果 |
|---|---|
| `flutter analyze` | No issues found! |
| `flutter test test/r4_06_contract_test.dart` | All tests passed! |
| `flutter test test/t13_statusbar_test.dart test/recheck_rr02_03_test.dart test/t15a_statusbar_test.dart` | t13(2) + recheck(5) 通过；末个 t15a 用例在同进程第 9 个用例处 "did not complete"（已知 flutter_tester 泄漏），单独重跑 All tests passed! |
| `flutter test test/t05_shell_chrome_test.dart` | All tests passed! |
| `flutter test test/r3_visual_dpi_shell_test.dart` | All tests passed!（100/125/150/200% DPI × 三布局 + 800x600 可达） |
| `flutter build windows --release` | 未运行（本次仅 Dart UI/token 改动，无原生/Rust 改动；门禁要求仅原生改动需构建） |

说明：`flutter test` 偶发 exit 79，重试即通过；多 MainShell 页面构建同进程并发会触发已知 flutter_tester 资源泄漏崩溃，故按文件单独运行。
