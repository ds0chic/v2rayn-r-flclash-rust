# R4-11 命令记录

工作目录：`apps/desktop`；Flutter：`C:\Users\Colby\toolchains\flutter\bin\flutter.bat`。

| 命令 | 结果 |
|---|---|
| `flutter analyze` | No issues found! (ran in 3.5s) |
| `flutter test test/repair/r4_11_repro_test.dart --reporter expanded` | 修复前：用例1 失败（缺 `fakeip-toggle` 键）；用例2 禁用 merge 时失败（`EnableCacheFile4Sbox isTrue`）。修复后：All tests passed! (+2) |
| `flutter test test/r4_11_contract_test.dart --reporter expanded` | All tests passed! (+5) |
| `flutter test test/repair/r4_08_repro_test.dart` | All tests passed!（回归，未受影响） |
| `flutter test test/r4_08_contract_test.dart` | All tests passed!（回归） |
| `flutter test test/r4_17_contract_test.dart test/r4_27_contract_test.dart` | All tests passed!（回归切片） |
| `flutter test test/fix16_settings_field_test.dart test/r3_wpf_option_window_test.dart test/fix08_option_apply_test.dart test/fix08_option_cancel_test.dart test/fix08_option_error_test.dart` | All tests passed!（+11，settings 回归） |
| `python -c "import yaml; yaml.safe_load(...)"` | YAML OK（fields.settings.yaml / fields.entities.yaml） |

批跑注意：`flutter test test/r4_11_contract_test.dart test/repair/r4_11_repro_test.dart` 同进程会因 flutter_tester 资源泄漏出现 `did not complete`（与 R4-08 记录一致）；单文件运行均通过。

## 复现方法（先失败后通过）
1. 用例1：修复前无 `fakeip-toggle`/`global-fakeip-toggle` 键 → `Found 0 widgets with key`。加键后通过。
2. 用例2：将 `option_setting_window.dart` 的 `_draft = mergeWithSettingsDefaults(document)` 临时改回 `_draft = document` → 失败于 `EnableCacheFile4Sbox isTrue`；恢复后通过。

说明：`flutter test` 偶发 exit 79 / `did not complete`，单独重试即可。本轮未运行 Rust 门禁：本次未改 `crates/**`，Rust 仅登记潜在字段缺口（见任务卡状态）。未运行 `flutter build windows --release`（本卡只改 Dart UI 默认/联动，不涉及原生/运行）。
