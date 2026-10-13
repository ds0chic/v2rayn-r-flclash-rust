import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/menu/main_menu.dart';

import 'support/profiles_harness.dart';

void main() {
  test('subscription update ledger rows are all kept, folded by default', () {
    final ids = flattenMenu(mainMenuModel).map((e) => e.actionId).toSet();
    for (final id in [
      'ACT-MAIN-019',
      'ACT-MAIN-020',
      'ACT-MAIN-021',
      'ACT-MAIN-022',
      'ACT-MAIN-023',
    ]) {
      expect(ids, contains(id));
    }
    final primary = flattenMenu(mainMenuModel)
        .firstWhere((e) => e.label == '更新订阅');
    expect(primary.actionId, 'ACT-MAIN-022');
    expect(primary.isSubmenu, isFalse);
    final more = flattenMenu(mainMenuModel)
        .firstWhere((e) => e.actionId == 'UI-SUB-MORE');
    expect(more.isSubmenu, isTrue);
    expect(more.submenu.map((e) => e.actionId).toList(), [
      'ACT-MAIN-020',
      'ACT-MAIN-021',
      'ACT-MAIN-022',
      'ACT-MAIN-023',
    ]);
  });

  testWidgets('folded submenu lists the four original entries', (tester) async {
    await pumpApp(tester, rows: 0);
    // Idle shell shows no subscription progress row.
    expect(find.byKey(const ValueKey('subs-global-progress')), findsNothing);
    await tester.tap(find.byKey(const ValueKey('menu-订阅分组')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-更多更新方式…')));
    await tester.pumpAndSettle();
    for (final label in [
      '更新全部订阅 (不通过代理)',
      '更新全部订阅 (通过代理)',
      '更新当前订阅 (不通过代理)',
      '更新当前订阅 (通过代理)',
    ]) {
      expect(find.byKey(ValueKey('menu-item-$label')), findsOneWidget);
    }
  });
}
