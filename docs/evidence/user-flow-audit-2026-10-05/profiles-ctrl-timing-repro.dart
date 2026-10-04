// Copy to apps/desktop/test/_audit_profiles_ctrl_timing_repro_test.dart.
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
  testWidgets('Ctrl click preserves its pointer-time modifier after early release',
      (tester) async {
    final container = ProviderContainer(overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
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
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.tap(find.byKey(const ValueKey('cell-$b-Remarks')));
    await tester.pump(const Duration(milliseconds: 50));
    debugPrint('UF13 ctrl +50ms selected=${readState(container).selected} '
        'primary=${readState(container).primaryId}');
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump(const Duration(milliseconds: 350));
    debugPrint('UF13 ctrl +400ms selected=${readState(container).selected} '
        'primary=${readState(container).primaryId}');
    expect(readState(container).selected, <String>{a, b},
        reason: 'Ctrl held at pointer-down must survive an early key release.');
  });
}
