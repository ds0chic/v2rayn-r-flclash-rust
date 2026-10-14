# FIXREPORT-D-LEFT2 — 裸 messageKey 扫尾（export residual＋全仓同类）

- 基线：`audit/2026-10-13 @ 04b7618`（D-LEFT merge）；分支 `fix/D-LEFT2`（4 commits＋本报告）。
- 方式：test-first（先红后绿）、小步 commit＋双 trailer；全部串行 `flutter test -j 1`（持 `flutter.lock`，Idle 优先级，单文件 300s 上限，exit 79→solo 裁决）；fake  only，无真实启动/无网络/无 cargo。
- 门禁：`dart format` 521 files 0 changed；`flutter analyze` No issues。本轮无需 release build（未建 `build/`）。
- Rust 零改动；`status_bar_view.dart` 未触碰；无 push。

## 0. 改动一览（3 commits）

- `0a2e5c1` fix: `ErrorLocalizer.forLanguage`＋`localizeStatusKey`（`error_localizer.dart`）；ref 系 toast 经它本地化（`subs_actions.dart` ×2、`profile_actions.dart` ×3、`settings_controller.dart` 应用失败文案＋异常文本不再拼入）；新 `d_left2_export_keys_test.dart`。
- `31202c7` fix: 有 context 处一律 `context.messageText()`（`dns_window.dart` ×4、`routing_windows.dart` ×1、4 个 editor dialog 横幅/行错、`template_window.dart` 异常文本不再拼入、`theme_setting_dialog.dart` 状态行）；新增 `error.import_custom_failed` 映射（`strings.g.dart`＋`_errorKeyMap`，keys 冻结）。
- `46f8aa8` test: `t06a_editor_error_test.dart` 改断言本地化文本（行为变更所致，见 §3）。

## 1. Before / After

| 位置 | Before（用户可见） | After |
|---|---|---|
| 订阅导出 clipboard/文件（`subs_actions.dart:367/399`） | `导出失败：error.no_profiles_selected` | `导出失败：<人话，如"未选择节点">`（error 缺席仍为"无导出项"） |
| 节点导出/保存（`profile_actions.dart:133/160/435`） | `导出失败：error.…`／`保存失败：error.…` | 同上（缺席回落文案不变） |
| 设置应用失败（`settings_controller.dart:694/699`） | `…：error.…`／`…：FormatException: …(路径)` | 人话；异常分支改通用句，code 留 `phaseErrors` |
| DNS 窗 4 snackbar、路由嵌入窗 snackbar | `…失败：error.…`／`…：null` | 人话；null→"未知错误"（`error.unknown` 既有映射） |
| custom/group/profile editor 横幅＋profile 行错 | `保存失败: error.… (CODE)` | `保存失败: <人话> (CODE)`（code 后缀保留供诊断） |
| custom 导入回落 key | `error.import_custom_failed` 无映射→通用文案 | 新增映射"自定义配置导入失败"（en/zh-Hans/zh-Hant） |
| template `_serverError` | `…: 保存失败 (FormatException…)` | `…: 保存失败`；key 分支同 editor 横幅 |
| theme `_status` | key 原样渲染 | `messageText`（"已应用"等人话原样通过） |

## 2. 新增映射的到达-UI 证据（只补证明到达的）

- `error.import_custom_failed`：Dart 自造回落（`custom_editor_dialog.dart:274`，error 缺席时）→file-error 行。`error.no_profiles_selected`／`error.not_found`（export 路径）早已有映射；其余引擎 key 经 `key()` 未映射→通用文案，不泄漏。
- 不补：`error.routing_no_selection`、`error.webdav_ok`、`error.update_unsupported`、`error.core_not_found`、`error.update_manual`、`error.update_app_source_unconfigured`、`error.scheduler_start_failed`——或已有渲染侧处理（`check_update_view.dart:295-304` `_noteLabel`、status 行走 `messageText`），或未在 lib/UI 路径发现，统一由 `fallbackFor` 兜底为通用文案。

## 3. 行为变更导致的测试更新（1 处）

- `t06a_editor_error_test.dart:66` 原断言 `find.text('error.remarks_required') findsWidgets`（裸 key 双处显示）→ 改为裸 key `findsNothing`＋`textContaining('请填写备注') findsWidgets`。意图不变（双错误面可见），仅文本本地化。

## 4. 故意不动（含理由）

- `status_bar_view.dart:460,896`：约束禁碰；R4-30 契约（headline 本地化＋code 仅详情弹层）。
- backup（`backup_controller.dart:92-96,146`＋view `:472-474`）、update（`update_controller.dart:195,260-261,579`＋view `:328-330`）、monitor（`monitor_bridge.dart:203-204`）的 `CODE / key` detail 栏：人工 message＋技术 detail 的既定诊断格式，不动。
- 各 controller `status:` 状态字段：渲染侧已走 `messageText`（路由窗 `:182-186`、DNS 窗 `:411-416`、option 窗 `:432,595,606`、hotkey 窗 `:262`），state 存 key 无妨。
- `check_update_view.dart:304` 未知 note 回落原文：非 messageKey 契约面，保守不动。
- `import_persistence.dart:268`、`subs_controller.dart:664,671`：前者 code 分桶＋后者 D-LEFT 已在订阅窗渲染侧本地化；`describeImportFailure` 的 `_classifyImportError` 回落原文仅当 code 桶未命中且 message 非 key——`error.storage` 已有映射（D-LEFT），不再原文。

## 5. 回归（串行，solo 裁决记录）

- 新单测 `d_left2_export_keys_test.dart`：先红（裸 key）后绿。
- 引用触及文件全跑（约 100 文件，见 transcript）：全绿。初跑非绿→solo 绿（环境 loader flake，与改动无关）：`fix03_special_editors`、`r4_12`、`r4_15`×2、`r4_18_p11_contract`（连 flake 两次，逐个 solo＋重跑全绿）、`r4_18_p08_repro`、`fix08b_dns_apply`、`ux_ub2_11`、`ux_ub2_08`、`t21e_dialogs_responsive`、`b15_routing_subs`、`reprof10`。唯一真断言失败即 §3（已随改）。
- `t06a_editor_error/save/cancel/form`、`t10_*`、`t11_dns`、`fix08_dns*`、`r4_18` 全系（11 contract＋11 repro）、`r4_19/20/24` 双件、`r4_30` 双件、`r3_wpf/routing/dpi` 系、`fix16` 系、`b13/b14` 系、`sp/r4_13` 系等其余全部一次绿。

## 6. 命令（实际执行）

- 每文件：`powershell <scratchpad>\dleft2_one.ps1 -File test/<f> [-Name '<sub>' ] [-TimeoutSec N]`（持 flutter.lock，Idle，300s 上限，taskkill 仅自身树）。
- `dart format --output=none --set-exit-if-changed lib test` → 0 changed。
- `flutter analyze`（持锁）→ No issues（6.6s）。
- `git log --oneline`（fix/D-LEFT2）：`0a2e5c1`、`31202c7`、`46f8aa8`＋本报告。
