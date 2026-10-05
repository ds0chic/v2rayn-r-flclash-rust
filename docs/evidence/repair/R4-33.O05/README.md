# R4-33.O05 Linux x64 / Avalonia — 证据

- 实例：Linux x64（上游 RID `linux-x64`，仅 Avalonia；发行物 `v2rayN-linux-64.deb`/`.rpm`）。
- 固定 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`；应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。
- `armed=false`；无内核/网络/宿主写入。

## 结论

状态：**blocked**。

本机无 WSL 分发（`wsl --status` exit 50，未安装）、无 `docker`（见 `../R4-33/wsl_and_tooling.log`）。无法构建/运行 Linux 产物。**未构建、未运行**。

## 阻塞原因

无 Linux 运行环境（WSL 未安装 / 无 Docker）。

## 解除条件与最小验证步骤

1. `wsl --install`（管理员 + 重启）或 Docker Desktop，取得 x86_64 Linux 环境。
2. 装 `build-essential`、`pkg-config`、GTK/Ninja/CMake 等 Flutter Linux 依赖，Rust `x86_64-unknown-linux-gnu`。
3. `cargo build --workspace --release --locked` + `flutter build linux --release` exit 0。
4. 安装运行 GUI，走核/代理/TUN 权限（sudo/helper）/窗口/热键/流量适用项；对照 `compat/platform-matrix.md` §4（sudo 提权、autostart `.desktop`）。
5. 截图 + 包内容比对 + 哈希。
