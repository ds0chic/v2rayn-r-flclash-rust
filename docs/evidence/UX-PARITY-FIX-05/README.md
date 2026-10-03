# FIX-05 证据 — 扫描图片中的二维码→导入

日期：2026-10-03。卡：`docs/tasks/FIX-05.md`（repair-queue 第 23 行）。应用 HEAD：`b6388215bb5622d28b89622ccb66da53175d3036`（工作树含其它代理并行改动，本卡未触碰）。上游冻结：v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`。

## 结论

- 状态 `implemented`：图片选择→解码→既有导入管线→SQLite 落库→重开已实现，并由真 `bridge_api.dll` 测试验证；主菜单 ACT-MAIN-018 的 dispatch 属根代理（`main_shell.dart`），接线前真实入口仍指向旧粘贴窗口；未做原版实机双窗口逐事件对照，不写 `verified`。
- 取消选图静默（无提示、不改状态）；空图、非 QR、打开失败均有可操作提示；不崩溃。
- Ctrl+S 归属：上游 `MainWindow.xaml.cs:226` 的 Ctrl+S 调 `ScanScreenTaskAsync`（屏幕截图扫描，主菜单 ACT-MAIN-017）。图片扫描 ACT-MAIN-018 无快捷键。本卡不接 Ctrl+S；屏幕扫描登记 FIX-05B。
- 分享窗口 `shareProfilesQr` / `sub_share_dialog` 未改（PR-24 分享文本属另一卡/窗口卡）。

## 上游对照

| 上游符号/行 | 行为 | 本卡实现 |
|---|---|---|
| `MainWindow.xaml.cs:110-118` | 文件对话框 `PNG\|*.png\|All\|*.*`，取消 `SetOutput(null)` | `file_selector.openFile` 图片扩展名组；取消返回 null |
| `MainWindowViewModel.cs:522-526` | `AddServerViaImageAsync` → `ScanImageResult(fileName)` | `scanImageQr` → picker → 解码 |
| `MainWindowViewModel.cs:528-537` | 文件名空直接 return（静默） | picker 返回 null 直接 return，无提示无状态改动 |
| `QRCodeUtils.cs:53-73` | 文件不存在/解码异常返回 null | 字节不读/非图片 `decodeQrImage` 返回 null，命令给可操作提示 |
| `QRCodeUtils.cs:94-124` | 正向解码失败→水平翻转重试 | 同样先正向、失败后水平翻转重试（测试覆盖镜像码） |
| `MainWindowViewModel.cs:539-559` | 空结果 → `NoValidQRcodeFound`；成功 `AddBatchServers(_config, result, _config.SubIndexId, false)` → Refresh → 成功提示；否则 OperationFailed | 空结果提示「扫描完成，未发现有效二维码（请选择含清晰二维码的 PNG/JPG 图片）」；成功走既有 `importShareText`（`importFromText`+`persistImportedProfiles`+`reload`）并提示「已从扫描导入 N 个节点」 |
| `ResUI.zh-Hans.resx:252/291` | `扫描完成，未发现有效二维码` / `扫描导入分享链接成功` | 保留上游原文并补可操作指引；成功提示用既有队列风格 |
| `MainWindow.xaml.cs:226-228` | **Ctrl+S → 屏幕扫描** | 不改 Ctrl+S；FIX-05B |

## 命令与结果

| 命令 | 结果 | 日志 |
|---|---|---|
| `flutter pub add file_selector image zxing2` | 成功（1.1.0 / 4.10.1 / 0.2.4） | `format.log` 同批记录（pubspec 变更见 git diff） |
| `flutter pub add dev:qr` | 成功（3.0.2，仅测试夹具） | — |
| `dart format --output=none --set-exit-if-changed`（4 个改动文件） | 0 changed | `format.log` |
| `flutter analyze` | No issues found | `analyze.log` |
| `flutter test test/scan_image_qr_test.dart` | 8/8 通过 | `fix05-tests.log` |
| `flutter test test/ux_parity_fix05_scan_test.dart` | 2/2 通过（真 DLL + temp data dir；扫描→落库→重开仍在；取消/空图不动库） | `fix05-tests.log` |
| 回归 4 文件（t09/t21e ux/t21e real bridge/fix04 inner） | 11/11 通过 | `import-regression.log` |

未运行：`flutter build windows`、真实文件对话框人工点击、原版实机对照、屏幕扫描。真实窗口端到端（菜单点击）需根代理接线后跑；建议复用 `test/ux_parity_fix05_scan_test.dart` 的夹具注入思路，在 integration_test 中注入 `QrImagePicker`（真实文件对话框无法自动化选择）。

## 根代理接线补丁（main_shell.dart）

```dart
// import 区增加：
import 'package:v2rayn_desktop/features/subs/scan_image_qr.dart';

// _onMenuAction 内：
case 'ACT-MAIN-018':
  scanImageQr(context, ref);
```

Ctrl+S 的 `shareProfilesQr` 分支（`main_shell.dart:158-159`）保持不变，属 ACT-MAIN-017 屏幕扫描（FIX-05B）。

## 接口缺口 / 下一步

- FIX-05B：屏幕扫描（隐藏→截图→恢复，所有错误/取消分支恢复窗口），复用 `importShareText` / `decodeQrImage`。
- PR-24：分享窗口 400x400 + 只读可复制文本，独立卡；本卡不动分享语义。
- 真实窗口回归：根代理接线后跑一次主菜单点击→文件选择（可用环境变量注入夹具路径做无头证据）。