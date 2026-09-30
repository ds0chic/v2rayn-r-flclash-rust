# T01 依赖冻结 — Flutter / Rust / FRB

状态：accepted（T01）
范围：本文件记录 T01 实际锁定并通过门禁的依赖版本与来源。后续任务升级任何一项都必须新建 ADR。

## 1. 工具链

| 工具 | 版本 | 来源 / 备注 |
|---|---|---|
| Flutter | 3.47.5 stable（engine af7e796e16, framework 6a19cca564） | `C:\Users\Colby\toolchains\flutter` |
| Dart | 3.13.4 | 随 Flutter 3.47.5 |
| Rust | 1.98.1 stable-x86_64-pc-windows-msvc | `C:\Users\Colby\.cargo\bin` |
| flutter_rust_bridge_codegen | 2.13.0 | `C:\Users\Colby\.cargo\bin\flutter_rust_bridge_codegen.exe` |
| VS BuildTools | 2022 17.14（CMake 4.4.3 / Ninja 1.13.2 / Win10 SDK 10.0.26100） | 构建 Windows x64 release |

## 2. Rust 依赖（`Cargo.lock`）

| crate | 版本 | 用途 |
|---|---|---|
| flutter_rust_bridge | 2.13.0 | bridge 运行时（workspace.dependencies 精确 `"=2.13.0"`） |
| rustls-pemfile | 2.2.0 | S3：解析浏览器根证书 PEM（dev-dependency） |
| rustls-native-certs | 0.8.4 | S3：读取系统信任库（dev-dependency） |
| windows | 0.58.0（features: Win32_Foundation, Win32_Security, Win32_System_Threading） | S5：`CreateMutexW` 单实例探针（dev-dependency） |

workspace 成员：`crates/domain`、`crates/application`、`crates/bridge_api`。

## 3. Flutter/Dart 依赖（`apps/desktop/pubspec.lock`）

| package | 版本 | 用途 |
|---|---|---|
| flutter_rust_bridge | 2.13.0 | 与 Rust crate / codegen 同版本 |
| flutter_riverpod | 3.4.3 | 唯一状态管理 |
| riverpod | 3.4.3 | 传递依赖 |
| two_dimensional_scrollables | 0.5.5 | 二维虚拟表格（flutter.dev 维护） |
| flutter_lints | 6.0.0 | 静态分析 |
| cupertino_icons | 1.0.9 | 默认图标 |

FRB 三处版本一致性：Dart `flutter_rust_bridge 2.13.0` = Rust `Cargo.lock flutter_rust_bridge 2.13.0` = codegen `flutter_rust_bridge_codegen 2.13.0`。

## 4. FRB wiring 布局决策

- bridge crate 物理位于 `crates/bridge_api`（符合 AGENTS 目录约定），包名 `bridge_api`，`crate-type = ["cdylib","staticlib","lib"]`。
- Flutter 侧构建胶水位于 `apps/desktop/rust_builder/`（FRB `create` 生成），其 `windows/CMakeLists.txt` 通过 cargokit 指向 `crates/bridge_api`：
  `apply_cargokit(${PROJECT_NAME} ../../../../../../../../crates/bridge_api bridge_api "")`。
  该相对路径已由真实 `flutter build windows --release` 验证（产物 `Release/bridge_api.dll`，PE machine `0x8664`）。
- codegen 配置：`apps/desktop/flutter_rust_bridge.yaml`（`rust_input: crate::api`，`rust_root: ../../crates/bridge_api/`，`dart_output: lib/bridge`）。
- 重建可复现：`flutter_rust_bridge_codegen generate` 二次运行后 `lib/bridge/**` 与 `crates/bridge_api/src/frb_generated.rs` 哈希完全一致（见 `docs/evidence/T01.md` §依赖冻结）。

## 5. 测试方法学约束（重要）

锁定的 Flutter 3.47.5 在本机使用软件渲染时，`flutter_tester` 会在若干次重度 `pumpWidget` 后随机/累积地以 `0xC0000005`（exit `-1073741819`）崩溃，且无 Dart 侧异常。实测：单个 `MaterialApp(Text)` 建树连续约 25 次即崩溃；单个真实页面构建约 10–30% 概率崩溃。该行为与测试内容无关，属引擎/环境级问题（详情与实测矩阵见 `docs/evidence/T01.md`）。

因此本任务采用：
- 每个 widget 测试文件只做一次页面构建（`test/support/profiles_harness.dart` 共用；`test/support/` 不以 `_test.dart` 结尾，不被收集）。
- `apps/desktop/dart_test.yaml`：`concurrency: 1`。
- `tools/flutter_test_retry.ps1`：在门禁层重试 `flutter test`，逐次保留原始日志。
- 视窗改为响应式（工具条/状态栏可横向滚动）以消除 `RenderFlex overflow` 造成的确定性失败。

这不是对需求的分母削减：所有要求的键鼠场景仍由 WidgetTester 在真实 10k 行虚拟表格上驱动；重试只用于规避引擎崩溃。
