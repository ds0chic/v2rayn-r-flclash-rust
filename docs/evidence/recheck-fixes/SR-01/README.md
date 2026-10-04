# SR-01 证据 — 订阅更新逐组真实结果

状态：`implemented`。开始 HEAD `d27eff0`。只改 `crates/application/src/subs.rs`、`crates/bridge_api/src/api/subs.rs`、`apps/desktop/lib/features/subs/subs_controller.dart`、`apps/desktop/test/**`、本目录与 `docs/tasks/SR-01.md`；`compat/features.yaml` 仅追加 evidence 行。未 `git add/commit`，未占用 127.0.0.1:10808，未改宿主代理/注册表/路由/TUN。

## 修复要点

- Rust `report_to_json`：`updated` 条目输出真实 `added` 与 `removed`；不再把 `removed` 误填 `existing`。
- Rust bridge：终态 report 缓存进进程级 `SUB_REPORTS`，并加 `#[frb(sync)] sub_update_report(job_id)`（未写入 `frb_generated`）；同一 job 的 `stage_key` 以 `subs.report:<json>` 前缀停驻完整 report，现有 `job_view` 即可取回，无需改 FRB 生成物。
- Dart `SubsController`：终态优先用 `_reportFromJob` 解析 report 逐组构造 entries；删除 Done 时“把 `_targets` 全构造成 updated”和按节点净差算 added 的逻辑；无 report 的 Done 返回 `E_UNAVAILABLE/error.sub_update_unconfirmed`，不伪造成功；`_targets` 排除空 URL；`_summarize` 汇总成功/跳过/保留/失败。

## 命令与结果

见 `evidence.txt`。摘要：

- `cargo test -p application --lib subs::tests --locked` → 21 passed / 0 failed。
- `cargo test -p bridge_api --lib api::subs --locked` → 13 passed / 0 failed。
- `cargo fmt -p application -p bridge_api -- --check` → exit 0。
- `dart format`（controller + 两测试）→ 已格式化。
- `flutter analyze` → No issues found。
- `flutter test`（fix09、sr01、fix09d、t09_sub_edit、t09_sub_setting）→ 15 passed。

## 上游对照

`SubscriptionHandler.cs:17-60` 逐组处理、`:21` 排除无 Id/URL/非 http(s)、`:27` 跳过禁用、`:41` 仅成功计数、`:47` 单组异常回报、`:57` `successCount>0` 作结束回调 bool。实现与原版一致：逐组结果独立，空 URL/失败组不计成功。

## FRB 重生成需求

`sub_update_report` 需 FRB 2.13.0 codegen 重生成 `apps/desktop/lib/bridge/frb_generated*.dart`（及 Rust 胶水），随后在 `BridgePort`/`FrbBridgePort` 暴露 `subUpdateReport(jobId)`，controller 改为优先调用，移除 `stage_key` 载体。
