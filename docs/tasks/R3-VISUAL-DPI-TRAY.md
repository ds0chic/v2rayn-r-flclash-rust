# R3-VISUAL-DPI-TRAY — 高 DPI/托盘视觉验收与四状态托盘图标资产

状态：`implemented`（四状态图标资产与声明、100/125/150/200% widget 验收矩阵、宿主 96 DPI 真机浅/深主题截图与托盘图标/菜单实测均已完成；更高 DPI 的真机窗口与核心运行/代理/PAC 三态真机图标未实测，登记为边界）。

任务 ID：R3-VISUAL-DPI-TRAY

本次唯一用户流程：完成 `remaining-boundaries-2026-10-05.md` 第 6 项的本地视觉验收——(a) 补齐四状态托盘图标 `.ico` 资源并在 `pubspec.yaml` 声明，`trayIconResourceName` 指向真实资产且无资产时仍回退不崩；(b) 用 widget 测试以 `tester.view.devicePixelRatio` + 物理尺寸模拟 100%/125%/150%/200% 缩放，验证主窗口/设置窗口 5 页/路由窗口/节点选择器无溢出、按钮可达、文本不截断；(c) 在宿主当前 DPI 下用隔离数据目录启动发布构建截图（主窗口 + 托盘通知区域含应用图标）；(d) 浅/深主题各一轮截图并记录差异。

前置任务及已验证证据：冻结原版 v2rayN 7.25.4 commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；起始 repo HEAD `8cba2c6`。`remaining-boundaries-2026-10-05.md:12` 登记本项为本地验收工作。上游四状态映射与「仓库无 ico 资产」登记见 `docs/evidence/recheck-fixes/R3-PROF-11-R3-09/README.md:46,52` 与 `observations.json`。WPF 三窗口 96 DPI 对照见 `docs/evidence/recheck-fixes/R3-WPF-COMPARE/README.md`。

对应 feature / field / action / layout ID：`ACT-TRAY-014`、`ACT-TRAY-015`（tray domain）；Wave J `TrayIconStatus` 四态（`tray_menu_model.dart:261-307`）；`pubspec.yaml` assets；`AppWindowMetrics`（LAY-MAIN-002）。

必读上游文件、符号和固定 commit：`work/research-v2rayn/source-latest/2dust-v2rayN-7d6a967` 的 `v2rayN/ServiceLib/ViewModels/StatusBarViewModel.cs`（`RefreshIconInteraction` / `RefreshIcon`，按系统代理状态与运行态切换任务栏图标）、`v2rayN/v2rayN/Views/StatusBarView.xaml:106-234`（托盘 ContextMenu 顺序与 PAC 可见性）、`v2rayN/v2rayN.Desktop/App.axaml`（Avalonia `TrayIcon`）。

输入、输出、错误、取消、权限、持久化及生效语义：
- 输入：`TrayReadModel{desiredMode, coreRunning, pacRunning, ...}` → `trayIconStatus()` → `TrayIconStatus{normal, coreRunning, proxyActive, proxyPac}`。
- 输出：`trayIconResourceName(status)` 返回 `tray_icon.ico` / `tray_icon_core.ico` / `tray_icon_proxy.ico` / `tray_icon_pac.ico`；`DesktopIntegration._trayIconPathFor` 在 `data/flutter_assets/assets/tray/` 下解析，缺失时回退 `tray_icon.ico`，再回退 exe 同级 `app_icon.ico`。
- 错误：任何图标文件缺失或插件 `setImage` 失败只 `debugPrint`，不崩溃、不伪造成功。
- 取消：不适用（无长任务）。
- 权限：仅本机 UI + `system_tray` 插件；不启动内核、不写系统代理/注册表/路由/TUN、不监听 10808。
- 持久化：无（图标状态为运行态派生，不落库）。
- 生效：托盘初始化与每次 read-model 变化时 `applyIcon`，状态去重。

允许修改的模块：`apps/desktop/assets/tray/**`（新增）、`apps/desktop/pubspec.yaml`、`apps/desktop/lib/app/shell/{tray_menu_model.dart,desktop_integration.dart}`、`apps/desktop/test/**`、`docs/tasks/R3-VISUAL-DPI-TRAY.md`、`docs/evidence/recheck-fixes/R3-VISUAL-DPI-TRAY/**`、`compat/actions.yaml`（仅追加注释）。未改任何业务功能、`main_shell.dart`、`app.dart`、两处 `frb_generated`、`lib/bridge/api/**`、`features/settings/**`、`features/subs|runtime|monitor|update|backup/**`、`crates/**`、`services/**`。

禁止改变的已有行为：Wave J 四状态映射语义（`trayIconStatus` 判定顺序与 `TrayIconStatus` 成员）、托盘菜单条目/顺序/勾选/子菜单、`trayIconFallbackName` 语义、内嵌设置/路由窗口形态；不改业务功能；不降分母、不伪造截图或托盘状态。

测试夹具和原版预期：合成节点/订阅（RFC 5737 文档地址、假 UUID），无网络、无内核；真机截图用隔离 `V2RAYN_R_DATA_DIR`，预置 `SystemProxyItem.SysProxyType=2`（Unchanged）与 `Inbound.LocalPort=11808`。原版预期：托盘图标随代理/运行态切换；托盘菜单按 `StatusBarView.xaml` 顺序且当前模式有勾选。

本次必须通过的命令/真实场景：
- `dart format --output=none --set-exit-if-changed lib test`
- `flutter analyze`
- `flutter test test/r3_visual_tray_assets_test.dart test/r3_visual_dpi_shell_test.dart test/r3_visual_dpi_windows_test.dart test/r3_visual_dpi_picker_test.dart`
- `flutter test test/r3_09_tray_icon_today_test.dart test/recheck_rr08_tray_sync_test.dart test/fix15_tray_pac_test.dart`（回归）
- `flutter build windows --release`
- 真机：`capture_visual.ps1 -Theme light|dark`、`capture_tray_overflow.ps1`、`capture_tray_menu.ps1`（隔离数据目录，宿主 DPI 96）。

证据文件位置：`docs/evidence/recheck-fixes/R3-VISUAL-DPI-TRAY/`（`README.md`、`observations.json`、`make_tray_icons.ps1`、`capture_visual.ps1`、`capture_tray_overflow.ps1`、`capture_tray_menu.ps1`、`tray-icons-preview.png`、`main-light.png`、`main-dark.png`、`desktop-light.png`、`desktop-dark.png`、`desktop-overflow-light.png`、`desktop-tray-tooltip-light.png`、`desktop-tray-menu-light.png`、`probe-light.json`、`probe-dark.json`）。

完成条件：四状态图标为可辨识、多尺寸、已打包且被 `trayIconResourceName` 指向；无资产时 fallback 不崩；DPI 矩阵 widget 测试覆盖主窗口（三布局）/设置 5 页/路由/选择器无溢出且 chrome 不截断、按钮可达；宿主 DPI 真机主窗口浅/深主题截图与托盘图标/菜单证据齐全；门禁通过；未实测项如实登记为边界。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。

- 接口缺口（登记）：`_trayIconPathFor` 依赖 exe 布局 `data/flutter_assets/assets/tray/`，在 `flutter test`（无打包）下必然走回退；无法用 widget 测试验证“特定状态文件被真正 `setImage`”。建议后续给 `TraySurface.applyIcon` 注入可测的解析结果（返回值/回调）。
- 边界（登记）：真机仅覆盖宿主 96 DPI；125%/150%/200% 真机窗口未测（本机仅一个 DPI 监视器），由 widget 矩阵替代。真机仅 `normal`（核心停止）托盘态；`coreRunning`/`proxyActive`/`proxyPac` 真机图标需启动内核或写系统代理，受安全约束未做，仅资源/映射层验证。

本轮实际结果：见 `docs/evidence/recheck-fixes/R3-VISUAL-DPI-TRAY/README.md`。
