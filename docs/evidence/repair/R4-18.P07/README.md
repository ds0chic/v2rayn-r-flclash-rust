# R4-18.P07 协议节点完整闭环 — Hysteria2 证据

状态：implemented（合成 widget/单元测试通过；真实 FRB+SQLite 重开、真实内核握手未运行）。

## 固定引用 / 无副作用

- 应用 HEAD `db3371a`；上游固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；UP=`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/`。
- `armed=false`；合成数据；未监听端口；未触碰 `127.0.0.1:10808`；未改宿主代理/TUN/路由/自启；未读写用户凭据。

## 上游依据

- `AddServerWindow.xaml.cs:102-112` Hysteria2 绑定 password/salamanderPass/ports/hopInterval/upMbps/downMbps/hy2RealmUrl/gecko*。
- `AddServerWindow.xaml.cs:238-244` `gridTransport` 折叠、fingerprint/alpn 禁用；stream-security 列表为 `["", tls]`（无 reality）。
- `ConfigHandler.cs:769-836` `AddHysteria2Server`：TrimEx、fingerprint/alpn 清空、`Network=""`、空 StreamSecurity -> tls、密码必填、gecko 归一。
- 应用保存侧 `crates/application/src/custom.rs:195-206` 已实现同样归一。
- sing-box 消费：`crates/config_codegen/src/singbox/outbound.rs:483-486,596-696`（含 obfs/gecko/up/down/ports/hop/realm）。

## 改动

- `apps/desktop/lib/features/profiles/profile_fields.dart`：`supportsTls` 纳入 Hysteria2；新增 `supportsTransport`（Hysteria2/TUIC/WireGuard/Anytls/Naive 折叠）。
- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`：传输段按 `supportsTransport` 折叠；空 `streamSecurity` 默认 `tls`；TLS/Reality 段保留。
- 测试：`test/repair/r4_18_p07_repro_test.dart`、`test/r4_18_p07_contract_test.dart`。

## 结果

| 命令 | 结果 |
|---|---|
| `flutter test test/repair/r4_18_p07_repro_test.dart`（修复前） | FAIL 2/2（supportsTls 排除 Hysteria2；渲染 `row-network`） |
| `flutter test test/repair/r4_18_p07_repro_test.dart`（修复后） | PASS 2/2 |
| `flutter test test/r4_18_p07_contract_test.dart` | PASS 4/4（保存/重开、缺密码可读、取消、能力集） |
| `flutter analyze <范围内文件>` | No issues found |
| Rust gate | `cargo fmt --check` PASS；`cargo clippy ... -D warnings` PASS；`cargo test --workspace` 见阻塞 |
| `flutter build windows --release` | 见阻塞（main_shell.dart 编译错误，非本卡文件） |

## 缺口

1. 真实 FRB+SQLite 重开、真实 Hysteria2 握手未运行。
2. `ports/hopInterval/gecko` 实际内核生效未验证。
3. 无截图。

## 下一步前置

真实 bridge DTO + 项目目录重开，隔离环境合成 TLS Hysteria2 服务（端口 ≥11808）。
