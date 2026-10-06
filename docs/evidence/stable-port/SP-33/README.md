# SP-33 六平台独立交付：准备（基线 393fafd）

状态：identified（跨平台验收脚本/清单/证据模板已建；实例真实验收未运行）。
本次只写 `tools/acceptance/**`、`fixtures/acceptance/**`、本目录，
另向 `SP-28/` 追加只读说明 1 份（未改 SP-28 既有文件）；
未改生产代码、tools/release 现有脚本、FRB、Cargo 锁；未 commit。

前置证据（只读引用，不宣称完成）：SP-28 14 核夹具（`../SP-28/core-matrix.csv`，
L0 可跑 / L1 blocked）；SP-30 验收清单（`../SP-30/README.md`，
verdict 前置 fail+incomplete 待复核）；平台矩阵 `compat/platform-matrix.md`
§6–§6.4（O01–O06 登记与实测 exit：arm64CLI exit 64、wsl exit 50）。

## 平台矩阵摘要（本机实探，`sp33_platform_matrix.ps1` exit 0）

| 实例 | 平台/架构 | 状态 | 解除条件 |
|---|---|---|---|
| O01 | Windows x64 | available（本机可执行） | 无；按 `checklists/windows-x64.md` P0–P7 执行 |
| O02 | Windows ARM64 | blocked | ARM64 Windows 主机 + ARM64 Flutter 引擎（Rust 交叉 check 已 5/5） |
| O03 | macOS x64 | blocked | Intel Mac/CI + Xcode（`xcode-select --install`） |
| O04 | macOS ARM64 | blocked | Apple Silicon 主机/CI + Xcode |
| O05 | Linux x64 | blocked | `wsl --install`/Docker + build-essential/pkg-config/GTK3/cmake/ninja + rustup target |
| O06 | Linux ARM64 | blocked | 同 O05 + `gcc-aarch64-linux-gnu`/ARM64 主机 |

本机探针：flutter_pinned=true、cargo_pinned=true、wsl_state=not-found、
docker_present=False。version/在位输出仅环境识别，不代替任何实际链路；
各实例验收必须走清单真实 build/package/runtime/tray/hotkey/window/DPI/
update/cleanup，不交叉借证据，额外架构不删库存。

## 本轮实际命令与 exit（只读；日志见 `commands.log`）

| 命令 | exit | 结果 |
|---|---|---|
| `sp33_platform_matrix.ps1 -DryRun` | 0 | 计划输出（identified） |
| `sp33_launch_prep.ps1 -DryRun` | 0 | 计划输出（identified） |
| `sp33_platform_matrix.ps1`（实探） | 0 | O01 available，其余 5 blocked（上表） |
| `sp33_launch_prep.ps1 -VerifyFixture -ProbePorts` | 0 | 夹具 4 行形状 pass；11808..11815 空闲；10808 未触碰；包身份 skipped（无 -Zip） |

修过 1 缺陷：ss 分支 `$Matches` 被 `-notmatch` 覆盖导致空端口误报，
已按 sp30 写法存局部变量后复测 pass。

## Windows x64 已跑可执行项

包身份/启动准备中不依赖新包的部分：合成订阅形状校验 pass、
回环端口空闲探测 pass。P0 包身份待从基线干净重建候选包后补；
P2–P7 真实 GUI/重开待隔离机（宿主为日常使用机）。

## 未运行范围（按 VALIDATION_POLICY.md 定向规则，未改代码故不跑门禁）

cargo fmt/clippy/test（无 Rust 改动）；flutter analyze/test（无 Dart 改动）；
`tools/gates/run_all.ps1`（发布候选才跑）；S0 真实包身份（无新包）；
P2–P7 真实链路（被新包 + 隔离机阻塞）；其余五实例全部 blocked（见上表）。

## 本目录文件

`README.md`（本文件）、`observations.json`、`commands.log`、
`evidence-template.md`（单实例证据模板）、`checklists/`（4 份逐平台清单）。
