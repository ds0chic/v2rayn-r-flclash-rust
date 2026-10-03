# FIX-02 — TUIC 独立 UUID/密码字段与编辑器默认/联动/无损修复

状态：`implemented`（TUIC 新增→保存→重开→字段 与 仅改备注无损 已在本机真实 Windows 窗口 + FRB/Rust/SQLite 验证；未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-02

本次唯一用户流程：`配置项 → 添加 [TUIC]`，填写独立的 **用户 ID (id)**（UUID）与 **密码 (password)**，保存后在节点表重开编辑，两字段各自保留；同一 TUIC 节点仅改备注保存时，security/transport 值不被重置。其它协议各自另卡。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `1251cbc6821276e35d082f7739b8b9c15b6f93dd`。审查结论见 `docs/evidence/parity-review-2026-10-03/README.md`、`repair-queue.md` FIX-02 行、`profiles-report.md` PR-03/04/05/26/27、`all-items.json` 的 FLD-ENT-012/013/018/022/023/024/028/029/030/047、ACT-ADDSRV-001。复现证据：`ui-run-04` 记录 TUIC 表单 `usernameFieldCount=0`、`passwordFieldCount=1`，唯一密码字段标签写 UUID。

对应 feature / field / action / layout ID：`CFG-008`(EConfigType.TUIC)、`ACT-MAIN-008`(添加 TUIC)、`ACT-ADDSRV-001`、`FLD-ENT-013`(Username)、`FLD-ENT-012`(Password)、`FLD-ENT-018`(StreamSecurity)、`FLD-ENT-022`(Fingerprint)、`FLD-ENT-047`(VlessEncryption)、`FLD-ENT-029/030`(Cert/CertSha)。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的
- `v2rayN/v2rayN/Views/AddServerWindow.xaml:680`（gridTuic：`txtId8`=用户 ID、`txtSecurity8`=密码、`cmbCongestionControl8`）与 `:1446/1497`（cmbStreamSecurity/cmbFingerprint IsEditable）。
- `v2rayN/v2rayN/Views/AddServerWindow.xaml.cs:114-118`（TUIC 绑定 Username←txtId8、Password←txtSecurity8）、`:307-325`（StreamSecurity 联动 gridRealityMore/gridTlsMore）、`:327-331`（btnGUID 生成 UUID）。
- `v2rayN/ServiceLib/ViewModels/AddServerViewModel.cs:339-458`（保存校验/归一：备注/地址/端口、非 SOCKS/HTTP 需 Password、Cert→CertSha、reality 默认指纹）、`:467-491`（UpdateCertSha，ParsePemChain + GetCertSha256Thumbprint）、`:493-544`（FetchCert / FetchCertChain）。
- `v2rayN/ServiceLib/Handler/ConfigHandler.cs:847-881`（AddTuicServer：CoreType=sing_box、Trim、Network 清空、Fingerprint 清空、CongestionControl 归一、StreamSecurity 空→tls、Alpn 空→h3、Password 空→-1）、`:1201-1248`（AddServerCommon：StreamSecurity 非 tls/reality 清空、reality 空指纹补默认）。
- `v2rayN/ServiceLib/Global.cs`（StreamSecurity="tls"、Fingerprints 含 randomized/空、TuicCongestionControls、SsSecuritiesInXray/Singbox、RunDef 默认）。
- 当前实现定位：`apps/desktop/lib/features/profiles/profile_fields.dart`、`profile_editor_dialog.dart`、`crates/config_codegen/src/singbox/outbound.rs:487-495`（uuid<-username、password<-password，已正确，仅补断言）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：普通编辑器草稿；TUIC 输入 `username`（UUID）与 `password`（认证密码）两个独立字段。
- 输出：`ProfileDraft.toDto()` → `ProfileDto.username/password` → `Engine.save_profile` → SQLite `ProfileItem.username/password`；sing-box 生成取 `uuid=username`、`password=password`。
- 默认：TUIC 核心固定 `singBox`；新建/空值时 `streamSecurity=tls`（改非空值不动）。
- 校验：UUID 字段非空且需为 8-4-4-4-12 十六进制；密码非空；端口 1-65535。
- 错误：字段级提示（必填 / UUID 格式无效 / 端口范围）；服务端错误仍显示在横幅。
- 取消：关闭编辑器丢弃本地草稿，不改持久化。
- 权限：仅本机 UI + FRB/Rust/SQLite；取证书走 `dart:io` TLS 握手，不写系统代理/TUN、不监听端口。
- 持久化：SQLite；保存→重开一致。
- 生效：保存后刷新节点表。

联动/无损/兜底修复：
- PR-04：`streamSecurity` 下拉 onChanged 现调用 `setState`，选择 `reality` 后公钥/ShortId/SpiderX/ML-DSA 字段立即出现，无需重开。
- PR-26：Fingerprint 改为可编辑 combo（候选含 `randomized` 与空值）；VLESS `vlessEncryption` 改为可编辑文本框；SS 方法按核（Xray / sing-box）给出支持列表；下拉初值不在候选时自动追加 `<value> (未知候选)` 项，避免 Debug 断言且保留可编辑。
- 无损：仅改备注保存时 security/transport 原值原样回写（不重置）；导入已有值打开编辑不 assert。

辅助动作（PR-05）：
- 恢复「生成」UUID 按钮（VMess/VLESS 的 password、TUIC 的 username）。
- Cert→CertSha 自动推导：纯 Dart 解析 PEM 链并计算 SHA-256（大小写不敏感、去空白，支持单行/多行 PEM）。
- 获取证书 / 获取证书链：`dart:io` `SecureSocket` 取对端叶子证书转 PEM。`dart:io` 只暴露叶子证书，故「证书链」按钮与「证书」等价（登记遗留，未擅自伪造链）。

允许修改的模块：`apps/desktop/lib/features/profiles/{profile_fields.dart,profile_editor_dialog.dart}`（`profile_draft.dart` 未改，username/password 字段本就存在）、`apps/desktop/test/**`、`apps/desktop/integration_test/**`、`docs/evidence/UX-PARITY-FIX-02/**`、本卡、`compat/features.yaml` 与 `compat/actions.yaml`（仅追加 notes/status）。Rust 仅在 `crates/config_codegen/tests/singbox_protocols.rs` 追加回归断言，未改生成逻辑（映射本已正确）。未改 menu/布局/main_shell/profiles_table/profiles_page；未跑 FRB 生成。

禁止改变的已有行为：已有菜单结构/坐标/关闭、17 根条目/分隔/高度、中文列头、编辑器间距契约；不删协议可选值或降分母。

测试夹具和原版预期：合成 TUIC 节点（`192.0.2.77:443`、合成 UUID、不连接）；合成 PEM。原版预期：Username/Password 独立、保存映射 username/password、空 TLS 默认 tls、Reality 立即联动、未知候选保留、仅改备注无损。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test integration_test`
- `flutter analyze`
- `flutter test`（逐文件，`tools/flutter_test_retry.ps1 -PerFile`）
- `flutter build windows --release`
- 真实窗口：`flutter test integration_test/ux_parity_fix02_test.dart -d windows`（`V2RAYN_R_DATA_DIR` 隔离目录、`V2RAYN_R_FIX02_EVIDENCE`、`V2RAYN_R_FIX02_IMAGES=1`、`AUTOSTART=0/AUTO_SMOKE=0`）
- Rust 断言：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`

证据文件位置：`docs/evidence/UX-PARITY-FIX-02/`（`observations.json`、`01-tuic-fields.png`、`02-tuic-saved.png`、`03-tuic-reopen.png`、`04-tuic-remarks-only.png`）。

完成条件：TUIC 两字段独立且保存/重开各自保留；sing-box 生成 uuid<-username、password<-password 有断言；Reality 立即联动；未知候选兜底；仅改备注不重置；UUID 生成与 Cert→CertSha 可用；门禁通过。未做原版实机双窗口逐事件对照，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`fetchPeerCertChainPem` 受 `dart:io` 限制只能返回叶子证书，等价于「获取证书」；完整链需要 bridge/Rust 侧的 TLS 证书链能力或 `file_selector` 依赖，未在未获批准的 pubspec 变更下强造。
- 接口缺口（登记）：上游 `AddTuicServer` 在 Alpn 空时补 `h3`，当前只在编辑器补 `StreamSecurity=tls`，未补 `Alpn=h3`；后续应由后端 `save_profile` 的逐协议归一（PR-27）统一补齐，避免只在 UI 兜底。
- 接口缺口（登记）：`Engine.save_profile` 对普通协议仍只做 `Profile.validate`（ID/备注/地址/端口），未按 `ConfigHandler.Add*Server` 做逐协议归一（默认 TLS、强制核、HTTP Headers/Realm 等，PR-27）；本卡只在 TUIC 编辑器侧处理默认，属最小范围，完整归一登记为后续工作。

本轮实际结果：`profile_fields.dart` 拆出 TUIC `username`/`password` 两字段、Fingerprint 改 combo（+randomized/空）、VLESS encryption 改文本框、SS 方法按核给列表、新增 `uuidSetter`/`uuidFieldKey`/`generateUuidV4`/`parsePemChain`/`certSha256Thumbprint`/`certShaFromChain`/`fetchPeerCertPem`；`profile_editor_dialog.dart` 下拉 onChanged 统一 `setState`（Reality 联动）、未知候选兜底、combo 控件、UUID 生成按钮、取证书菜单、Cert→CertSha 自动推导、TUIC 空 TLS 默认 tls、UUID 格式校验、`row-<key>` 供间距断言。Rust 追加 `singbox_tuic_uuid_username_password_split`（通过）。widget 测试 `fix02_tuic_editor_test.dart` 11/11；`ux_space03_vless_editor_test.dart` 5/5（间距断言改用强制 field 行测量）。真实窗口 observation 4/4、`recordingComplete=true`、`failures=[]`。门禁：format 0 改变、analyze No issues、release 构建成功、逐文件测试全绿（个别文件因已知 flutter_tester 0xC0000005 崩溃重试后通过）、Rust fmt/clippy/workspace test 通过（`cargo test --workspace --locked` 0 failed）。
