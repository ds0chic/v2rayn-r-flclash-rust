# FIX-05B — 扫描屏幕上的二维码→导入

状态：`implemented`（隐藏→截图→解码→恢复→导入全链实现；widget/单元测试用可注入截图源与窗口控制覆盖成功/取消/无码/异常四分支并断言 hide/show 序列，4/4 通过；真实窗口集成测试已写好但未在本卡运行，待根代理统一跑；主菜单 ACT-MAIN-017 与 Ctrl+S 接线补丁已给出、由根代理执行，接线前入口不可达，故不写 `verified`）。

任务 ID：FIX-05B（repair-queue.md:23 拆出的屏幕扫描半边；PR-14 屏幕流程部分）

本次唯一用户流程：主菜单「配置项 → 扫描屏幕上的二维码」或 `Ctrl+S` → 隐藏主窗口 → 截图 → 解码 QR → 恢复窗口 → 文本进入既有导入管线（`importShareText` → `importFromText` + `persistImportedProfiles`）→ 节点落库。分享窗口（PR-24）另卡，本卡不动。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `066d8d3`（本卡只读审查；FIX-05 已实现图片扫码半边并导出 `decodeQrImage`/`importShareText`）。审查结论见 `docs/evidence/parity-review-2026-10-03/repair-queue.md:23`（FIX-05「屏幕扫描和分享窗口分开卡」）。复现证据：`ui-run-04/observations.json` 与 FIX-05 notes 记录 ACT-MAIN-017 当前错接分享窗口。

对应 feature / field / action / layout ID：`F-IMPORT-006`、`ACT-MAIN-017`。

必读上游文件、符号和固定 commit：
- `v2rayN/v2rayN/Views/MainWindow.xaml.cs:265-276`：`ScanScreenTaskAsync` = `ShowHideWindow(false)` → `QRCodeWindowsUtils.CaptureScreen(window)` → `ViewModel.ScanScreenResult(bytes)` → `ShowHideWindow(true)`（原版只在成功路径恢复；本卡按验收合同改为 finally 全分支恢复）。
- `v2rayN/v2rayN/Views/MainWindow.xaml.cs:226-228`：`Ctrl+S` → `ScanScreenTaskAsync`。
- `v2rayN/v2rayN/Common/QRCodeWindowsUtils.cs:27-49`：`CaptureScreen` 用 `Graphics.CopyFromScreen` 抓工作区，异常返回 null。
- `v2rayN/ServiceLib/ViewModels/MainWindowViewModel.cs:516` `ScanScreenResult(bytes)`：解码→`AddBatchServers`，无码 `ResUI.NoValidQRcodeFound=扫描完成，未发现有效二维码`。
- 复用 FIX-05：`apps/desktop/lib/features/subs/scan_image_qr.dart` 的 `decodeQrImage`（zxing2+image，含水平镜像重试）、`subs_actions.dart` 的 `importShareText`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：注入式 `ScreenCapture`（默认真实截图，返回 PNG 字节）。
- 输出：`decodeQrImage(bytes)` → `importShareText(..., sourceLabel:'屏幕扫描')`，成功提示「已从屏幕扫描导入 N 个节点」。
- 错误：截图抛异常/空图→「屏幕截图失败，请重试」；无码→「扫描完成，未发现有效二维码（请让二维码清晰完整地显示在屏幕上后重试）」；非分享文本沿用既有 `describeImportFailure`/订阅分类提示。
- 取消：注入截图返回 null→静默、不提示、不改状态（对齐 FIX-05 图片选择器取消语义）。
- 恢复：`hide()` 后无论成功/取消/无码/异常都在 `finally` 调用 `show()`（真实实现为 `windowManager.show()+focus()`）。
- 权限：仅本机 UI + FFI 屏幕读取 + FRB/SQLite；不启动内核、不写系统代理/注册表/路由/TUN、不监听端口、不触碰 10808。
- 持久化：复用 `saveImportedProfile`（FIX-04）；重开同一 data dir 节点仍在（由 FIX-05 真桥测试已覆盖同一导入管线）。
- 生效：导入后刷新节点表（`profilesController.reload()`），无新增 IPC / 无 FRB 生成。

允许修改的模块（本卡实际改动）：`apps/desktop/lib/features/subs/scan_screen_qr.dart`（新增）、`apps/desktop/pubspec.yaml`（+`ffi`）、`apps/desktop/test/scan_screen_qr_test.dart`（新增）、`apps/desktop/integration_test/ux_parity_fix05b_screen_test.dart`（新增）、`docs/evidence/UX-PARITY-FIX-05B/**`、本卡、`compat/actions.yaml`（仅追加 notes）。未改 `main_shell.dart`、`app.dart`、两处 `frb_generated`、`sub_share_dialog.dart`、`subs_actions.dart`、`features/settings|profiles|runtime|monitor|update|backup/**`。

截图方案（自选并说明）：**纯 Dart FFI 调 Win32 GDI（`user32!GetDC/GetSystemMetrics` + `gdi32!CreateCompatibleDC/CreateCompatibleBitmap/SelectObject/BitBlt/GetDIBits`）**，top-down 32bpp DIB → `image` 包 `Image.fromBytes(order:bgra)` → `encodePng`。理由：不新增原生插件构建、不加 Rust bridge、**不动两处 `frb_generated`**（因此本卡无 FRB 重生成需求）；Windows SDK 自带 user32/gdi32。窗口隐藏/恢复用已有依赖 `window_manager`。截图仅在 `Platform.isWindows` 生效，失败抛 `ScreenCaptureException`（绝不返回 null，避免把失败误当取消静默）。

main_shell 接线补丁（根代理执行；本卡不改 main_shell）：
```dart
// main_shell.dart 顶部 import 增加（scan_image_qr.dart 已存在）
import 'package:v2rayn_desktop/features/subs/scan_screen_qr.dart';

// 1) Ctrl+S 绑定（当前 :159-160 → shareProfilesQr，改为：）
const SingleActivator(LogicalKeyboardKey.keyS, control: true): () =>
    _guarded(ref, () => scanScreenQr(context, ref)),

// 2) _onMenuAction 内 ACT-MAIN-017 分支（当前 :329-330 standalone shareProfilesQr，改为：）
case 'ACT-MAIN-017':
  scanScreenQr(context, ref);
```
`ACT-MAIN-018` 分支（:331-332 `scanImageQr`）与分享窗口 `shareProfilesQr`/`sub_share_dialog` 语义均保持不变。

禁止改变的已有行为：`main_shell`、菜单结构/条目/快捷键标签、分享窗口 `shareProfilesQr`/`sub_share_dialog`、图片扫码 `scan_image_qr.dart`、`importShareText` 默认 `sourceLabel='剪贴板'`；不删入口、不降分母、不伪造落库结果。

测试夹具和原版预期：运行期生成合成 QR PNG（`qr` 矩阵 → `image` 编码，4 模块 quiet zone；文本合成 `vless://…@127.0.0.1:11998…#fixed05b-screen-node`）+ 纯白无码 PNG；不联网、不含真实订阅 URL/凭据。原版预期：屏幕无码只报 `NoValidQRcodeFound`；解码文本走 `AddBatchServers` 落库刷新。

本次必须通过的命令/真实场景（实际结果）：
- `flutter pub get`：成功（`ffi` 由传递依赖升为直接依赖 2.2.0）。
- `dart format <3 文件>`：3 files，1 changed（已格式化）。
- `flutter analyze <3 文件>`：No issues found。
- `flutter test test/scan_screen_qr_test.dart`：4/4 通过（成功并断言 hide→show+导入；取消静默且恢复；无码可操作提示；异常恢复并提示）。
- 未运行：`flutter test integration_test/ux_parity_fix05b_screen_test.dart -d windows`（真实截图+真实隐藏/恢复；避免与并行子代理的 `features/settings` 改动同时触发 windows 构建，待根代理统一跑）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-05B/`（`README.md`、`analyze.log`、`format.log`、`scan-screen-qr-test.log`）。

完成条件：屏幕流程隐藏→截图→解码→恢复→导入完整；全部错误/取消分支恢复窗口；取消不落库不改状态不假成功；无码给可操作提示；分享窗口语义未动；接线补丁已给出。因未做真实窗口集成与主菜单接线前入口不可达，故保持 `implemented`。

发现接口缺口时的处理（登记，不自行削减需求）：
- 真实屏幕内合成 QR 的端到端验证受限：同一进程窗口隐藏后自身 QR 也被隐藏，无法在单测内把 QR 留在屏幕上；需要外部进程/第二个顶层窗口展示 QR 才能做真机对照，登记给根代理的隔离平台矩阵。
- 未新增 Rust bridge / IPC，无需 FRB 重生成。

本轮实际结果：新增 `apps/desktop/lib/features/subs/scan_screen_qr.dart`（`scanScreenQr` / `ScreenCapture` / `ScanWindowControl` / `WindowManagerScanWindowControl` / `capturePrimaryScreenPng` / `ScreenCaptureException`）；`pubspec.yaml` 增加 `ffi: ^2.1.0`；新增 `test/scan_screen_qr_test.dart`（4 项）与 `integration_test/ux_parity_fix05b_screen_test.dart`（真实截图/真实窗口往返，未运行）。未运行：真实窗口集成、release 构建、原版实机双窗口逐事件对照、真实屏幕内展示合成 QR。
