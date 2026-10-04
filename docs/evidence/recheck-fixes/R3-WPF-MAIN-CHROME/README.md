# R3-WPF-MAIN-CHROME — 主窗口可见结构对齐证据

日期：2026-10-04。基线：repo HEAD `f9ebe29`。冻结原版 v2rayN 7.25.4，commit `7d6a967c18c697f28dc6917122ed3a4993fcf336`。
对照输入：`docs/evidence/recheck-fixes/R3-WPF-COMPARE/{README.md,original-main.png,rc-main.png}`；上游 `v2rayN/v2rayN/Views/{MainWindow.xaml,ProfilesView.xaml,StatusBarView.xaml}` 与 `ServiceLib/Resx/ResUI.zh-Hans.resx`。
本轮无网络、无内核、无端口监听（未触碰 10808）、无系统代理/注册表/路由/TUN 改动、无用户凭据读取。

## 1. 上游对照结论（M1–M8）

| # | 项 | 上游（WPF 源 + 原版截图） | RC 处理 | 结果 |
|---|---|---|---|---|
| M1 | 主题 | 固定 Light；右上 `pbTheme` PopupBox | 跟随持久化 system；顶部主题按钮 | 有意增强，登记不回退 |
| M2 | 重载命名 | `menuReload` = 「重启服务」，无 InputGestureText | 原为「重载 F5」 | 已改为「重启服务」，移除 F5 chip（F5 仍由 shell 快捷键触发同一用例） |
| M3 | `#` 索引列 | 无 `#` 列；`RowHeaderWidth="40" HeadersVisibility="All"` 的无标题行头 | 行头表头渲染 `#` | 已移除 `#`；新增 `kProfilesRowHeaderWidth=40`，行号仍在行头 |
| M4 | 工具栏按钮 | 顶部仅主题 PopupBox + 隐藏 `btnNewUpdate`；节点工具栏=分组 chips/编辑订阅/新增订阅/200 过滤框/自动列宽/快速测试/混合测试 | 顶部多 `应用/停止/布局/主题`；节点工具栏多 编辑备注/列设置 | `主题`居中右上（同上游）；其余登记为有意增强，`main_shell` 补丁见 §3 |
| M5 | 状态栏文案 | `本地:[…]`、`局域网:none`、`启用 Tun`、系统代理 4 模式、路由下拉 | `入站/LAN/TUN` | 已改为 `本地:`/`局域网:`/`启用 Tun`；模式文案本已对齐 |
| M6 | 空态 | 原版空表格无提示 | 居中「暂无节点 / 可从剪贴板导入或添加节点」 | 有意增强，登记 |
| M7 | 底部信息区 | 过滤器/复制所有/清除所有/自动刷新/自动滚动 | 左导航 信息/当前代理/当前连接 + 过滤/Trace+/自动刷新/自动滚动/暂停采集/复制全部/清空 | 结构增强，登记 |
| M8 | 菜单分隔线 | `MainWindow.xaml` 子菜单分隔：配置项 3、订阅分组 1、设置 2、帮助 1 | 菜单模型无分隔字段 | 模型新增 `separatorAfter` 并按上游标记；渲染补丁见 §3 |

## 2. 落地改动（允许范围内）

- `apps/desktop/lib/features/profiles/profiles_models.dart`：新增 `kProfilesRowHeaderWidth = 40`（上游 `ProfilesView.xaml:113`）；默认列集保持 14 列上游 `ExName`，无 `#`。
- `apps/desktop/lib/features/profiles/profiles_table.dart`：行头表头由 `#` 文本改为无标题（`header-handle` key 保留）；行头宽度/`fixedChrome`/拖拽分隔阈值由 `AppTokens.tableHandleWidth(48)` 改为 `kProfilesRowHeaderWidth(40)`。
- `apps/desktop/lib/app/menu/main_menu.dart`：`AppMenuEntry` 新增 `separatorAfter`；配置项 / 订阅分组 / 设置 / 帮助 按上游打分隔；根菜单「重载」→「重启服务」并去掉 F5。
- `apps/desktop/lib/app/shell/status_bar_view.dart`：`入站`→`本地:`、`LAN`→`局域网:`、`TUN`→`启用 Tun`。
- 测试：`test/r3_wpf_main_chrome_test.dart`（新增，M2/M3/M8）；`test/t17_menu_structure_test.dart`、`test/t05_shell_chrome_test.dart` 修正「重启服务」并新增状态栏/行头断言。

## 3. `main_shell.dart` 接线补丁（根代理落地；本卡未改该文件）

### 3a. M8 分隔线渲染 — `_MenuToolbarBar._menuChildren`

在每条 entry 渲染后追加分隔（注意唯一 key，避免同列表重复）：

```dart
  List<Widget> _menuChildren(List<AppMenuEntry> entries) {
    final widgets = <Widget>[];
    for (final entry in entries) {
      if (entry.isSubmenu) {
        widgets.add(
          SubmenuButton(
            key: ValueKey('menu-item-${entry.label}'),
            menuChildren: _menuChildren(entry.submenu),
            child: _menuItemLabel(entry),
          ),
        );
      } else {
        widgets.add(
          _tooltip(
            entry,
            MenuItemButton(
              key: ValueKey('menu-item-${entry.label}'),
              onPressed: entry.isInvocable ? () => onAction(entry) : null,
              child: _menuItemLabel(entry),
            ),
          ),
        );
      }
      // R3-WPF-Main-Chrome M8: 对应上游 MainWindow.xaml 的
      // <Separator Margin="-40,5" />。菜单模型已在 main_menu.dart 标记。
      if (entry.separatorAfter) {
        widgets.add(Divider(height: 1, key: ValueKey('menu-sep-${entry.label}')));
      }
    }
    return widgets;
  }
```

### 3b. M4 工具栏取舍（登记，非强制回退）

- 保留：右上主题切换（对应上游 `pbTheme` 位置）。
- 保留并登记为有意增强：`_RuntimeToolbar`（`运行时/应用/停止`，net_host 显式 apply 架构）、`layout-selector`（上游布局切换不在顶栏）、隐藏的 `btn-new-update`。
- 若需严格上游视觉，可将 `layout-selector` 移入 `设置/主界面` 子菜单（已有 `UI-LAYOUT-*` 入口）——本轮不改，仅登记。

### 3c. `app_theme.dart`（不在允许范围，登记）

`AppTokens.tableHandleWidth` 仍为 48；RC 表格已局部改用 `kProfilesRowHeaderWidth=40`。若根代理统一 token，可将其改为 40（上游 `RowHeaderWidth=40`）。

## 4. 未回归确认

- 节点右键菜单坐标/关闭/Esc 契约由 `test/context_menu_model_test.dart` 断言「17 根 / 4 分隔线」保持绿色；未触碰 `context_menu.dart`/`profiles_table` 菜单生命周期代码。

## 5. 命令与结果

见 `test-run.log`。摘要：
- `dart format`（7 文件，3 重排）exit 0。
- `flutter analyze` → `No issues found!` exit 0。
- `flutter test r3_wpf_main_chrome_test/t17_menu_structure/profiles_models/t13_statusbar/context_menu_model` → 21 通过 exit 0。
- `flutter test t05_shell_chrome_test` → 1 通过 exit 0。

## 6. 未完成 / 未验证 / 下一步前置

- 未做原版实机双窗口逐事件对照、未跑 `flutter build windows --release`、未做高 DPI/托盘视觉；`V2RAYN_R_THEME=light` 证据钩子仍缺。
- 前置：根代理落地 §3a 补丁并复跑 `t05_shell_chrome_test`（菜单展开后应出现分隔线）；决定 M4 `layout-selector` 是否移入设置菜单。
