# R4-33 六平台完整移植 — 总表（2026-10-05，HEAD `8651e19`）

- 冻结上游：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`；应用基线 `77c74ed8e5ae8ddffbeca0812e4998a09bbd9bc5`。
- `armed=false`；本卡未启动生产内核、未改宿主系统代理/TUN/路由/注册表/自启、未读用户凭据、未占用 `127.0.0.1:10808`、未监听任何端口（仅 `cargo check` + Flutter CLI 参数探测 + WSL/工具探测）。
- 口径：单实例“完成条件”= 六单元 GUI **实际构建/安装/运行/核/代理/TUN 权限/窗口/热键/流量**全部适用项 verified（不是 `cargo check`）。任一适用项未实测则实例整体 `blocked`，已实测部分如实登记。
- 库存说明：`docs/repair/coverage.csv` 中 `owner_task=R4-33` 行数 = **0**（该卡不由 coverage.csv 分配条目；平台维度证据见 `compat/platform-matrix.md`）。不虚构 ID。

## 六平台总表

| 实例 | 目标 OS×架构 / 上游界面 | 适用 | 当前状态 | 已实现证据 | 阻塞原因 | 解除条件（最小） | 证据引用 |
|---|---|---|---|---|---|---|---|
| R4-33.O01 | Windows x64 / WPF | 是 | **blocked**（部分 implemented） | 打包构建+冒烟、14 核真实最小会话、真机系统代理/自启/helper 路由/TUN 适配器(auto_route=false) | R4-32 未武装包交付回归**未完成**（`docs/evidence/repair/R4-32/` 仅部分构建日志，缺 README/observations/门禁/哈希；任务卡仍 identified）；真实 auto-route TUN 未实测 | 完成 R4-32 全门禁并补齐证据；授权隔离 VM 跑真实 auto-route TUN | `docs/evidence/T20.md`、`T21-real-os.md`、`recheck-fixes/R3-CORE-MATRIX/`；见 O01/README |
| R4-33.O02 | Windows ARM64 / WPF | 是 | **blocked** | Rust `domain/config_codegen/ipc_contract/subscriptions/updater` 交叉 `check` 5/5 通过（本机复跑 exit 0） | pinned Flutter Windows 引擎/CLI 无 ARM64 目标（`--target-platform` 不存在，exit 64） | ARM64 主机 + 支持 ARM64 的 Flutter Windows 引擎；产出并安装运行 GUI | `T21-kcp-cross.md` §6.1；`R4-33/build_arm64_target.log`；见 O02/README |
| R4-33.O03 | macOS x64 / Avalonia | 是 | **blocked** | 无 | 无 Apple 硬件/SDK，Windows 无法产出 macOS 产物 | Apple 硬件 + Xcode/SDK（`xcode-select --install`）或 macOS CI runner | `R4-33/wsl_and_tooling.log`；见 O03/README |
| R4-33.O04 | macOS ARM64 / Avalonia | 是 | **blocked** | 无 | 同 O03 | 同 O03（Apple Silicon 主机优先） | 见 O04/README |
| R4-33.O05 | Linux x64 / Avalonia | 是 | **blocked** | 无 | 本机无 WSL 分发（`wsl --status` exit 50）、无 docker | `wsl --install`（管理员+重启）或 Docker Desktop；装 `build-essential`/`pkg-config`/`rustup target` | `R4-33/wsl_and_tooling.log`；见 O05/README |
| R4-33.O06 | Linux ARM64 / Avalonia | 是 | **blocked** | 无 | 同 O05，另缺 ARM64 交叉/原生工具链 | 同 O05 + `gcc-aarch64-linux-gnu` 或 ARM64 主机 | 见 O06/README |

## 额外架构差异（保留，不在六单元内）

上游另有 Windows x86、Linux riscv64、Linux loong64 发行物；标准 Flutter 引擎无对应目标。结论：**不能宣称与上游全部架构等价**，一律 `unverified`/`blocked`，不由本卡降低。证据：`compat/platform-matrix.md` §2、§5。

## 本卡实际执行命令（平台可执行部分）

| 命令 | 结果 |
|---|---|
| `cargo check -p domain -p config_codegen -p ipc_contract -p subscriptions -p updater --target aarch64-pc-windows-msvc --locked`（CC/AR=LLVM clang/llvm-ar） | `Finished dev profile ... in 10.50s`，exit 0（5/5） |
| `rustup target list --installed` | `aarch64-pc-windows-msvc`、`x86_64-pc-windows-msvc` |
| `flutter build windows --release --target-platform=windows-arm64` | `Could not find an option named "--target-platform".` exit 64 |
| `wsl --status` / `wsl -l -v` | 未安装 WSL 分发，exit 50 |
| `Get-Command docker` / `xcode-select` | 均不存在 |

GUI 构建/安装/运行、核/代理/TUN 权限/窗口/热键/流量在非 Windows x64 平台**全部未运行**；Windows x64 的 R4-32 交付回归**未运行**。未运行写“未运行”，未实测写“未验证”。
