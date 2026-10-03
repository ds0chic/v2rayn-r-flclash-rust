# UX-PARITY-FIX-09B — 订阅转换目标（SubConvert）

状态：`implemented`（Rust 纯函数 + 回环 mock 端点单测通过；未跑真实窗口/真实转换服务，故非 `verified`）。
关联：`docs/tasks/FIX-09B.md`、`docs/tasks/FIX-09.md`（拆卡）、`repair-queue.md` 第 34 行、`settings-report.md` SET-05、`F-SUB-007`/`FLD-ENT-093`/`FLD-CFG-084`。
上游冻结：`2dust-v2rayN` `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 结论

1. **转换请求按上游构造**：`subscriptions::convert::build_convert_url` 复刻 `DownloadMainSubscription`——`template.replace("{0}", UrlEncode(GetPunycode(url)))`，且仅在模板不含 `target=`/`config=` 时追加 `&target=<ConvertTarget>` / `&config=<Global.SubConvertConfig.First()>`。`ConvertContext::from_const_item` 经 `dns::effective_sub_convert_url` 解析 `ConstItem.SubConvertUrl`（空回退 `Global.SubConvertUrls[0]`）。
2. **UA/header 处理顺序不变**：转换请求复用同一 `DownloadOptions`（`user_agent` + `parse_request_headers` 解析后的自定义 header + proxy），因此 UA/header 与普通下载完全一致，仅 URL 变化。
3. **对象排除规则按上游**：`download_all_configured` 仅在 `ConvertTarget` 为空时下载并拼接 `MoreUrl`；转换时 `MoreUrl` 被跳过（回环 mock 断言 captured 仅 1 条请求）。主结果 base64 先解码、附加结果逐条 punycode+base64 判定拼接的顺序保留。
4. **失败明确报错且不替换旧组**：转换端点 500 时 `download_with_fallback` 返回结构化错误，候选先行管线报 `PreservedError`/`Failed`，旧节点集保留（单测断言 `old-conv` 仍在）。
5. **接线**：bridge `spawn_sub_update` 与 scheduler `run_scheduler_pass` 均改走 `application::subs::refresh_subscriptions_with_convert`；FIX-09 的「先返回真实 job id / 取消绑定 id / 取消不替换」与 FIX-09D 的「到期过滤 / 代理选择 / 停止」语义未改。

## 上游对照

- `SubscriptionHandler.cs:121`：`ConvertTarget.IsNullOrEmpty() && MoreUrl` → 本实现 `!converted && more_url` 才处理 MoreUrl。一致。
- `SubscriptionHandler.cs:129-152`：`GetPunycode` → `string.Format(subConvertUrl, UrlEncode(url))` → 补 `target=`/`config=`。一致。
- `SubscriptionHandler.cs:158-195`：附加订阅主结果 base64 先解码、逐条拼接。一致。
- `Global.cs:142-162`：`SubConvertUrls[0]=https://sub.xeton.dev/sub?url={0}`、`SubConvertConfig[0]=ACL4SSR_Online.ini`。写入 `dns::BUILTIN_SUB_CONVERT_URL`（FIX-16B）与 `subs::BUILTIN_SUB_CONVERT_CONFIG`。一致。
- `Utils.cs:182`：`Uri.EscapeDataString`；复用既有 `util::url_encode`（RFC3986 unreserved）。一致。
- `Utils.cs:280`：`GetPunycode`；`convert::punycode_url` 对 ASCII URL 原样返回、对 IDN 解析为 punycode。一致。

## 实际命令与结果

Rust（workdir = repo 根，工具 `C:\Users\Colby\.cargo\bin\cargo.exe`）：

| 命令 | 结果 |
|---|---|
| `cargo test -p subscriptions --lib convert --locked` | `4 passed; 0 failed` |
| `cargo test -p application --lib subs --locked` | `20 passed; 0 failed`（原 16 + 新增 4） |
| `cargo test -p application --test subs_pipeline --locked` | `11 passed; 0 failed`（engine 旧管线回归） |
| `cargo test -p bridge_api --lib api::subs --locked` | `8 passed; 0 failed` |
| `cargo clippy -p application -p subscriptions -p bridge_api --all-targets --locked -- -D warnings` | `Finished`，无告警 |
| `rustfmt --edition 2021 --check <4 个改动文件>` | exit 0 |

新增单测：
- `subscriptions::convert::tests::{builds_format_url_plus_target_and_config, does_not_duplicate_existing_target_or_config, template_without_placeholder_is_left_untouched_then_extended, punycode_only_rewrites_non_ascii_hosts}`
- `application::subs::tests::{subscription_request_url_builds_convert_request_for_target, download_configured_uses_converter_and_skips_more_url, converted_pipeline_replaces_group_with_converted_nodes, convert_failure_preserves_old_group}`

回环 mock 端点全部落在 `127.0.0.1:11808..11950`，先 `TcpListener::bind` 探测（未占用 `10808`，未改系统代理/注册表/路由/TUN）。夹具只用合成 URL，不含真实订阅/凭据。

Flutter：本卡未改 Dart，未运行 Flutter 测试（转换目标字段与 picker 既有）。

## 未完成 / 接口缺口 / 下一步前置

- **管线重复（接口缺口）**：因禁改 `crates/application/src/engine.rs`，conversion-aware 管线以 `subs::refresh_subscriptions_with_convert` 形式与 engine `refresh_one` 并存（结构相同）。建议根代理将转换上下文经 `SubUpdateRequest`/engine 入口统一，消除漂移风险。
- **compat**：`compat/features.yaml` 的 `F-SUB-007` 仍标 `preserved_only`；按「仅追加」硬约束未改行，建议根代理更新其 status/summary/test_ids。
- **未验证**：真实窗口集成（`flutter test integration_test/... -d windows`）、真实转换服务下载未跑。
- 下一步前置：FIX-09C（PrevProfile/NextProfile，本卡登记不做）。

## 改动文件

- `crates/subscriptions/src/convert.rs`（新增）
- `crates/subscriptions/src/lib.rs`
- `crates/application/src/subs.rs`
- `crates/bridge_api/src/api/subs.rs`
- `docs/tasks/FIX-09B.md`（新增）
- `docs/evidence/UX-PARITY-FIX-09B/README.md`（新增）
