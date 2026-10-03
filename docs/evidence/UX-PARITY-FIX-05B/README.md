# UX-PARITY-FIX-05B 证据

任务：`docs/tasks/FIX-05B.md`（ACT-MAIN-017 扫描屏幕上的二维码→导入）
开始 HEAD：`066d8d3`（工作树干净起点）
状态：`implemented`（widget 全分支通过；真实窗口集成未运行，待根代理；main_shell 接线待根代理）

## 截图方案
纯 Dart FFI → Win32 GDI：`user32!GetSystemMetrics/GetDC` + `gdi32!CreateCompatibleDC/CreateCompatibleBitmap/SelectObject/BitBlt/GetDIBits`（top-down 32bpp BGRA DIB），`image` 包 `Image.fromBytes(order:bgra)` → `encodePng`。窗口隐藏/恢复用已有依赖 `window_manager`。未新增原生插件、未加 Rust bridge、**未改两处 frb_generated**，无 FRB 重生成需求。

## 分支语义
- capture 返回 null → 取消：静默、不改状态。
- capture 抛异常 / 空字节 → `屏幕截图失败，请重试`。
- 图片无可解码 QR → `扫描完成，未发现有效二维码（请让二维码清晰完整地显示在屏幕上后重试）`。
- 成功 → `importShareText(sourceLabel:'屏幕扫描')`（复用 FIX-05 导入管线）。
- 所有分支 `finally` 恢复窗口（真实实现 `windowManager.show()+focus()`）。

## 命令与结果
- `flutter pub get`：成功；`ffi 2.2.0`（传递依赖升为直接依赖）。
- `dart format --output=none --set-exit-if-changed <3 文件>`：0 changed（见 `format.log`；中间一次写格式化见 `format-write.log`）。
- `flutter analyze <3 文件>`：No issues found（见 `analyze.log`）。
- `flutter test test/scan_screen_qr_test.dart`：4/4 通过（见 `scan-screen-qr-test.log`）：
  - success captures, restores the window and imports（断言 hide→show + `已从屏幕扫描导入 1 个节点`）
  - cancelled capture is silent but still restores the window（断言 hide→show，消息不变）
  - image without a QR reports an actionable message（断言 hide→show + `未发现有效二维码`）
  - capture exception restores the window and reports failure（断言 hide→show + `屏幕截图失败`）

## 未运行（登记）
- `flutter test integration_test/ux_parity_fix05b_screen_test.dart -d windows`：真实截图解码 + 真实窗口隐藏/恢复往返；避免与并行子代理的 `features/settings` 改动同时触发 windows 构建，交根代理统一跑。
- `flutter build windows --release`、原版实机双窗口逐事件对照、真实屏幕内展示合成 QR 端到端（同进程窗口隐藏会一并隐藏自身 QR，需要外部顶层窗口）。
- 主菜单「扫描屏幕上的二维码」与 `Ctrl+S` 接线：`main_shell.dart` 由根代理按本卡补丁执行，接线前入口不可达。
