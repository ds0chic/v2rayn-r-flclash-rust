// UX-SPACE-01: the restored top entries are reachable at their upstream
// 30x30 size and still open the right surface. One page build per process.
//
// SP-16 note: `toolbar-sub-edit`/`toolbar-sub-add` no longer open the total
// subscription list (that was the UI-07 violation). They go straight to the
// edit dialog: edit targets the node page's current group G (All view is
// gated per upstream `EditSubAsync`), add always opens a blank draft. The
// total list keeps its own menu entry (ACT-MAIN-019).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/shared/theme/app_theme.dart';

import 'support/profiles_harness.dart';
import 'support/subs_harness.dart';

void main() {
  testWidgets('restored top entries are reachable and 30x30', (tester) async {
    final bridge = SeededSubsBridge();
    final container = await pumpApp(tester, rows: 5, bridge: bridge);
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

    // SP-16 direct entries: edit opens G itself (never the total list);
    // add opens a blank draft. The seeded bridge provides the current group.
    final gid = bridge.listSubItems().items.first.id;
    expect(
      container.read(profilesControllerProvider.notifier).setGroupSubId(gid),
      isTrue,
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('toolbar-sub-edit')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-edit-window')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('sub-edit-cancel')));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('toolbar-sub-add')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-edit-window')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('sub-edit-cancel')));
    await tester.pumpAndSettle();

    // 备注 (quick rename) is only reachable from this restored row today; the
    // UX-SPACE-01 evidence counts it under reachability. Back to All first:
    // the group filter above hides the generated rows.
    expect(
      container.read(profilesControllerProvider.notifier).setGroupSubId(null),
      isTrue,
    );
    await tester.pump();
    await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));
    await tester.tap(find.byKey(const ValueKey('toolbar-备注')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('remarks-dialog')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('remarks-cancel')));
    await tester.pumpAndSettle();
  });
}
