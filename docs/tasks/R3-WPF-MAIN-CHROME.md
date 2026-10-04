# R3-WPF-MAIN-CHROME — 主窗口可见结构对齐（M2/M3/M5/M8）

状态：`implemented`（代码已落地，widget 测试与 `flutter analyze` 通过；未做原版实机双窗口逐事件对照、未跑 `flutter build windows`，故不写 `verified`）。

任务 ID：R3-WPF-MAIN-CHROME

本次唯一用户流程：打开主窗口 → 阅读顶部菜单/工具栏、节点表列头与底部状态栏，对照冻结原版 v2rayN 7.25.4 的可见结构（不改变任何运行时/代理/TUN 行为）。

前置任务及已验证证据：冻结上游 v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`；对照基线 `docs/evidence/recheck-fixes/R3-WPF-COMPARE/README.md`（M1–M8）与截图 `original-main.png` vs `rc-main.png`；菜单坐标/关闭/Esc 既有契约 `docs/evidence/context-menu-review-2026-10-03/README.md`（17 根条目 / 4 分隔线）。

必读上游文件、符号和固定 commit：
- `v2rayN/v2rayN/Views/MainWindow.xaml:41-329`（工具栏/菜单/主题 PopupBox）、`:263-277`（`menuReload`，无 InputGestureText）。
- `v2rayN/v2rayN/Views/ProfilesView.xaml:24-99`（顶部 WrapPanel 入口集）、`:100-114`（`RowHeaderWidth="40"`、`HeadersVisibility="All"`）、`:281-374`（14 列 `MyDGTextColumn.ExName`）。
- `v2rayN/v2rayN/Views/StatusBarView.xaml:17-104`（DockPanel 分区：左 本地/局域网/Tun/系统代理/路由，右 代理/直连速度，中 运行信息）。
- `ServiceLib/Resx/ResUI.zh-Hans.resx`：`menuReload=重启服务`、`menuSystemProxy*`、`TbEnableTunAs=启用 Tun`。

对应 feature / action / layout ID：`LAY-PROFILES-002`（列集/RowHeaderWidth）、`LAY-PROFILES-004`（分隔线非回归）、`LAY-MAIN-004`（顶级菜单）、`LAY-STATUSBAR-001`（状态栏）、`ACT-MAIN-035`（重启服务）。

本次修复合同与结论：
1. `#` 索引列（M3）：上游无 `#` 列，只有 `RowHeaderWidth="40"` 的无标题行头。RC 的行头表头曾渲染 `#` 文本；已移除，并新增 `kProfilesRowHeaderWidth = 40`（对齐上游宽度），行号仍在行头内。
2. 工具栏按钮集（M4，涉及 `main_shell.dart`）：上游顶部只有主题 PopupBox（`MainWindow.xaml:314`）与隐藏的 `btnNewUpdate`；`应用/停止`（net_host 运行控制）、`布局选择`、主题切换按钮为 RC 增强。`主题`位置与上游一致（右上）；`应用/停止`、`布局`登记为有意增强（不静默保留结构偏离），精确补丁见 `docs/evidence/recheck-fixes/R3-WPF-MAIN-CHROME/README.md`。
3. 状态栏文案与分区（M5）：`入站`→`本地:`、`LAN`→`局域网:`、`TUN`→`启用 Tun`（对齐 `ResUI`）；系统代理与路由下拉保留上游模式文案；`今日`/速度/运行信息等为 RC 增强，位置贴近上游（右侧速度）。
4. 子菜单分隔线（M8）：菜单模型新增 `AppMenuEntry.separatorAfter`，按 `MainWindow.xaml` 标记 配置项 3 / 订阅分组 1 / 设置 2 / 帮助 1；渲染留在 `main_shell._menuChildren`（补丁）。已修好的节点右键菜单契约不回归（`context_menu_model_test` 断言 17 根 / 4 分隔线仍绿）。
5. 主题（M1）：原版固定 Light、RC 跟随 system 属有意增强，登记不强行回退；高 DPI/托盘视觉登记为未验证。

允许修改的模块：`apps/desktop/lib/features/profiles/{profiles_models.dart,profiles_table.dart}`、`apps/desktop/lib/app/shell/status_bar_view.dart`、`apps/desktop/lib/app/menu/main_menu.dart`、`apps/desktop/test/**`、本卡、`docs/evidence/recheck-fixes/R3-WPF-MAIN-CHROME/**`、`compat/layouts.yaml`（仅追加 notes）。未修改 `main_shell.dart`（给出补丁），未改 `app_theme.dart`。

禁止改变的已有行为：菜单根顺序/条目、节点右键菜单坐标/关闭/Esc、17 根/4 分隔线契约、代理端口 10808、系统代理/路由/TUN/注册表；不删入口或降分母。

本次必须通过的命令/真实场景：
- `dart format`（仅改动文件）
- `flutter analyze`
- `flutter test test/r3_wpf_main_chrome_test.dart test/t17_menu_structure_test.dart test/profiles_models_test.dart test/t13_statusbar_test.dart test/context_menu_model_test.dart`
- `flutter test test/t05_shell_chrome_test.dart`
- `flutter test test/t21e_responsive_test.dart`（首次 flutter_tester "did not complete" 为已知抖动，重试通过）

证据文件位置：`docs/evidence/recheck-fixes/R3-WPF-MAIN-CHROME/`（`README.md`、`test-run.log`）。

完成条件：列集/行头、菜单命名与分隔线、状态栏文案的 widget 断言通过；既有菜单/状态栏/列模型测试修正后仍绿；`flutter analyze` 无问题。未做原版实机与 release 构建，故保持 `implemented`。

发现接口缺口时的处理：登记阻塞与建议，不自行削减需求。
- 接口缺口（登记）：`main_shell.dart` 未渲染 `separatorAfter`，需根代理落地补丁；`app_theme.dart` 的 `tableHandleWidth=48` 未改（不在允许范围），RC 行头宽度由 `kProfilesRowHeaderWidth=40` 局部接管。
- 接口缺口（登记）：高 DPI/托盘视觉、`V2RAYN_R_THEME=light` 稳定证据钩子仍缺（承接 R3-WPF-COMPARE 边界）。

本轮实际结果：见 `docs/evidence/recheck-fixes/R3-WPF-MAIN-CHROME/README.md`。门禁：`dart format` 3 文件重排；`flutter analyze` No issues；6 个测试文件 22 用例通过（含 t05 渲染整壳断言 `本地:`/`局域网:`/`启用 Tun`/无 `#`）。
