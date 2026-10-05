// R4-13.S05 CoreBasicItem save-to-effect repro.
//
// This assertion expresses the R4-13 contract and intentionally fails on the
// pre-fix implementation, which closed the settings window and reported
// success before the real plan apply ran (or failed). It is derived from
// docs/evidence/user-flow-audit-2026-10-05/settings-audit.md (UFS-06 / D14)
// and is NOT adjusted to the current buggy behavior.
//
// Synthetic only: in-memory bridge / runtime / platform. No native library, no
// kernel, no port, no host proxy/TUN/registry, no user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import '../support/counting_runtime_bridge.dart';
import '../support/fake_platform_bridge.dart';

ProviderContainer _container(BridgePort bridge, RuntimeBridge runtime) =>
    ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        runtimeBridgeProvider.overrideWithValue(runtime),
        platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      ],
    );

Future<void> _pump(WidgetTester tester, ProviderContainer container) async {
  tester.view.physicalSize = const Size(1200, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: OptionSettingWindow(standalone: true)),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('R4-13.S05: an apply failure is visible, not a fake success', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final runtime = CountingRuntimeBridge(
      applyError: const RuntimeErrorView(
        code: 'E_CORE',
        messageKey: 'error.core_start_failed',
      ),
    );
    final container = _container(bridge, runtime);
    addTearDown(container.dispose);

    await _pump(tester, container);
    await tester.tap(find.byKey(const ValueKey('settings-save')));
    await tester.pumpAndSettle();

    expect(runtime.applyCalls, 1, reason: 'save must drive a real apply');
    expect(
      find.byKey(const ValueKey('settings-save')),
      findsOneWidget,
      reason: 'a failed apply must not close the window as a success',
    );
    expect(
      find.textContaining('应用失败'),
      findsOneWidget,
      reason: 'the saved-but-not-applied failure must be visible',
    );
  });
}
