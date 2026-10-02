# 终审发现项整改复核报告 (Quick Follow-up Verification)

- **目标 HEAD**: `6e00512`
- **复核基准**: 只读审查，无重构建；比对 `FINAL.audit.muse.md` 与 `FINAL.audit.gemini.md` 发现项。
- **发布包抽查**: `dist/build-info.json` 记录 `git_commit=6091dd1112f0ac53fa7ecdf227bd730d0f4a1331` (`6091dd1`)，`git_dirty=false`；`dist/SHA256SUMS` 为 `8e01647b29cde0f26ad598498a7e4360ae5941a6f2d3918e482520b4d6046ec3`，经 `Get-FileHash` 实测与 `dist/v2rayN-R-1.0.0+1-windows-x64.zip` 完全一致。

---

## 逐项核验结果

1. **F-01 (clean-HEAD 重建)**: **resolved**
   - **文件证据**: `dist/build-info.json:9-12` (`git_commit=6091dd1...`, `git_dirty=false`)；`docs/evidence/T20.runs/rc-apply/build_official_rc.log` 记录干净树构建；`docs/evidence/T20.md:14-16,42-67,375-386` 详细登记整改闭环。

2. **F-02 / F-03 (实时 applied journal + core.log 归档)**: **resolved**
   - **文件证据**: 归档工件已完整落盘于 `docs/evidence/T20.runs/rc-apply/`：
     - `live/s-1790909660878-2_journal.json`: `stage=applied, pid=40404, port=11808, rev=2, config_sha256=3c168c4d...`。
     - `live/s-1790909660878-2_core.log`: 包含 Xray 26.3.27 真实启动日志 `Xray 26.3.27 started`。
     - 附带 `live/s-1790909660878-2_config.json`、`finalized/` 对照组、`rc-apply-probe.json` 及截图 `rc-apply-applied-running.png`；`docs/evidence/T20.md:388-405` 表述已自洽修正。

3. **F-04 (非武装负向测试 negative-unarmed.json)**: **resolved**
   - **文件证据**: `docs/evidence/T20.runs/rc-apply/negative-unarmed.json` 真实运行 35s 且全项 PASS：
     - `core_descendant: false`, `port11808: false`, `port11808_ever_seen: false`, `journal: []`, `core_logs: []`。
     - 配合 `negative_unarmed.log` 验证正式 Release 包在注入 `V2RAYN_R_AUTO_SMOKE=1`/`AUTOSTART=1` 时默认忽略，ISSUE-08 回归风险闭环；`docs/evidence/T20.md:406-422` 完整登记。

4. **F-05 (README / T20 说明明确内核非包内)**: **resolved**
   - **文件证据**: `README.md:24-26` 明示「The portable package bundles no proxy core. The packaged smoke evidence used a developer-local tools/cores/xray binary; end users obtain cores at runtime via Check updates」；`docs/evidence/T20.md:424-431` 显式澄清并指引声明。

---
**结论**: 5 项整改工件真实齐全，哈希与版本自洽，阻断项均已 resolved。
