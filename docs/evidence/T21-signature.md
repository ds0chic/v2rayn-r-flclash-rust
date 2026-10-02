# T21-B 证据 — 更新签名验证（真实资产实签）

- 目标：让"签名验证"从 `Unsupported` 变成真实可执行的信任链，并完成一次**真实上游资产的验签实跑**。
- 结论：**已实现并真实验证通过**（上游 7.25.4 资产 PGP 验签 GOODSIG；篡改拒绝；Xray `.dgst` 正反例通过）。

## 1. 信任根事实

- 上游 `2dust/v2rayN` 的 `v2rayN-public-key.asc`（547B）是 **OpenPGP v5（LibrePGP）EdDSA 公钥**（首包 `98 49 05 16`：old-format tag 6、版本 5、算法 22 EdDSA）。
- 发布资产 `.sig` 为 ASCII-armored PGP 分离签名（297B，`-----BEGIN PGP SIGNATURE-----`）。
- 指纹（GnuPG 2.5.24 计算）：`76945E9F3E9A168F8070F195805D661C134DFAF68903C199463C31E5AE903AE0`。
- **纯 Rust 后端均不支持 v5**：rpgp 0.16 源码 `packet/public_key_parser.rs:83-88` 对 `KeyVersion::V5` 直接 `unsupported_err!`；Sequoia 2.4.1 报 `Unsupported primary key: Malformed packet: unknown version`（本机实测）。

## 2. 实现（crates/updater）

- `PgpDetachedVerifier`（rpgp，v4/v6 路径）保留；新增 `GpgCliVerifier`（`openpgp-v5-gpg-cli`）：
  - 隔离临时 GNUPGHOME 导入内置公钥 → `--status-fd 1 --verify` → 要求 `GOODSIG` 且 `VALIDSIG` 指纹与内置公钥指纹一致；`BADSIG/NO_PUBKEY/ERRSIG/超时` 均映射为结构化失败；30s 超时；临时目录用后删除。
  - GnuPG 定位：`V2RAYN_R_GPG` → PATH → `C:\Program Files\GnuPG\bin\gpg.exe` 等；缺失时 fail-closed（`SignatureUnsupported`）。
- `v2rayn_app_verifier()`：优先 rpgp；解析失败（v5）→ GpgCliVerifier；两者都不行 → `SignatureUnsupported`，绝不静默通过。
- 接线点：`updater::verify_app_release_asset(asset, sig)` 供应用自更新在下载后、外部替换前调用；核心更新走 `.dgst`/GitHub digest（既有）。
- `.dgst`：`updater::dgst` 解析 `SHA2-256` 并校验（正/负例均有测试）。

## 3. 真实资产实跑（决定性证据）

环境：GnuPG 2.5.24（winget 安装）；资产下载经本机既有代理；资产与日志不入库（只存哈希）。

| 步骤 | 命令/方法 | 结果 |
|---|---|---|
| 真值验证（GnuPG） | 隔离 homedir 导入公钥后 `gpg --status-fd 1 --verify` | `GOODSIG 76945E9F3E9A168F 2dust`；`VALIDSIG 76945E9F…3AE0`（v5 EdDSA）；`Good signature` |
| 我们的实现 | `cargo run -p updater --example t21_verify_release -- --asset <deb> --sig <sig>` | `scheme: openpgp-v5-gpg-cli`；**RESULT: PASS** |
| 篡改负例 | 同上 `--tamper`（内存翻转 1 字节） | **RESULT: FAIL（tampered asset rejected）** |
| dgst 正例 | `--example t21_verify_dgst`（本机 Xray v26.3.27 zip + .dgst） | PASS，`d004c392…1ad` 匹配 |
| dgst 负例 | 同上 `--tamper-expected` | FAIL（篡改摘要被拒绝） |

资产：`v2rayN-linux-loong64.deb` 73,729,064B，SHA-256 `f0a4fa06…d532`；签名 297B，SHA-256 见 `T21-signature.runs/manifest.json`。日志原文在该目录（`gpg-groundtruth.log`、`our-verifier-pass.log`、`our-verifier-tamper.log`、`dgst-ok.log`、`dgst-tamper.log`）。

## 4. 测试与门禁

- `cargo test -p updater --locked`：全绿（含 v4 rpgp 正反例、`bundled_upstream_key_selects_a_real_backend_or_fails_closed`、错误钥负例、dgst 正反例；v5 路径在无 GnuPG 机器上会 fail-closed 并跳过负例断言）。
- `cargo fmt/clippy -p updater --all-targets --locked -- -D warnings`：通过。

## 5. 未决项（诚实登记）

1. **v5 信任根依赖 GnuPG CLI**（本机已装；无 GnuPG 的机器上应用自更新验签为 fail-closed 不可用）。若要求零外部依赖，需要自实现 LibrePGP v5 验签或等待纯 Rust 后端支持。
2. 真实 GitHub 端点仅验证到"资产下载 + 验签"；`update_service` 的应用更新目前只产出外部升级 spec，尚未把 `verify_app_release_asset` 串进完整 GUI 自动更新流程（属发布前 B1/B2 项）。
3. 密钥轮换/多签名/离线信任根导入策略未定义。
4. GnuPG 安装与隔离 homedir 使用会在系统上创建临时目录（已清理）；用户主目录误用问题已修复并清理（见 T21-signature.runs 注记）。
