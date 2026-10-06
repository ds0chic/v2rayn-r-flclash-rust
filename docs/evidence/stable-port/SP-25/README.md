# SP-25 应用 HTTPS 信任消费者 — 证据索引

状态：implemented（机制实现；live TLS 矩阵待运行，不写 verified）。
基线：`788adf6`（开工时 `git rev-parse HEAD` 一致；工作树另有并行卡未提交改动，本卡未碰）。
不 commit。

## 结论

- `RootCertProvider` 选择已接到全部三类应用 HTTPS 客户端：订阅
  (`subscriptions`)、更新 (`updater` fetch + artifact + dgst + signature) 与
  WebDAV (`application::webdav`)，经 `UpdateService::tls_trust` 透传。
- `system` = OS/native roots；`chrome`/`mozilla` = 仅 bundled PEM（`tls_built_in_root_certs(false)` +
  `add_root_certificate`），不装 OS 证书、不读环境代理、不改既有调用签名。
- 离线合同全绿：`cargo test -p subscriptions -p updater --locked` 共 313 passed / 0 failed；
  `cargo clippy -p subscriptions -p updater --all-targets --locked -- -D warnings` exit 0；
  12 个本卡文件 `rustfmt --check` exit 0。
- live TLS 矩阵（`crates/application/tests/sp25_root_cert_trust.rs`，16 tests）已写完，
  **未运行**：`application` 包当前无法编译，外卡 SP-13 在
  `crates/application/src/dns.rs` 留下未闭合 `mod tests`（`error: unexpected closing delimiter`），
  非本卡写锁范围，未碰。待其修复后运行。

## 文件

- `SP-25-trust-matrix.md`：实例证据（ID、行为矩阵、命令与 exit、未验证项）。
- `commands.log`：真实命令输出摘要。

## 关联 ID（本次实例）

FLD-CFG-064（RootCertProvider；消费者归属 SP-25）。R4-13 / R4-29 为来源任务；
Geo/SRS/路由模板等其余字段归各自子卡，不在本卡标 verified。
