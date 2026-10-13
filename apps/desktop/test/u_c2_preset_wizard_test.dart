import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/onboarding/first_run_wizard.dart';
import 'package:v2rayn_desktop/features/onboarding/onboarding_store.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_monitor_bridge.dart';
import 'support/fake_platform_bridge.dart';
import 'support/profiles_harness.dart';

/// U-C2 C2-2 + approval condition A: the wizard's step-1 recommended-setup
/// button opens the preset dialog stacked above the wizard; Escape closes only
/// the top dialog and never touches the wizard completion flag.
ProviderContainer _container({MemoryUiStateStore? store}) {
  return ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(store ?? MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(0),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      firstRunAutoPopupProvider.overrideWithValue(false),
    ],
  );
}

Future<void> _openWizardFromMenu(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('menu-帮助')));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const ValueKey('menu-item-首次启动向导…')));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('step 1 ends with the recommended-setup button', (tester) async {
    await pumpApp(tester, container: _container());
    await _openWizardFromMenu(tester);
    expect(find.byKey(const ValueKey('wizard-dialog')), findsOneWidget);
    expect(find.byKey(const ValueKey('wizard-go-preset')), findsOneWidget);
  });

  testWidgets('preset button stacks the preset dialog above the wizard', (
    tester,
  ) async {
    await pumpApp(tester, container: _container());
    await _openWizardFromMenu(tester);
    await tester.tap(find.byKey(const ValueKey('wizard-go-preset')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('wizard-dialog')), findsOneWidget);
    expect(find.text('一键推荐方案'), findsOneWidget);
  });

  testWidgets('Escape closes only the preset dialog, completion untouched', (
    tester,
  ) async {
    final store = MemoryUiStateStore();
    await pumpApp(tester, container: _container(store: store));
    await _openWizardFromMenu(tester);
    await tester.tap(find.byKey(const ValueKey('wizard-go-preset')));
    await tester.pumpAndSettle();
    expect(find.text('一键推荐方案'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('一键推荐方案'), findsNothing);
    expect(find.byKey(const ValueKey('wizard-dialog')), findsOneWidget);
    expect(OnboardingHints.isCompleted(store), isFalse);
  });
}
