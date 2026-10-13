import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/settings/theme_setting_dialog.dart';

/// Counts whole-group UiItem persists so per-keystroke save storms are visible.
class _CountingBridge extends SyntheticBridgePort {
  int uiItemSaves = 0;

  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String patchJson,
    int expectedRevision,
  ) {
    if (group == 'UiItem') uiItemSaves++;
    return super.saveSettingsGroup(group, patchJson, expectedRevision);
  }
}

/// D2-03: rapid edits in the theme dialog must coalesce into one persist,
/// not one `saveGroup` per keystroke.
void main() {
  testWidgets('rapid theme edits persist once', (tester) async {
    final bridge = _CountingBridge();
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
        child: const MaterialApp(
          home: Scaffold(body: ThemeSettingDialog()),
        ),
      ),
    );
    await tester.pumpAndSettle();

    // Font-family box is the first TextField in the dialog.
    final field = find.byType(TextField).first;
    expect(field, findsOneWidget);
    await tester.enterText(field, 'ab');
    await tester.pump();
    await tester.enterText(field, 'abc');
    await tester.pump();
    // Let the debounce window elapse.
    await tester.pump(const Duration(milliseconds: 600));
    await tester.pumpAndSettle();

    expect(
      bridge.uiItemSaves,
      1,
      reason: 'two rapid edits must coalesce into one UiItem save',
    );
  });
}
