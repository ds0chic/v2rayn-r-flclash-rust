// T11: settings menu carries the routing/DNS entries (ACT-MAIN-025/026);
// the routing window opens from the menu and the status bar exposes the
// Rule/Global/Direct switch plus the scheme list.
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
  test('menu model carries the routing and DNS entries', () {
    expect(_find(mainMenuModel, 'ACT-MAIN-025').label, '路由设置');
    expect(_find(mainMenuModel, 'ACT-MAIN-026').label, 'DNS 设置');
  });

  testWidgets('menu opens the routing window; status bar shows the switch', (
    tester,
  ) async {
    await pumpApp(tester, rows: 200);
    await tester.tap(find.byKey(const ValueKey('menu-设置')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-路由设置')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('routing-setting-window')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('routing-close')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('routing-setting-window')), findsNothing);
    // Status bar: mode switch + scheme selector are wired, not placeholders.
    expect(find.byKey(const ValueKey('routing-mode-selector')), findsOneWidget);
    expect(find.byKey(const ValueKey('routing-selector')), findsOneWidget);
    expect(find.textContaining('路由模式:'), findsOneWidget);
  });
}
