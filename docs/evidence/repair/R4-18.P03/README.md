# R4-18.P03 协议节点完整闭环 — Shadowsocks 证据

状态：implemented（合成 widget/单元测试已运行；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定基线 / 环境

- 应用 HEAD：`a296892`；上游固定：`7d6a967`（UP = `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`）。
- `armed=false`；合成数据；未占用端口；未触碰 `127.0.0.1:10808`。

## 上游语义（7d6a967）

- `AddServerWindow.xaml.cs:203` 基础安全列表 `["", tls]`，SS 不追加 `reality`（仅 VLESS/Trojan/Anytls）。
- `Global.cs:286 SsSecuritiesInXray` / `:301 SsSecuritiesInSingbox`；`AppManager.GetShadowsocksSecurities` 按核返回列表（`AppManager.cs:658-663`）。
- `AddServerWindow.xaml.cs:68-69` 绑定 `cmbSecurity3` + `togUotEnabled3`。
- `AddServerViewModel.cs:366-369` 保存要求 `SsMethod` 非空；`NodeValidator.cs:103-104` 校验方法属于所选核（sing-box）。
- 消费者 `V2rayOutboundService.cs:99-101`：方法不在核列表时回落 `"none"`（因此 UI 必须在提交前拦截，避免字段被静默改写）。

## 改动

- `apps/desktop/lib/features/profiles/profile_fields.dart`
  - 抽出 `shadowsocksMethods(CoreType?)`（Xray/sing-box 列表），`protocolFields` 复用。
- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`
  - `_validate`：SS 方法非空且不属于所选核列表时报“当前内核不支持该加密方式”。
  - `securityFields(..., realityAllowed: supportsReality)` → SS 不再出现 `reality`。
- 测试：`test/repair/r4_18_p03_repro_test.dart`、`test/r4_18_p03_contract_test.dart`。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p03_repro_test.dart`（修复前） | FAIL 2/2：SS 仍提供 reality；sing-box+`plain` 仍可保存 |
| `flutter test test/repair/r4_18_p03_repro_test.dart`（修复后） | PASS 2/2 |
| `flutter test test/r4_18_p03_contract_test.dart` | PASS 4/4（方法表随核、method+uot 重开、空方法必填、取消） |
| `flutter analyze`（范围） | No issues found |
| Rust 门禁 | `cargo fmt`/`clippy`/`test --workspace` 均 PASS |
| `flutter build windows --release` | PASS |

## 缺口

1. 真实 FRB+SQLite 重开与真实 SS 握手未运行。
2. UOT 真实 UDP 行为未验证。
3. 无截图。

## 下一步前置

- 真实 bridge DTO + 数据目录重开；隔离环境本地合成 SS 服务握手。
