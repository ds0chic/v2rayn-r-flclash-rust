# ADR: SP-26 HWA runner 接线（诚实降级映射）

状态：accepted（runner owner 实现；Dart/Rust 同步待主控接线）。
基线：`788adf6`。上游：v2rayN 7.25.4 / `7d6a967`。

## 上游语义（只读核对）

- `ServiceLib/Models/Configs/ConfigItems.cs:75`：`EnableHWA = false`（默认关）。
- `Views/MainWindow.xaml.cs:151-154`：OFF 时 `RenderOptions.ProcessRenderMode = RenderMode.SoftwareOnly`
 （全进程软件渲染）；ON 恢复 WPF 硬件渲染。进程启动时一次性设置。
- `ViewModels/OptionSettingViewModel.cs:308-312`：HWA 变化置 `needReboot`（重启生效）。
- 本仓已对齐：`crates/domain/src/settings_timing.rs:122` FLD-CFG-062 = `RestartApp`；
  Dart 开关文案已是"启用硬件加速 (需重启)"（`option_setting_window.dart:1033`）。

## embedder 能力边界（可行性证据结论）

- `flutter_windows.h`：`GpuPreference`（仅选 GPU，类内 fallback）、
  `ImpellerSwitch`（Disabled 回落 ANGLE/Skia GPU 路径），均非软件渲染；
  `enable-software-rendering` 未经 `FlutterDesktopEngineProperties` 暴露，
  Windows 无逐实例透传通道。严格 SoftwareOnly 对等不可实现，不在本卡做。

## 决策

映射（方案 A，诚实降级）：

| EnableHWA | ImpellerSwitch | GpuPreference | 含义 |
|---|---|---|---|
| ON | `Default` | `HighPerformancePreference` | Impeller 默认行为 + 独显优先 |
| OFF（默认） | `Disabled` | `LowPowerPreference` | 非 Impeller 路径 + 核显优先，**不是 SoftwareOnly** |

读取优先级（engine 创建前 Dart/FRB 尚未起来，只能读进程级信号）：
CLI（`--hwa=on|off`、`--hwa=<bool>`、`--enable-hwa`、`--disable-hwa`、`--no-hwa`）
> 环境变量 `V2RAYNR_ENABLE_HWA`
> `<exe-dir>\v2raynr_hwa.ini`（`[rendering] enable_hwa=1|0`）
> 默认 OFF（与上游默认 false 一致）。

- 接线点：`main.cpp`（`DartProject` engine 创建前）+ `flutter_window.cpp`
 （engine 就绪后调 `FlutterEngine::GetGraphicsAdapter` 记录 DXGI adapter）。
- HWA-only CLI 参数被消费，不转发给 Dart（`StripHwaFlags`）。
- 重启语义：映射在进程生命周期内固定，复用现有 RestartApp；
  单实例互斥下，带不同 flag 的二次启动只会唤醒首实例（需先关再开）。
- 失败语义：非法值逐级忽略、最终回落 OFF；adapter 查询失败只记日志；
  任何 HWA 路径不断言、不中断启动（engine 创建失败仍走原有 `EXIT_FAILURE`）。

## 登记返回（主控接线，Dart/Rust owner）

1. Rust 设置保存时同步 runner 可读信号：向 `<exe-dir>\v2raynr_hwa.ini`
   写 `[rendering] enable_hwa=<0|1>`（随 RestartApp 重启后 runner 下次启动读到）。
   文件格式/位置如需改，由整合者定并同步更新本 ADR 与 runner 读取侧。
2. Dart 开关文案建议追加"（非软件渲染：关闭=低功耗 GPU 路径）"，Dart 侧改动不属本卡。
3. 若产品要求严格软件对等：标 blocked，另开 embedder 扩展任务
   （透传 `--enable-software-rendering` 并验证 Windows release 真实效果）。

## 验证

一次 `flutter build windows --release` 产物、两次运行（ON/OFF）：
adapter 日志（`%TEMP%\v2raynr-hwa.log` + OutputDebugString）+ 主窗截图。
证据见 `docs/evidence/stable-port/SP-26/`。
