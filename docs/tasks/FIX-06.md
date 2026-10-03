# FIX-06 — 仅备注普通分组保存

状态：`implemented`（真实窗口四检查通过；真实外网下载与进程级重启未验证，见执行记录）。
本次执行基线 HEAD `1251cbc6821276e35d082f7739b8b9c15b6f93dd`，工作树含本轮改动，未 commit。
证据见 `docs/evidence/UX-PARITY-FIX-06/`。

任务 ID：FIX-06

本次唯一用户流程：用户在“订阅分组设置”里新增一个只填备注、URL 留空的分组 → 保存 →
主窗口顶部分组 chips 出现该普通分组 → 关闭并重开订阅设置窗口后该分组仍在；整个流程不下载、
不产生节点、不改宿主代理/TUN。

前置任务及已验证证据：
- `docs/evidence/parity-review-2026-10-03/README.md` 六场景之一“新增仅备注普通分组被 URL 必填拒绝”；
  `settings-report.md` SET-01（P1）定位 `sub_edit_window.dart:132-143` 与 `application/subs.rs:164-175`；
  `root-report.md`、`ui-run-02/observations.json`（subscriptionCount=0/editorStillOpen=true/urlRequiredError=true）。
- 冻结上游 `7d6a967c18c697f28dc6917122ed3a4993fcf336`：`SubEditViewModel.cs:54-99` 仅备注必填、
  URL 非空才 `TryUri`；`SubscriptionHandler.cs:60-81` 更新时跳过空 URL。
- `repair-queue.md` FIX-06 行；`fields.entities.yaml` FLD-ENT-084（SubItem.Url）。
- FIX-01 并发进行，其组上下文接口 `CommandContext.groupSubId` 读取 `ProfilesState.groupSubId`；
  本卡只刷新 `profilesControllerProvider`、不改 `min_shell`/`profiles` 菜单编辑器，两者兼容。

对应 feature / field / action / layout ID：
- `SET-01`：普通分组空 URL 可保存；`SET-02`：更新对象用稳定 ID（本卡不改进口）。
- `F-SUB-002` / `ACT-SUB-001` / `ACT-SUB-008`：订阅新增保存。
- `FLD-ENT-084`（SubItem.Url）、`FLD-ENT-097`（SubItem.Memo）。

必读上游文件、符号和固定 commit：
- 冻结 commit 的 `ServiceLib/ViewModels/SubEditViewModel.cs`（SaveSubAsync 的 URL 空判断）、
  `ServiceLib/Handler/SubscriptionHandler.cs`（IsValidSubscription/UpdateProcess）、
  `ServiceLib/Handler/ConfigHandler.cs`（AddSubItem）、`v2rayN/Views/SubEditWindow.xaml`（Url 无必填标记）。
- 当前 `apps/desktop/lib/features/subs/{sub_edit_window.dart,subs_controller.dart,sub_setting_window.dart}`、
  `crates/application/src/subs.rs`（validate）、`crates/application/src/engine.rs`（save_sub_item/refresh_one）、
  `crates/bridge_api/src/api/subs.rs`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入为编辑草稿字段；输出为保存成功（可持久化）或结构化字段错误。
- Remarks 空 → 拒绝（UI 本地 + Rust `error.remarks_required`）。Url 空 → 放行，视为普通分组，不下载。
  Url 非空且非法 → `E_FIELD_FORMAT` / `error.url_invalid`，UI 显示同一 messageKey。
- 取消关闭窗口不落库。普通分组不进入任何内核配置，不产生节点。
- 无系统权限申请、无系统代理/TUN/自启写入，不碰 10808，不杀外部进程。
- 保存后主窗口顶部分组 chips 同步刷新；更新全部跳过普通分组且不替换其（空）节点。

允许修改的模块：
- `apps/desktop/lib/features/subs/**`、`apps/desktop/test/**`、`integration_test/**`、
  `crates/application/**`（校验/更新跳过）、`crates/bridge_api/src/api/subs.rs`、
  `docs/evidence/UX-PARITY-FIX-06/**`、`docs/tasks/FIX-06.md`、`compat`（追加）。

禁止改变的已有行为：
- 不改 `profiles` 菜单/编辑器、`main_shell`、布局；不改 FRB 生成（本卡签名未变）。
- 不改“更新当前组”入口（另卡），不得回归；不降低台账分母；不 commit。

测试夹具和原版预期：
- Rust：内存 engine 合成订阅 + 11808 起的 loopback HTTP；空 URL 普通分组保存/重开/更新全部跳过。
- Flutter widget：`Fix06SubsBridge`（仅测试内实现正确规则）验证仅备注保存、非法 URL 拒绝、取消不落库。
- 真实窗口：隔离 `V2RAYN_R_DATA_DIR`，真实 FRB/Rust/SQLite；`AUTOSTART=0/AUTO_SMOKE=0`，不下载。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test integration_test`、`flutter analyze`、
  `flutter test` 逐文件（`tools/flutter_test_retry.ps1`）、`flutter build windows --release`、
  `cargo fmt/clippy/test --workspace --locked`。
- 真实窗口：新增仅备注普通分组→保存→顶部分组出现→重开仍在，截图与 observations.json。

证据文件位置：`docs/evidence/UX-PARITY-FIX-06/`。

完成条件：UI 与 Rust 同规则；普通分组可保存且不下载；保存后顶部分组同步；更新全部跳过普通分组；
widget/Rust/真实窗口测试与截图齐全；门禁通过。未跑项明确标注。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

## 执行记录（2026-10-03，deepseek-v4.1-flash 子代理）

改动：
- `crates/application/src/subs.rs`：`SubItem::validate` 空 URL 放行，非空才校验 http(s)；
  新增单元测试 `validate_allows_empty_url_plain_group`、`validate_still_rejects_invalid_nonempty_url`。
- `crates/application/tests/subs_pipeline.rs`：`update_all_skips_empty_url_plain_group_without_failing`、
  `empty_url_plain_group_survives_reopen`。
- `crates/bridge_api/src/api/subs.rs`：`save_empty_url_plain_group_round_trips`、
  `save_nonempty_invalid_url_is_rejected`。
- `sub_edit_window.dart`：移除空 URL/协议本地拦截，委托 Rust `validate_sub_item`；Url 标签去星号。
- `subs_controller.dart`：`save`/`delete` 成功后 `profilesControllerProvider.notifier.reload()`，
  顶部分组 chips 同步（不改 main_shell / profiles 文件）。
- 新增/更新测试：`test/ux_parity_fix06_test.dart`、`integration_test/ux_parity_fix06_test.dart`、
  `test/t09_sub_edit_test.dart`、`integration_test/parity_review_smoke_test.dart`。

真实窗口结果：`observations.json` `recordingComplete=true`、`failures=[]`；四检查全通过
（空 URL 保存、editor 关闭、顶部 chip 出现、重开仍在），截图三张。

门禁：`dart format` 0 / `flutter analyze` No issues / `flutter test` 逐文件（1 个失败为并发 FIX-01
改 `profile_editor_dialog.dart`/`profile_fields.dart` 引发的 `ux_space03_vless_editor_test` 间距断言，
与本卡无关）/ `flutter build windows --release` 成功 / `cargo fmt+clippy+test --workspace --locked` 全通过。

未验证：真实外网订阅下载；真实进程级冷启动重开（Rust engine 为进程内单例，改由临时目录
open/reopen 与重开窗口覆盖）；多平台/多 DPI。更新当前组入口本卡未改，未做专项回归。
