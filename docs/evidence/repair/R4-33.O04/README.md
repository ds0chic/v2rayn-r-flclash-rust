# R4-33.O04 macOS ARM64 / Avalonia — 证据

- 实例：macOS ARM64（上游 RID `osx-arm64`，仅 Avalonia；发行物 `v2rayN-macos-arm64.zip`/`.dmg`）。
- 固定 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`；应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。
- `armed=false`；无内核/网络/宿主写入。

## 结论

状态：**blocked**。

同 O03：本机为 Windows x64，无 Apple 硬件/SDK（`xcode-select` 不存在），无法产出/运行 macOS ARM64 产物。**未构建、未运行**。

## 阻塞原因

需 Apple Silicon（或 ARM64 macOS CI runner）+ Xcode/SDK。

## 解除条件与最小验证步骤

1. Apple Silicon 主机（或 `macos-14`/arm64 CI runner）装 Xcode CLT、Flutter 3.47.5、Rust `aarch64-apple-darwin`。
2. `cargo build --workspace --release --locked` + `flutter build macos --release` exit 0。
3. 安装运行 GUI，走核/代理/TUN 权限/窗口/热键/流量适用项；对照 `compat/platform-matrix.md` §4。
4. 截图 + 包内容比对 + 哈希。
