# SP-26 硬件渲染设置可行性研究（基线 92d46dd）

状态：identified（可行性证据完成；生产接线未做，需主控派单 runner owner）。

本次范围：只读研究 + 可观测证据。未改任何生产 Dart/Rust/C++ 代码，未改 FRB/Cargo 锁，未 commit。
实际结果：engine/源码级可行性证据齐全；运行时 renderer 观测方法已定义但本次未执行（需接线后的 release 候选）。
未运行范围：`flutter build windows --release`（本次不重建，与其它代理重建互斥）；release 真机 renderer 观测（截图/adapter 日志/帧耗时对比）；`flutter analyze`（见下）。

## 1. 原版 HWA 语义（work 只读，固定 commit 7d6a967）

- `ServiceLib/Models/Configs/ConfigItems.cs:75`：`EnableHWA bool = false`（默认关闭）。
- `v2rayN/Views/MainWindow.xaml.cs:151-154`：`if (!EnableHWA) RenderOptions.ProcessRenderMode = RenderMode.SoftwareOnly;`
  即：原版默认=全进程软件渲染；勾选=恢复 WPF 硬件渲染；进程启动时一次性设置，改动需重启（与本体重启语义 `settings_timing.rs:122 RestartApp` 一致）。

## 2. 当前 release 下 Flutter Windows renderer 实际选项（可观测证据）

锁定工具链 Flutter 3.47.5 / engine ab59836859（见 `probe_baseline_92d46dd.log`，exit 0）。
engine 二进制证据（`flutter_windows.dll` sha256 `8BD7B61E…675251`，ASCII 扫描 `tools/perf/engine_strings.py`）：

| 开关/符号 | 计数 | 语义 |
|---|---|---|
| `--enable-impeller[=true\|=false]` | 3/1/1 | Impeller 开关存在 |
| `enable-software-rendering` | 1 | "Enable rendering using the **Skia software backend**. … useful when testing Flutter on **emulators**"（帮助文本原文） |
| `FlutterDesktopEngineGetGraphicsAdapter` | 1 | 可观测钩子：返回渲染用 DXGI adapter |
| `--enable-flutter-gpu` | 3 | 需 Impeller |

embedder 头文件证据（`flutter_windows.h` sha256 `BDA68D72…938D4C2C`；同文件已同步到本仓 `apps/desktop/windows/flutter/ephemeral/`）：

- `FlutterDesktopGpuPreference { NoPreference, LowPowerPreference, HighPerformancePreference }` —— **仅选 GPU，按文档类内 fallback，不是软件渲染**。
- `FlutterDesktopImpellerSwitch { Default, Enabled, Disabled }` —— **Disabled 回落到 ANGLE/Skia GPU 路径，不等于软件渲染**。
- C++ 包装 `dart_project.h:121-154` 暴露且仅暴露 `set_gpu_preference` / `set_impeller_switch`；**无 software-rendering 旋钮**。
  `enable-software-rendering` 未经 `FlutterDesktopEngineProperties` 暴露，Windows embedder 无逐实例透传通道。

环境探针（本机实际 GPU）：NVIDIA RTX 5070 Ti（32.0.16.1692）+ AMD Radeon Graphics（32.0.21045.5002）+ 2 块虚拟显示适配器。
注意：虚拟适配器 + 双 GPU 使"当前用哪块 adapter"必须以 `GetGraphicsAdapter` 日志为准，不能从设备管理器推断。

## 3. 关键结论（防错映射）

1. **禁 Impeller ≠ 软件渲染；选低功耗 GPU ≠ 软件渲染**（头文件 fallback 语义原文为证）。
2. **Flutter Windows 无 `RenderMode.SoftwareOnly` 的对等物**：严格原版语义（默认全软件渲染）在当前 embedder 能力下不可直接实现。
3. runner 现状：`apps/desktop/windows/runner/*.cpp/*.h` 对 `hwa|HWA|impeller|GpuPreference|GetGraphicsAdapter` **零命中**（G-03 复核通过）；`main.cpp:124-126` 只调 `set_dart_entrypoint_arguments`，从未设置 gpu/impeller。Dart 侧仅持久化 bool（`option_setting_window.dart:1034-1035`，默认 false `:104`）。

## 4. 最小接线方案（供主控派单，不在本卡实施）

runner patch 归主控指定的 runner owner；需先行 ADR（`docs/decisions/`）。

- **语义映射（推荐 A，诚实降级）**：HWA=ON → `ImpellerSwitch::Default`（或 Enabled）+ `GpuPreference::HighPerformancePreference`；
  HWA=OFF → `ImpellerSwitch::Disabled` + `GpuPreference::LowPowerPreference`，并在设置文案/证据中明确"OFF 为低功耗 GPU + 非 Impeller 路径，**不是**原版 SoftwareOnly"。严格软件对等若被要求，则标 blocked，走独立 embedder 扩展任务（engine-switch 透传，需验证 `--enable-software-rendering` 在 Windows release 的真实效果与 fallback）。
- **接线点**：`apps/desktop/windows/runner/main.cpp`（engine 创建前 `flutter::DartProject project` 处，`flutter_window.cpp:24` 之前生效）。
  runner 启动时（Dart/FRB 尚未起来）需读到 `enable_hwa`：由 Rust 侧在设置保存时同步一个 runner 可读的前置信号（flag 文件/env/重启参数三选一，owner 定；`rebootas` 前例见 `main.cpp:97-109`），重启语义复用现有 RestartApp。
- **观测验证命令（owner 执行）**：在 `flutter_window.cpp` engine 就绪后调 `FlutterDesktopEngineGetGraphicsAdapter` 记录 adapter desc；两次 release 候选（ON/OFF）各取：adapter 日志 + 主/独立窗截图 + 帧耗时（SP-31 方法）；失败回滚语义（engine 创建失败恢复旧模式）由 SP-26 接线卡定义。
- **owner 需求**：runner C++ owner 1 人 + ADR；release 构建只做一次（ON/OFF 两候选可同一次构建、两次运行）；不与其它代理并行重建。

## 5. 定向检查

`flutter analyze`：本卡零 Dart/Rust/C++ 改动（仅新增 `tools/perf/*.ps1/*.py` 与证据 md/json/log），按 VALIDATION_POLICY §1 文档/工具改动不触发应用级门禁，**未运行**。`flutter --version` 在探针中 exit 0（有一次 flutter 锁争用 exit 1，重试后 exit 0，已记录）。

命令与 exit：见 `probe_baseline_92d46dd.log`（probe exit 0）；`python3 tools/perf/engine_strings.py` exit 0。
`git status --short`：见本卡 observations.json。
