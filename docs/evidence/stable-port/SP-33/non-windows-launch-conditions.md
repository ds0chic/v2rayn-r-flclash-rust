# SP-33 非 Windows / 非 x64 实例启动前置条件（准备文档，不执行）

状态：identified（仅文档；5 个阻塞实例均未解除；无任何构建/启动/网络动作）。
基线：`393fafd687fb32dc4270f5332dac86e1efc57961`（与 `docs/evidence/stable-port/SP-33/README.md` 一致）。
约束（AGENTS.md）：合成数据 only；测试端口一律 ≥11808，先探测；永不占用/修改 127.0.0.1:10808；
不改宿主系统代理/注册表；不碰路由/TUN/DNS/Run-key；只停 owned PID；历史 ZIP 不得复用。

实测阻塞依据（不重复探测，直接引用）：
`compat/platform-matrix.md` §6（win-arm64 `flutter build windows --target-platform=windows-arm64` exit 64；
`wsl --status` exit 50，无 docker）、§6.1（Rust 交叉 `check` 5/5 通过清单）、§6.4（O01–O06 登记表）；
`docs/evidence/stable-port/SP-33/README.md` 平台矩阵摘要（本机 `wsl_state=not-found`、`docker_present=False`）；
各实例清单：`docs/evidence/stable-port/SP-33/checklists/{windows-arm64,linux,macos}.md`。
单实例证据模板：`docs/evidence/stable-port/SP-33/evidence-template.md`（每实例一份存 `SP-33/<Oxx>/`，P0–P7 + 未运行/未验证必填）。

## 通用工具链版本（四实例一致，以 AGENTS.md 锁定为准）

| 工具 | 版本 | 说明 |
|---|---|---|
| Flutter | 3.47.5 stable（Dart 3.13.4） | 平台侧可用且能产出对应 target（`flutter build linux/macOS/windows --release`） |
| Rust | 1.98.1 stable | 对应 `rustup target` 已安装（见各实例） |
| FRB | 2.13.0 | Dart/Rust/codegen 三处同版本（T01 锁定） |
| pwsh | PowerShell 7+ | 运行 `tools/acceptance/*` 的跨平台外壳（Windows 上 `powershell.exe` 亦可，但 Linux/macOS 必须 `pwsh`） |

## O02：Windows ARM64（checklist：`checklists/windows-arm64.md`）

解除前置（必须全部满足，本机 x64 不可代替）：
1. ARM64 Windows 主机（隔离机；宿主日常机不执行真实 OS 副作用，见 SP-33 observations.json B2）。
2. 支持 ARM64 的 Flutter Windows 引擎 + CLI：能实际产出 `windows-arm64` 产物（当前 pinned CLI 无
   `--target-platform`，T20 实测 exit 64；`compat/platform-matrix.md` §6）。
3. Rust 侧已就绪（本机已验证，可复用结论，不复跑）：
   `cargo check -p <domain|config_codegen|ipc_contract|subscriptions|updater> --target aarch64-pc-windows-msvc --locked`
   exit 0（5/5，见平台矩阵 §6.1；`subscriptions`/`updater` 需 `CC_aarch64_pc_windows_msvc=clang`、
   `AR_aarch64_pc_windows_msvc=llvm-ar`，前置 `winget install LLVM.LLVM`）。
4. Release 侧：`tools/release/build_windows.ps1` 的 ARM64 分支缺失须由 release 整合者补（本卡不改
   tools/release 现有脚本；checklist 原文）；隔离机 + 代码签名证书（未配则“未签名包仅隔离验收，不发布”）。

环境就绪后前三个命令（ARM64 主机上，仓库根）：
```powershell
flutter --version
cargo check -p domain --target aarch64-pc-windows-msvc --locked
powershell -NoProfile -ExecutionPolicy Bypass -File tools/acceptance/sp33_launch_prep.ps1 -VerifyFixture -ProbePorts
```
说明：命令 1 确认引擎含 ARM64 目标（若 CLI 仍无该目标则仍 blocked，停）；命令 2 复核 Rust 交叉；
命令 3 与 O01 完全相同（见下“可原样运行”）。之后按 `checklists/windows-arm64.md`：
真实 `flutter build windows --release`（ARM64 主机）→ P0 包身份（`TargetTriple` 断言改为
`aarch64-pc-windows-msvc`，另起包名/哈希记录）→ P1–P7；包命名对照上游 `v2rayN-windows-arm64.zip`（不宣称等价）。

## O05：Linux x64（checklist：`checklists/linux.md`，O05/O06 共用）

解除前置：
1. 隔离 Linux 机（二选一）：`wsl --install`（管理员 + 重启）后发行版可用，或 Docker Desktop / 原生
   Linux x64 主机（本机 `wsl --status` exit 50、无 docker，见平台矩阵 §6.1）。
2. 发行版内依赖：`build-essential`、`pkg-config`、GTK3 dev 头、`cmake`/`ninja`、
   `rustup target add x86_64-unknown-linux-gnu`。
3. pinned Flutter 3.47.5 Linux 侧可用（`flutter build linux --release` 可执行；cargokit 经
   `rust_builder/linux/CMakeLists.txt`，见 `tools/release/README-platforms.md`）。

环境就绪后前三个命令（Linux 侧仓库根，`pwsh` 须已安装）：
```sh
flutter --version
rustup target list --installed
pwsh -NoProfile -File tools/acceptance/sp33_launch_prep.ps1 -VerifyFixture -ProbePorts
```
说明：命令 2 输出须含 `x86_64-unknown-linux-gnu`；命令 3 与 O01 相同。之后：
`cargo build --workspace --release --locked` + `flutter build linux --release` →
P0（Linux 包布局：Flutter bundle + `net_host`/`privileged_helper` + LICENSE/NOTICE/README，
**不捆绑内核**；`.deb`/`.rpm` 未实现则登记未验证）→ P1–P7 + Linux 特有分支
（`.desktop` 自启写/删、sudo 密码流、TUN 权限路径、非 Windows 代理分支与自定义脚本路径、
`export` 前缀复制命令、截屏取码 null；见平台矩阵 §4）。

## O06：Linux ARM64（checklist：`checklists/linux.md`）

解除前置 = O05 全部 + 以下之一：`gcc-aarch64-linux-gnu` 交叉工具链，或 ARM64 Linux 主机；
`rustup target add aarch64-unknown-linux-gnu`。
环境就绪后前三个命令：同 O05，把命令 2 的期望改为含 `aarch64-unknown-linux-gnu`：
```sh
flutter --version
rustup target list --installed
pwsh -NoProfile -File tools/acceptance/sp33_launch_prep.ps1 -VerifyFixture -ProbePorts
```
之后与 O05 同标准，不借 O05 证据；`.deb`/`.rpm` 签名未实现，登记未验证。

## O03 / O04：macOS x64 / ARM64（checklist：`checklists/macos.md`，O03/O04 共用）

解除前置：
1. 隔离 Mac（二选一）：Intel Mac（O03）/ Apple Silicon（O04），或 macOS CI runner
  （Windows 本机无法产出 macOS 产物，见平台矩阵 §6.1）。
2. `xcode-select --install` + CocoaPods + pinned Flutter 3.47.5 macOS 侧可用
  （cargokit 经 `bridge_api.podspec`，见 `tools/release/README-platforms.md`）。
3. Rust 对应 target：O03 `rustup target add x86_64-apple-darwin`；
   O04 `rustup target add aarch64-apple-darwin`。

环境就绪后前三个命令（Mac 侧仓库根）：
```sh
flutter --version
rustup target list --installed
pwsh -NoProfile -File tools/acceptance/sp33_launch_prep.ps1 -VerifyFixture -ProbePorts
```
说明：命令 2 输出须含对应 `*-apple-darwin` target。之后：
`cargo build --workspace --release --locked` + `flutter build macos --release` →
P0（macOS 包布局：`.app` bundle + `net_host`/helper 对应物 + LICENSE/NOTICE/README，
**不捆绑内核**；`.dmg`/`.zip` 命名对照上游 `v2rayN-macos-<arch>.zip/.dmg`，不宣称等价）→
P1–P7 + macOS 特有分支（Dock accessory 策略与 `MacOSShowInDock` 切换、
`~/Library/LaunchAgents` plist 写/删、非 Windows 代理分支、热键设置项不可见、
二维码截屏 null；见平台矩阵 §4）。签名/公证/entitlements 未覆盖，登记未验证；
配齐 Apple 签名身份前“未签名包仅隔离验收，不发布”。

## SP-33 launch prep 可原样运行的部分（四实例通用）

脚本：`tools/acceptance/sp33_launch_prep.ps1`（已读全文 123 行；探针脚本另见
`tools/acceptance/sp33_platform_matrix.ps1`）。以下在 Linux/macOS 的 `pwsh` 下可原样运行，
结论跨平台有效：
- `-DryRun`：只打印计划（identified），零副作用。
- `-VerifyFixture`：读 `fixtures/acceptance/sp33/synthetic-sub.txt`，断言 4 行形状
  （vmess 可解码 / `192.0.2.0/24` 文档地址 / 端口 ≥11808 / remarks `sp33-synth-*`）。
  纯文件读取，OS 无关（SP-33 README 实测 pass 可复用结论，但新环境仍重跑一次）。
- `-ProbePorts`：对 127.0.0.1:11808..11815 做 TCP connect 探测（refused=空闲；
  只 connect、不 bind、不 listen）。基于 `Net.Sockets.TcpClient`，pwsh 下跨平台；
  永不触碰 10808（脚本内写死标注，见第 117 行）。
- `-Zip <pkg>`：sha256 + zip 条目检查逻辑跨平台可用，但**布局断言是 Windows-x64 专用的**
  （flat 四 exe：`v2rayn_desktop.exe`/`net_host.exe`/`privileged_helper.exe`/`v2rayN-upgrade.exe`；
  无内核捆绑；`build-info.json` 的 `smoke_armed=false`/`git_dirty=false`/`commit==预期`）。
  在 Linux/macOS 上跑 `-Zip` 时四 exe 断言预期失败——这是布局口径差异，不是包缺陷；
  按实例清单的包布局（bundle / `.app`）另行记录，不改脚本。包身份的通用判定
  （sha 记录、无内核捆绑、unarmed 断言、commit 一致）保持与 O01 同标准。

## 验收子集：OS 无关 vs OS 特定

OS 无关（结论与方法可跨实例复用口径，但证据不互借，每实例重跑并按
`evidence-template.md` 逐行 JSON 记录）：
- P0 通用判定：sha256 记录、无内核捆绑（policy: not_bundled）、`smoke_armed=false`、
  `git_dirty=false`、commit==基线。核对脚本：`tools/acceptance/sp30_package_identity.ps1 -Zip <新包>`
 （自测见 `tools/acceptance/sp30_package_identity_selftest.ps1`）。
- P1 端口探测（≥11808 空闲；10808 未触碰）。
- P3 合成导入（`fixtures/acceptance/sp33/synthetic-sub.txt` 4 节点，remarks `sp33-synth-*`；失败行不半写）。
- P4 选择/应用合同：desired/applied revision 分离、journal `stage=applied`
 （pid/config_sha256/rev）、回环端口 ≥11808。
- P7 备份/重开/恢复（同 SP-30 S5–S7：备份 zip 记 hash → 同目录重开一致 → 清空后恢复一致，配置 diff 为空）。
- 隔离数据目录机制：`V2RAYN_R_DATA_DIR=%TEMP%/sp33_accept_<guid>` 空目录
  （准备脚本范式见 `tools/acceptance/sp30_prepare_datadir.ps1 -DryRun`）。

OS 特定（必须在目标 OS 真机/隔离机实测，Windows x64 证据不可代替）：
- P2 启动入口与产物形态：Windows `v2rayn_desktop.exe` 双击 / Linux Flutter bundle / macOS `.app`；
  首跑落点 `guiNConfig.json`（惰性）+ `guiNNDB.db`。
- P5 托盘/热键/窗口/DPI：WPF vs Avalonia 基线差异（平台矩阵 §3：菜单容器/主题入口/竖排 Tab/
  默认尺寸 800 vs 600 等）；平台分支（平台矩阵 §4）：热键仅 Windows（`HotkeyManager.cs:32` 非 Windows
  直接 return）；二维码截屏非 Windows 返回 null；复制命令 `set` vs `export`；
  macOS Dock accessory + `MacOSShowInDock`；Linux sudo 密码框/TUN 权限/`.desktop` 自启；
  自定义代理脚本路径仅非 Windows 显示；PAC 项 Windows 可见。
- P6 更新检查/清理：自启写/删按平台（Windows 注册表 Run/任务计划 / Linux `~/.config/autostart/*.desktop` /
  macOS `~/Library/LaunchAgents/v2rayN-LaunchAgent.plist` + `launchctl`）；owned PID/端口/目录回收。
- 签名/安装器/自动更新真实执行：全平台登记未验证（O01 清单 P6 约束行；Linux/macOS 清单签名节）。
- 额外架构（Windows x86、Linux riscv64/loong64）：Flutter 无官方 target，保持 unverified，
  不由本轮降低（平台矩阵 §2、§5）。

## 解除后的记录要求

每实例在 `docs/evidence/stable-port/SP-33/<Oxx>/` 下按 `evidence-template.md` 建证据：
P0–P7 JSON 行 + 未运行/未验证必填；`version` 探针只记在位识别，不计入链路通过；
任一 `actual != expected` 即停并记 blocked；历史 ZIP 不得复用（P0 须从基线干净重建，
见 SP-34 `prep-note.md` 同源规则）。
