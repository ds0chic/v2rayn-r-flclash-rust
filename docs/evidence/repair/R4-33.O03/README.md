# R4-33.O03 macOS x64 / Avalonia — 证据

- 实例：macOS x64（上游 RID `osx-x64`，仅 Avalonia；发行物 `v2rayN-macos-64.zip`/`.dmg`）。
- 固定 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`；应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。
- `armed=false`；无内核/网络/宿主写入。

## 结论

状态：**blocked**。

本机为 Windows x64，无 Apple 硬件与 Xcode/SDK：`Get-Command xcode-select` 不存在（见 `../R4-33/wsl_and_tooling.log`）。Windows 无法产出/运行 macOS 产物。**未构建、未运行**，不提升 verified。

## 阻塞原因

需 Apple 硬件 + Xcode/SDK（`xcode-select --install`），或 macOS CI runner。

## 解除条件与最小验证步骤

1. macOS x64 主机（或 `macos-13`/x64 CI runner）装 Xcode CLT、Flutter 3.47.5、Rust `x86_64-apple-darwin`。
2. `cargo build --workspace --release --locked` + `flutter build macos --release` exit 0。
3. 安装运行 GUI，走核/代理/TUN 权限（sudo/helper）/窗口（Dock、`MacOSShowInDock`）/热键/流量适用项；平台特有条目对照 `compat/platform-matrix.md` §4。
4. 截图 + 包内容比对，记录哈希。
