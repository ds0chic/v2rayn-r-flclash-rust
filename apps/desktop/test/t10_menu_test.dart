// T10: server/settings menus expose the group/custom/template entries and
// the custom entry opens the real AddServer2 editor.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';

import 'support/profiles_harness.dart';

AppMenuEntry _find(List<AppMenuEntry> entries, String actionId) {
  for (final entry in entries) {
    if (entry.actionId == actionId) return entry;
    if (entry.submenu.isNotEmpty) {
      try {
        return _find(entry.submenu, actionId);
      } catch (_) {
        // Continue with the next submenu.
      }
    }
  }
  throw StateError('missing menu entry $actionId');
}

void main() {
  test('menu model carries the T10 entries', () {
    expect(_find(mainMenuModel, 'ACT-MAIN-012').label, '添加自定义配置');
    expect(_find(mainMenuModel, 'ACT-MAIN-013').label, '添加自定义出站');
    expect(_find(mainMenuModel, 'ACT-MAIN-014').label, '添加策略组');
    expect(_find(mainMenuModel, 'ACT-MAIN-015').label, '添加链式代理');
    expect(_find(mainMenuModel, 'ACT-MAIN-027').label, '完整配置模板设置');
  });

  testWidgets('menu opens the custom editor', (tester) async {
    await pumpApp(tester, rows: 200);
    await tester.tap(find.byKey(const ValueKey('menu-配置项')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-添加自定义配置')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('custom-editor')), findsOneWidget);
    // Cancel leaves no trace.
    await tester.tap(find.byKey(const ValueKey('custom-cancel')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('custom-editor')), findsNothing);
  });
}
