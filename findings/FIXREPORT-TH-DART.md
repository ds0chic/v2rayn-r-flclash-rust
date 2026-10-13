# FIXREPORT-TH-DART（Dart 合并后验证 + Flutter 测试健康，2026-10-13）

集成分支（本批基线 `e45c6c7`，B01…B17 全合入）。worktree `worktrees/TH-DART`，分支 `fix/TH-DART`，19 个 commit（9 fix + 10 docs，全带双 trailer），未 push。
注意：写报告时主分支 `audit/2026-10-13` 已前移到 `8c787c9`（TH-RUST 合并，与本批无文件交集）；本批 merge-base 仍为 `e45c6c7`，§6 范围按 `e45c6c7...HEAD` 核对。
全程低负载：349 文件逐文件串行（Idle 优先级、`-j 1`，单文件 300s 超时记 HANG——实际 0 HANG）；禁 cargo/build/全量 analyze。

## 1. 批次摘要与门禁表

- 改动：`lib/app/app.dart` 1 行（门禁 format，协调人明确指示修）；`test/` 6 文件原地修 + 6 新增拆分文件 + 2 原文件删除；`findings/TH-DART-VERIFY.md`（阶段 1 定稿）。
- commit：`bcf078e` format；`47e6cf7` r4_23/r4_31；`b1b0e64` t11；`4e1d8da` t18b；`a4c992d` t21e；`eee4fd1` r4_11；`f46ff14` wave_b 拆分；`20df897` r3 拆分；`687d22d` 079 fixup；+ 10 docs 进度 commit。

| 命令 | 结果 | 与基线对比 |
|---|---|---|
| `flutter pub get --enforce-lockfile` | PASS exit 0 | 同基线（lock 未动） |
| `dart format --output=none --set-exit-if-changed lib test` | 初 FAIL（`lib/app/app.dart` 1 处，B16 手工折行非 canonical 输出）→ 修后 PASS（473 文件 0 改动） | 基线 PASS；FAIL 系 B16（`1110a63`）引入，已修 |
| `flutter analyze`（全量，阶段 1） | PASS No issues | 同基线 |
| `dart analyze`（13 个改动文件作用域） | 1 warning（079 拆分残留未用 helper）→ 修后 No issues | — |
| `flutter test` 全量串行 | 349 文件：首轮 292 绿；两轮 solo 后 334 文件至少一次绿；6 确定性断言失败全部修好并转绿（§2）；2 拆分修复多构建崩溃（§3）；残留见 §4 | 基线 10 确定性红→0；5 loader→1（ux_space01）；9 flaky→抖动残留（§4） |

## 2. finding 结果表（确定性失败 6/6 fixed，无生产代码改动）

| ID | 结论 | 改动文件 | 回归测试 | 修复前 → 修复后 |
|---|---|---|---|---|
| C1 r4_11 `edit-save-reopen`（窗口未关闭） | fixed（测试 harness 缺口，**非真 bug**；探针证伪） | `test/r4_11_contract_test.dart`（+FakePlatformBridge override） | 同文件 | +4 -1 → 5/5。根因：`saveAndApply` 含 autostart 阶段，无 override 时打到真实 FrbPlatformBridge 得 E_AUTOSTART_APPLY（saved=true），窗口按 R4-13/D14 设计诚实保持打开。 |
| C2 r4_23 `log filtering…` Expected 2 Actual 0 | fixed（陈旧测试；**非 B16 引入**） | 同文件（`emitLogs` 后等 `logCoalesceWindow×2`） | 同文件 | +9 -1 → 10/10。`logCoalesceWindow`=150ms 在基线前 `467607e` 即存在、基线 `6617f47` 源码已含；B16 只改 logs_view 表现层。 |
| C3 r4_31 `bounded under flood` | fixed（陈旧测试，同根因） | 同文件 | 同文件 | +4 -1 → 5/5 |
| C4 t18b `failed apply…` | fixed（陈旧测试） | 同文件（R4-30 契约：headline 不显示裸 code；点查看失败弹窗断言 `E_NO_ACTIVE_PROFILE (error.no_active_profile)`） | 同文件 | +2 -1 → 3/3（首轮 Idle 下曾呈 did-not-complete，solo2 露出真形态） |
| C5–C9 t21e_import_forms 5 测 | fixed（harness 缺陷） | 同文件（`setUpAll` 的 `markTestSkipped` 只记 phantom `~1`，守卫移入各 test 体，仿 `t21e_import_real_bridge_test`） | 同文件 | +0 ~1 -5 → 5 skip exit 0 |
| C10 t11_menu `路由模式:` | fixed（陈旧测试） | 同文件（SP-19 值断言：`routing-mode-selector` 内文 `Rule`） | 同文件 | +1 -1 → 2/2（`路由模式:` 在 `lib/` 零命中；`路由模式：`全角版另有它用） |

## 3. 崩溃型 loader（2 组拆分 fixed；其余 flaky 化）

- wave_b_059_079（单进程 12 次窗体构建 → 第 2 测起 segfault）：拆分为 `059/060/079` 三文件，各自绿（059 1/2、060 2/2、079 2/2）。 doctrine 依据：`profiles_harness.dart:20-22` + `dpi_assertions.dart:93-97`（DPI 按 scale 分文件先例）。
- r3_visual_dpi_windows（4 scale×2 size≈百级帧；200% 光栅 2560×1600）：拆分为 `100_125/150/200` 三文件，各自绿。
- r4_07 / ux_space01 / drag_scroll_up / profiles_keyboard / r4_11_repro / sp_24 / t21e_responsive / r4_19 / recheck_a / wave_b_110_112 / ux_parity_fix01 / r3-150：全部至少一次全绿 → flaky（tester 进程级 segfault 竞争；print 扰动即可翻转，如 r4_07/ux_space01 实测）。未改源码。
- sp_16_group_chip_menu（基线 D5）：首轮即绿，**被前批修好**（profiles 域，疑 B13 D1 相关），本批 no-op。
- **新增红归属**：4 个新增稳定红（profiles_keyboard、r3_windows、r4_11_repro、t21e_responsive）经 normal 优先级探针全绿或拆分转绿——均为负载/进程崩溃型，**无新增确定性断言失败，不能归属 B13/B14/B15/B16/B07b 任一批的逻辑改动**。

## 4. 未处理清单

1. `ux_space01_group_flow_test`（基线 D）：累计 6/6 崩溃（exit 79），单构建单测，print 扰动下通过 1 次。本批未改——无可拆分结构（拆单测流程会动断言）；崩溃为 tester  segfault，需引擎/CI 侧投入。建议：集成机空闲 normal 复测；仍红则按 §5 拆成多文件（未做）。
2. 残留崩溃抖动（Idle/游戏负载偶发，normal 探针多已绿；属环境不属回归）：r4_07、t11、r4_11（他测 `User/Pass…`）、060 等在最终验证轮各翻转 1 次（均为 did-not-complete/exit79，非断言失败；对应断言内容均已绿过）。建议集成阶段空闲机器 normal 全量确认一次。
3. UX/行为取舍：无。本批零生产代码改动（除 format 单行），无 UX 决策事项。

## 5. 给 UX 阶段的建议

- R4-30/R4-13 的"诚实失败"语义（headline 可读文案 + code 进弹窗；saved-but-apply-failed 保持窗口打开）已有 t18b/r4_11 锁死，UX 改文案/布局时别把裸 code 拿回 headline，也别把失败保存当成功关窗。
- DPI 200%（2560×1600 软件光栅）是 tester 崩溃高发区，真机/GPU CI  lane 覆盖后再 claims。
- 本批新增的 6 个拆分文件命名即覆盖声明（059/060/079、100_125/150/200），UX 加新窗体测试时沿用"一 scale/一入口一文件"预算。

## 6. 范围与实际运行命令

- `git diff --name-only e45c6c7...HEAD`：`lib/app/app.dart`、`test/{r4_11,r4_23,r4_31,t11_menu,t18b_runtime_ui,t21e_import_forms}_*.dart`、新增 `test/r3_visual_dpi_windows_{100_125,150,200}_test.dart`、`test/repair/wave_b_settings_{059,060,079}_test.dart`、删除 `test/r3_visual_dpi_windows_test.dart` + `test/repair/wave_b_settings_059_079_test.dart`、`findings/TH-DART-VERIFY.md`。零越界（`lib/app` 系协调人指示的门禁修复）。
- 运行过：worktree 创建；`flutter pub get --enforce-lockfile`；`dart format`（门禁+单文件修）；`flutter analyze`（全量 1 次）+ `dart analyze`（作用域 2 次）；`run_chunk.ps1` 全量 9 chunk + solo/solo2/探针/拆分验证/最终验证共约 20 个后台串行 run（全部 Idle，探针 9 文件 Normal）；`flutter test <单文件>` 前台约 15 次（验证修复）；`git checkout --` 还原 generated_plugin*（每次跑后）；删 `apps/desktop/build`（每 chunk 后）。
- 未运行：cargo 全系、`flutter build`、全量 `flutter test` 一把梭（均按禁令未跑）。
