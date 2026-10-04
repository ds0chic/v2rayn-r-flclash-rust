# RE-PROF-12 / RR-MON-REBIND 证据

状态：`implemented`。日期 2026-10-04；开始/结束 HEAD `cbf4fa7`（工作树开始时干净）。

本回合只跑 Dart 单元与 Rust bridge_api 单 crate 测试；没有启动内核/监听端口、没有写宿主系统代理/注册表/TUN、没有读取真实节点/证书秘密；未占用 127.0.0.1:10808；测试数据全部为合成 PEM / 临时目录字符串。

## 改动文件

- `apps/desktop/lib/features/profiles/profile_fields.dart`
  - 新增 `selectCertFetchServerName`（SNI → transportHost → address）。
  - `fetchPeerCertPem` 以 serverName 为握手/连接名（非裸 IP）；`fetchPeerCertChainPem` 返回 `PeerCertChainResult{pem, leafOnly=true}` + `peerCertChainResult`。
  - `certSha256Thumbprint` 捕获非法 base64 的 `FormatException` 返回 `null`（`certShaFromChain` 因此对无效块返回 `null`）。
- `apps/desktop/lib/features/profiles/profile_editor_dialog.dart`
  - `_fetchCert` 按上游顺序选择 serverName；「获取证书链」显示「已获取证书（仅叶子；该平台无法获取完整链）」。
- `apps/desktop/lib/features/monitor/monitor_controller.dart`
  - 新增 `monitorSessionSignature` 与 `_lastSessionSig`，`syncRuntimeSession` 仅在 applied 签名变化时 `syncSession`。
  - `refreshStats` 将 `statsSnapshot().error` 映射到 `MonitorState.error`（store 失败可见）。
- `crates/bridge_api/src/api/monitor.rs`
  - `MonitorHub` 新增 `store_dir`/`store_error`/`session_sig`；`sync_from_engine_session` 仅在 applied 会话签名变化时清 `source_sig`。
  - 新增 `rebind_store`：打开/加载失败不静默 bound，`store_bound=false` 可重试，dataDir 变化强制重绑（回退 `InMemoryTrafficStore`）。
  - `stats_snapshot().error` 返回 `store_error`；新增常量 `STORE_BIND_ERROR_CODE=“E_MONITOR_STORE”`。
  - 新增测试 `store_bind_failure_is_visible_and_retryable`。
- `apps/desktop/test/support/fake_monitor_bridge.dart`：新增 `statsError` 用于故障注入。
- `apps/desktop/test/re_prof_12_cert_test.dart`（新）：SNI 选择 / 非法 base64 / 叶子提示 / 链 SHA。
- `apps/desktop/test/rr_mon_rebind_test.dart`（新）：签名去重 / 心跳不重绑 / 真实变化重绑 / store 失败可见。

## 实际运行的命令与结果

| 命令 | 结果 |
|---|---|
| `dart format <6 文件>` | 1 changed（editor 格式化），其余已规范 |
| `cargo fmt -p bridge_api -- --check` | 退出 0 |
| `cargo test -p bridge_api --lib monitor::tests --locked` | 8 passed / 0 failed（含新 store 用例） |
| `flutter analyze` | 1 issue：既有无关 `tray_menu_model.dart:1` unused import |
| `flutter test test/re_prof_12_cert_test.dart test/rr_mon_rebind_test.dart test/fix11_monitor_session_test.dart`（含 fix02 同批） | 两个新文件 12 用例全通过 |
| `flutter test test/fix02_tuic_editor_test.dart` | 首个用例后 “did not complete”（已知 Flutter 原生偶发；见下） |
| `flutter test test/fix02_tuic_editor_test.dart --plain-name “Cert SHA-256 helpers parse PEM chains deterministically”` | 1 passed |
| `flutter test test/fix02_tuic_editor_test.dart --plain-name “Fingerprint candidates include randomized and empty”` | 1 passed |

### 关于 fix02 “did not complete”

同批/整文件运行在累积到第 5~6 个 widget 用例后出现 `did not complete`，与本卡改动无因果关系：单用例隔离运行（含直接命中 base64 边界的新逻辑）均通过；该现象与仓库 AGENTS「flutter test 偶发 exit 79 重试即可」及 `profiles_selection_test.dart` 记录的 Flutter 原生资源/segfault 历史一致。未删除或篡改既有断言。

## 上游对照结论

- SNI：上游 `AddServerViewModel.cs:493-544` 以 `Sni → GetCurrentTransportHost → Address` 选 serverName 并将 `Address:Port` 作为域；本卡实现同序选择，并以选定名连接（`dart:io` 无独立 SNI 参数，已在 task 卡登记 blocked）。
- 全链：上游 `GetCertChainPemAsync` 可取完整链并 `ConcatenatePemChain`；`dart:io` 只暴露叶子，本卡显式返回 `leafOnly=true` 并在 UI 明示，登记 blocked，未冒充全链。
- SHA：上游 `GetCertSha256Thumbprint` 出错即返回空、`UpdateCertSha` 无有效块即保持原值；本卡捕获 `FormatException` 返回 `null`、`_syncCertSha` 不写，行为一致。
- 监控：上游统计 source 随会话生命周期重建；本卡仅在 applied 会话签名变化（core/端点/统计开关/活动节点）时重建，无关日志/心跳不动，store 失败可见可重试。

## 未完成 / 下一步前置

- 未做真实 TLS 端口（≥11808，先探测）的实机握手与完整证书链；`SecureSocket` 无法“连 IP 另发 SNI”的平台边界已登记 blocked。
- 未做真实内核 + Flutter 同屏非零流量的整链验证，也未覆盖 runtime.md 风险第 2 条（current-thread Tokio + 阻塞 sleep 的 sing-box WS pump 调度）；需独立卡在同一 production hub 验证。
- 下一步前置：接入真实受管 sing-box/Xray 会话，观测 source 重建计数与 store 错误恢复，再决定是否升 `verified`。
