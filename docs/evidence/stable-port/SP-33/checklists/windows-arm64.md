# SP-33 实例 O02：Windows ARM64 —— 验收清单（blocked）

阻塞（实测依据）：pinned Flutter CLI 无 `--target-platform`，
`flutter build windows --target-platform=windows-arm64` exit 64（T20 回填，
平台矩阵 §6）。本地 x64 引擎不含 ARM64 目标。

解除条件：
1. ARM64 Windows 主机（本机 x64 不可代替）；
2. 支持 ARM64 的 Flutter Windows 引擎 + CLI（能实际产出 `windows-arm64` 产物）；
3. Rust 侧已就绪：`cargo check -p <5包> --target aarch64-pc-windows-msvc` exit 0
  （平台矩阵 §6.1；`subscriptions`/`updater` 需 `CC/AR=clang/llvm-ar` 工具链）。

解除后执行项（与 O01 同标准，不借 O01 证据）：
- 真实 `flutter build windows --release`（ARM64 主机）→ P0 包身份（`TargetTriple` 断言
  改为 `aarch64-pc-windows-msvc`，另起包名/哈希记录）；
- P1–P7 全步：隔离数据目录 → 普通入口启动 → 合成导入 → 回环 apply →
  托盘/热键/窗口/DPI → 更新检查/清理 → 备份/重开/恢复；
- 包格式：zip（沿上游 `v2rayN-windows-arm64.zip` 命名对照，不宣称等价）；
  签名/安装器缺失则登记未验证。

包格式/签名/隔离机前置：ARM64 隔离机 + 代码签名证书（未配则“未签名包仅隔离验收，
不发布”）；`build_windows.ps1` 的 ARM64 分支缺失须由 release 整合者补（本卡不改
tools/release 现有脚本）。
