# FIX-09B — 订阅转换目标（SubConvert）

状态：`implemented`（Rust 转换 URL 组装 + 本地回环 mock 端点验证请求构造/UA/header + 失败不替换旧组均有针对性单测通过；未跑真实窗口/真实转换服务，故非 `verified`）。

任务 ID：FIX-09B（拆卡自 FIX-09，见 `docs/tasks/FIX-09.md` 「拆卡登记」）。

本次唯一用户流程：订阅下载→转换目标（SubConvert）→更新后对象正确（过滤/排除/保留顺序）。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；审查结论见 `docs/evidence/parity-review-2026-10-03/` 的 `repair-queue.md` 第 34 行（FIX-09）、`settings-report.md` SET-05、`all-items` 的 `F-SUB-007`。上游基准：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `v2rayN/ServiceLib/Handler/SubscriptionHandler.cs:115-195`（`DownloadAllSubscriptions`/`DownloadMainSubscription`/`DownloadAdditionalSubscriptions`）、`v2rayN/ServiceLib/Global.cs:142-162`（`SubConvertUrls`/`SubConvertConfig`/`SubConvertTargets`）、`v2rayN/ServiceLib/Common/Utils.cs:182/280`（`UrlEncode`/`GetPunycode`）。

对应 feature / field ID：`F-SUB-007`、`SET-05`、`FLD-ENT-093`（ConvertTarget）、`FLD-CFG-084`（SubConvertUrl）。

## 输入、输出、错误、取消、权限、持久化及生效语义

- 输入：订阅下载管线（`SubUpdateRequest`）中的 `ConvertTarget`（`SubItem.convert_target`）与设置里的 `ConstItem.SubConvertUrl`；`SubConvertConfig` 取上游内置 `Global.SubConvertConfig.First()`。
- 输出：主订阅 URL 在 `ConvertTarget` 非空时按上游 `string.Format(template, UrlEncode(GetPunycode(url)))` 组装为转换服务请求，再补 `&target=<ConvertTarget>`、`&config=<SubConvertConfig>`（仅在模板不含 `target=`/`config=` 时追加）；转换服务返回的正文进入既有解析/过滤/替代管线。
- 错误：转换服务下载失败按既有 `download_with_fallback` 语义上报结构化 `PreservedError`/`Failed`；`via_proxy` 无端点仍为 `E_PROXY_UNAVAILABLE`（FIX-09 语义不变）。
- 取消：候选先行的管线在取消时返回 `Cancelled` 且 `break`，旧节点集不变（FIX-09 语义不变）。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/TUN、不监听端口（mock 仅测试期回环、端口 ≥11808 且先探测）。
- 持久化：仅成功 `Updated` 分支替换该 subid 节点并 touch `UpdateTime`；失败/取消不落盘替换。
- 生效：转换请求在后台 worker（bridge）与 scheduler 到期 pass 中经同一条 conversion-aware 管线执行。

## 上游对照（逐项）

| 上游 | 本实现 | 结论 |
|---|---|---|
| `ConvertTarget.IsNotEmpty()` 触发转换 | `has_convert_target` / `subscription_request_url` | 一致 |
| `config.ConstItem.SubConvertUrl` 空则 `Global.SubConvertUrls.First()` | `dns::effective_sub_convert_url` | 一致（FIX-16B 提供） |
| `string.Format(subConvertUrl, Utils.UrlEncode(url))` | `build_convert_url` + `util::url_encode`（RFC3986 unreserved） | 一致 |
| `!url.Contains("target=")` 才补 `&target=` | `build_convert_url` 同判定 | 一致 |
| `!url.Contains("config=")` 才补 `&config=Global.SubConvertConfig.First()` | `build_convert_url` + `BUILTIN_SUB_CONVERT_CONFIG` | 一致 |
| `Utils.GetPunycode` 先规范化主机 | `subscriptions::convert::punycode_url` | 一致（ASCII 原样，IDN 转 punycode） |
| `DownloadAllSubscriptions`：`ConvertTarget` 非空则跳过 `MoreUrl` | `download_all_configured` 的 `!converted` 判定 | 一致 |
| `DownloadAdditionalSubscriptions` 主结果 base64 先解码再逐条拼接 | `download_all_configured` 保留该顺序，并对 MoreUrl 逐个 `punycode_url` | 一致 |
| `DownloadSubscriptionContent` 代理失败回退直连 | `download_with_fallback` | 一致 |

## 允许修改的模块（本轮实际改动）

- `crates/subscriptions/src/convert.rs`（新增）：`punycode_url`、`build_convert_url` 纯函数 + 单测。
- `crates/subscriptions/src/lib.rs`：导出 `convert` 模块函数。
- `crates/application/src/subs.rs`：`BUILTIN_SUB_CONVERT_CONFIG`、`ConvertContext`、`subscription_request_url`、`has_convert_target`；`download_all` 拆出 `download_all_configured`（保留原签名供 engine 调用，默认用内置模板）；新增 `refresh_subscriptions_with_convert` + `refresh_one_with_convert`（conversion-aware 候选先行管线）；`run_scheduler_pass` 改走该管线；新增 4 个单测。
- `crates/bridge_api/src/api/subs.rs`：`spawn_sub_update` 改调 `application::subs::refresh_subscriptions_with_convert`（先返回 job id/取消语义不变）。

无 FRB 签名变化，未改 `frb_generated`。

## 禁止改变的已有行为

不改 FIX-09 已交付的 job id/取消与 `finish_sub_job` 语义；不改 FIX-09D 的到期过滤/代理选择/停止语义；不改 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`features/**`、`engine.rs`/`dns.rs`/`routing.rs`/`speedtest.rs`/`monitor.rs`、`crates/updater/**`。`PrevProfile`/`NextProfile` 属 FIX-09C，本卡登记不做。

## 测试夹具和原版预期

- 纯函数：模板含 `{0}`、已含 `target=`/`config=` 不重复追加、IDN→punycode。
- 集成（`subs.rs` 单测）：回环 mock 转换端点（`127.0.0.1:11808+`，先 bind 探测）验证请求行含 URL 编码后的源地址、`target=`、`config=`、`User-Agent`、自定义 header，且 `ConvertTarget` 非空时 `MoreUrl` 不被请求（captured 仅 1 条）；转换端点 500 时旧组保留、报结构化失败。
- 原版预期：指定目标后请求转换服务并成功导入；转换失败按空结果处理（保留旧组）；转换时忽略附加订阅。

## 本次必须通过的命令/真实场景（实际运行见下）

- Rust：`cargo test -p subscriptions --lib convert --locked`、`cargo test -p application --lib subs --locked`、`cargo test -p application --test subs_pipeline --locked`、`cargo test -p bridge_api --lib api::subs --locked`、`cargo clippy -p application -p subscriptions -p bridge_api --all-targets --locked -- -D warnings`、`rustfmt --check`（改动文件）。
- Flutter：本卡未改 Dart（转换目标字段与 picker 既有），未运行 Flutter 测试。
- 真实窗口集成与真实转换服务：登记为待根代理统一跑（本轮未跑）。

## 证据文件位置

`docs/evidence/UX-PARITY-FIX-09B/README.md`。

## 完成条件

- `ConvertTarget` 非空时按上游组装转换请求（含 target/config、URL 编码、punycode、UA/header），转换失败不替换旧组且结构化报错 → 已实现并有回环 mock 单测。
- 处理顺序与对象排除规则逐项对照上游 → 见上表。
- 针对性测试通过；未做真实窗口/真实转换服务，保持 `implemented`。

## 接口缺口 / 拆卡

- **管线重复（登记）**：因本卡禁改 `crates/application/src/engine.rs`，conversion-aware 管线落在 `subs.rs`（`refresh_subscriptions_with_convert`），与 engine 内 `refresh_one` 结构相同。建议根代理后续把转换上下文经 `SubUpdateRequest` 或 engine 入口统一，消除两条管线的漂移风险。本卡不自行改动 engine。
- **FIX-09C**：PrevProfile/NextProfile 节点链，另卡。
- **compat 台账**：`compat/features.yaml` 的 `F-SUB-007` 仍为 `preserved_only`；按「compat 仅追加」硬约束本卡未改行，建议根代理把其 status/summary/test_ids 更新为 implemented。

## 本轮实际结果

- `cargo test -p subscriptions --lib convert --locked`：4/4 通过。
- `cargo test -p application --lib subs --locked`：20/20 通过（含新增 4：`subscription_request_url_builds_convert_request_for_target`、`download_configured_uses_converter_and_skips_more_url`、`converted_pipeline_replaces_group_with_converted_nodes`、`convert_failure_preserves_old_group`）。
- `cargo test -p application --test subs_pipeline --locked`：11/11 通过（engine 旧管线回归）。
- `cargo test -p bridge_api --lib api::subs --locked`：8/8 通过。
- `cargo clippy -p application -p subscriptions -p bridge_api --all-targets --locked -- -D warnings`：通过，无告警。
- 改动文件 `rustfmt --check`：exit 0。
- 未运行：真实窗口集成、真实转换服务下载。
