# R4-33.O02 Windows ARM64 / WPF — 证据

- 实例：Windows ARM64，上游界面 WPF。
- 固定 HEAD：`8651e19d1049d69e27951c98e4884560d56f4c97`；应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。
- `armed=false`；无内核/网络/宿主写入；未占用 `127.0.0.1:10808`。

## 结论

状态：**blocked**。

| 项 | 结果 | 证据 |
|---|---|---|
| Rust 交叉 `check`（5 包） | **5/5 通过，exit 0**（本机复跑，2026-10-05） | `../R4-33/cargo_check_aarch64.log`；`docs/evidence/T21-kcp-cross.md` §6.1 |
| `rustup target list --installed` | `aarch64-pc-windows-msvc` 已装 | 同上 |
| Flutter Windows ARM64 GUI 构建 | **不支持**：`flutter build windows --release --target-platform=windows-arm64` → `Could not find an option named "--target-platform".` exit 64 | `../R4-33/build_arm64_target.log`；`compat/platform-matrix.md` §6 |
| GUI 实际构建/安装/运行/核/代理/TUN 权限/窗口/热键/流量 | **未运行**（无 ARM64 主机/引擎） | — |

命令（本机，CC/AR 指向 LLVM）：
`cargo check -p domain -p config_codegen -p ipc_contract -p subscriptions -p updater --target aarch64-pc-windows-msvc --locked` → `Finished dev profile in 10.50s`。

说明：交叉 `check` 仅证明源码可对 ARM64 目标**编译检查**，**不链接、不运行**；不等于 GUI 可构建/安装/运行。故不提升 verified。

## 阻塞原因

pinned Flutter（3.47.5）Windows 桌面引擎/CLI 不提供 ARM64 目标（`--target-platform` 仅 Android/iOS）；本地 x64 引擎不含 ARM64 目标。

## 解除条件（最小）

1. ARM64 主机 + 支持 ARM64 的 Flutter Windows 引擎（`flutter build windows` 可产出 arm64 产物）。
2. 在该主机安装/运行 GUI，走核/代理/TUN 权限/窗口/热键/流量适用项并截图取证。
3. 保留 Windows x86 差异登记（`compat/platform-matrix.md` §2），不宣称等价。
