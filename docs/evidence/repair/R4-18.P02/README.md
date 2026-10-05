# R4-18.P02 协议节点完整闭环 — VLESS 证据

状态：implemented（合成 widget/单元测试已运行；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定基线 / 环境

- 应用 HEAD：`a296892`；上游固定：`7d6a967`（UP = `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`）。
- `armed=false`；合成数据；未占用端口；未触碰 `127.0.0.1:10808`；无真实凭据。
- Flutter 3.47.5 / Dart 3.13.4；Rust 1.98.1；Windows 11 25H2 26220。

## 上游语义（7d6a967）

- `AddServerViewModel.cs:309`：空 `VlessEncryption` 回落到 `Global.None = "none"`（`Global.cs:52`）。
- `AddServerViewModel.cs:303`：空 `Flow` 保持空（落库 `NullIfEmpty()` → null）。
- `AddServerWindow.xaml.cs:228`：VLESS 追加 `reality`（`:91-92` 绑定 `cmbFlow5`/`txtSecurity5`）。
- 消费者 `V2rayOutboundService.cs:153` 写 `encryption = VlessEncryption`；`:155-163` 有 flow 时写 flow 并禁用 mux。

## 改动

- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`
  - `initState` 对空 `vlessEncryption` 填 `none`（对齐 `AddServerViewModel.cs:309`）。
- `apps/desktop/lib/features/profiles/profile_fields.dart`
  - `supportsReality` 覆盖 VLESS（保留原有行为）；`securityFields` 通过 `realityAllowed` 提供 reality。
- 测试：`test/repair/r4_18_p02_repro_test.dart`、`test/r4_18_p02_contract_test.dart`。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p02_repro_test.dart`（修复前） | FAIL：encryption 期望 `none` 实际 `null`（reality 用例通过） |
| `flutter test test/repair/r4_18_p02_repro_test.dart`（修复后） | PASS 2/2 |
| `flutter test test/r4_18_p02_contract_test.dart` | PASS 3/3（flow+encryption 保存/重开、Reality 展开、取消） |
| `flutter analyze`（范围） | No issues found |
| Rust 门禁 | `cargo fmt`/`clippy`/`test --workspace` 均 PASS（见 P01 日志，同一工作树） |
| `flutter build windows --release` | PASS |

## 缺口

1. 真实 FRB+SQLite 重开与真实 VLESS 握手未运行。
2. 未验证 Reality 真实节点握手。
3. 无截图。

## 下一步前置

- 真实 bridge DTO + 数据目录重开；隔离环境本地合成服务握手（端口 ≥11808）。
