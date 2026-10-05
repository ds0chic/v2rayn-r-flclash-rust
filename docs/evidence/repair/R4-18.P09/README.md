# R4-18.P09 协议节点完整闭环 — WireGuard 证据

状态：implemented（合成 widget/单元测试通过；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定引用 / 无副作用

- 应用 HEAD `db3371a`；上游固定 `7d6a967`；UP=`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。
- `armed=false`；合成数据；未监听端口；未触碰 `127.0.0.1:10808`；未改宿主代理/TUN/路由/自启；未读写用户凭据。

## 上游依据

- `AddServerWindow.xaml.cs:120-128` WireGuard 绑定 `txtId9`->Password（私钥）、`txtPublicKey9`->WgPublicKey、`txtPreSharedKey9`->WgPresharedKey、`txtPath9`->WgReserved、`txtRequestHost9`->WgInterfaceAddress、`txtShortId9`->WgMtu、`txtDns`->WgDns。
- `AddServerWindow.xaml.cs:257-264` `gridTransport` 与 **`gridTls` 均折叠**（无 stream security/transport）。
- `ConfigHandler.cs:891-933` `AddWireguardServer`：TrimEx、base64 Reserved 归一、`WgMtu` 空/<=0 -> `Global.TunMtus.First()`、密码必填。
- 保存侧 `crates/application/src/custom.rs:247-260` 已实现（`wg_mtu` 默认 1280）。
- sing-box 消费：`crates/config_codegen/src/singbox/outbound.rs:391-394,735-766`（endpoint wireguard/address/private_key/mtu/peers/reserved）。

## 改动

- `profile_fields.dart`：新增 `supportsTransport`。
- `profile_editor_dialog.dart`：TLS/Reality 段按 `supportsTls||supportsReality` 折叠（WireGuard 无 TLS）；传输段折叠；空 `wgMtu` 默认 1280。
- 测试：`test/repair/r4_18_p09_repro_test.dart`、`test/r4_18_p09_contract_test.dart`。

## 结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p09_repro_test.dart`（修复前） | FAIL 1/1（渲染 `field-streamSecurity`/`row-network`） |
| `flutter test test/repair/r4_18_p09_repro_test.dart`（修复后） | PASS 1/1 |
| `flutter test test/r4_18_p09_contract_test.dart` | PASS 4/4（保存/重开 + MTU 默认、缺私钥可读、取消、能力集） |
| `flutter analyze <范围内文件>` | No issues found |
| Rust gate | fmt/clippy PASS；test 见阻塞 |
| `flutter build windows --release` | 见阻塞（非本卡文件） |

## 缺口

1. 真实 FRB+SQLite 重开、真实 WireGuard 握手未运行。
2. Reserved base64->列表、MTU 实际内核生效未验证。
3. 无截图。

## 下一步前置

真实 bridge + 重开，隔离环境合成 WireGuard 端点（端口 ≥11808，仅用户态，不建 TUN/路由）。
