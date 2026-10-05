// R4-07 contract: context-menu lifecycle and the captured command target.
//
// One page build covers the whole card contract because the locked
// flutter_tester leaks native resources per `pumpWidget`.
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_table.dart';

import 'support/profiles_harness.dart';

void main() {
  testWidgets('context menu lifecycle and captured target', (tester) async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          SystemChannels.platform,
          (call) async => null,
        );
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, null),
    );

    final container = await pumpApp(tester, rows: 20);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final controller = container.read(profilesControllerProvider.notifier);
    final ids = readState(container).visible.map((r) => r.id).toList();
    final a = ids[0], b = ids[1], c = ids[2], d = ids[3];

    final menuActivate = find.byKey(const ValueKey('ctx-设为活动'));

    Future<void> rightClickAt(Offset point) async {
      await tester.tapAt(point, buttons: kSecondaryButton);
      await tester.pumpAndSettle();
    }

    Future<void> rightClickRow(String id) => rightClickAt(
      tester.getCenter(find.byKey(ValueKey('cell-$id-Remarks'))),
    );

    // 1. Row right-click on an unselected row replaces the selection with that
    //    single row and makes it the primary; the command follows it.
    await tapRow(tester, ValueKey('cell-$a-Remarks'));
    await pressWithCtrlTap(tester, ValueKey('cell-$b-Remarks'));
    expect(readState(container).selected, <String>{a, b});
    await rightClickRow(c);
    expect(readState(container).selected, <String>{c});
    expect(readState(container).primaryId, c);
    expect(menuActivate, findsOneWidget);
    await tester.tap(menuActivate);
    await tester.pumpAndSettle();
    expect(bridge.getActiveProfile(), c);
    expect(menuActivate, findsNothing);

    // 2. Row right-click on an already-selected row keeps the multi-selection
    //    and only moves the primary/current row to the row under the pointer.
    controller.setActive(a);
    await tapRow(tester, ValueKey('cell-$a-Remarks'));
    await pressWithCtrlTap(tester, ValueKey('cell-$b-Remarks'));
    expect(readState(container).selected, <String>{a, b});
    expect(readState(container).primaryId, b);
    await rightClickRow(a);
    expect(readState(container).selected, <String>{a, b});
    expect(readState(container).primaryId, a);
    await tester.tap(menuActivate);
    await tester.pumpAndSettle();
    expect(bridge.getActiveProfile(), a);
    expect(readState(container).selected, <String>{
      a,
      b,
    }, reason: 'a single-object command never collapses the batch selection');

    // 3. Header right-click preserves the current row as primary and the
    //    command uses that frozen primary even if live state drifts meanwhile.
    await tapRow(tester, ValueKey('cell-$a-Remarks'));
    await pressWithCtrlTap(tester, ValueKey('cell-$b-Remarks'));
    final headerTitle = readState(container).visibleColumns.first.title;
    await rightClickAt(
      tester.getCenter(find.byKey(ValueKey('header-$headerTitle'))),
    );
    expect(menuActivate, findsOneWidget);
    expect(readState(container).selected, <String>{
      a,
      b,
    }, reason: 'header right-click keeps the current selection');
    // Programmatic drift while the menu is open: without the fixed snapshot the
    // command would silently fall back to this live primary.
    controller.selectRow(c);
    await tester.pump();
    await tester.tap(menuActivate);
    await tester.pumpAndSettle();
    expect(
      bridge.getActiveProfile(),
      b,
      reason: 'the captured primary, not the drifted live row',
    );
    expect(readState(container).selected, <String>{a, b});

    // 4. Empty-area right-click in a genuinely empty view (all rows filtered
    //    out) moves focus off the table, so no row is targeted and no stale
    //    primary survives.
    controller.setActive(d);
    final tableRect = tester.getRect(find.byType(ProfilesTable));
    await tapRow(tester, ValueKey('cell-$a-Remarks'));
    controller.setFilter('zzz-no-such-node');
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('profiles-empty')), findsOneWidget);
    final emptyPoint = Offset(
      tableRect.center.dx,
      (tableRect.bottom - 24).clamp(0.0, tableRect.bottom - 6),
    );
    await rightClickAt(emptyPoint);
    expect(menuActivate, findsOneWidget);
    expect(readState(container).selected, <String>{});
    expect(readState(container).primaryId, isNull);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    controller.setFilter('');
    await tester.pumpAndSettle();

    // 5. Two consecutive open/close cycles keep only one session at a time.
    controller.setActive(d);
    await tapRow(tester, ValueKey('cell-$a-Remarks'));
    await rightClickRow(a);
    expect(menuActivate, findsOneWidget);
    await tester.tap(menuActivate);
    await tester.pumpAndSettle();
    expect(menuActivate, findsNothing);
    await rightClickRow(a);
    expect(menuActivate, findsOneWidget, reason: 'only one session may exist');

    // 6. Esc closes the chain and returns focus to the table.
    controller.setActive(d);
    await tapRow(tester, ValueKey('cell-$a-Remarks'));
    await rightClickRow(a);
    expect(menuActivate, findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    expect(menuActivate, findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(bridge.getActiveProfile(), a);

    // 7. Window deactivation closes the session and reactivation never
    //    restores it.
    await rightClickRow(a);
    expect(menuActivate, findsOneWidget);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    await tester.pumpAndSettle();
    expect(menuActivate, findsNothing);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pumpAndSettle();
    expect(menuActivate, findsNothing);

    // 8. A refresh/filter that hides the captured target refuses the command
    //    with a visible message instead of retargeting the live selection.
    await tapRow(tester, ValueKey('cell-$a-Remarks'));
    await rightClickRow(a);
    expect(menuActivate, findsOneWidget);
    final activeBefore = bridge.getActiveProfile();
    controller.setFilter('zzz-no-such-node');
    await tester.pumpAndSettle();
    expect(menuActivate, findsOneWidget);
    await tester.tap(menuActivate);
    await tester.pump();
    expect(
      bridge.getActiveProfile(),
      activeBefore,
      reason: 'a hidden captured target must not run against the live row',
    );
    expect(container.read(uiShellControllerProvider).message, contains('失效'));
    expect(menuActivate, findsNothing);
    controller.setFilter('');
    await tester.pumpAndSettle();

    // 9. A vertical drag on the table body emits scroll notifications that close
    //    the single session (the menu overlays the clicked cell, so the gesture
    //    starts on a lower row's handle column).
    await rightClickRow(a);
    expect(menuActivate, findsOneWidget);
    final dragRow = ids[5];
    final handle = tester.getCenter(find.byKey(ValueKey('handle-$dragRow')));
    await tester.dragFrom(handle, const Offset(0, -160));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    expect(
      menuActivate,
      findsNothing,
      reason: 'a scroll closes the single session instead of leaving a stale anchor',
    );

    // 10. Edge positioning: near the right edge the menu stays inside the
    //     window and never pushes an entry off-screen. The height stays tall
    //     enough that no unrelated overlay overflows.
    tester.view.physicalSize = const Size(700, 520);
    await tester.pumpAndSettle();
    final edgeTable = tester.getRect(find.byType(ProfilesTable));
    await rightClickAt(Offset(edgeTable.right - 4, edgeTable.bottom - 4));
    expect(menuActivate, findsOneWidget);
    final edgeItem = tester.getRect(menuActivate);
    expect(edgeItem.left, greaterThanOrEqualTo(-0.5));
    expect(edgeItem.top, greaterThanOrEqualTo(-0.5));
    expect(edgeItem.right, lessThanOrEqualTo(700.5));
    expect(edgeItem.bottom, lessThanOrEqualTo(520.5));
    await tester.tapAt(const Offset(2, 2));
    await tester.pumpAndSettle();

    // 11. Coordinate correctness: a roomy open appears inside the window and
    //     its entries stay on-screen.
    tester.view.resetPhysicalSize();
    await tester.pumpAndSettle();
    final roomyRow = readState(container).visible.first.id;
    final roomyPoint = tester.getCenter(
      find.byKey(ValueKey('cell-$roomyRow-Remarks')),
    );
    await rightClickAt(roomyPoint);
    expect(menuActivate, findsOneWidget);
    final roomyItem = tester.getRect(menuActivate);
    expect(roomyItem.left, greaterThanOrEqualTo(-0.5));
    expect(roomyItem.top, greaterThanOrEqualTo(-0.5));
    await tester.tapAt(const Offset(2, 2));
    await tester.pumpAndSettle();

    // 12. R4-08 contract preserved: a plain click still selects immediately.
    await tester.tap(find.byKey(ValueKey('cell-$b-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    expect(readState(container).selected, <String>{b});
    expect(readState(container).primaryId, b);
  });
}
