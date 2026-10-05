# R4-18.P01 协议节点完整闭环 — VMess 证据

状态：implemented（合成 widget/单元测试已运行；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定基线 / 环境

- 应用 HEAD：`a296892`（工作树含其它代理并行改动；本卡仅改 `profile_fields.dart` / `profile_editor_dialog.dart`）。
- 应用审查基线：`77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。
- 上游固定：`7d6a967c18c697f28dc6917122ed3a4993fcf336`（UP = `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`）。
- `armed=false`；无 AUTO_SMOKE、无预置 active、无开发 Xray 环境变量。
- Flutter 3.47.5 / Dart 3.13.4；Rust 1.98.1 stable-msvc；Windows 11 25H2 26220。
- 端口：本轮未监听/未连接任何端口，未触碰 `127.0.0.1:10808`。
- 数据：仅合成 DTO/草稿，无真实节点或凭据。

## 上游语义（7d6a967）

- `Global.cs:49 DefaultSecurity = "auto"`；`:267 Global.VmessSecurities` 顺序为 `aes-128-gcm, chacha20-poly1305, auto, none, zero`。
- `AddServerViewModel.cs:308`：空 `VmessSecurity` 回落到 `DefaultSecurity`（编辑与新建都适用）。
- `AddServerViewModel.cs:423`：`AlterId > 0 ? ToString() : null` → 新节点 alterId 落库为 `null`。
- `AddServerWindow.xaml.cs:203` 基础安全列表 `["", tls]`，`:228/:234/:271` 只为 VLESS/Trojan/Anytls 追加 `reality`；VMess **不**支持 Reality。
- 消费者 `V2rayOutboundService.cs:82-89` / `SingboxOutboundService.cs:117-123`：security 不在候选集时回落 `DefaultSecurity`。

## 改动

- `apps/desktop/lib/features/profiles/profile_fields.dart`
  - `_vmessSecurities` 顺序改为上游 `Global.VmessSecurities`。
  - `ProfileCapabilities.supportsReality` 改为仅 VLESS/Trojan/Anytls（移除 VMess，补 Anytls 以对齐上游且不回退）。
  - `securityFields` 新增 `realityAllowed`，非 Reality 协议不再提供 `reality` 选项。
- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`
  - `initState` 对空 `vmessSecurity` 填 `auto`（对齐 `AddServerViewModel.cs:308`）。
  - `securityFields(..., realityAllowed: supportsReality(configType))`。
- 测试：`test/repair/r4_18_p01_repro_test.dart`、`test/r4_18_p01_contract_test.dart`。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p01_repro_test.dart`（stash 掉修复后预检） | FAIL 2/2：security 顺序不符；security 期望 `auto` 实际 `null` |
| `flutter test test/repair/r4_18_p01_repro_test.dart`（修复后） | PASS 2/2 |
| `flutter test test/r4_18_p01_contract_test.dart` | PASS 4/4（保存/重开、取消、非法 UUID、DTO 边界） |
| `flutter analyze <改动+测试>` | No issues found |
| `cargo fmt --all -- --check` | PASS（exit 0） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS（exit 0） |
| `cargo test --workspace --locked` | PASS（exit 0） |
| `flutter build windows --release` | PASS（`build\windows\x64\runner\Release\v2rayn_desktop.exe`） |

复现证据（修复前）：失败断言见 `commands.log`，2 项均按上游预期失败，修复后转绿。

## 缺口（未运行 / 未实测）

1. 真实 FRB + SQLite 的“新建→保存→关进程重开”未运行；本轮仅 DTO/草稿边界。
2. VMess 合成服务真实握手未运行（无隔离环境与内核）。
3. 未做 TUIC/Naive/其它协议回归；`realityAllowed` 已按上游补齐 Anytls。
4. 无截图（本机无可用 computer-use；仅 widget 断言）。

## 下一步前置

- 整合者提供真实 bridge DTO + 测试数据目录，补“UI→真实 FRB→SQLite→重开”。
- 隔离环境用本地合成 VMess 服务做真实握手（端口 ≥11808）。
