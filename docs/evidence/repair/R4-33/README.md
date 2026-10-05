# R4-33 六平台完整移植 — 证据（总）

- 固定 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`（应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`）
- `armed=false`；未启动内核、未写宿主系统代理/TUN/路由/注册表/自启、未读用户凭据、未占用 `127.0.0.1:10808`、未监听端口。
- 平台：Windows 11 25H2 (build 26220) x64；Flutter 3.47.5 / Dart 3.13.4；Rust 1.98.1。

## 结论

R4-33 主卡状态：**blocked**（六实例无一达成“GUI 实际构建/安装/运行/核/代理/TUN 权限/窗口/热键/流量全部适用项 verified”）。

本机（Windows x64）可执行部分已核对：
- O01 Windows x64：已有构建/冒烟、14 核真实最小会话、真机 OS 集成证据；但 R4-32 未武装包交付回归**未完成**（`docs/evidence/repair/R4-32/` 仅部分构建日志，缺 README/observations/门禁/哈希；任务卡仍 identified），且真实 auto-route TUN 未实测 → blocked。
- O02 Windows ARM64：Rust 交叉 `check` **本机复跑 5/5 通过**（exit 0）；Flutter Windows ARM64 目标 CLI 不存在（exit 64）→ blocked。
- O03–O06：本机无 Apple 硬件/SDK、无 WSL/Docker → blocked。

`docs/repair/coverage.csv` 中 `owner_task=R4-33` 行数 = **0**；平台库存以 `compat/platform-matrix.md` 为准，本卡未虚构 ID、未降分母。

## 文件

- `platform-matrix.md`：六平台总表（适用/状态/原因/解除条件/证据引用）。
- `cargo_check_aarch64.log`、`build_arm64_target.log`、`wsl_and_tooling.log`：本机命令原始输出。
- `../../R4-33.O01..O06/`：逐实例 README + observations。
