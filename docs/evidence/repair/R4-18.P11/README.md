# R4-18.P11 协议节点完整闭环 — NaiveProxy 证据

状态：implemented（合成 widget/单元测试通过；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定引用 / 无副作用

- 应用 HEAD `db3371a`；上游固定 `7d6a967`；UP=`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。
- `armed=false`；合成数据；未监听端口；未触碰 `127.0.0.1:10808`；未改宿主代理/TUN/路由/自启；未读写用户凭据。

## 上游依据

- `AddServerWindow.xaml.cs:134-141` Naive 绑定 `txtId12`->Username、`txtSecurity12`->Password、`togNaiveQuic12`->NaiveQuic、`cmbCongestionControl12`->CongestionControl、`txtInsecureConcurrency12`->InsecureConcurrency、`togUotEnabled12`->Uot。
- `Global.cs:615-621` `NaiveCongestionControls = [bbr, bbr2, cubic, reno]`。
- `AddServerWindow.xaml.cs:275-286` `gridTransport`/`gridFinalmask` 折叠、核心禁改（sing-box）、fingerprint/alpn/allowInsecure 禁用；stream-security 无 reality。
- `ConfigHandler.cs:971-993` `AddNaiveServer`：core=sing_box、fingerprint/alpn/allowInsecure/Network 清空、空 StreamSecurity -> tls、密码必填。
- 保存侧 `crates/application/src/custom.rs:236-246` 已实现同样归一。
- sing-box 消费：`crates/config_codegen/src/singbox/outbound.rs:499-516`（username/password/quic/congestion/insecure_concurrency/uot）。

## 改动

- `profile_fields.dart`：新增 `_naiveCongestionControls` 并在 `protocolFields(ConfigType.naive)` 增加 `congestionControl` 字段；`supportsTls` 纳入 Naive；新增 `supportsTransport`。
- `profile_editor_dialog.dart`：Naive 传输段折叠；空 `streamSecurity` 默认 `tls`。
- 测试：`test/repair/r4_18_p11_repro_test.dart`、`test/r4_18_p11_contract_test.dart`。

## 结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p11_repro_test.dart`（修复前） | FAIL 2/2（缺 congestionControl；渲染 `row-network`） |
| `flutter test test/repair/r4_18_p11_repro_test.dart`（修复后） | PASS 2/2 |
| `flutter test test/r4_18_p11_contract_test.dart` | PASS 4/4（保存/重开 + TLS 默认、缺密码可读、取消、能力集） |
| `flutter analyze <范围内文件>` | No issues found |
| Rust gate | fmt/clippy PASS；test 见阻塞 |
| `flutter build windows --release` | 见阻塞（非本卡文件） |

## 缺口

1. 真实 FRB+SQLite 重开、真实 Naive 握手未运行。
2. QUIC/拥塞/并发实际内核生效未验证。
3. 无截图。

## 下一步前置

真实 bridge + 重开，隔离环境合成 Naive HTTPS/QUIC 服务（端口 ≥11808）。
