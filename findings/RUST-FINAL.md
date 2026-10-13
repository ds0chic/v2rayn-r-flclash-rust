# RUST-FINAL（Rust 最终全量门禁，tip `04b7618`）

- 基点：`audit/2026-10-13`（`04b7618`，R-LEFT + D-LEFT 已合入）；worktree `worktrees\R-VERIFY`，分支 `verify/rust-final`
- 性质：纯验证，产品代码零改动（`git status` 全程干净，除本报告文件）

## 1. 门禁表（原文命令）

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过（0 warning） |
| `cargo check --workspace --locked` | 通过 |
| `cargo test --workspace --locked --no-fail-fast` | **2070 passed / 0 failed / 5 ignored**（147 个 test-target 结果行全 ok，exit 0） |
| `cargo audit`（cargo-audit 0.22.2，DB 1310 条，401 crate） | 1 漏洞 + 1 警告，均为已知接受项（§3），无新增 advisory |

## 2. 总数与基线对比

- 基线 `f0d74c9`：2069 pass / 0 fail / 5 ignored。本轮：**2070 / 0 / 5**。
- Delta +1 pass = R-LEFT 新增的 soak 测试（`67adf24` `quiesce_reopen_keeps_store_views_consistent`，`f0d74c9..04b7618` 间唯一 Rust 测试增量；D-LEFT 为纯 Dart 批次）。Ignored 5 项不变：`cert_store` 1、`real_windows` 3、`xray_smoke` 1。
- Real-core e2e（sp22/t10/t10-live/t11/t15b/t18b×8 等）本次**全部通过**（1/2/1/1/3/8），无需 skip 计数：历史缺核失败已由后续批次转为有二进制/优雅通过（R-LEFT 轮在 `d42f598` 同样全绿）。含混点特记：这不是本轮修的，是之前轮次的成果。
- 旁证：D-LEFT（`c9cbabd`）已补 recovery/sub-update/storage 中文文案，R-LEFT 移交事项关闭。

## 3. cargo audit

- `rsa 0.9.10` RUSTSEC-2023-0071（medium，无修复版）= **RA-01 已接受**（`docs/decisions/RISK-ACCEPTED.md`）。
- `unsafe-libyaml 0.2.11` unmaintained RUSTSEC-2024-0449 = **RA-02 保留+加固**（同上）。
- 相对 `AUDIT-DEPS.md` 基线（9 漏洞 + 2 警告）：B17 已消灭 8+1，**无新增**。退出码 1 系"有漏洞即 1"，符合预期。

## 4. 结论

Rust 侧最终状态绿：fmt/clippy/check/test/audit 全部符合预期，无失败、无需修、无新增风险。本报告为本分支唯一新增文件；`target/` 已删；未 push。
