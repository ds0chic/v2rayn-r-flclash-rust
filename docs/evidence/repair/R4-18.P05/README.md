# R4-18.P05 协议节点完整闭环 — HTTP 证据

状态：implemented（合成 widget/单元测试已运行；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定基线 / 环境

- 应用 HEAD：`a296892`；上游固定：`7d6a967`（UP = `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`）。
- `armed=false`；合成数据；未占用端口；未触碰 `127.0.0.1:10808`。

## 上游语义（7d6a967）

- `AddServerWindow.xaml.cs:203` 安全列表 `["", tls]`；HTTP **不**追加 `reality`（`:80` 绑定 HTTP 专有控件）。
- `AddServerViewModel.cs:390-394`：非空 `HttpHeadersJson` 必须能解析为 JSON，否则拒绝保存。
- 消费者 `V2rayOutboundService.cs:125-146`：`HttpHeaders` 非空时 `ParseJson` 写入；auth 仅在 user/pass 都非空时写。
  - Fortify：若 UI 不拦截非法 JSON，codegen `parse_json_text` 返回 `None` 会静默丢掉 headers（字段被静默降级）。

## 改动

- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`
  - `_validate`：HTTP 的 `httpHeaders` 非空且 `jsonDecode` 失败时报“JSON 格式无效”。
  - `securityFields(..., realityAllowed)` → HTTP 不再出现 `reality`。
- 测试：`test/repair/r4_18_p05_repro_test.dart`、`test/r4_18_p05_contract_test.dart`。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p05_repro_test.dart`（修复前） | FAIL 2/2：HTTP 仍提供 reality；非法 headers JSON 仍保存 |
| `flutter test test/repair/r4_18_p05_repro_test.dart`（修复后） | PASS 2/2 |
| `flutter test test/r4_18_p05_contract_test.dart` | PASS 3/3（合法 headers 重开、空 headers、取消） |
| `flutter analyze`（范围） | No issues found |
| Rust 门禁 | `cargo fmt`/`clippy`/`test --workspace` 均 PASS |
| `flutter build windows --release` | PASS |

## 缺口

1. 真实 FRB+SQLite 重开与真实 HTTP 代理握手未运行。
2. 合法 headers 到生成配置的端到端 JSON 未在本轮实测（Rust codegen 既有测试覆盖映射）。
3. 无截图。

## 下一步前置

- 真实 bridge DTO + 数据目录重开；隔离环境本地合成 HTTP 代理握手。
