# RE-PROF-12 — 草稿证书辅助动作边界：SNI 握手、叶子不冒充全链、非法 base64 不崩溃

状态：`implemented`（Dart 单元 12/12 通过；未做真实 TLS 端口实机握手与完整证书链，故不写 `verified`）。

任务 ID：RE-PROF-12

本次唯一用户流程：在节点编辑器 TLS/Reality 区填写地址、端口、SNI/传输 Host 后点「获取证书」或「获取证书链」；握手按 SNI→transportHost→address 选择服务器名而非裸连 IP；「获取证书链」如实告知只有叶子（完整链 blocked）；粘贴合法/非法 PEM 文本时 CertSha 派生不抛异常、无有效证书不更新。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；当前复核 HEAD `cbf4fa7`。复核结论见 `docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-12（`:113-118`），关联 FIX-02 已实现的 Cert→SHA 基础。上游定位 `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/ViewModels/AddServerViewModel.cs:493-544`（`FetchCert`/`FetchCertChain`）与 `ServiceLib/Manager/CertPemManager.cs:211-309`（`ParsePemChain`/`GetCertSha256Thumbprint`/`GetLeafCertSha256Thumbprint`）。

对应 feature / field / action / layout ID：`ACT-ADDSRV-002`（添加/编辑节点-获取证书）、`ACT-ADDSRV-003`（生成 UUID/证书指纹）、`F-PROFILE-*` 证书字段、`FLD-ENT-*`（Cert/CertSha/SNI/Host）。

必读上游文件、符号和固定 commit：`AddServerViewModel.FetchCert`/`FetchCertChain`（SNI→`GetCurrentTransportHost`→Address 传 serverName，domain=Address:Port）、`UpdateCertSha`（`Cert` 空或无有效块时 return，保持原 SHA）、`CertPemManager.ParsePemChain`（按 `BEGIN/END CERTIFICATE` 切块）、`GetCertSha256Thumbprint`（`try/catch`，无效返回空串）、`GetCertChainPemAsync`（.NET 可取完整链）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：草稿的 `address`/`port`/`sni`/`host`/`streamSecurity`；Cert 文本框粘贴的 PEM（可合法/非法 base64）。
- 输出：`selectCertFetchServerName` 选择握手名；`fetchPeerCertPem` 返回叶子 PEM；`fetchPeerCertChainPem` 返回 `PeerCertChainResult{pem, leafOnly=true}`；`certShaFromChain`/`certSha256Thumbprint` 返回 64 位小写 hex 或 `null`。
- 错误：非法 base64 由 `base64.decode` 抛 `FormatException`，被捕获返回 `null`；无有效证书时 `_syncCertSha` 不写 `_draft.certSha`；握手失败提示「未能获取证书」/「获取证书失败」。
- 取消：编辑器取消不落盘（草稿本地）。
- 权限：只读本地/远端 TLS 握手，无系统代理/注册表/TUN；测试端口一律 ≥11808 且先探测。
- 持久化：保存走既有 `onSave`；本卡不改存储。
- 生效：获取成功后写入草稿 `cert`/`certSha`，保存时随既有 DTO 落库。

允许修改的模块：`apps/desktop/lib/features/profiles/{profile_editor_dialog.dart,profile_fields.dart}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/RE-PROF-12-MON/**`、`compat` 台账（仅追加）。

禁止改变的已有行为：`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、其它 features、`crates/**`、`services/**`；不改 FIX-11/11B/11C 统计/Clash/日志语义，不改 RR-07/10 端点/预检语义；不删入口/降分母；不占用/修改 10808。

测试夹具和原版预期：纯合成 PEM（`AA==` 合法、`%%%`/`not base64 !!!` 非法）与合成域名/IP；原版预期 SNI 优先、链由 `ConcatenatePemChain` 拼接、SHA 用 DER body 的 SHA-256 小写 hex、无效块不更新 SHA。

本次必须通过的命令/真实场景（实际结果见证据）：
- `dart format --output=none --set-exit-if-changed lib test`（本卡改动文件）
- `flutter analyze`
- `flutter test test/re_prof_12_cert_test.dart test/rr_mon_rebind_test.dart test/fix11_monitor_session_test.dart`（隔离运行）
- `flutter test test/fix02_tuic_editor_test.dart --plain-name "<单用例>"`（该文件整体存在已知 Flutter 原生偶发不完成，单用例通过）

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-12-MON/README.md`。

完成条件：SNI→transportHost→address 选择可测；非法 base64/PEM 不抛、不更新 SHA；「获取证书链」如实报告 leaf-only；上述命令通过。完整链与真实 TLS 握手未做，保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（blocked）：`dart:io` `SecureSocket.connect` 只有单一 host，无法「连 IP 同时发另一 SNI」。当前以选定 serverName 作为连接名以保证使用正确证书；与 .NET `SslStream.TargetHost` 的精确分离行为仍需平台层补足，登记为 blocked，不伪造完整链。
- 接口缺口（登记）：`GetCertChainPemAsync` 的完整链在 `dart:io` 不可得；UI 已显式提示叶子。

本轮实际结果：`profile_fields.dart` 新增 `selectCertFetchServerName`、`PeerCertChainResult`/`peerCertChainResult`，`fetchPeerCertPem` 以 serverName 为握手/连接名，`fetchPeerCertChainPem` 返回 leaf-only 结果，`certSha256Thumbprint` 捕获非法 base64；`profile_editor_dialog.dart` 的 `_fetchCert` 按上游顺序选择 serverName 并对「获取证书链」显示叶子提示。Dart 单元（RE-PROF-12 + RR-MON-REBIND + FIX-11）12/12 通过；`flutter analyze` 仅报与本卡无关的既有 `tray_menu_model.dart` unused import。
