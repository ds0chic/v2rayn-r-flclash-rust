import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/theme_setting_dialog.dart';

void main() {
  testWidgets('theme dialog applies immediately and persists', (tester) async {
    final bridge = SyntheticBridgePort(count: 4);
    final container = ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
        profileRowCountProvider.overrideWithValue(4),
      ],
    );
    addTearDown(container.dispose);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: ThemeSettingDialog())),
      ),
    );
    await tester.pump();

    // Choose the Dark theme: applied immediately.
    await tester.tap(find.text('FollowSystem'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Dark').last);
    await tester.pumpAndSettle();
    expect(container.read(uiShellControllerProvider).themeMode, ThemeMode.dark);

    // Choose the Blue accent.
    await tester.tap(find.text('（默认）'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Blue').last);
    await tester.pumpAndSettle();
    expect(container.read(uiShellControllerProvider).accentName, 'Blue');

    // Persisted to UIItem and restored on reopen.
    final controller = container.read(settingsControllerProvider);
    expect(
      (controller.document['UiItem'] as Map<String, dynamic>)['CurrentTheme'],
      'Dark',
    );
    expect(
      (controller.document['UiItem']
          as Map<String, dynamic>)['ColorPrimaryName'],
      'Blue',
    );
  });
}
