# FIXREPORT-D-LEFT — Dart 侧遗留项实施报告

- 基线：`audit/2026-10-13 @ d42f598`；分支 `fix/D-LEFT`（7 commits：1 plan + 5 fix/docs + 本报告待提交）。
- 计划：`findings/PLAN-D-LEFT.md`（已批；实施中按 coordinator 条件调整两处，见 §5）。
- 门禁：`dart format` 0 changed（520 files）；`flutter analyze` No issues；串行 `flutter test -j 1`（持 flutter.lock，Idle 优先级）。
- Rust 零改动（只读核查两处）；`status_bar_view.dart` 未触碰；无 push。

## 1. D3a-11：QR 解码移出 UI isolate（commit a9c90bd）

- 改动：`scan_image_qr.dart`（`QrDecoder` typedef＋`compute` 入口 `_decodeQrImageEntry`＋`decodeQrImageAsync` 默认实现；`scanImageQr` 加 `decoder` 参数，解码失败映射为"未发现有效二维码"）、`scan_screen_qr.dart`（`scanScreenQr` 加 `decoder` 参数；isolate 失败读作 no-QR，不进截图失败分支）。同步 `decodeQrImage` 本体不动（同解码核，红绿锚点之一）。
- 测试：新 `d_left_qr_isolate_test.dart` 3/3 绿（含 `decodeQrImageAsync` 小 PNG 与同步核一致，300s cap 内终止——保留）；既有三扫描文件全部改为 fake decoder（成功/空/抛错），另对永不到达解码的分支加 `must not decode` 哨兵。
- 预先存在的 hang（与本次改动无关，证据：fake decoder 瞬时返回仍 hang）：`scan_screen_qr_test.dart` 首个 success 用例与 `scan_image_qr_test.dart` 的 success 用例在 `tester.runAsync(importShareText…)` 处 hang，TH-DART 跳过扫描文件的原因即此。成功路径 widget 测试未纳入。
- 回归：扫描三文件非导入用例逐个 `--plain-name` 全绿；`ux_parity_fix05_scan_test.dart` 无 dll 时干净 skip（+0 ~2）。

## 2. D1-09：explorer.exe 启动器（commit fae24e7）

- 改动：`custom_editor_dialog.dart`（`osOpenProgram='explorer'`＋`buildOsOpenArgs`＋`OsProcessLauncher` 缝；`openConfigFileExternally` 经 explorer 启动，exit code 忽略、仅 spawn 异常算失败；守卫收紧为仅 drive-letter 绝对路径——`!`/空格/CJK/括号放行，UNC/device/`/`-开头/bare名/`"&|<>^%`$;`/控制字符拒绝；删除已无用的 `quoteExternalOpenPath`）、`settings_actions.dart`（`openCoreWebsite` 经 explorer＋http/https scheme 门＋忽略 exit code；删除 `buildOpenUrlArgs`）。
- 测试：新 `d_left_launcher_test.dart` 6/6（fake launcher 零真实启动：`!`＋空格＋CJK 真实临时文件、`%&^` 等拒绝、缺失/UNC 不调用、exit 1 仍成功、未知 core 不调用）；`b13_d109_open_path_test.dart` 同步更新（bare 名与 UNC 改判拒绝——旧断言恰为条件要求的行为变更）；`r3_root_03_actions_test.dart` 两处断言变更：
  1. `core website URLs…` 用例删除 `buildOpenUrlArgs('https://github.com/XTLS/Xray-core') == ['/c','start','','…']` 断言（函数已删除）；
  2. `openCoreWebsite launches cmd start…` 改名为 `…launches explorer…`，期望 `['explorer','https://github.com/XTLS/Xray-core']`。
- 回归：`t10_custom_editor_test`、`fix03_special_editors_test`（初跑 2 flaky did-not-complete，solo 全绿、重跑全文件绿）、`fix03b_custom_browse_test` 全绿。

## 3. rt- vs ''：未改（comment＋guard test，commit 5aa70a5）

- 只读 Rust 核查：`normalize_routing`（`crates/application/src/routing.rs:75-77`）确对空 id 分配 `new_routing_id()`（`rt-<nanos>-<seq>`，`:18-25`），Rust 侧安全成立。
- 但 Dart 端到端不安全：SP-13 relay 按中继 id 回查权威列表验核（`verifyRoutingIncrement` saveScheme 分支，`routing_actions.dart:121-123`；`_applyRoutingAction` 用 pop 出的 id，`:244`）。`''` 将导致"保存成功却报失败"，用户重试即造重复行。另 `_commitAction` 成功后 `onSuccess` 按 `edited.profile.id` 追加本地行（`routing_windows.dart:2353-2375`），`''` 行与后端分配行分叉。
- 结论：保持 client-minted `rt-`（后端对非空 id 直接沿用，天然一致）；`newRuleId` 随机段已在 HEAD 存在（`routing_controller.dart:67-73`），无需改动。在原址加注释锁＋新 `d_left_scheme_id_test.dart`（新方案 id 非空且 `rt-` 前缀；红→绿：先断言空 id 失败确认测试有效，再回退为守卫）。

## 4. http 警告持久横幅（commit 0251a8e）

- 改动：`sub_edit_window.dart`（`_showHttpWarning` getter 复用 `isPrivateNetworkHost`＋`Uri.tryParse` 现有逻辑；`_url` listener 实时刷新；Url 行下常显 `sub-http-warn` 横幅，`semantics.warning` 主题色；`_save` 内 SnackBar 保留为第二通道）。阻塞语义不变（可保存）。
- 测试：新 `d_left_http_warn_test.dart` 3/3（800-light 与 1024-dark 横幅＋warning 色＋`takeException` 无溢出；私网/https/空 URL 无横幅）。
- 附带：既有 `b15_routing_subs_api_test.dart` 的 `findsOneWidget` 因双通道（横幅＋snackbar 同文案）改为 `findsNWidgets(2)` 并注明原因；全文件绿。`t09_sub_edit_test`、`ux_parity_fix06_test`、`ux_ub2_03_subedit_test`（初跑 2 flaky，solo＋重跑绿）、`ux_space01_entries_test` 全绿。

## 5. 错误 key 映射＋订阅窗 detail 本地化（commit c9cbabd）

- 改动（keys 冻结，只加映射）：`strings.g.dart` 新增 `errorRecoveryRequired`（恢复中，请稍后重试）、`errorSubUpdateFailed`（订阅更新失败）、`errorStorage`（存储写入失败，请重试），各仅著录 en＋zh-Hans/zh-Hant（其余 locale 经 `L10n.t` 英文回退，表内既定机制）；`_errorKeyMap` 加三条。`sub_setting_window.dart` 状态行经 `context.messageText(detail)`（裸 key 变人话，已是人话/混合串原样通过）。`subs_controller.dart:380` 不再把异常文本冒充 `messageKey`，改用 `error.bridge_unavailable`（已有映射）＋异常文本进 `detail`。
- 到达 UI 的证据（映射范围依据）：`error.recovery_required`（Rust `engine.rs:2861`→状态条通用文案）；`error.sub_update_failed`（`subs_controller:456,521` 默认 key→`_firstError`→订阅窗 detail）；`error.storage`（`bridge_port:2135`→`describeImportFailure` 经 `_classifyImportError` 回落原文进 toast，`import_persistence.dart:268-285`）。
- 残留（未做，有意）：`subs_actions.dart:367,394` 导出失败 toast 仍逐字插 `messageKey`（导出 key 全集未普查，另案）；backup 诊断栏保持 code/key 原样（有意）。
- 测试：新 `d_left_error_keys_test.dart` 2/2（9 语言无 `error.` 泄漏；订阅窗 widget 实测无裸 key）。回归：`t09_sub_setting_test`、`b15_routing_subs_test`、`r4_30_contract_test`、`sp_16` 四件（delete/batch/keyboard/group-persistence? 本轮跑 delete/batch/keyboard＋persistence＋toolbar＋chip）、`r4_03`、`r4_21`（E_UNAVAILABLE 码前缀保留，不断言受影响）全绿。

## 6. UI-DEVIATIONS sweep（commit c278944，纯文档）

- 翻转 11 行＋1 部分，均附 HEAD 证据（文件:行/测试名），其余（含 8 用户决策项）不动：
  ACCENT（`app_theme.dart:439,461`＋`theme_setting_dialog.dart:197-203`）、THEME-OPTIONS（`:184-193`）、FONT-12（`app_theme.dart:40`）、FORM-GAP（部分：fieldGap `app_theme.dart:256`，checkbox 行高未核实）、SELECTION-TINT（`routing_windows.dart:445,1008`）、SUB-MORE（`main_menu.dart:180-182`）、DENSITY（`ui_shell_controller.dart:35-38,68`，决策本体不动）、STATUS-BADGE（`status_badge.dart`＋`u_b1_status_badge_test.dart` 本轮绿）、DANGER-CONFIRM（两文件＋`u_b1_danger_confirm_test.dart` 本轮绿）、ROUTING-FOLD（嵌入式 `:106-117`；独立窗 `:2640` 常显如实注明）、ONBOARDING＋RESET-HINTS（两文件＋`main_menu.dart:363`＋`main_shell.dart:479`＋`u_b1_onboarding_test.dart` 本轮绿）。
- `compat/` 未动；只增量备注（先增后改同一行，净效果 12 行备注更新）。

## 7. 终轮引用全回归（本节）

除各节回归外，补跑全部引用触及文件的其余测试（串行，exit 79→solo 裁决）：
`u_b1_subs_progress_test`、`t21e_import_ux_test`、`sr01_subs_report_test`、`t21e_dialogs_responsive_test`（初跑 loader flake exit 79，solo 绿）、`sp_16_group_persistence/toolbar/chip`、`r4_30_repro`、`r4_17_repro`、`r4_17`、`r4_34`、`fix09d`、`fix09_subs_job_cancel`、`audit_r2_flows`、`fix08`、`fix08c`、`r4_12`、`r4_14`、`r3_wpf_routing_window/structure`、`r3_visual_dpi_100_125`（初跑 exit 79，solo 绿）/`150`/`200`——全部绿。扫描成功路径两用例与初跑 flakes 均为既有环境行为（§1）。
终门禁：format 520 files 0 changed；analyze No issues（5.4s/4.4s 两次）。

## 8. 发布构建

- `flutter build windows --release` 于分支尖（commit `c278944` 之后、工作树仅增本报告）：exit 0，131s（持 flutter.lock，构建用时 128.1s）。
- 产物：`v2rayn_desktop.exe`＋7 dll（`bridge_api`、`flutter_windows`、`file_selector_windows_plugin`、`hotkey_manager_windows_plugin`、`screen_retriever_windows_plugin`、`system_tray_plugin`、`window_manager_plugin`）＋`data/app.so`＋`data/flutter_assets`。exe 未运行。
- 构建后已删 `apps/desktop/build/`；flutter.lock 已释放。
