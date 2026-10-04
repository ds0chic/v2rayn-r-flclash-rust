# RE-PROF-07 — 普通协议后端归一 + Mux/Finalmask/适用 UOT 控件

状态：`implemented`。冻结上游 v2rayN 7.25.4 `7d6a967`；开始 HEAD `256c3dd`。只运行合成 Rust 单 crate 测试与 Dart widget 测试；没有启动内核、监听端口、改宿主代理/注册表/TUN，没有碰 10808，没有读写用户凭据。

## 结果

- **后端归一**：新增 `crates/application/src/custom.rs::normalize_server`，在 `save_profile`（编辑器）与 `save_imported_profile`（导入/订阅）两路径复用，逐协议对应冻结 `ConfigHandler.Add*Server` + `AddServerCommon`：
  - TUIC：强制 sing-box、Trim username/password、清 Network/Fingerprint、拥塞默认 `cubic`、空 TLS→`tls`、空 ALPN→`h3`。
  - Trojan/AnyTLS/Naive：空 TLS→`tls`；AnyTLS/Naive 强制 sing-box；Naive 清 Fingerprint/Alpn/Network/AllowInsecure。
  - Hysteria2：清 Fingerprint/Alpn/Network，Gecko 非法区间→512/1200。
  - VLESS：空 encryption→`none`、非法 flow→`""`；VMess/SS/VLESS Network/StreamSecurity Trim。
  - 共享尾：非法 `StreamSecurity` 清空、`Network` 不在 `Global.Networks`→`raw`、`configVersion=4`。
- **清空字段规范形**：被清空的 `Fingerprint`/`Alpn`/`AllowInsecure`/非法 `StreamSecurity` 用 `None` 表示，与 SQLite 空串→`None` 的读回一致，保证 `save_profile` 返回值与 `reopen` 结果相等。
- **编辑器校验**：`validate_server` 明确返回 `E_FIELD_FORMAT`/`E_FIELD_REQUIRED` 并带 `fieldPath`（VMess security 非候选、SS 方法非候选、缺密码），不静默修正。
- **导入边界**：保留 FIX-04/PR-09 宽容导入——只归一，不因空凭据丢弃节点（bridge 回归通过）。
- **控件**：VMess/VLESS/Trojan/SS 补 Mux；SS/Naive 补 UOT，移除 TUIC 的 UOT（上游无）；除 TUIC/AnyTLS/Naive 外补 Finalmask。

## 上游对照

| 协议 | 上游 `Add*Server` | 本仓库归一 |
|---|---|---|
| TUIC | `ConfigHandler.cs:847-881` | 一致（含 h3/cubic/sing-box） |
| Trojan | `:740-758` | 一致（空 TLS→tls, trim） |
| Hysteria2 | `:769-836` | 一致（清 fingerprint/alpn/network, gecko clamp） |
| AnyTLS | `:943-961` | 一致（sing-box, 空 TLS→tls, 清 network） |
| Naive | `:971-993` | 一致（sing-box, 清 4 字段） |
| VLESS | `:1122-1147` | 一致（none/flow 默认） |
| VMess | `:308-333` | 一致（trim + security 校验 + password） |
| SS | `:669-692` | 一致（按核方法表 + password） |
| 共享尾 | `:1201-1248` | 一致（Network/Security/configVersion） |

控件：Mux `AddServerWindow.xaml.cs:63/70/93/99`；UOT `:69/140`；Finalmask `:190/252/272/280`。

## 实际命令与结果

见 `runs.txt`。要点：Rust `re_prof_07_normalize` 10/10；`profiles_persist` 5/5（reopen 保真断言更新）；`t10_core_matrix`/`t11_codegen_matrix`/`t18b_runtime_plan`/`fix04`/`fix10` 回归通过；`bridge_api` 宽容导入 1/1；`cargo fmt`/`clippy -D warnings` 通过；`flutter analyze` 无问题；`re_prof_07_fields_test` 7/7；`fix02`/`ux_space03`/`t06a` 编辑器回归通过。

超出允许清单的最小改动：`crates/application/tests/profiles_persist.rs` 的 `save_reopen_round_trips_all_eleven_protocols` 原断言 "保存无损"，与本任务的上游归一冲突，改为断言 "重开等于归一后已保存的 Profile"（记录于 observations.json 与任务卡）。

## 未完成 / 未验证

- 未跑真实 Windows 窗口链路，未做原版实机双窗口逐事件对照。
- 未对归一结果做真实内核/配置生成应用验证（仅字段形状）。
- Config 依赖的 Reality 默认指纹（`CoreBasicItem.DefFingerprint`）未应用：Rust 保存路径不带 Config，已在 observations.json 登记。

## 安全边界

合成节点（RFC 5737 地址、合成 UUID/密码）；测试端口无监听；不启动内核、不改宿主网络/代理。`t06a_editor_cancel_test` 的多文件同进程加载 flake 属已知 `flutter_tester` 原生资源问题，单独重跑通过，未删断言。
