# R4-18.P06 协议节点完整闭环 — Trojan 证据

状态：implemented（合成 widget/单元测试已运行；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定基线 / 环境

- 应用 HEAD：`a296892`；上游固定：`7d6a967`（UP = `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`）。
- `armed=false`；合成数据；未占用端口；未触碰 `127.0.0.1:10808`。

## 上游语义（7d6a967）

- `AddServerWindow.xaml.cs:96-98`：Trojan 显示 `gridTrojan` 并把 `cmbFlow6`（`Global.Flows`，`Global.cs:323`）绑定到 `Flow`。
- `AddServerWindow.xaml.cs:234`：Trojan 追加 `reality`。
- `AddServerViewModel.cs:374-378`：非 SOCKS/HTTP 必须有密码；`:423/424` Flow/AlterId 空值落库为 null。
- 消费者 `V2rayOutboundService.cs:166-177` / `SingboxOutboundService.cs:224`：Trojan 写 password/ota/level（Flow 仅存节点、导出链接用，见 `TrojanFmt.cs:32`）。

## 改动

- `apps/desktop/lib/features/profiles/profile_fields.dart`
  - `protocolFields(ConfigType.trojan)` 新增 `flow` 下拉（`['', xtls-rprx-vision, xtls-rprx-vision-udp443]`）。
  - `securityFields(..., realityAllowed)`（Trojan 保持允许 reality）。
- 测试：`test/repair/r4_18_p06_repro_test.dart`、`test/r4_18_p06_contract_test.dart`。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p06_repro_test.dart`（修复前） | FAIL 2/2：无 flow 字段、编辑器不渲染 `field-flow` |
| `flutter test test/repair/r4_18_p06_repro_test.dart`（修复后） | PASS 2/2 |
| `flutter test test/r4_18_p06_contract_test.dart` | PASS 4/4（Reality 能力、Flow 重开、Reality 缺公钥拦截、缺密码拦截） |
| `flutter analyze`（范围） | No issues found |
| Rust 门禁 | `cargo fmt`/`clippy`/`test --workspace` 均 PASS |
| `flutter build windows --release` | PASS |

## 缺口

1. 真实 FRB+SQLite 重开与真实 Trojan 握手未运行。
2. Flow 的真实运行效果未验证（上游 Xray/sing-box Trojan 出站亦不消费 Flow）。
3. 无截图。

## 下一步前置

- 真实 bridge DTO + 数据目录重开；隔离环境本地合成 TLS 服务做 Trojan 握手（端口 ≥11808）。
