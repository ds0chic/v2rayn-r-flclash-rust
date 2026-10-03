// UX-SPACE-01: the restored top entries are reachable at their upstream
// 30x30 size and still open the right surface. One page build per process.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('restored top entries are reachable and 30x30', (tester) async {
    final container = await pumpApp(tester, rows: 5);
    for (final key in <String>[
      'toolbar-sub-edit',
      'toolbar-sub-add',
      'toolbar-自动列宽',
      'toolbar-快速真延迟',
      'toolbar-混合',
      'column-settings-button',
    ]) {
      final finder = find.byKey(ValueKey<String>(key));
      expect(finder, findsOneWidget, reason: 'missing $key');
      expect(
        finder.hitTestable(),
        findsOneWidget,
        reason: '$key not reachable',
      );
    }

    expect(
      tester.getSize(find.byKey(const ValueKey('filter-field'))).width,
      AppTokens.toolbarFilterWidth,
    );
    expect(
      tester.getSize(find.byKey(const ValueKey('toolbar-自动列宽'))),
      const Size(AppTokens.toolbarIconButton, AppTokens.toolbarIconButton),
    );

    await tester.tap(find.byKey(const ValueKey('toolbar-自动列宽')));
    await tester.pump();
    expect(readState(container).lastEvent?.action, 'autofit-columns');

    await tester.tap(find.byKey(const ValueKey('toolbar-sub-edit')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-setting-window')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('sub-close')));
    await tester.pumpAndSettle();

    // 备注 (quick rename) is only reachable from this restored row today; the
    // UX-SPACE-01 evidence counts it under reachability.
    await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));
    await tester.tap(find.byKey(const ValueKey('toolbar-备注')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('remarks-dialog')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('remarks-cancel')));
    await tester.pumpAndSettle();
  });
}
