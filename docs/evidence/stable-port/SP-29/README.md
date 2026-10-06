# SP-29 稳定完整测试门禁 — 证据

状态：implemented（门禁入口已交付并复跑；门禁本身非全绿，见阻塞项，故不写 verified）。

任务 ID：SP-29。唯一用户流程：开发者运行锁定工具链门禁，获得可复现完整结果且没有静默未完成用例。

## 1. 交付物

| 文件 | 说明 |
|---|---|
| `tools/gates/run_all.ps1` | 唯一可复现门禁入口（新建）。逐文件隔离 `flutter test`（递归枚举，含 `test/repair/`）、原生崩溃识别分类并重试到稳定结论、机器可读 JSON（pass/fail/skip/incomplete）、工具链版本写入输出、无静默未完成（结果数 == 库存数断言）。Exit：0 全绿 / 1 断言或静态失败 / 2 仍有未完成。 |
| `apps/desktop/test/repair/sp_29_contract_test.dart` | 本卡正确行为测试（新建，11 用例）：分类器合同（exit 79 / did-not-complete / 加载期崩溃判 incomplete；完成断言失败判 fail；全跳过判 skip）、库存完整性（递归枚举、无静默缺失）、门禁入口存在性。 |
| `docs/evidence/stable-port/SP-29/runs/<stamp>/gate-result.json` | 机器可读结果（3 次运行，见 §3）。 |
| `docs/evidence/stable-port/SP-29/runs/<stamp>/*.log` | 逐文件逐次原始日志 + 5 个静态/Rust 阶段日志。 |

生产代码未改动；`work/`、`outputs/` 未触碰；门禁只 spawn 并等待自有子进程，不按名杀进程、不绑测试端口（Flutter 单测与 Rust loopback 均为进程内传输）。

## 2. 工具链锁定（run-2 实测写入值）

Flutter 3.47.5 stable（Dart 3.13.4，`C:\Users\Colby\toolchains\flutter\bin\flutter.bat`）、
cargo 1.98.1 / rustc 1.98.1（`C:\Users\Colby\.cargo\bin\`）、FRB Dart 2.13.0 / Rust 2.13.0 /
codegen 2.13.0 三处一致、git HEAD `7466bee13f2c82241efb41ebad2cf74368ae09ab`。完整取值见各次
`gate-result.json#/toolchain`。

## 3. 运行记录（真实 exit）

| 运行 | 入口与参数 | 结果 |
|---|---|---|
| run-1 `runs/20261006-235008` | 初版门禁全量（289 文件） | flutter 273 pass / 9 fail / 7 incomplete；Rust 阶段失败。事后确认为两类污染（见 §5），该次 JSON 保留为分类器缺陷证据，不作为结论。 |
| run-2 `runs/20261007-001449` | 修复版门禁全量（290 文件，含 SP-05 并行新增 1 文件）+ `-CompareWith run-1` | 静态 4 阶段全 exit 0；`cargo test --workspace --locked` exit 0（115 suites / 1487 passed / 0 failed / 5 ignored）；flutter 282 pass / 7 fail / 0 skip / 1 incomplete。`reproducibility.mismatches` 逐项列出与 run-1 的 10 处差异（污染+修复+新文件）。 |
| run-3 `runs/20261007-003811` | 同一入口子集复跑（8 个受影响文件，`-SkipStatic -SkipCargo`） | 0 pass / 7 fail / 1 incomplete，与 run-2 对 8 文件结论完全一致（稳定结论可复现）。 |
| 现状基线（门禁前） | `cargo test -p privileged_helper --locked` exit 0（8 passed；含 `idle_connection_times_out_at_the_idle_bound`，HEAD 上合同已对齐，无需再改）；3 个审计 flaky 文件单跑各 exit 0（与审计 did-not-complete 对照，确认 flaky）。 | 基线真实，exit 以重定向到文件方式取真值（管道后 `$LASTEXITCODE` 不可信，门禁内已用文件重定向规避）。 |
| 既有工具抽验 | `flutter test test\t11_menu_test.dart`（与 `tools/flutter_test_retry.ps1 -PerFile` 同调用形）pass；`flutter test test\r4_07_contract_test.dart` exit 79 + did-not-complete，与门禁分类一致。 | 无分歧。 |

## 4. 门禁结果（run-2 全量，结论性）

- pass 282 / fail 7 / skip 0 / incomplete 1，共 290 文件，JSON 条目数 == 库存数，无静默未完成。
- 7 fail 同一根因（各日志均含 1 处 `Content hash on Dart side (-642643966) is different from Rust side`）：
  `r4_16_matrix`、`t06a_frb_bridge`、`t21e_import_forms`、`t21e_import_real_bridge`、
  `ux_parity_fix04_inner`、`ux_parity_fix04b_config_import`、`ux_parity_fix05_scan`。
  预置原生 DLL 与当前生成 Dart 不同步，需重建 FRB 产物（bridge/DLL 属 SP-00 整合者范围，本卡不碰）。
- 1 incomplete：`r4_07_contract_test`，3 次均 exit 79 + did-not-complete（run-3 复现，另见旧入口同形复现 exit 79）。
  flutter_tester 原生崩溃，无 Dart 断言、无原生栈，根因未定（产品断言归属业务卡，本卡只保证显式 incomplete）。
- cargo 5 ignored：提权路由 1（需 elevation，预期内）等；`self_update` 日志 1 行 `Input redirection is not supported` 为被测子进程 stderr 输出，用例本身通过。详见 `cargo-test.log:2140` 前后。

## 5. 修过的门禁自身缺陷（本卡范围内，已修并验证）

1. 初版分类器把“加载期崩溃”（`Failed to load ... Connection closed before test suite loaded`，exit 1）误判为 fail（run-1 中 `r4_06`/`r4_07` 第三次尝试）。已加标记并在 `sp_29_contract_test.dart` 锁定；run-2 复核 `r4_06` 转 pass、`r4_07` 正确 incomplete。
2. 初版 `cargo_summary` 解析未剥 ANSI 转义，suites 计 0。已修；run-2 计 115 suites。
3. 旧 `flutter_test_retry.ps1 -PerFile` 只枚举顶层 `test/*.dart`，漏 `test/repair/`（静默未完成）。新门禁递归枚举，`sp_29_contract_test.dart` 用例锁定该行为。
4. run-1 Rust 阶段失败（`application` E0609）系与 SP-05 并行编辑的瞬时污染（当时 `engine.rs` 在写中；稍后 `cargo check -p application --locked` 真值 exit 0），非 HEAD 条件；run-2 树安静后全绿。教训已记入 `unverified`/下一步：全工作区 Rust 门禁应在单写者窗口运行。

## 6. 未完成/阻塞（诚实项）

- 门禁 verdict 非绿：`fail+incomplete`（7 fail 跨卡 FRB 产物不同步 + 1 incomplete 原生崩溃），故本卡保持 implemented，不写 verified。
- `r4_07` 原生崩溃根因（采集崩溃栈/dump）与 7 fail 的 DLL 重建均超出本卡允许模块，已登记归属（SP-00 整合者 / 相关业务卡），本卡不自行修生产代码。
- 24h/500 切换、平台实测、最终发布包均未在本卡运行（属 SP-30~SP-35）。

## 7. 可复现性说明

同一入口：`powershell -NoProfile -ExecutionPolicy Bypass -File tools/gates/run_all.ps1`。
run-2 → run-3 对 8 个不稳定文件结论逐项一致；run-1 → run-2 的全部 10 处差异在
run-2 `reproducibility.mismatches` 中逐条有据（7 项去污染转绿、2 项分类器修正、1 项并行新文件）。
下次运行可用 `-CompareWith <gate-result.json>` 自动比对。
