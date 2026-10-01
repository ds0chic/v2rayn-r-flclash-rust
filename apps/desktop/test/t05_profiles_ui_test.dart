import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';

import 'support/profiles_harness.dart';

/// Profiles table enhancements (columns, sort anchor, context menu, scope) in
/// a single page build to stay within the flutter_tester build budget.
void main() {
  testWidgets(
    'profiles columns, sort selection, context menu, keyboard scope',
    (tester) async {
      final store = MemoryUiStateStore();
      final container = await pumpApp(tester, rows: 100, store: store);
      final controller = container.read(profilesControllerProvider.notifier);

      double widthOf(String key) => container
          .read(profilesControllerProvider)
          .columns
          .firstWhere((c) => c.key == key)
          .width;

      // Column resize persists to ui_state.json (LAY-PROFILES-003).
      final before = widthOf('Remarks');
      await tester.drag(
        find.byKey(const ValueKey('resize-Remarks')),
        const Offset(40, 0),
      );
      await tester.pump();
      final after = widthOf('Remarks');
      expect(after, greaterThan(before));
      final widths = store.loadSection('column_layout')!['widths'] as Map;
      expect((widths['Remarks'] as num).toDouble(), after);

      // Column visibility via the settings dialog.
      await tester.tap(find.byKey(const ValueKey('column-settings-button')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('colvis-StreamSecurity')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('column-settings-close')));
      await tester.pumpAndSettle();
      expect(
        container
            .read(profilesControllerProvider)
            .visibleColumns
            .any((c) => c.key == 'StreamSecurity'),
        isFalse,
      );
      final visible = store.loadSection('column_layout')!['visible'] as Map;
      expect(visible['StreamSecurity'], false);

      // Order change persists.
      controller.moveColumn('Port', 1);
      await tester.pump();
      final order = (store.loadSection('column_layout')!['order'] as List)
          .cast<String>();
      expect(
        container
            .read(profilesControllerProvider)
            .columns
            .map((c) => c.key)
            .toList(),
        order,
      );

      // Scope rule: right click keeps the existing multi-selection.
      await tapRow(tester, const ValueKey('cell-syn-000000-Remarks'));
      await pressWithCtrlTap(tester, const ValueKey('cell-syn-000002-Remarks'));
      final selectedBefore = container
          .read(profilesControllerProvider)
          .selected;
      expect(selectedBefore.length, 2);
      await tester.tapAt(
        tester.getCenter(find.byKey(const ValueKey('cell-syn-000002-Remarks'))),
        buttons: kSecondaryButton,
      );
      await tester.pumpAndSettle();
      expect(
        container.read(profilesControllerProvider).selected,
        selectedBefore,
      );
      expect(
        container.read(profilesControllerProvider).lastEvent?.action,
        'context-menu',
      );
      // Context menu entries render from the ledger.
      for (final label in <String>['编辑', '全选', '移至上下', '分享']) {
        expect(
          find.byKey(ValueKey('ctx-$label')),
          findsOneWidget,
          reason: 'missing context entry $label',
        );
      }

      // Tapping the filter closes the menu; then stable sort keeps selection.
      await tester.tap(find.byKey(const ValueKey('filter-field')));
      await tester.pump();
      final selectedAfterRightClick = container
          .read(profilesControllerProvider)
          .selected;
      await tester.tap(find.byKey(const ValueKey('header-Port')));
      await tester.pumpAndSettle();
      expect(
        container.read(profilesControllerProvider).selected,
        selectedAfterRightClick,
      );
      expect(container.read(profilesControllerProvider).sort.columnKey, 'Port');

      // Input focus keeps keyboard scope; no table action fires (HKR-001/002).
      await tester.tap(find.byKey(const ValueKey('filter-field')));
      await tester.pump();
      final selected = container.read(profilesControllerProvider).selectedCount;
      final events = container.read(profilesControllerProvider).events.length;
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyD);
      await tester.pump();
      expect(
        container.read(profilesControllerProvider).selectedCount,
        selected,
      );
      expect(container.read(profilesControllerProvider).events.length, events);

      // Shell records backend entries as not implemented.
      container
          .read(uiShellControllerProvider.notifier)
          .notImplemented('检查更新', 'ACT-WIN-005');
      await tester.pump();
      expect(
        container.read(uiShellControllerProvider).message,
        contains('尚未实现'),
      );
    },
  );
}
