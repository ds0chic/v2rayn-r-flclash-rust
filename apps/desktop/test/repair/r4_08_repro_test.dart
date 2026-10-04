// R4-08 repro: copied from docs/evidence/user-flow-audit-2026-10-05/
// profiles-ctrl-timing-repro.dart and profiles-enter-timing-repro.dart.
// Repair-contract assertions; expected to fail on ef02954 and pass after the
// pointer-down selection fix. Synthetic only.
//
// One page build covers both short-timing scenarios because the locked
// flutter_tester leaks native resources per `pumpWidget` (see
// test/support/profiles_harness.dart).
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

import '../support/counting_runtime_bridge.dart';
import '../support/fake_monitor_bridge.dart';
import '../support/fake_platform_bridge.dart';
import '../support/profiles_harness.dart';

void main() {
  testWidgets('UF13 immediate selection and keyboard target', (tester) async {
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
    const a = 'syn-000000';
    const b = 'syn-000001';

    // --- profiles-ctrl-timing-repro.dart ---
    // Ctrl held at pointer-down must survive an early key release.
    await tapRow(tester, const ValueKey('cell-$a-Remarks'));
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.tap(find.byKey(const ValueKey('cell-$b-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    debugPrint(
      'UF13 ctrl +50ms selected=${readState(container).selected} '
      'primary=${readState(container).primaryId}',
    );
    expect(readState(container).selected, <String>{
      a,
      b,
    }, reason: 'Ctrl held at pointer-down must survive an early key release.');
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump(const Duration(milliseconds: 350));
    debugPrint(
      'UF13 ctrl +400ms selected=${readState(container).selected} '
      'primary=${readState(container).primaryId}',
    );
    expect(readState(container).selected, <String>{a, b});

    // --- profiles-enter-timing-repro.dart ---
    // Enter 50ms after clicking B must target B rather than the old A.
    await tapRow(tester, const ValueKey('cell-$a-Remarks'));
    container.read(profilesControllerProvider.notifier).setActive(a);
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('cell-$b-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    debugPrint(
      'UF13 enter +50ms selected=${readState(container).selected} '
      'primary=${readState(container).primaryId} '
      'active=${bridge.getActiveProfile()}',
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    final commandTarget = bridge.getActiveProfile();
    debugPrint(
      'UF13 Enter accepted active=$commandTarget '
      'primary=${readState(container).primaryId}',
    );
    await tester.pump(const Duration(milliseconds: 350));
    expect(
      commandTarget,
      b,
      reason: 'A click must update the command target before the next Enter.',
    );
  });
}
