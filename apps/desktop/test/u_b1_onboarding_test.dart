import 'package:flutter/material.dart';
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

ProviderContainer _onboardingContainer({
  int rows = 0,
  MemoryUiStateStore? store,
  bool autoPopup = false,
}) {
  return ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(store ?? MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(rows),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      monitorBridgeProvider.overrideWithValue(FakeMonitorBridge()),
      firstRunAutoPopupProvider.overrideWithValue(autoPopup),
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
  testWidgets('auto popup shows on empty first run and marks completed', (
    tester,
  ) async {
    final store = MemoryUiStateStore();
    await pumpApp(
      tester,
      container: _onboardingContainer(store: store, autoPopup: true),
    );
    expect(find.byKey(const ValueKey('wizard-dialog')), findsOneWidget);
    expect(store.loadSection('onboarding')?['completed'], isTrue);
  });

  testWidgets('no popup when already completed', (tester) async {
    final store = MemoryUiStateStore()
      ..saveSection('onboarding', <String, dynamic>{'completed': true});
    await pumpApp(
      tester,
      container: _onboardingContainer(store: store, autoPopup: true),
    );
    expect(find.byKey(const ValueKey('wizard-dialog')), findsNothing);
    expect(find.byKey(const ValueKey('profiles-empty')), findsOneWidget);
  });

  testWidgets('popup stays off by default in widget tests', (tester) async {
    await pumpApp(tester, rows: 0);
    expect(find.byKey(const ValueKey('wizard-dialog')), findsNothing);
  });

  testWidgets('wizard step buttons dispatch the same menu actions', (
    tester,
  ) async {
    await pumpApp(tester, container: _onboardingContainer(autoPopup: true));
    await tester.tap(find.byKey(const ValueKey('wizard-go-sub')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('sub-setting-window')), findsOneWidget);
  });

  testWidgets('skip closes and help menu reopens the wizard', (tester) async {
    final store = MemoryUiStateStore();
    await pumpApp(
      tester,
      container: _onboardingContainer(store: store, autoPopup: true),
    );
    await tester.tap(find.byKey(const ValueKey('wizard-skip')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('wizard-dialog')), findsNothing);
    await _openWizardFromMenu(tester);
    expect(find.byKey(const ValueKey('wizard-dialog')), findsOneWidget);
  });

  testWidgets('reset hints clears confirm section and completion', (
    tester,
  ) async {
    final store = MemoryUiStateStore()
      ..saveSection('onboarding', <String, dynamic>{'completed': true})
      ..saveSection('confirm_hints', <String, dynamic>{'tun-enable': true});
    await pumpApp(tester, container: _onboardingContainer(store: store));
    await tester.tap(find.byKey(const ValueKey('menu-帮助')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('menu-item-重置界面提示')));
    await tester.pumpAndSettle();
    expect(store.loadSection('confirm_hints'), isNotNull);
    expect(store.loadSection('confirm_hints'), isEmpty);
    expect(store.loadSection('onboarding')?['completed'], isNot(equals(true)));
  });

  testWidgets('launch-verify is disabled with no nodes', (tester) async {
    await pumpApp(tester, container: _onboardingContainer(autoPopup: true));
    await tester.tap(find.byKey(const ValueKey('wizard-next')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('wizard-next')));
    await tester.pumpAndSettle();
    final button = tester.widget<FilledButton>(
      find.byKey(const ValueKey('wizard-launch-verify')),
    );
    expect(button.onPressed, isNull);
  });

  testWidgets('launch-verify runs start and reports a message', (tester) async {
    await pumpApp(
      tester,
      container: _onboardingContainer(rows: 5, autoPopup: false),
    );
    await _openWizardFromMenu(tester);
    await tester.tap(find.byKey(const ValueKey('wizard-next')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('wizard-next')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('wizard-launch-verify')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('wizard-verify-result')), findsOneWidget);
  });

  test('evidence env suppresses the auto popup', () {
    expect(suppressAutoPopupForEnv(<String, String>{}), isFalse);
    expect(suppressAutoPopupForEnv(<String, String>{'PATH': 'x'}), isFalse);
    expect(
      suppressAutoPopupForEnv(<String, String>{'V2RAYN_R_OPEN_SUBS': '1'}),
      isTrue,
    );
    expect(
      suppressAutoPopupForEnv(<String, String>{'V2RAYN_R_AUTO_SMOKE': '1'}),
      isTrue,
    );
  });

  test('onboarding store round-trips completion and suppressions', () {
    final store = MemoryUiStateStore();
    expect(OnboardingHints.isCompleted(store), isFalse);
    OnboardingHints.setCompleted(store, true);
    expect(OnboardingHints.isCompleted(store), isTrue);
    expect(OnboardingHints.isSuppressed(store, 'tun-enable'), isFalse);
    OnboardingHints.setSuppressed(store, 'tun-enable', true);
    expect(OnboardingHints.isSuppressed(store, 'tun-enable'), isTrue);
    OnboardingHints.resetHints(store);
    expect(OnboardingHints.isSuppressed(store, 'tun-enable'), isFalse);
    expect(OnboardingHints.isCompleted(store), isFalse);
  });
}
