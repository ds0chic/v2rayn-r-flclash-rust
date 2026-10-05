# R4-16 17 格式逐条 matrix + 真实数据目录重开互通

本轮（2026-10-05，matrix 子代理）补齐 R4-16 未完成部分：上游 `compat/features.yaml`
`fmt_formats` 清单 FMT-001..FMT-017 的逐条 import→export→re-import matrix，以及真实
SQLite 数据目录的 导入→重开→导出→再导入 互通验证。

## 基线 / 环境

- 起点 HEAD：`a95897f`（未 commit）。上游固定 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
- Flutter 3.47.5 / Dart 3.13.4；Rust 1.98.1 stable-msvc；Windows 11 25H2 26220。
- 真实桥：`target/debug/bridge_api.dll`（既有构建，`2026/10/5 15:22`，未在本轮重编）。
- `armed=false`；无 AUTO_SMOKE / 预置 active / 开发 Xray 环境变量；未监听任何端口，未触碰 `127.0.0.1:10808`。
- 数据仅合成 fixture（合成分享链接 / 合成 config JSON/YAML），未读写任何真实订阅 URL 或用户凭据。

## 复现入口

- 纯解析层逐条：`crates/subscriptions/tests/r4_16_matrix.rs`（18 tests，`cargo test -p subscriptions`）。
- 真实桥 + 真实 SQLite 重开：`apps/desktop/test/r4_16_matrix_test.dart`（`flutter test`，加载 `bridge_api.dll`，
  `engine.initEngine(dataDir: 临时目录)`，`subs.importFromText(subid=...)` 单事务落库 → `engine.queryProfiles`
  读回 → `subs.exportProfiles` / `FrbBridgePort.exportClientConfigText` → `subs.importFromText` 再导入）。

## 逐格式结果（FMT-001..FMT-017）

| ID | 上游格式 | 导入 | 重开（SQLite 读回） | 导出 seam | 再导入字段/未知键 | 结果 |
|---|---|---|---|---|---|---|
| FMT-001 | VmessFmt | OK (vmess) | OK | share URI | 一致（`aid` 默认 0/缺省归一） | verified |
| FMT-002 | VLESSFmt | OK (vless, reality) | OK | share URI | 一致 | verified |
| FMT-003 | ShadowsocksFmt | OK (ss; Rust 侧含 SIP008) | OK | share URI | 一致 | verified |
| FMT-004 | SocksFmt | OK (socks 旧式 base64) | OK | share URI | 一致 | verified |
| FMT-005 | TrojanFmt | OK (trojan) | OK | share URI | 一致 | verified |
| FMT-006 | Hysteria2Fmt | OK (hy2; Rust 侧含 realm) | OK | share URI | 一致 | verified |
| FMT-007 | TuicFmt | OK (tuic) | OK | share URI | 一致 | verified |
| FMT-008 | WireguardFmt | OK (wireguard; Rust 侧含 .conf 双 peer) | OK | share URI | 一致 | verified |
| FMT-009 | AnytlsFmt | OK (anytls) | OK | share URI | 一致 | verified |
| FMT-010 | NaiveFmt | OK (naive+https) | OK | share URI | 一致（`security` none/缺省归一） | verified |
| FMT-011 | InnerFmt | OK (v2rayn://vless) | OK | inner `v2rayn://` | 一致 | verified |
| FMT-012 | V2rayFmt | OK (Custom/Xray) | OK | full config | `x-future` 存活，Custom/Xray 一致 | verified |
| FMT-013 | SingboxFmt | OK (Custom/SingBox) | OK | full config | `x-future` 存活，Custom/SingBox 一致 | verified |
| FMT-014 | ClashFmt | OK (Custom/Mihomo) | OK | full config | `x-future` 存活，Custom/Mihomo 一致 | verified |
| FMT-015 | HtmlPageFmt | 检测为 HTML，无可导入节点 | 无落库 | 不适用 | 可读失败（无 profile） | verified（detection-only） |
| FMT-016 | BaseFmt | 无独立 codec（公共基类） | — | — | 由各具体 codec 传递覆盖 | not_applicable |
| FMT-017 | FmtHandler | 统一分发入口 | — | — | 全部 scheme 分发矩阵通过 | verified |

字段比较口径（在测试内显式归一，非改预期）：可选分享参数 `null` 与空串等价；`security=none` 与缺省等价；
VMess `aid=0` 与缺省等价。归一后 stage1（fixture 导入）与 stage2（canonical 导出再导入）字段/未知键一致。

### 本轮发现并修复的解析不一致（最小改动）

matrix 暴露 IPv6 分享链接 re-import 字段不一致：`url::Url::host_str()` 对 IPv6 字面量保留 `[]`，
而存储 `Address` 与 `bracket_ipv6` 导出期望裸主机。新增 `fmt::base::host_addr()`（剥离一对外层 `[]`），
10 个 codec 统一改用它（anytls/hysteria2/naive/shadowsocks/socks/trojan/tuic/vless/vmess/wireguard）。
纯解析层 `fmt_008_wireguard_roundtrip_and_conf` 现在断言 `@[2001:db8::40]:51820` 回读为 `2001:db8::40`。

## 合同项验证

- 手工来源规则：真实桥 `importFromText(deduplicate:false)` 重复行保留 2 条；`deduplicate:true` 折叠为 1 条；
  导入行 `isSub=false`（R4-17 删除范围前提）。
- 单事务组提交：有组 commit 走既有 `import_from_text` 单事务入口；Dart 侧 `commitImport` 不再逐行重写（回归见 `test/r4_16_contract_test.dart`）。
- preview 分离：`subs.previewImportText` 真实桥 parse-only，读回该组 0 行（未落库）。
- 坏行：`good\nnot-a-uri` 导入 1 条并定位错误，好行落库。
- 文件不可写：`writeExportFile` 指向缺失父目录返回结构化错误（ok=false, error 非空）。
- 导出可被原版再导入：share/inner/full 三类导出文本均被真实桥再导入成功且字段一致。
- 取消不落库：preview 后不调用 commit，SQLite 无写入（`test/r4_16_contract_test.dart` 合成桥断言 + 真实桥 preview 断言）。

## 命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test -p subscriptions --locked` | PASS（67+11+23+9+13+18，含新增 18） |
| `cargo fmt --all -- --check` | PASS（exit 0） |
| `cargo clippy -p subscriptions --all-targets --locked -- -D warnings` | PASS（exit 0） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS（exit 0；首轮曾被并行卡 `bridge_api/src/api/speedtest.rs` 临时编译错误阻塞，重试通过） |
| `cargo test --workspace --locked` | PASS（exit 0；首轮 `application --test t15_monitor` 为并行卡瞬态失败，单跑与重跑均 2/2 通过） |
| `flutter test test/r4_16_matrix_test.dart` | PASS：All tests passed（17 行 + 合同守卫） |
| `flutter test test/t21e_import_real_bridge_test.dart` | PASS（关闭原缺口 #2） |
| `flutter analyze lib/features/subs test/r4_16_matrix_test.dart` | PASS：No issues found |
| `flutter analyze`（整仓） | PASS：No issues found（首轮曾被并行卡 `r4_22` / `settings` / `r4_23` 文件阻塞，并行卡修复后通过） |

## 缺口 / 未完成

1. 无组单事务桥入口 `commit_import_text` 仍缺失（FRB 未暴露）；无组导入仍走 `saveImportedProfile` 逐行兜底。`preview_import_text` 绑定现已存在。
2. 未做真正的进程级重开：重开 = 同进程从 SQLite 读回。跨进程重启读回未实测。
3. 本轮未重编 `bridge_api.dll`（沿用既有构建）；`base::host_addr` 修正由 Rust 单测验证，Dart fixture 未含 IPv6。
4. 有组 commit 二次解析（复用 `import_from_text`）未计时；10000 条响应时间未实测。
5. 真实文件系统权限失败未实测（仅缺父目录分支）。
6. 完整配置导出对 Custom 节点本轮已实测（x-future 存活），但仅合成 fixture，未与原版 v2rayN 实机对拉。
