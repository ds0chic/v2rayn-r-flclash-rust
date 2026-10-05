# R4-04 命令日志

工作目录：`apps/desktop` 或仓库根（见每行）。环境：Windows PowerShell 7；Flutter `C:\Users\Colby\toolchains\flutter\bin\flutter.bat`；Cargo `C:\Users\Colby\.cargo\bin\cargo.exe`。

```
# apps/desktop
flutter analyze
# -> No issues found! (EXIT=0)

# apps/desktop
flutter test test/r4_04_contract_test.dart
# -> All tests passed! (6) (EXIT=0)

# apps/desktop (回归；并行 5 套件时 r4_08/t18b 偶发 "did not complete"，单独重跑通过)
flutter test test/r4_01_contract_test.dart test/r4_02_contract_test.dart test/r4_08_contract_test.dart test/r4_27_contract_test.dart test/t18b_runtime_ui_test.dart
flutter test test/r4_08_contract_test.dart   # All tests passed
flutter test test/t18b_runtime_ui_test.dart  # All tests passed

# 仓库根
cargo fmt --all -- --check                  # clean (EXIT=0)
cargo clippy --workspace --all-targets --locked -- -D warnings
# -> Finished `dev` profile ... (EXIT=0, no warnings)
cargo test --workspace --locked
# -> all test result: ok ... (EXIT=0)
#    新增 net_host_client 单测：request_slot_deadline_includes_queue_wait / request_slot_does_not_accumulate_workers / net_host_client_exposes_worker_counts 通过
cargo fmt --all                             # 提交前格式化（无输出）

# apps/desktop
dart format lib/features/runtime/runtime_controller.dart lib/features/runtime/runtime_bridge.dart test/r4_04_contract_test.dart

# 构建前实例检查（只读，不杀进程）
Get-Process -Name v2rayn_desktop,net_host,v2rayN
# -> net_host PID 32340 (dist\v2rayN-R-1.0.0+1-windows-x...)，非本卡启动、未锁 build\，未停止
# -> v2rayN PID 11728 (上游 C# 程序)，未触碰

# apps/desktop
flutter build windows --release
# -> Built build\windows\x64\runner\Release\v2rayn_desktop.exe (EXIT=0)
```
