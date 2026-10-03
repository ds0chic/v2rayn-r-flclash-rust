# FIX-05 — 扫描图片中的二维码→导入

状态：`implemented`（图片选择/解码/取消/错误/导入全链已实现；真 `bridge_api.dll` + 临时 data dir 的扫描→落库→同目录重开已在本机 `flutter test` 通过；主菜单接线由根代理完成前入口尚不可达；未做原版实机双窗口逐事件对照，故不写 `verified`）。

任务 ID：FIX-05

本次唯一用户流程：主菜单「配置项 → 扫描图片中的二维码」→ 文件选择 → 解码 QR → 文本进入既有导入管线（`subs.importFromText` + `persistImportedProfiles`）→ 节点落库 → 重开仍在。屏幕扫描（ACT-MAIN-017）与分享窗口分开卡，本卡不做屏幕扫描，登记 FIX-05B。

前置任务及已验证证据：冻结 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；开始应用 HEAD `b6388215bb5622d28b89622ccb66da53175d3036`（本卡只读；工作树含其它代理并行改动 `crates/**`、`status_bar_view.dart`，均未触碰、未回退）。审查结论见 `docs/evidence/parity-review-2026-10-03/repair-queue.md:23`（FIX-05）、`profiles-report.md` PR-14（:41）与 PR-24（:158）、`root-report.md` ROOT-02（:35）与入口表（:18-19）、`all-items.json` 的 `ACT-MAIN-018` / `F-IMPORT-006` / `F-PROFILE-013`。复现证据：`ui-run-04/observations.json` 中图片扫码实际打开粘贴文本窗口。

对应 feature / field / action / layout ID：`F-IMPORT-006`、`F-PROFILE-013`、`ACT-MAIN-018`。

必读上游文件、符号和固定 commit：
- `work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967/v2rayN/ServiceLib/ViewModels/MainWindowViewModel.cs:522-537`：`AddServerViaImageAsync`（文件选择）→ `ScanImageResult(fileName)`：文件名空直接 `return`（取消静默）；`QRCodeUtils.ParseBarcode(fileName)`；结果空 → `ResUI.NoValidQRcodeFound`；否则 `ConfigHandler.AddBatchServers(_config, result, _config.SubIndexId, false)`，`ret>0` → `RefreshSubscriptions` + `RefreshServersDispatcherAsync` + `ResUI.SuccessfullyImportedServerViaScan`，否则 `OperationFailed`（:539-559）。
- `MainWindow.xaml.cs:110-118`：`BrowseImageFileInteraction` 文件对话框 `"PNG|*.png|All|*.*"`，取消 `SetOutput(null)`；`MainWindow.xaml.cs:226-228`：`Ctrl+S` → `ScanScreenTaskAsync`（屏幕扫描，不是图片扫描）。
- `ServiceLib/Common/QRCodeUtils.cs:53-124`：`ParseBarcode(fileName)` 文件不存在返回 null；解码失败 try/catch 后返回 null；`ReaderBarcode` 先正向解码，失败后水平翻转重试（镜像码）。
- `ServiceLib/Resx/ResUI.zh-Hans.resx:252/291`：`NoValidQRcodeFound=扫描完成，未发现有效二维码`；`SuccessfullyImportedServerViaScan=扫描导入分享链接成功`。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`file_selector` 的 `openFile`（图片扩展名 `png/jpg/jpeg/bmp/gif/webp`）；字节不可读/为空不崩溃。
- 输出：`decodeQrImage(bytes)` 返回 QR 文本（含水平镜像重试）；成功路径复用 `importShareText` → `bridge.importFromText(deduplicate:true)` → `persistImportedProfiles`（`saveImportedProfile`，FIX-04）→ `profilesController.reload()`。
- 错误：打开失败「打开图片失败，请重试」；空图片「所选图片为空，请重新选择」；无码/非 QR「扫描完成，未发现有效二维码（请选择含清晰二维码的 PNG/JPG 图片）」（保留上游原文并补可操作指引）；文本不是分享链接/订阅 URL 时沿用既有 `describeImportFailure` / `_offerAddSubscription` 分类提示。
- 取消：选择器返回 null → 直接 return，无提示、不改状态（对齐上游 `ScanImageResult` 文件名空 return）。
- 权限：仅本机 UI + FRB/Rust/SQLite；不启动内核、不写系统代理/注册表/路由/TUN、不监听端口（未使用 10808）。
- 持久化：SQLite `ProfileItem`，经既有 `Engine::save_imported_profile`；重开同一 data dir 节点仍在。
- 生效：导入后刷新节点表（复用既有成功语义），无新增 IPC / 无 FRB 生成。

允许修改的模块（本卡实际改动）：`apps/desktop/pubspec.yaml`、`apps/desktop/lib/features/subs/**`（新增 `scan_image_qr.dart`；`subs_actions.dart` 抽出公共 `importShareText`）、`apps/desktop/test/**`、`docs/evidence/UX-PARITY-FIX-05/**`、本卡、`compat/features.yaml`（仅 notes 追加）。未改 `main_shell.dart`（根代理接线）、`sub_share_dialog.dart`、`features/profiles/**` 及其它禁止目录。

Ctrl+S 结论：上游 `MainWindow.xaml.cs:226` 将 Ctrl+S 绑定到 `ScanScreenTaskAsync()`（隐藏窗口→`QRCodeWindowsUtils.CaptureScreen`→`ScanScreenResult`→恢复窗口），即**屏幕扫描 ACT-MAIN-017**，图片扫描 ACT-MAIN-018 没有快捷键。因此本卡不改 Ctrl+S，也不把图片扫描接到 Ctrl+S；`main_menu.dart` 中 ACT-MAIN-017 的 `Ctrl+S` 快捷标签与上游一致。屏幕扫描按验收合同登记 **FIX-05B**，可复用本卡导出的 `importShareText`。

禁止改变的已有行为（本卡实际未动）：`main_shell`、菜单结构/条目/快捷键标签、分享窗口 `shareProfilesQr`/`sub_share_dialog` 语义、编辑器与其它 features；不删入口、不降分母、不伪造落库结果。

测试夹具和原版预期：测试内运行时生成合成 QR PNG（`qr` 包矩阵 → `image` 包编码，含 4 模块 quiet zone；另有水平镜像夹具），文本为合成 `vless://…@127.0.0.1:11998…#fixed05-scan-node`；不联网、不下载、不含真实订阅 URL/凭据。原版预期：取消静默；无码只报 `NoValidQRcodeFound`；解码文本走 `AddBatchServers` 落库并刷新。

本次必须通过的命令/真实场景（实际结果）：
- `flutter pub add file_selector image zxing2`：成功（file_selector 1.1.0、image 4.10.1、zxing2 0.2.4；另 `dev:qr` 3.0.2 供夹具；自动重生成 Windows 插件注册文件）。
- `dart format --output=none --set-exit-if-changed` 四个改动文件：0 changed。
- `flutter analyze`：No issues found。
- `flutter test test/scan_image_qr_test.dart`：8/8 通过（正向/镜像解码、非 QR/空/垃圾字节、导入落库+重开、取消、空图、非 QR、picker 异常）。
- `flutter test test/ux_parity_fix05_scan_test.dart`：2/2 通过（真 `bridge_api.dll` + 临时 data dir：扫描→落库→`initEngine` 同目录重开仍在；取消/空图不改库）。
- 回归：`t09_import_export` / `t21e_import_ux` / `t21e_import_real_bridge` / `ux_parity_fix04_inner`：11/11 通过（`importShareText` 抽取后粘贴导入语义未变）。

证据文件位置：`docs/evidence/UX-PARITY-FIX-05/`（`README.md`、`fix05-tests.log`、`import-regression.log`、`analyze.log`、`format.log`）。

完成条件：图片文件选择/取消/解码/错误/导入完整；真桥落库并重开存在；Ctrl+S 归属有上游对照结论；分享窗口语义未动；主菜单接线补丁已给出。主菜单入口在根代理接线前不可达，且未做原版实机双窗口逐事件对照，故保持 `implemented`。

发现接口缺口时的处理（登记，不自行削减需求）：
- FIX-05B（屏幕扫描）：主窗口隐藏→截图→恢复（含所有错误/取消分支恢复窗口）需要 app 窗口层配合；本卡不实现，建议根代理作为独立卡接线，复用 `scan_image_qr.dart` 的 `importShareText` 与 `decodeQrImage`。
- PR-24（分享窗口只读分享文本/400x400）与 FIX-05B 一起留在分享窗口卡；本卡保留 `shareProfilesQr` 与 `sub_share_dialog` 现有语义。
- 主菜单接线（根代理最小补丁）：
  ```dart
  // main_shell.dart 顶部 import 增加
  import 'package:v2rayn_desktop/features/subs/scan_image_qr.dart';

  // _onMenuAction 内 ACT-MAIN-018 分支替换
  case 'ACT-MAIN-018':
    scanImageQr(context, ref);
  ```
  `Ctrl+S` 的 `shareProfilesQr` 分支（:158-159）保持不变，属 ACT-MAIN-017，由 FIX-05B 处理；`importFromTextDialog` 仍由粘贴文本路径使用（`importShareText` 默认 sourceLabel 不变）。

本轮实际结果：新增 `apps/desktop/lib/features/subs/scan_image_qr.dart`（`scanImageQr` / `decodeQrImage` / `pickQrImageFile` / 可注入 `QrImagePicker`）；`subs_actions.dart` 抽出公共 `importShareText(context, ref, text, {sourceLabel})` 并参数化 `_importSuccessToast`，`importFromTextDialog` 改用它（粘贴路径行为不变）；`pubspec.yaml` 增加 file_selector/image/zxing2 与 dev qr；测试 `scan_image_qr_test.dart`（8 项）与真桥 `ux_parity_fix05_scan_test.dart`（2 项）；`flutter analyze` 0 issue。未运行：`flutter build windows`、原版实机对照、真实文件对话框人工点击（选择器默认实现未在真窗口走）、屏幕扫描。