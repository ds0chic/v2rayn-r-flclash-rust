// R4-08 contract: immediate selection and the keyboard/command target.
//
// One page build covers the whole card contract because the locked
// flutter_tester leaks native resources per `pumpWidget`.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/counting_runtime_bridge.dart';
import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';
import 'support/profiles_harness.dart';

void main() {
  testWidgets('click is immediate, modifier frozen, command target correct', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(20),
        platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
        runtimeBridgeProvider.overrideWithValue(
          CountingRuntimeBridge(activeId: 'syn-000000'),
        ),
      ],
    );
    await pumpApp(tester, container: container);
    debugPrint('R408 setup done');
    final controller = container.read(profilesControllerProvider.notifier);
    const a = 'syn-000000';
    const b = 'syn-000001';
    const c = 'syn-000002';
    const d = 'syn-000003';

    // 7. Double click follows DoubleClick2Activate: false opens the editor,
    //    true activates. Done first, like the existing pointer test, because
    //    repeated taps can leave a tooltip overlay over the cell.
    await doubleTap(tester, find.byKey(const ValueKey('cell-$a-Remarks')));
    expect(readState(container).lastEvent?.action, 'edit');
    await tester.tap(find.byKey(const ValueKey('editor-cancel')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('profile-editor')), findsNothing);

    controller.toggleDoubleClick2Activate();
    await tester.pump();
    await doubleTap(tester, find.byKey(const ValueKey('cell-$a-Remarks')));
    expect(readState(container).lastEvent?.action, 'activate');
    await tester.pumpAndSettle();
    debugPrint('R408 phase7 done');

    // 1. Plain click selects immediately (within 50ms, never the 300ms
    //    double-tap timeout) and must NOT activate (R4-02 contract preserved).
    final activeBefore = bridge.getActiveProfile();
    await tester.tap(find.byKey(const ValueKey('cell-$b-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    expect(readState(container).selected, <String>{b});
    expect(readState(container).primaryId, b);
    expect(
      bridge.getActiveProfile(),
      activeBefore,
      reason: 'plain click only selects; it never activates',
    );
    debugPrint('R408 phase1 done');

    // 2. Ctrl click toggles immediately; the pointer-time modifier survives an
    //    early key release and is not read late in the tap callback.
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.tap(find.byKey(const ValueKey('cell-$c-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    expect(readState(container).selected, <String>{b, c});
    expect(readState(container).primaryId, c);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump(const Duration(milliseconds: 350));
    expect(readState(container).selected, <String>{
      b,
      c,
    }, reason: 'frozen Ctrl must not be re-read as released');
    // A second Ctrl click toggles exactly once (no pointer/onTap double toggle).
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.tap(find.byKey(const ValueKey('cell-$c-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    expect(readState(container).selected, <String>{b});
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    debugPrint('R408 phase2 done');

    // 3. Shift extends immediately from the main row.
    await tester.tap(find.byKey(const ValueKey('cell-$b-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.tap(find.byKey(const ValueKey('cell-$d-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    expect(readState(container).selected, <String>{b, c, d});
    expect(readState(container).primaryId, b, reason: 'anchor stays current');
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();
    debugPrint('R408 phase3 done');

    // 4. Fast Ctrl click then Enter targets the just-clicked row.
    controller.setActive(a);
    controller.selectRow(b);
    await tester.pump(const Duration(milliseconds: 400));
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.tap(find.byKey(const ValueKey('cell-$c-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(
      bridge.getActiveProfile(),
      c,
      reason: 'Enter must act on the pointer-selected primary',
    );
    debugPrint('R408 phase4 done');

    // 5. Plain click then Enter within 50ms targets the clicked row.
    controller.setActive(a);
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('cell-$b-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(bridge.getActiveProfile(), b);
    debugPrint('R408 phase5 done');

    // 6. Arrow navigation still moves from the main row.
    await tester.tap(find.byKey(const ValueKey('cell-$a-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(readState(container).selected, <String>{b});
    debugPrint('R408 phase6 done');
  });
}
