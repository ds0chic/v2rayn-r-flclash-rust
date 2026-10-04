import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';

import 'support/profiles_harness.dart';

/// T17: preserved-only menu entries must render disabled (onPressed null) and
/// expose the "保留原版入口（未实现）" tooltip — they can never fire and
/// pretend to succeed. Wired desktop entries (R3-ROOT-03) are enabled.
void main() {
  testWidgets('preserved-only top-level entry is disabled with a tooltip', (
    tester,
  ) async {
    await pumpApp(tester, rows: 5);

    // 推广 (ACT-WIN-003) is the remaining upstream entry with no backend yet.
    final button = tester.widget<MenuItemButton>(
      find.byKey(const ValueKey('menu-推广')),
    );
    expect(button.onPressed, isNull);
    expect(find.byTooltip(AppMenuEntry.preservedTooltip), findsWidgets);
  });

  testWidgets('wired 以管理员身份重启 entry is enabled', (tester) async {
    await pumpApp(tester, rows: 5);

    await tester.tap(find.byKey(const ValueKey('menu-设置')));
    await tester.pumpAndSettle();

    final button = tester.widget<MenuItemButton>(
      find.byKey(const ValueKey('menu-item-以管理员身份重启')),
    );
    expect(button.onPressed, isNotNull);
  });
}
