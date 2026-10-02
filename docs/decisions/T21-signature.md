# T21-B 决策 — 更新签名信任模型

1. **信任根**：内置上游 `v2rayN-public-key.asc`（OpenPGP v5 EdDSA）。验签必须同时满足 `GOODSIG` 与 `VALIDSIG` 指纹等于内置公钥指纹；不接受"未知签名者但签名有效"。
2. **后端选择**：优先纯 Rust rpgp（v4/v6）；上游为 v5（LibrePGP），rpgp 与 Sequoia 当前均不解析 v5（实测证据见 T21-signature.md §1），因此 v5 走 GnuPG CLI 隔离 homedir 后端。两者都不可用时 **fail-closed**（`SignatureUnsupported`），绝不静默通过。
3. **进程边界**：GnuPG 只以子进程方式用于验签；固定超时 30s；临时 homedir 用后删除；不读取用户默认 keyring；`V2RAYN_R_GPG` 仅供测试/企业环境指定路径。
4. **核心资产**：优先 `<asset>.dgst` 的 SHA2-256（Xray 等），GitHub `digest` 作为后备；两者都缺失时该核心的自动更新视为未验证（不落盘）。
5. **应用自更新**：下载后必须先 `verify_app_release_asset` 再生成外部升级 spec；未验签的资产不得进入替换流程。
6. **已知边界**：无 GnuPG 的机器上应用自更新不可用（可改用核心更新）；发布前需在目标环境明确 GnuPG 依赖或补纯 Rust v5 实现。
