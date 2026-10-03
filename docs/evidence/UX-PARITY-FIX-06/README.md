# UX-PARITY-FIX-06 — 仅备注普通分组保存

任务：FIX-06（`docs/tasks/FIX-06.md`）。修复“新增仅备注普通分组被 URL 必填拒绝”，
统一 UI 与 Rust 的 URL 规则为“空 URL = 普通分组可保存；非空才校验”，并让保存后顶部
分组 chips 同步、更新流程跳过普通分组。

基线 HEAD：`1251cbc6821276e35d082f7739b8b9c15b6f93dd`（开始时记录）。工作树含本轮改动，未 commit。
冻结上游：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 规则与上游对照

| 侧 | 位置 | 规则 |
|---|---|---|
| 上游 | `SubEditViewModel.cs:54-99` | 仅 `Remarks` 必填；`Url` 非空才 `Utils.TryUri` |
| 上游 | `SubscriptionHandler.cs:60-81` `IsValidSubscription` | `Url` 为空/非 http(s) 的项在更新时跳过 |
| 本卡 Rust | `crates/application/src/subs.rs` `SubItem::validate` | 空 URL 放行；非空才 `is_http_url`，否则 `E_FIELD_FORMAT error.url_invalid` |
| 本卡 UI | `apps/desktop/lib/features/subs/sub_edit_window.dart` | 只本地校验备注；URL 交给 Rust `validate_sub_item`（同一错误码语义）；标签 `Url` 去星号 |

更新全部：`crates/application/src/engine.rs:1294` `refresh_one` 已对空 URL 返回
`SubUpdateOutcome::Skipped`，不下载、不替换节点；`update_subscriptions` 的 `ok` 仍由真实成功数决定。

## 改动文件

- `crates/application/src/subs.rs`：`validate` 允许空 URL；新增两个单元测试。
- `crates/application/tests/subs_pipeline.rs`：新增 `update_all_skips_empty_url_plain_group_without_failing`、`empty_url_plain_group_survives_reopen`。
- `crates/bridge_api/src/api/subs.rs`：新增真实桥测试 `save_empty_url_plain_group_round_trips`、`save_nonempty_invalid_url_is_rejected`。
- `apps/desktop/lib/features/subs/sub_edit_window.dart`：去掉空 URL/协议本地拦截，委托 Rust；Url 标签去星号。
- `apps/desktop/lib/features/subs/subs_controller.dart`：`save`/`delete` 成功后刷新 `profilesControllerProvider`，顶部分组 chips 同步。
- `apps/desktop/test/ux_parity_fix06_test.dart`：widget 三场景。
- `apps/desktop/integration_test/ux_parity_fix06_test.dart`：真实窗口场景。
- `apps/desktop/test/t09_sub_edit_test.dart`、`apps/desktop/integration_test/parity_review_smoke_test.dart`：错误文本断言改为修正后行为。

未改 `profiles/**`、`main_shell`、布局、FRB 签名。

## 真实窗口运行

```
$env:V2RAYN_R_DATA_DIR = <fresh temp>
$env:V2RAYN_R_FIX06_EVIDENCE = <repo>/docs/evidence/UX-PARITY-FIX-06
$env:V2RAYN_R_FIX06_IMAGES = 1
$env:V2RAYN_R_AUTOSTART = 0; $env:V2RAYN_R_AUTO_SMOKE = 0
flutter test integration_test/ux_parity_fix06_test.dart -d windows
```

结果：`All tests passed`（约 6s，PID 生命周期 < 120s）。`observations.json`
`recordingComplete=true`、`failures=[]`，四条检查：

- `isolated-first-launch`：subscriptionCount=0
- `plain-group-saves-with-empty-url`：remarks=仅备注普通分组，url=""，editorStillOpen=false
- `top-group-chip-appears-after-save`：chipFound=true（nodeCount=0，普通分组不产生节点）
- `plain-group-persists-after-reopen`：重开订阅窗口仍在，url="" 

截图：`01-plain-group-saved.png`、`02-top-group-chip.png`（可见顶部“全部 | 仅备注普通分组”chip）、`03-reopen-persisted.png`。

首次运行曾出现 flutter_tools 监听目录清理竞态的瞬时失败（`No tests were found` + listener path 删除错误），重跑即通过；非产品行为差异。

## 门禁

| 命令 | 结果 |
|---|---|
| `dart format --output=none --set-exit-if-changed lib test integration_test` | 0 changed |
| `flutter analyze` | No issues found |
| `flutter test` 逐文件（`tools/flutter_test_retry.ps1 -PerFile`） | 除 `ux_space03_vless_editor_test.dart` 外全通过；该文件因并发 FIX-01 对 `profile_editor_dialog.dart`/`profile_fields.dart`（501 行重写）导致间距断言失败，与本卡无关，未改其文件 |
| `flutter build windows --release` | Built `build/windows/x64/runner/Release/v2rayn_desktop.exe` |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass |
| `cargo test --workspace --locked` | 全通过（含新增用例） |
| 真实窗口 integration | pass，见上 |

## 未验证 / 遗留

- 真实进程级“重启应用后仍在”未在单个 integration 进程内实现（Rust engine 为进程内单例）；
  持久化重开由 Rust `empty_url_plain_group_survives_reopen`（临时目录 open/reopen）与
  真实窗口“关闭/重开订阅设置窗口”共同覆盖。
- 更新全部混合空/非空不失败由 `subs_pipeline` loopback 测试覆盖；未做真实外网下载。
- “更新当前组”入口按本卡要求不动（另卡），未回归。
