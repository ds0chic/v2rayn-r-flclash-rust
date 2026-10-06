# SP-19 底栏同屏与子窗呈现 — 证据

基线 `3635392`（工作树另有并行卡未提交改动，本卡仅动写锁内文件，不 commit）。
卡状态：identified（实现完成，verified 需真实 800px 高 DPI 截图/正式包验收后由整合者复核）。
manifest `execution-manifest.json` 中 SP-19 仍为 identified：如实，未改（SP-17 同款先例；manifest 正被并行卡修改，避免冲突）。

## 改动（仅写锁内）

- `apps/desktop/lib/app/shell/status_bar_view.dart`
  - 底栏由单条横向 `SingleChildScrollView` 改为双行无滚动条：row A 保持原版
    DockPanel 顺序（两行端口 / 系统代理 180 宽 / 规则模式 / 路由 160 宽 /
    flex 服务摘要 / 右停靠两行速率），row B 收容 TUN 块、今日聚合、详情入口
    与通知 headline；诊断与二级反馈不再加宽主行（UI-05）。
  - 选择器面 `_SelectorFace` 标签 flex + 全局 `_statusText` ellipsis；faces 只
    显示值（与原版 combo 一致， control 名留在 tooltip，菜单列出全部选项）；
    secondary 消息只进详情弹窗。
  - 可用性测试按钮与失败重试/查看按钮收紧为桌面 tap 尺寸（行高不再被撑大）。
- `apps/desktop/lib/shared/theme/app_theme.dart`
  - 新增同屏预算常量（`statusInbound/SysProxy/RuleMode/Routing/Rate/Today/
    NoticeMaxWidth` 等，注释记录实测依据）；`statusBarHeight` 40→80
    （row A ~33 + row B ~40 switch 固有高度 + padding；字体/字号未动）。
- `apps/desktop/lib/features/settings/option_setting_window_entry.dart`
  - `OptionSettingsWindowApp` 新增 presentation 信封
   （`brightness/accentName/fontFamily/fontSize/locale`），第二引擎只渲染共享
    编辑器 body（UI-08）；默认仍为 light，后续 runner 转发由 SP-00 整合。
- 新增 `apps/desktop/test/repair/sp_19_{status_viewport,shell_chrome,
  settings_entry}_test.dart`（合成数据；单文件单 `pumpWidget`，DPI 矩阵沿用
  `support/dpi_assertions.dart` 的 `applyDpi`/`kDpiScales` 模式逐文件拆分）。

## 先红（日志见 `red-*.log`）

- `sp_19_status_viewport`：横向 scroller 仍存在 → 红。
- `sp_19_shell_chrome`：800x800 下 `status-proxy-speed` 不可达 → 红。
- `sp_19_settings_entry`：`brightness` 参数不存在，编译失败 → 红。

## 定向检查（exit 均为 0，详见 `checks.log`）

- `dart format`（6 个自有文件）：clean。
- `flutter analyze`：No issues found。
- 新增 sp_19 三文件（逐个单独进程跑，避免 flutter_tester 多 pumpWidget
  崩溃）：3/3 pass。
- 既有受影响（逐个单独跑）：`r3_visual_dpi_shell_{100,125,150,200}`、
  `sp_17_{actual_summary,message_ordering,availability_probe,status_bar}`（12）、
  `t13_statusbar`（2）、`t15a_statusbar`（2）、`t05_shell_chrome`、
  `r4_06_contract`、`recheck_rr02_03`（5）、`profiles_render`：全部 pass。
- `r3_visual_dpi_shell_150` 途中一次偶发失败，重跑即绿（并行卡同机抢 CPU
  下的已知抖动，非断言回归）；`r3_visual_dpi_windows_test` 在干净基线
  `3635392`（stash 全清后）复现完全相同挂起（did not complete），既有失败，
  非本卡引入。

## 未运行范围

- Rust 未改：`cargo test/clippy/fmt` 未运行；`flutter build windows --release`
  未运行（无 native/AOT 接线变更）。
- 真实 800px 高 DPI 截图（亮暗/多语言/100–200%）与正式包主窗/独立设置窗
  实测未做；独立窗 runner 侧信封转发未接线。

## 阻塞（需 SP-00 整合者）

- 原生 runner 把主窗 presentation（theme/accent/font/locale/窗口几何）转发给
  独立设置窗引擎（`windows/runner` 不在本卡写锁内，未动）。
- 真实高 DPI 截图矩阵与正式包验收（SP-30 范畴），本卡只做到 widget 级
  DPI 矩阵（devicePixelRatio + physicalSize，逻辑尺寸 800x800/1200x800/
  1280x800 × 100/125/150/200）。
