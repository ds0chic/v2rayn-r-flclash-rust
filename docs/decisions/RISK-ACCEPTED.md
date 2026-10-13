# 风险接受台账（RISK-ACCEPTED）

本文记录经评审后**接受并保留**的已知依赖风险（cargo-audit advisory）。
每条含影响面论证与跟随条件。功能台账（`compat/`）不记录此类条目。

## RA-01 — RUSTSEC-2023-0071（rsa / Marvin，5.9 medium）【接受】

- 登记：2026-10-13，B17-TOOLS-DEPS（`findings/AUDIT-DEPS.md` V7）。
- 现状：`rsa 0.9.10` 经 `pgp = "=0.16.0"`（精确钉死）引入 `crates/updater`，
  用于 `crates/updater/src/signature.rs` 的 detached 签名**验证**
  （`verify(artifact, signature)`，公钥为捆绑的上游发布公钥
  `fixtures/keys/v2rayn-public-key.asc`，属公开信息）。
- 影响面论证：Marvin 攻击需要 RSA **解密** oracle + 攻击者高精度观测解密
  时序。本路径只做签名验证（公钥运算），无解密、无私钥、无 oracle，
  时序侧信道无载体；威胁不适用。
- 无修复版本（advisory 自述）；`pgp` 被精确钉死，动它需全量重验
  T21 签名证据链，成本远超收益。
- 跟随条件：rpgp 发布去 rsa 版本，或 RUSTSEC 更新修复版本 → 重做本条，
  升级 `pgp` 并重跑 T21 签名证据。

## RA-02 — RUSTSEC-2024-0449（unsafe-libyaml unmaintained）【保留 + 已加固】

- 登记：2026-10-13，B17-TOOLS-DEPS（`findings/AUDIT-DEPS.md` W2）。
- 现状：`unsafe-libyaml 0.2.11` 经 `serde_yaml 0.9.34+deprecated` 引入，
  `crates/application` 的 `src/mixin.rs` / `src/custom.rs` 用其解析远端
  订阅与用户 YAML（不可信输入）。
- 已做加固（替代替换）：`mixin.rs` 的 YAML 输入预算守卫
  （1 MiB 字节 + 64 锚点 / 1024 别名预检，64 深度 / 100k 节点后检；
  `*.example.com` 类路由规则不计入别名），`custom.rs` 同 caps；
  恶意 YAML（超大/别名 spam/深嵌套）单测锁定 fail-closed，
  merge-key 锚点正向用例锁定不过度拦截。
- 未替换理由：候选 fork `serde_yaml_ng 0.9.36` 经闭包验证
  （隔离试算，未提交）仍依赖同一 `unsafe-libyaml 0.2.11`，无实质收益；
  纯 Rust 实现（`serde_yml` 系）非 drop-in，会漂移 codegen golden，
  已排除（见 `findings/PLAN-B17.md` §2）。
- 跟随条件：出现维护中且 API 兼容、无 C 解析器的 YAML 后端，或
  `serde_yaml` 上游恢复维护 → 重做替换；届时 golden 全绿为门槛。
