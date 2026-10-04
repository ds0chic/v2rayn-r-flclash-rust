// Copy to apps/desktop/test/_audit_profiles_enter_timing_repro_test.dart.
// Repair-contract assertion; expected to fail on 77c74ed. Synthetic only.
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
  testWidgets('Enter 50ms after clicking B must target B rather than old A',
      (tester) async {
    final bridge = SyntheticBridgePort();
    final container = ProviderContainer(overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(20),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      runtimeBridgeProvider.overrideWithValue(
        CountingRuntimeBridge(activeId: 'syn-000000'),
      ),
    ]);
    await pumpApp(tester, container: container);
    const a = 'syn-000000';
    const b = 'syn-000001';
    await tapRow(tester, const ValueKey('cell-$a-Remarks'));
    container.read(profilesControllerProvider.notifier).setActive(a);
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('cell-$b-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    debugPrint('UF13 enter +50ms selected=${readState(container).selected} '
        'primary=${readState(container).primaryId} active=${bridge.getActiveProfile()}');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    final commandTarget = bridge.getActiveProfile();
    debugPrint('UF13 Enter accepted active=$commandTarget '
        'primary=${readState(container).primaryId}');
    await tester.pump(const Duration(milliseconds: 350));
    debugPrint('UF13 enter +400ms selected=${readState(container).selected} '
        'primary=${readState(container).primaryId} active=${bridge.getActiveProfile()}');
    expect(commandTarget, b,
        reason: 'A click must update the command target before the next Enter.');
  });
}
