# RE-PROF-07 — 普通协议后端归一 + Mux/Finalmask/适用 UOT 控件

状态：`implemented`（Rust 单 crate 逐协议归一/校验/默认/Trim/清字段与 Dart widget 字段可见性/保存重开/取消均已通过；未跑原版实机双窗口、未跑真实窗口链路，故不写 `verified`）。

任务 ID：RE-PROF-07

本次唯一用户流程：对普通协议节点（TUIC/Trojan/Hysteria2/AnyTLS/Naive/VMess/VLESS/Shadowsocks）走编辑器保存或导入/订阅入库时，落库字段按冻结 `ConfigHandler.Add*Server` + `AddServerCommon` 归一（Trim、清无效字段、默认值、强制内核），不依赖 UI 默认；编辑各适用协议时能设置 Mux、Finalmask 及 SS/Naive 的 UDP-over-TCP 控件，按上游适用协议显示，保存/重开一致，取消无副作用。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；复核结论 `docs/evidence/parity-recheck-2026-10-04/profiles.md` 的 RE-PROF-07（状态 `identified`）；上游定位 `ServiceLib/Handler/ConfigHandler.cs` 的 `AddVMessServer:308`、`AddShadowsocksServer:669`、`AddSocksServer:702`、`AddHttpServer:721`、`AddTrojanServer:740`、`AddHysteria2Server:769`、`AddTuicServer:847`、`AddWireguardServer:891`、`AddAnytlsServer:943`、`AddNaiveServer:971`、`AddVlessServer:1122`、`AddServerCommon:1201`；控件绑定 `v2rayN/Views/AddServerWindow.xaml.cs:57-141/190/204-290` 与 `AddServerWindow.xaml:226/286/454/514/994/1386`。开始 HEAD `256c3dd`；本卡只读 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**` 及禁止清单内文件。

对应 feature / field / action / layout ID：`FLD-ENT-028`(MuxEnabled)、`FLD-ENT-033`(Finalmask)、`FLD-ENT-041`(Uot)、`FLG-ENT-005`(Subid)、以及 `compat/codegen-map.{xray,singbox}.yaml` 中 `ProfileItem.MuxEnabled` / `ProtocolExtraItem.Uot` / `ProfileItem.Finalmask` 合同。

必读上游文件、符号和固定 commit：
- `ServiceLib/Handler/ConfigHandler.cs`：逐类型 `Add*Server` 与共享 `AddServerCommon`（`Global.Networks`/`Global.StreamSecurity`/`Global.TuicCongestionControls`/`Global.VmessSecurities`/`Global.Flows` 归一）。
- `ServiceLib/Global.cs:50-63/330-338/575-583/600-621`：`DefaultNetwork=raw`、`StreamSecurity=tls`、`StreamSecurityReality=reality`、`Networks`、`TunMtus.First()=1280`、`TuicCongestionControls`。
- `v2rayN/Views/AddServerWindow.xaml.cs:57-141`：Mux（VMess(`togmuxEnabled`)/SS(`togmuxEnabled3`)/VLESS(`togmuxEnabled5`)/Trojan(`togmuxEnabled6`)）、UOT（SS `togUotEnabled3` / Naive `togUotEnabled12`）；`:190` Finalmask 绑定；`:252/272/280` Finalmask 对 TUIC/Anytls/Naive 折叠。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`save_profile` / `save_imported_profile` 收到的普通协议 `Profile`（编辑器草稿或导入/订阅 DTO）。归一只读 profile 自身，不读订阅地址/凭据秘密。
- 输出：编辑器保存返回归一后的 `Profile`；导入保存同样落归一后的 `Profile`。归一实现于 `crates/application/src/custom.rs::normalize_server`，两路径复用。
- 错误：编辑器保存路径 `validate_server` 明确返回 `E_FIELD_REQUIRED`/`E_FIELD_FORMAT`（如 VMess 缺 security、SS 方法非法、缺密码/密码全空白），带 `fieldPath`；不静默修正用户输入。
- 取消：编辑器草稿为本地副本，取消不调用 `onSave`、不落库。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不监听端口、不写系统代理/TUN、不碰 10808。
- 持久化：归一后的 `Profile` 经既有 `save_profile`/`save_imported_profile` 落 SQLite；重开一致。
- 生效：仅落库字段形状；不触发真实内核或网络。

允许修改的模块：`crates/application/src/{engine.rs,custom.rs}`、`crates/application/tests/re_prof_07_normalize.rs`（新增）、`apps/desktop/lib/features/profiles/{profile_fields.dart,profile_editor_dialog.dart}`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/RE-PROF-07/**`、`compat/features.yaml`/`compat/actions.yaml`（仅追加注释块）。未改 `main_shell.dart`、`app.dart`、`frb_generated.*`、`lib/bridge/api/**`、`profile_draft.dart`（字段已有）、`profiles_models.dart` 及其它禁止清单。

超出允许清单的最小改动（登记）：`crates/application/tests/profiles_persist.rs` 的 `save_reopen_round_trips_all_eleven_protocols` 原断言 "save_profile 原样回存" 与本任务新增的上游归一（TUIC/Hysteria2/AnyTLS/Naive 清 Network/Fingerprint/Alpn 等）直接冲突。改为断言 "重开等于归一后已保存的 Profile"，即持久化保真，不再假设保存无副作用；语义等价于原测试目的（reopen 存活），并覆盖新归一行为。

禁止改变的已有行为：FIX-02 已提交的 TUIC username/password 双字段、Reality 即时联动、无损编辑语义；FIX-04/PR-09 的宽容导入合同（空备注/地址/端口、空凭据节点仍落库并如实标记，不静默丢弃）；Custom/Outbound/PolicyGroup/ProxyChain 既有归一。

接口缺口（登记，未新增 FRB 符号）：无需新 FRB 字段/函数；`ProfileDto` 已带 `muxEnabled`/`finalmask`/`protoExtra.uot`，归一在既有保存路径内完成，未改生成桥接。

测试夹具和原版预期：合成节点（RFC 5737 地址 `192.0.2.x`、合成 UUID/密码、`127.0.0.1` 不连接）。原版预期：TUIC 强制 sing-box、Trim、清 Network/Fingerprint、拥塞默认 `cubic`、空 TLS→`tls`、空 ALPN→`h3`；Trojan/AnyTLS/Naive 空 TLS→`tls`；Hysteria2 清 Fingerprint/Alpn/Network；AnyTLS/Naive 强制 sing-box；VLESS 空 encryption→`none`、非法 flow→`""`；SS 按核候选、非法方法/空密码拒绝；`Network` 非 `Global.Networks` 归 `raw`、非法 `StreamSecurity` 清空。

本次必须通过的命令/真实场景（实际运行）：
- `cargo fmt -p application -- --check`：通过（0 退出）。
- `cargo clippy -p application --all-targets --locked -- -D warnings`：通过。
- `cargo test -p application --test re_prof_07_normalize --locked`：10 passed。
- `cargo test -p application --test profiles_persist --locked`：5 passed（含更新后的 reopen 保真回归）。
- `cargo test -p application --test t10_core_matrix --test t11_codegen_matrix --test t18b_runtime_plan --test fix04_inner_import --test fix10_speedtest_result --locked`：2+11+1+2+4 passed（回归）。
- `cargo test -p bridge_api --lib save_imported_profile --locked`：1 passed（宽容导入回归）。
- `dart format`（3 文件）：通过。
- `flutter analyze`：No issues found。
- `flutter test test/re_prof_07_fields_test.dart`：7 passed。
- `flutter test test/fix02_tuic_editor_test.dart test/ux_space03_vless_editor_test.dart test/t06a_editor_form_test.dart test/t06a_editor_save_test.dart test/t06a_editor_error_test.dart`：通过；`t06a_editor_cancel_test.dart` 因多文件同进程原生资源 flake 需单独重跑，单独运行 1 passed。

证据文件位置：`docs/evidence/recheck-fixes/RE-PROF-07/`（`observations.json`、`README.md`、`runs.txt`）。

完成条件：后端两路径逐协议归一 + 编辑器校验错误明确、Dart 控件按上游适用协议显示、保存重开/取消一致；FIX-02/04 回归通过；门禁通过。未跑真实窗口/原版实机，保持 `implemented`。

本轮实际结果：见 `docs/evidence/recheck-fixes/RE-PROF-07/README.md`。
