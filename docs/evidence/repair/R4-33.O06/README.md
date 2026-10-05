# R4-33.O06 Linux ARM64 / Avalonia — 证据

- 实例：Linux ARM64（上游 RID `linux-arm64`，仅 Avalonia；发行物 `v2rayN-linux-arm64.deb`/`.rpm`）。
- 固定 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`；应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。
- `armed=false`；无内核/网络/宿主写入。

## 结论

状态：**blocked**。

同 O05：本机无 WSL 分发、无 `docker`；且无 ARM64 Linux 工具链/主机。**未构建、未运行**。

## 阻塞原因

无 Linux 运行环境，且缺 ARM64 交叉/原生工具链。

## 解除条件与最小验证步骤

1. 先满足 O05 的 Linux 环境（`wsl --install` 或 Docker，或原生 ARM64 Linux 主机）。
2. 装 `gcc-aarch64-linux-gnu`/`libc6-dev-arm64-cross`（交叉）或 ARM64 主机工具链；Rust `aarch64-unknown-linux-gnu`。
3. `cargo build --workspace --release --locked` + `flutter build linux --release` exit 0。
4. 安装运行 GUI，走核/代理/TUN 权限/窗口/热键/流量适用项；对照 `compat/platform-matrix.md` §4。
5. 截图 + 包内容比对 + 哈希。
