# R3-VISUAL-DPI-TRAY — 高 DPI/托盘视觉验收 + 四状态托盘图标资产

日期：2026-10-05。起始 repo HEAD `8cba2c6`。冻结原版 v2rayN 7.25.4 `7d6a967c18c697f28dc6917122ed3a4993fcf336`（UP）。本卡只读 `work/`、`outputs/`，未改业务功能，未 commit。所有测试为合成数据；未占用/修改 `127.0.0.1:10808`，未改系统代理/注册表/路由/TUN，未改宿主显示缩放/DPI，只停止本脚本启动的 PID 及后代。

注意：开工时工作树已含另一子代理对 `lib/main.dart`、`features/routing/routing_windows.dart`、`windows/runner/routing_window*` 等的并发改动（非本卡改动）；本卡未触碰这些文件。发布构建在并发工作树上进行，仅用于本卡截图。

## 1. 四状态托盘图标资产

- 生成脚本 `make_tray_icons.ps1`（PowerShell + `System.Drawing`）程序化绘制 128px 超采样后降采样，写出经典 DIB 多尺寸 `.ico`（16/32/48，32bpp，非 PNG 压缩，`LoadImage` 可直接加载）。
- 形状/颜色与会话状态一一对应（见 `tray-icons-preview.png`，从左到右）：
  | 状态 | 文件 | 图形 |
  | --- | --- | --- |
  | `normal` | `tray_icon.ico` | 灰色圆环（代理关、核心停） |
  | `coreRunning` | `tray_icon_core.ico` | 绿色圆盘 + 白色播放三角（核心运行） |
  | `proxyActive` | `tray_icon_proxy.ico` | 蓝色圆角方块 + 白色上箭头（系统代理自动配置） |
  | `proxyPac` | `tray_icon_pac.ico` | 橙色菱形 + 白色圆点（PAC 服务） |
- 落地 `apps/desktop/assets/tray/`，`pubspec.yaml` 声明 `assets/tray/`；`desktop_integration.dart` 的 `_trayAssetsDir` 指向 `data/flutter_assets/assets/tray/`，`trayIconResourceName` 指向真实文件，缺失时回退 `tray_icon.ico` → exe 同级 `app_icon.ico`。未改 `trayIconStatus` 四态映射语义。
- 构建后打包确认：`build/windows/x64/runner/Release/data/flutter_assets/assets/tray/` 含四个 `.ico`（各 15086 字节）。

## 2. DPI widget 验收矩阵（不改宿主）

用 `tester.view.devicePixelRatio`（1.0/1.25/1.5/2.0）与 `tester.view.physicalSize`（逻辑尺寸 × 缩放）模拟，逻辑窗口保持 1280×800 与 800×600（`AppWindowMetrics.minWidth/minHeight`）：

| 覆盖对象 | 测试 | 断言 |
| --- | --- | --- |
| 主窗口（三布局：垂直/水平/标签） | `r3_visual_dpi_shell_test.dart` | 无 `RenderFlex` 溢出异常；chrome（表头/工具栏/菜单/状态栏，按 key 前缀）不 `didExceedMaxLines`；800×600 下 `layout-selector/theme-toggle/filter-field/status-proxy-speed/status-inbound/tun-toggle` 命中可达 |
| 设置窗口 5 页 | `r3_visual_dpi_windows_test.dart` | 逐 Tab 无溢出/截断 |
| 路由窗口 | `r3_visual_dpi_windows_test.dart` | 无溢出/截断 |
| 节点选择器 `showNodePicker` | `r3_visual_dpi_picker_test.dart` | 无溢出/截断 |
| 图标资产 | `r3_visual_tray_assets_test.dart` | 四文件存在、ICO type=1、含 16/32 尺寸、四者非字节相同、pubspec 声明 `assets/tray/` |

节点表数据单元（备注/地址）按设计 `TextOverflow.ellipsis`，不纳入「chrome 不截断」，只校验表头与状态栏等 chrome；未发现需要修复的溢出/截断，故未改任何布局文件。

## 3. 真机窗口截图（宿主 DPI 96 = 100%）

隔离数据目录 `%TEMP%\opencode\r3-visual-data`，预置 `guiNConfig.json`：`SystemProxyItem.SysProxyType=2`（Unchanged）、`Inbound.LocalPort=11808`、无 active 节点 → 启动不写系统代理、不监听 10808、不启动内核。`V2RAYN_R_THEME=light|dark`。

| 项 | 值 |
| --- | --- |
| 宿主 DPI | `GetDpiForSystem()` = 96；屏幕 1920×1080；窗口 `GetDpiForWindow()` = 96 |
| 主窗口 | 标题 `v2rayN`，rect `40,40,1240,840` |
| 浅色 | `main-light.png`（窗口）、`desktop-light.png`（全屏含任务栏） |
| 深色 | `main-dark.png`、`desktop-dark.png` |
| 差异 | 浅色：菜单/表头/空态为浅灰背景、深色文字；深色：整体深色背景、浅色文字/描边。状态栏均显示 `系统代理:不改变系统代理(未启用)`，与预置 Unchanged 一致。 |

托盘通知区域：
- 应用图标默认在 Windows 11 溢出浮出层；点击任务栏 `^` 展开后可见灰色圆环图标 → `desktop-overflow-light.png`。
- 悬停该图标显示 tooltip `v2rayN-R` → `desktop-tray-tooltip-light.png`。
- 右键该图标弹出应用托盘菜单 → `desktop-tray-menu-light.png`，条目与 `tray_menu_model.dart` 一致：`清除系统代理 / 自动配置系统代理 / ✓ 不改变系统代理 / Pac 模式 / 路由 > / 节点 > / 从剪贴板导入分享链接 / 扫描屏幕上的二维码 / 更新订阅(不通过代理) / 更新订阅(通过代理) / 复制代理命令到剪贴板 / 退出`；当前 `不改变系统代理` 有勾选，与预置模式一致。全部过程未点击任何菜单项，未触发代理/核心动作。

探针：`probe-light.json`、`probe-dark.json`。

## 4. 实际命令与结果

| 命令 | 结果 |
| --- | --- |
| `pwsh -File make_tray_icons.ps1` | 写出 4 个 `.ico`（16/32/48） |
| `dart format --output=none --set-exit-if-changed <changed>` | 6 文件，0 changed |
| `flutter analyze` | `No issues found!` |
| `flutter test test/r3_visual_tray_assets_test.dart` | 4/4 passed |
| `flutter test test/r3_visual_dpi_shell_test.dart` | 1/1 passed（3s） |
| `flutter test test/r3_visual_dpi_windows_test.dart` | 1/1 passed（3s） |
| `flutter test test/r3_visual_dpi_picker_test.dart` | 1/1 passed |
| `flutter test test/r3_09_tray_icon_today_test.dart test/recheck_rr08_tray_sync_test.dart test/fix15_tray_pac_test.dart` | 20/20 passed（无回归） |
| `flutter build windows --release` | 成功（35.8s），`assets/tray/*.ico` 打包 |
| `capture_visual.ps1 light/dark` | 成功，见上表 |
| `capture_tray_overflow.ps1` / `capture_tray_menu.ps1` | 成功，见上节 |

（注：把四个新测试文件放进同一次 `flutter test` 调用时命中已知的 `flutter_tester` 原生资源泄漏/flake——shell 与 windows 用例 `did not complete`；逐文件运行均通过，故以逐文件结果为准，与仓库既有 `-PerFile` 约定一致。未观察到 exit 79 文本，但行为相同。）

## 5. 上游对照结论

- 四态图标区分系统代理/运行态与 UP `StatusBarViewModel.RefreshIconInteraction` 语义一致；本轮只补齐 UP 托盘图标存在但 RC 未打包资源的缺口，未改 Wave J 映射。
- 托盘菜单顺序/勾选/PAC 可见性与 `StatusBarView.xaml:106-234` 一致；真机右键菜单实测条目与模型逐项吻合。
- 主题差异属 RC 有意增强（跟随持久化/`V2RAYN_R_THEME`），非本轮改动。

## 6. 未完成 / 边界（如实登记）

- 真机仅覆盖宿主 96 DPI；本机为单一 DPI 监视器，125%/150%/200% 真机窗口未能实测，由 widget 矩阵替代（未改宿主显示缩放）。
- 真机仅 `normal`（核心停止）托盘态。`coreRunning`/`proxyActive`/`proxyPac` 真机图标需启动内核或写系统代理，受本卡安全约束未做；仅以资产/映射层测试 + 预览图验证。
- `_trayIconPathFor` 依赖打包 exe 布局，`flutter test` 下必然走回退，无法在 widget 测试中断言“特定状态文件被 `setImage`”；已在任务卡登记为接口缺口。

## 7. 下一步前置

1. 若能申请带 200% 缩放的隔离显示器/VM，或经批准用 `SetProcessDpiAwareness` 之外的显示设置，补 125/150/200% 真机窗口与托盘图标截图。
2. 如需真机验证 `coreRunning`（无害，端口 ≥11808、不写系统代理），需准备可用的合成内核与节点配置并由本卡授权流程启动。
3. 给 `TraySurface` 增加可测的图标路径解析出口，使状态→文件映射可在 widget 测试中直接断言。
