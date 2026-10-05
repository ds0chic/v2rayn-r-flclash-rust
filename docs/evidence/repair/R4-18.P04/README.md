# R4-18.P04 协议节点完整闭环 — SOCKS 证据

状态：implemented（合成 widget/单元测试已运行；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定基线 / 环境

- 应用 HEAD：`a296892`；上游固定：`7d6a967`（UP = `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`）。
- `armed=false`；合成数据；未占用端口；未触碰 `127.0.0.1:10808`。

## 上游语义（7d6a967）

- `AddServerWindow.xaml.cs:203` 安全列表 `["", tls]`；SOCKS **不**追加 `reality`。
- `AddServerViewModel.cs:372-379`：只有 SOCKS/HTTP 允许空密码；其余协议必须填 UUID/密码。
- 消费者 `V2rayOutboundService.cs:109-124`：仅当 `Username` 与 `Password` 都非空才写 `user/pass/level/email`。

## 改动

- `apps/desktop/lib/features/profiles/profile_fields.dart`
  - `securityFields(..., realityAllowed)` → SOCKS 不再出现 `reality`。
- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`
  - `securityFields(..., realityAllowed: supportsReality(configType))`。
- 测试：`test/repair/r4_18_p04_repro_test.dart`、`test/r4_18_p04_contract_test.dart`。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p04_repro_test.dart`（修复前） | FAIL：SOCKS 仍提供 reality |
| `flutter test test/repair/r4_18_p04_repro_test.dart`（修复后） | PASS 1/1 |
| `flutter test test/r4_18_p04_contract_test.dart` | PASS 3/3（无凭据可保存、user/pass 重开、缺地址必填） |
| `flutter analyze`（范围） | No issues found |
| Rust 门禁 | `cargo fmt`/`clippy`/`test --workspace` 均 PASS |
| `flutter build windows --release` | PASS |

## 缺口

1. 真实 FRB+SQLite 重开与真实 SOCKS 握手未运行。
2. 未验证“仅用户名”时消费者省略认证的真实行为（Rust 既有测试覆盖映射）。
3. 无截图。

## 下一步前置

- 真实 bridge DTO + 数据目录重开；隔离环境本地合成 SOCKS 服务握手（端口 ≥11808）。
