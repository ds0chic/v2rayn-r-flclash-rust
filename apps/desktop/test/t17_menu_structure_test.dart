import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';

/// T17 menu-structure contract: the top-level order, the 帮助 submenu and the
/// preserved-only set must stay aligned with compat/actions.yaml main-menu.
void main() {
  final all = flattenMenu(mainMenuModel).toList();

  test('top-level menu order matches the upstream layout', () {
    expect(mainMenuModel.map((e) => e.label).toList(), <String>[
      '配置项',
      '订阅分组',
      '设置',
      '帮助',
      '重载',
      '推广',
      '关闭',
    ]);
  });

  test('30 main-menu action ids are reachable from the model', () {
    final ids = <String>{
      for (final entry in all)
        if (entry.actionId != null) entry.actionId!,
    };
    const mainMenuIds = <String>{
      'ACT-MAIN-001',
      'ACT-MAIN-002',
      'ACT-MAIN-003',
      'ACT-MAIN-004',
      'ACT-MAIN-005',
      'ACT-MAIN-006',
      'ACT-MAIN-007',
      'ACT-MAIN-008',
      'ACT-MAIN-009',
      'ACT-MAIN-010',
      'ACT-MAIN-011',
      'ACT-MAIN-012',
      'ACT-MAIN-013',
      'ACT-MAIN-014',
      'ACT-MAIN-015',
      'ACT-MAIN-016',
      'ACT-MAIN-017',
      'ACT-MAIN-018',
      'ACT-MAIN-019',
      'ACT-MAIN-035',
    };
    expect(ids.containsAll(mainMenuIds), isTrue, reason: 'missing main ids');
  });

  test('help submenu keeps 检查更新 and the dynamic core-website entry', () {
    final help = mainMenuModel.firstWhere((e) => e.label == '帮助');
    expect(help.submenu.map((e) => e.label).toList(), <String>['检查更新', '核心网站']);
    final check = help.submenu.firstWhere((e) => e.label == '检查更新');
    expect(check.actionId, 'ACT-WIN-005');
    expect(check.isInvocable, isTrue);
    final site = help.submenu.firstWhere((e) => e.label == '核心网站');
    expect(site.preservedOnly, isTrue);
    expect(site.isInvocable, isFalse);
    expect(site.actionId, 'ACT-WIN-008');
  });

  test('preserved-only entries are never invocable', () {
    final preserved = all.where((e) => e.preservedOnly).toList();
    expect(preserved, isNotEmpty);
    for (final entry in preserved) {
      expect(entry.isInvocable, isFalse, reason: entry.label);
    }
    final ids = preserved.map((e) => e.actionId).toSet();
    expect(
      ids.containsAll(<String>{
        'ACT-WIN-003', // 推广
        'ACT-WIN-008', // 核心网站
        'ACT-MAIN-029', // 以管理员身份重启
        'ACT-WIN-004', // UWP 回环
        'ACT-MAIN-032',
        'ACT-MAIN-033',
        'ACT-MAIN-034',
      }),
      isTrue,
      reason: 'preserved set: $ids',
    );
  });

  test('reload entry is invocable after the shared reload use case', () {
    final reload = all.firstWhere((e) => e.actionId == 'ACT-MAIN-035');
    expect(reload.preservedOnly, isFalse);
    expect(reload.isInvocable, isTrue);
  });

  test('implemented entries stay invocable', () {
    for (final label in <String>['配置项', '订阅分组', '设置', '帮助', '关闭']) {
      final entry = mainMenuModel.firstWhere((e) => e.label == label);
      expect(entry.isInvocable, isTrue, reason: label);
    }
  });
}
