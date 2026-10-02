# 终审整改复核 — Muse（只读，HEAD=6e00512）
- 范围：FINAL.audit.muse.md F-01~F-05 + gemini ISSUE-F-01~F-04 对应项；只读，未构建。
- HEAD=`6e00512`，`git status` 干净；`git diff 6091dd1..HEAD -- apps crates services` 为空。

| 项 | 判定 | 文件证据 |
|---|---|---|
| F-01 干净 HEAD 重建 | partially | `dist/build-info.json:57-60` commit=`6091dd1` dirty=`false`；`dist/SHA256SUMS`=`8e01647b…` 与 `Get-FileHash` 复算一致；但 ≠HEAD `6e00512`（漂移仅 `.gitignore`/dist/docs/tools，无功能源码） |
| F-02 live applied 决定性证据 | resolved | `rc-apply/live/s-…-2_journal.json:6-9` stage=`applied` pid=`40404` port=`11808` rev=`2`；`rc-apply-probe.json:22,56` listening=`true` core=`true`；`finalized/…journal.json` 同 session 转 `finalized`；config 为 mixed 11808+VLESS 192.0.2.10 |
| F-03 core.log 归档 | partially | 磁盘存在 `rc-apply/live/s-…-2_core.log` 419B（Reading config→Xray 26.3.27 started），probe 记录 sha `816706d1…`；但 `git ls-files` 未跟踪（`*.log` 忽略，`git status --ignored` 为 `!!`），`build_*.log`/`capture`/`negative` 日志同理 |
| F-04 未武装负向测试 | resolved | `rc-apply/negative-unarmed.json:37-43` AUTO_SMOKE+AUTOSTART+T18_BENCH=1 运行 35s：`core=false` `port11808=false/ever_seen=false` `journal=[]` `core_logs=[]` result=`PASS`；`README.md:30-33` 武装位说明一致 |
| F-05 README/T20 声明 | resolved | `README.md:24-26` 便携包不含内核/冒烟用本机 `tools/cores`/`Check updates` 获取；`T20.md §2.2/§6/§9.4` + `rc-apply-probe.json:7` source=`developer-local (not bundled)` 一致 |
| 抽查 build-info/SHA | 见 F-01 | `official-build-verify.json:9-14` match=`true` 但 `expected_head=6091dd1` 已过期；zip 哈希本身自洽 |

结论：F-02/F-04/F-05 已闭合；F-01（重建滞后 2 提交）、F-03（log 被忽略未入库）为部分解决，建议提交前 `git add -f` 补入库或放宽口径为“磁盘归档”。
