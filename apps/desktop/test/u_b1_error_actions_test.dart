import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/status_bar_view.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';
import 'support/profiles_harness.dart';

const _failedView = RuntimeView(
  state: 'Stopped',
  error: RuntimeErrorView(
    code: 'E_CORE_EXITED',
    messageKey: 'error.core_exited',
  ),
  failedTargetId: 'node-1',
  failedAtMs: 1720000000000,
);

void main() {
  test('error next-step routing is conservative', () {
    expect(
      errorNextTarget('E_TUN_HELPER_UNAVAILABLE'),
      ErrorNextTarget.adminGuidance,
    );
    expect(errorNextTarget('e_port_in_use'), ErrorNextTarget.availabilityProbe);
    expect(errorNextTarget('E_BIND_FAILED'), ErrorNextTarget.availabilityProbe);
    expect(errorNextTarget('E_SUB_FETCH'), ErrorNextTarget.subSettings);
    expect(errorNextTarget('E_SOMETHING_NEW'), ErrorNextTarget.logs);
    expect(errorNextTarget(''), ErrorNextTarget.logs);
  });

  testWidgets('no next-step button without a runtime error', (tester) async {
    await pumpApp(tester, rows: 5);
    expect(find.byKey(const ValueKey('runtime-error-next')), findsNothing);
  });

  testWidgets('failure dialog navigates to logs and retries', (tester) async {
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(0),
        platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      ],
    );
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Consumer(
            builder: (context, ref, _) => TextButton(
              key: const ValueKey('open-failure'),
              onPressed: () => showFailureDetails(context, ref, _failedView),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('open-failure')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('failure-details-target')),
      findsOneWidget,
    );
    // Rollback ships disabled: no backend snapshot API exists.
    final rollback = tester.widget<TextButton>(
      find.byKey(const ValueKey('failure-details-rollback')),
    );
    expect(rollback.onPressed, isNull);

    await tester.tap(find.byKey(const ValueKey('failure-details-logs')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('failure-details-target')), findsNothing);
    expect(container.read(uiShellControllerProvider).tabIndex, 0);

    await tester.tap(find.byKey(const ValueKey('open-failure')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('failure-details-retry')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('failure-details-target')), findsNothing);
  });
}
