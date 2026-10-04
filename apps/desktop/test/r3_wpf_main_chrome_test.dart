import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_models.dart';

/// R3-WPF-Main-Chrome contracts frozen from the WPF source
/// (`ProfilesView.xaml` / `MainWindow.xaml` in the 7d6a967 checkout):
/// M2 menu naming, M3 row-header (no `#` column), M4 toolbar取舍 (registered
/// outside this model), M5 status-bar wording (widget test in
/// `t05_shell_chrome_test.dart`), M8 submenu separators.
void main() {
  test(
    'M3: default column set has no `#` entry (WPF uses a 40px row header)',
    () {
      final columns = defaultProfileColumns();
      expect(columns.length, 14);
      expect(columns.map((c) => c.key), isNot(contains('#')));
      expect(columns.map((c) => c.title), isNot(contains('#')));
      // Upstream `ProfilesView.xaml:113` RowHeaderWidth.
      expect(kProfilesRowHeaderWidth, 40);
    },
  );

  test(
    'M2: reload root entry uses the upstream label 重启服务 without F5 chip',
    () {
      final labels = mainMenuModel.map((e) => e.label).toList();
      expect(labels, isNot(contains('重载')));
      expect(labels, contains('重启服务'));
      final reload = mainMenuModel.firstWhere((e) => e.label == '重启服务');
      expect(reload.actionId, 'ACT-MAIN-035');
      // Upstream menuReload has no InputGestureText in MainWindow.xaml:263-277.
      expect(reload.shortcut, isNull);
    },
  );

  test('M8: submenu separators match MainWindow.xaml', () {
    int separatorsOf(String group) {
      final entry = mainMenuModel.firstWhere((e) => e.label == group);
      return entry.submenu.where((e) => e.separatorAfter).length;
    }

    // 配置项 <Separator> after 扫描图片中的二维码 / 添加自定义出站 / 添加 [HTTP].
    expect(separatorsOf('配置项'), 3);
    // 订阅分组 after 订阅分组设置.
    expect(separatorsOf('订阅分组'), 1);
    // 设置 after 全局热键设置 / 清除所有服务统计数据.
    expect(separatorsOf('设置'), 2);
    // 帮助 after 检查更新 (dynamic core-website entries follow).
    expect(separatorsOf('帮助'), 1);
  });

  test('M8: separator positions follow the upstream entry boundaries', () {
    final servers = mainMenuModel.firstWhere((e) => e.label == '配置项').submenu;
    final splitServerLabels = <String>[
      for (final e in servers)
        if (e.separatorAfter) e.label,
    ];
    expect(splitServerLabels, <String>['扫描图片中的二维码', '添加自定义出站', '添加 [HTTP]']);

    final help = mainMenuModel.firstWhere((e) => e.label == '帮助').submenu;
    expect(help.where((e) => e.separatorAfter).single.label, '检查更新');
  });
}
