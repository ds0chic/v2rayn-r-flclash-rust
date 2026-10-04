# R3-SET-06 — 主菜单订阅 toast 复用逐组汇总并可查看失败详情

状态：`implemented`（纯汇总单元测试与调用链接线；未新增真实菜单网络测试）。

任务 ID：R3-SET-06

本次唯一用户流程：主菜单「更新全部订阅 / 更新当前组」后，toast 展示逐组成功/保留/失败/跳过；存在失败组时可查看失败详情，不谎报全部成功。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核 `docs/evidence/parity-recheck-2026-10-04/round3-settings.md` SR-01 / R3-SET-06。SR-01 的逐组真结果已进入 `SubsController.status`。

上游对照：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Handler/SubscriptionHandler.cs:47`（单组错误回调明确显示）。

对应 feature / field / action：`F-SUB-003/011`、`ACT-MAIN-020/021/022/023`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`c.SubUpdateResult`（entries 的 updated/preserved/skipped/failed/preserved_error）。
- 输出：toast 摘要 `订阅更新完成：成功 N，保留 N，失败 N，跳过 N`；失败行弹详情对话框。
- 错误：取消/无本地代理（`E_PROXY_UNAVAILABLE`）保持显式文案。
- 权限：`None`；不联网（测试用合成结果）。

允许修改的模块：`apps/desktop/lib/features/subs/subs_actions.dart`、`apps/desktop/test/**`、本卡、证据目录、compat 台账（仅追加）。

禁止改变的已有行为：`SubsController.update` 的真实逐组结果；`subs_controller.dart`（未改）；SR-01 不伪造失败为成功。

测试夹具与原版预期：`r3_set_06_subs_summary_test.dart` 构造含 updated/failed/preserved/skipped 的 `SubUpdateResult`，断言摘要包含四类计数且 `hasSubUpdateFailures=true`；取消与代理不可用路径显式。原版预期：单组错误可见。

本次必须通过的命令/真实场景：
- `flutter analyze`
- `flutter test test/r3_set_06_subs_summary_test.dart test/sr01_subs_report_test.dart`

证据文件位置：`docs/evidence/recheck-fixes/R3-MISC/README.md`。

完成条件：主菜单/当前组 toast 复用逐组汇总、失败可查看详情、不谎报全部成功；门禁通过。

接口缺口（登记）：协议 DTO 无 `removed` 字段，`SubsController` 未读取后端 removed；不影响新增 added 的真实性，但结果字段未全部呈现。

本轮实际结果：`subs_actions.dart` 将 `_updateToast` 改为公开 `subsUpdateSummary(c.SubUpdateResult)` 的逐组汇总，新增 `hasSubUpdateFailures`/`_showSubUpdateDetails`；`updateAllSubscriptions` 与 `updateCurrentGroup` 都复用。2 新用例、SR-01 回归通过。
