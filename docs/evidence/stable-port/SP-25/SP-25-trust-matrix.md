# SP-25.FLD-CFG-064 实例证据 — 信任来源逐 client 生效

本次唯一用户流程：用户切应用 RootCertProvider 后，订阅/更新/WebDAV HTTPS 按选择建立信任。

## 改动文件（本卡）

- `crates/subscriptions/src/tls.rs`（新）：`HttpsTrust::{System, BundledPem}`、
  `from_provider`（未知回退 system，与上游 `ConfigHandler.LoadConfig` 一致）、
  `apply_trust`（bundled 独占：`tls_built_in_root_certs(false)` + bundle）、
  `trust_failure_of`（source 链证书错误分类，输出无 URL/秘密）。
- `crates/subscriptions/src/download.rs`：新增 `build_client_with_trust`；
  原 `build_client` 委托之（System，行为不变）；`map_reqwest_error` 对信任拒绝
  返回 `Http("tls trust rejected: …")`。
- `crates/subscriptions/src/lib.rs`：`pub mod tls` + 导出。
- `crates/updater/src/tls.rs`（新）：同上合同 + `classify_request`。
- `crates/updater/src/fetch.rs`：`CoreReleaseApi::{new_with_tls, trust}`；
  `new`/`new_with_proxy` 委托（System，行为不变）；fetch 错误经 `classify_request`。
- `crates/updater/src/download.rs`：`FileDownloader::new_with_trust`；
  `new` 委托（System）；`classify` 经 `classify_request`。
- `crates/updater/src/lib.rs`：`pub mod tls` + 导出。
- `crates/application/src/webdav.rs`：`WebDavClient::new_with_tls`；
  `new` 委托（System）；`network_error` 对信任拒绝返回
  `UNAVAILABLE / error.webdav_tls`（新 key，UI 映射待补）。
- `crates/application/src/update_service.rs`：`UpdateService::tls_trust` 字段 +
  `with_tls_trust`；metadata fetch、artifact 下载、dgst（同 downloader）与
  detached signature 下载全部走该选择。
- 本卡测试：`crates/subscriptions/tests/tls_trust.rs`（5）、
  `crates/updater/tests/tls_trust.rs`（4）、
  `crates/application/tests/sp25_root_cert_trust.rs`（16，待运行）。
- 未改：workspace `Cargo.toml`/`Cargo.lock`（无新依赖）、engine.rs、各 `lib.rs`
  除上述两 crate 内部 `mod` 行、IPC/DTO/BridgePort/FRB、config_codegen。
- `services/upgrade_runner/**`：无改动。核查 `src` 内无 reqwest/HTTP 客户端
  （仅文件内原子替换 + 重启），无信任消费者，记 not_applicable。

## 先红后绿

- 红：先写三份测试再实现。`cargo test -p subscriptions -p updater --locked
  --test tls_trust` 报 `E0432 unresolved import subscriptions::tls /
  build_client_with_trust`（exit 非 0），合同缺失得证。
- 绿：实现后同命令 9/9 通过；全包 `cargo test -p subscriptions -p updater
  --locked` 313 passed / 0 failed（见 commands.log）。

## 行为矩阵（离线已证 / live 待跑）

| # | client | 选择 | 合成 CA 服务器 | 预期 | 状态 |
|---|---|---|---|---|---|
| 1 | 订阅下载 | mozilla bundle | 接受 | 200 + 正文 | 待运行（application 阻塞） |
| 2 | 订阅下载 | system | 拒绝 | `Http(tls trust rejected…)` + 分类器命中 | 待运行 |
| 3 | 订阅下载 | system→mozilla→system | 同 URL 三次 | 拒→受→拒 | 待运行 |
| 4 | updater fetch | bundle / system | 同上 | 成功解析 1 条 release / `Download(tls…)` | 待运行 |
| 5 | updater 文件下载 | bundle / system | 同上 | 48 KiB 落盘 / `Download(tls…)` | 待运行 |
| 6 | WebDAV | bundle / system | 同上 | check/upload/download/list / `error.webdav_tls` | 待运行 |
| 7 | UpdateService | bundle / system | 同上 | `remote_version=26.4.0` / 拒绝可分类 | 待运行 |
| 8 | 错 host（127.0.0.2，不在 SAN） | bundle | 拒绝 | 信任失败分类 | 待运行 |
| 9 | 空/垃圾 bundle | 建 client | — | 构建期 Err，不静默回退 system | **已绿**（离线） |
| 10 | provider 拼写/未知 | — | — | chrome/mozilla 取对应 PEM；未知/空回 system | **已绿**（离线） |
| 11 | 取消 | 订阅/updater（token，/slow 3s） | — | `Cancelled`，无残留完整文件 | 待运行 |
| 12 | WebDAV 截止 | blackhole + 500ms | — | `error.webdav_timeout` | 待运行 |
| 13 | 重开 | 重建 client | 接受 | 无全局/OS 状态，两次一致 | 待运行 |

live 服务器：rustls 阻塞式 TLS（复用 `fixtures/tls/`：CA `59E0A961…`、
leaf `419A5EC8…`，SAN DNS:localhost + IP:127.0.0.1），端口 `:0` 实绑后断言
`>=11808 且 !=10808`；无真实互联网、无 OS 证书写入、无用户秘密（全合成）。

## 如实未验证 / 阻塞（不标绿）

1. 上表“待运行”项：`application` 包被外卡 SP-13 改动
   （`crates/application/src/dns.rs` 多一个提前闭合 `mod tests` 的 `}`）阻塞编译，
   `cargo test -p application` / clippy 均止于 `unexpected closing delimiter`
  （见 commands.log）。本卡未碰该文件，待其修复后跑 `cargo test -p application
   --locked --test sp25_root_cert_trust` 及相关 `t16_webdav`/`t16_update` 回归。
2. 过期 leaf 负向：fixture 树无过期材料，未验证（需新 PKI 材料，跟进卡）。
3. Bundled 独占性的反向证明（system 信任的服务器在 Bundled 下被拒）：
   需向 OS 写证书，禁止，未验证；独占性以代码（built-in roots 关）+ 审查为准。
4. 生产选择流：`subs.rs`/`routing.rs`/`bridge_api t16.rs` 仍用 System 默认构造，
   设置值→client 的整线接线待 SP-00 整合者（文件在其写锁外，本卡只做到
   API 级机制 + `UpdateService` 透传）。
5. `error.webdav_tls` 的 Flutter 文案映射待补（Dart 不在本卡锁内）。
6. WebDAV 协同取消：方法无 `CancellationToken` 形参，仅 deadline；接口缺口已登记，
   待整合者排期。
7. 共享 `net_http` 低层模块：新建将触 workspace `Cargo.toml`（主控接线），
   本卡暂以两 crate 内同构 `tls.rs` 实现（注释互指），去重要待 SP-00。
