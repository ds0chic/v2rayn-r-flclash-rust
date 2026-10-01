# T09–T16 审计整改 — B 组（Flutter / 文档 / 台账侧）执行报告

- 承办：B 组（deepseek-v4.1-flash）
- 时间：2026-10-01
- 基线：工作区（未 commit）；起始 HEAD `083b084`（T11 routing+DNS），工作树含 A 组 `crates/**`、`services/**` 未提交改动
- 依据：`docs/evidence/audit/T09-T16.audit.muse.md`、`T09-T16.audit.gemini.md`
- 边界：仅动 `apps/desktop/{lib,test}`、`compat/features.yaml`、`compat/fields.entities.yaml`，以及指定的 `docs/evidence/*`；**未触碰 `crates/**`、`services/**`**；未 commit；未重新生成 FRB（现有生成物已含 `cancelJob`/`CancelResult`/`CancelOutcome`）
- 硬约束：未占用/修改 10808，未改系统代理/注册表/路由/TUN，未提权，未跑全量 cargo

## 1. 逐项状态

| 项 | 级别 | 状态 | 说明 / 位置 |
|---|---|---|---|
| F01 Flutter 假取消 | P0 | 完成 | `BridgePort` 新增抽象 `cancelJob`；`FrbBridgePort` 透传 `engine.cancelJob`；合成桥记录 `cancelledJobs`；`SubsController.cancel()` 真调用并按 `outcome` 更新状态（`subs_controller.dart:182`，`bridge_port.dart:98/341/930`）。测试 `t09_sub_setting_test.dart`「cancel forwards to the bridge cancelJob」断言 fake 记录到 `cancelJob` |
| F07 KCP 注释虚假/缺字段 | P1 | 完成 | `option_setting_window.dart` KCP 补齐 `CwndMultiplier`/`MaxSendingWindow`（6/6 可编辑），移除虚假「只读展示」注释，改「可编辑，共 6 项」。测试 `t12a_option_window_test.dart` 断言 6 字段存在 |
| F12 订阅分组过滤 bug | P1 | 完成 | `profiles_controller.dart:283-309` 改用存储侧 `ProfileDto.subid` 比对（与 `setGroupSubId(sub.id)`/`counts[p.subid]` 一致），`subRemarks` 仅作无存储行的回退。测试 `t10_groups_panel_test.dart`「group filter compares sub ids, not display remarks」（陷阱：A 的 remarks 撞 B 的 id） |
| F17 主题失败不回滚 | P2 | 完成 | `theme_setting_dialog.dart:54-85` 改为**先保存成功再应用**；失败时回滚草稿条目并保持旧主题。测试 `t12a_theme_test.dart`「theme dialog keeps the old theme when saving fails」 |
| F14 合成桥假成功 | P2 | 完成 | `SyntheticBridgePort` 类文档明确「仅 UI 状态断言，成功替换证据只认 Rust `subs_pipeline`」；`updateSubscriptions` 每条目 `message:'synthetic'` 标记；`writeExportFile` 改为**结构化未接线失败**（`ok:false` + `E_NOT_WIRED`），不再冒充成功。测试 `t09_import_export_test.dart`「synthetic file writes return a structured not-wired result」 |
| Gemini F-06 T09.md verified | P2 | 完成 | `docs/evidence/T09.md:3` 改为 `implemented（Rust 侧；全链路放通前不得标 verified）` |
| F05 台账夸大 | P1 | 完成 | `features.yaml`：F-SUB-007 降级 `preserved_only`（标注远端转换未实现、仅字段保留+custom_core_type hint，test 换真实用例）；F-SUB-008 标注持久化-only、链行为引 `t10_groups_templates::proxy_chain_save_reopen` 并补 `docs/evidence/T10.md`；F-SUB-003 `cancel_behavior` 更正为真实取消并补 cancel 测试 id |
| F13 台账精度 | P2 | 完成 | `fields.entities.yaml` 全文笔误 `subscription_pipeline.rs`→`subs_pipeline.rs`；新增 Appendix `remediation_implementation_records.fields`：FLD-ENT-151..158（FullConfigTemplateItem）补 location/test/evidence |
| C9 台账滞后 | P2 | 完成 | `features.yaml` `remediation_implementation_records.features` 追加 T13（F-SYSPROXY-001..004、F-DESKTOP-003/007）、T14（F-TUN-003/005）、T15（F-MONITOR-001/002/004/005/006）、T16（F-CORE-002/003/004），含 location/test_ids/evidence；T12a 的 F-APP-001/002/003 补 `docs/evidence/T12a.md` 与真实 test 文件名（追加式，未删行） |
| F06 文档矛盾（T12a） | P1 | 完成（沿用并校验工作树） | `docs/evidence/T12a.md:48-50` 以 `settings_timing.rs` 为准：`CurrentFontFamily`/`CurrentLanguage`/`MainGirdOrientation` 标 `RestartApp`，并说明 Flutter 会话内超前应用与重启收敛一致 |
| F20 小瑕疵登记 | P2 | 完成 | `T06b-validation.md §6` 追加 3 项（smoke 硬编码 11808、sing-box 空 error、TUN 驱动枚举 stderr）——仅追加，不改正文 |
| T17 输入登记 | — | 完成 | 新建 `docs/evidence/T17-inputs.md`：系统级首验隔离/授权、update 未签名不可发布、helper↔net-host 租约待接线、ACT/LAY 统计口径分裂 |

## 2. 门禁结果（真实运行，`apps/desktop`）

| 命令 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed lib test` | EXIT 0（94 files, 0 changed；首跑发现 4 文件需格式化，已 `dart format` 修复） |
| `flutter analyze` | EXIT 0（No issues found） |
| `flutter test` 逐文件（`tools` 同构逐文件脚本，34 个 `*_test.dart`） | 30/34 首次 EXIT 0；4 个（`t09_sub_edit`、`t10_groups_panel`、`t10_menu`、`t12a_theme`）首跑出现 `did not complete`（引擎崩溃），逐文件重试第 1 次全部 PASS → **34/34 最终通过** |
| `flutter test` 批跑（`tools/flutter_test_retry.ps1`） | 崩溃：多次尝试均 `did not complete`，退出 1。与 `docs/evidence/T01.md` 记录的 `flutter_tester` 引擎段错误（0xC0000005）一致，**非断言失败**（同一批未触碰的 profiles 用例也崩） |
| `flutter build windows --release` | EXIT 0（`build\windows\x64\runner\Release\v2rayn_desktop.exe`，39.7s） |
| `python yaml.safe_load`（features.yaml / fields.entities.yaml） | 均解析通过 |

## 3. 测试清单（本轮新增/改动）

- `apps/desktop/test/t09_sub_setting_test.dart`：新增 `cancel forwards to the bridge cancelJob`
- `apps/desktop/test/t09_import_export_test.dart`：新增 `synthetic file writes return a structured not-wired result`
- `apps/desktop/test/t10_groups_panel_test.dart`：新增 `group filter compares sub ids, not display remarks`
- `apps/desktop/test/t12a_option_window_test.dart`：既有 tab 测试追加 KCP 6 字段断言
- `apps/desktop/test/t12a_theme_test.dart`：新增 `_FailingSettingsBridge` + `theme dialog keeps the old theme when saving fails`

## 4. 台账 / 文档更新路径

- `compat/features.yaml`：F-SUB-003（cancel_behavior/test_ids）、F-SUB-007（preserved_only）、F-SUB-008（evidence）、F-APP-001/002/003（test_ids/evidence）、`remediation_implementation_records.features` 追加 16 条 T13–T16 记录
- `compat/fields.entities.yaml`：`subscription_pipeline.rs`→`subs_pipeline.rs`；`remediation_implementation_records.fields` 追加 FLD-ENT-151..158
- `docs/evidence/T09.md`：状态行 `implemented + verified`→`implemented`
- `docs/evidence/T12a.md`：立即生效清单以 `RestartApp` timing 为准
- `docs/evidence/T06b-validation.md`：§6 追加 3 行
- `docs/evidence/T17-inputs.md`：新建
- `apps/desktop/lib/bridge/bridge_port.dart`、`subs_controller.dart`、`subs_actions.dart`、`profiles_controller.dart`、`option_setting_window.dart`、`theme_setting_dialog.dart`：应用侧修复

## 5. 遗留项 / 未覆盖

1. **FRB 生成物**：未重新生成（现有 `contract.dart` 已含 `CancelResult`/`CancelOutcome`；`engine.cancelJob` 已存在）。若后续 A 组改动 bridge API 签名，需再生成。
2. **批跑崩溃**：`flutter_tester` 引擎段错误为锁定工具链已知问题（T01.md），逐文件可稳定通过；批跑结论不可作为门禁。
3. **A 组范围未动**：`crates/application` 的 `replace_sub_profiles` 事务（Gemini F-02）、`crates/bridge_api` 的 `SUB_JOBS` 死注册表（F04）、updater 路径逃逸（F02/A 组）、T10/T15 机器产物归档（F08/F09）等仍待 A 组。
4. **T12a 其余缺字段**：SystemProxy/SpeedTest/Clash Tab 静默缺字段（B5）本轮未逐一补齐（F07 仅 KCP），保留在 T12a/T17 输液清单。
5. **`fields.settings.yaml`**：工作树中已有 A 组/他人对 F15 的改动，B 组未触碰。
6. 未 commit（按要求）。
