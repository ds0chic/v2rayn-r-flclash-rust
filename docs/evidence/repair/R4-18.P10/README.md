# R4-18.P10 协议节点完整闭环 — AnyTLS 证据

状态：implemented（合成 widget/单元测试通过；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定引用 / 无副作用

- 应用 HEAD `db3371a`；上游固定 `7d6a967`；UP=`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。
- `armed=false`；合成数据；未监听端口；未触碰 `127.0.0.1:10808`；未改宿主代理/TUN/路由/自启；未读写用户凭据。

## 上游依据

- `AddServerWindow.xaml.cs:130-132` Anytls 仅绑定 `txtId11`->Password。
- `AddServerWindow.xaml.cs:266-273` `gridTransport`/`gridFinalmask` 折叠、核心禁改（sing-box）、stream-security 追加 `reality`。
- `ConfigHandler.cs:943-961` `AddAnytlsServer`：core=sing_box、`Network=""`、空 StreamSecurity -> tls、密码必填。
- 保存侧 `crates/application/src/custom.rs:229-235` 已实现同样归一。
- sing-box 消费：`crates/config_codegen/src/singbox/outbound.rs:496-498`（password）+ TLS/Reality 通用段。

## 改动

- `profile_fields.dart`：`supportsTls` 纳入 AnyTLS；新增 `supportsTransport`。
- `profile_editor_dialog.dart`：AnyTLS 传输段折叠；空 `streamSecurity` 默认 `tls`；TLS/Reality 段保留（reality 可选）。
- 测试：`test/repair/r4_18_p10_repro_test.dart`、`test/r4_18_p10_contract_test.dart`。

## 结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p10_repro_test.dart`（修复前） | FAIL 2/2（supportsTls 排除 AnyTLS；渲染 `row-network`） |
| `flutter test test/repair/r4_18_p10_repro_test.dart`（修复后） | PASS 2/2 |
| `flutter test test/r4_18_p10_contract_test.dart` | PASS 4/4（保存/重开 + Reality、空 TLS 默认、缺密码可读、能力集） |
| `flutter analyze <范围内文件>` | No issues found |
| Rust gate | fmt/clippy PASS；test 见阻塞 |
| `flutter build windows --release` | 见阻塞（非本卡文件） |

## 缺口

1. 真实 FRB+SQLite 重开、真实 AnyTLS 握手未运行。
2. Reality/TLS 实际内核生效未验证。
3. 无截图。

## 下一步前置

真实 bridge + 重开，隔离环境合成 AnyTLS 服务（端口 ≥11808）。
