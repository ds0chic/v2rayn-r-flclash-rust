import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/settings/global_hotkey_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

/// Bridge whose settings load can be toggled between failing and working.
class _FlappingBridge extends SyntheticBridgePort {
  bool failLoad = true;

  @override
  settings.SettingsLoadDto getSettings() {
    if (failLoad) {
      return settings.SettingsLoadDto(
        ok: false,
        revision: BigInt.zero,
        groupRevisionsJson: '{}',
        settings: null,
        settingsJson: '',
        error: null,
      );
    }
    return super.getSettings();
  }
}

/// D2-06: a failed settings load must render an explicit error + retry entry
/// in the hotkey window, never five misleading "unbound" rows.
void main() {
  testWidgets('hotkey window shows load failure with retry', (tester) async {
    final bridge = _FlappingBridge();
    final container = ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: GlobalHotkeyWindow())),
      ),
    );
    await tester.pumpAndSettle();

    expect(container.read(settingsControllerProvider).loadFailed, isTrue);
    expect(find.byKey(const ValueKey('hotkey-load-error')), findsOneWidget);
    expect(find.byKey(const ValueKey('hotkey-load-retry')), findsOneWidget);
    // No misleading "unbound" rows while the load failed.
    expect(find.byKey(const ValueKey('hotkey-label-0')), findsNothing);

    // Retry after the store recovers loads the real bindings.
    bridge.failLoad = false;
    await tester.tap(find.byKey(const ValueKey('hotkey-load-retry')));
    await tester.pumpAndSettle();
    expect(container.read(settingsControllerProvider).loaded, isTrue);
    expect(find.byKey(const ValueKey('hotkey-label-0')), findsOneWidget);
    expect(find.byKey(const ValueKey('hotkey-load-error')), findsNothing);
  });
}
