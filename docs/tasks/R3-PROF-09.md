# R3-PROF-09 — 证书获取连接节点 Address 并单独设置 SNI

状态：`implemented`（本地 loopback ClientHello 夹具断言 SNI=文档域名且不解析该域名；未运行真实节点证书、完整链仍 `blocked`）。

任务 ID：R3-PROF-09

本次唯一用户流程：节点 `Address` 为 IP、SNI 为另一域名时，点「获取证书」连接 `Address:Port`，把选定名称作为 TLS SNI 发送，而不是去解析/连接 SNI 的 DNS 地址。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核 `docs/evidence/parity-recheck-2026-10-04/round3-profiles.md` RE-PROF-12 / R3-PROF-09。

上游对照：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/Handler/CertPemManager.cs:40,48,89,97`（连接 `Address:Port`，单独设置 TLS `TargetHost`）；`ServiceLib/ViewModels/AddServerViewModel.cs`（SNI → transport host → address 选择）。

对应 feature / field / action：`RE-PROF-12`、`F-PROFILE-*`（获取证书）、`FLD-ENT-*`（SNI/Host）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`Address`（连接目标）、`port`、SNI/transport host。
- 输出：叶子证书 PEM（链仍 leafOnly）。
- 错误：无法连接返回 null（UI 提示未获取）。
- 权限：`None`；只读网络握手。
- 持久化：无（结果由调用方写入草稿）。

允许修改的模块：`apps/desktop/lib/features/profiles/{profile_fields.dart,profile_editor_dialog.dart}`、`apps/desktop/test/**`、本卡、证据目录、compat 台账（仅追加）。

禁止改变的已有行为：`selectCertFetchServerName` 的 SNI→host→address 顺序；leafOnly 诚信提示；非法 base64 保护。

测试夹具与原版预期：`apps/desktop/test/r3_prof_09_cert_sni_test.dart` 在 127.0.0.1 上从 11808 起探测空闲端口，起普通 listener 捕获 ClientHello；`fetchPeerCertPem(host:'127.0.0.1', serverName:'www.example.test')` 的 ClientHello 必须包含该文档域名。原版预期：连接地址与 SNI 分离。

本次必须通过的命令/真实场景：
- `flutter analyze`
- `flutter test test/r3_prof_09_cert_sni_test.dart`
- `flutter test test/ux_space03_vless_editor_test.dart`（如触及编辑器；未在本轮改动编辑器）

证据文件位置：`docs/evidence/recheck-fixes/R3-MISC/README.md`。

完成条件：连接目标为节点 `Address:Port`，`SecureSocket.secure(socket, host: serverName)` 设 SNI，且不依赖域名 DNS；门禁通过。

接口缺口（登记）：`dart:io` 只暴露叶子证书，完整链仍需额外 provider（保持 `blocked`）；本轮未改动链能力。

本轮实际结果：`profile_fields.dart` 的 `fetchPeerCertPem` 改为 `Socket.connect(Address)` 后 `SecureSocket.secure(raw, host: SNI)`；合成 ClientHello 夹具断言 SNI 为文档域名。1 新用例通过。
