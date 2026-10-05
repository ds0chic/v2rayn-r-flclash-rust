// R4-13.S08 settings-save-to-effect contract (GlobalHotkeys instance).
//
// Proves hotkey rows persist across a reopen. The OS registration effect needs
// an isolated authorized environment and is registered as blocked; the stored
// rows + key codec are the application-side consumer boundary.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('GlobalHotkeys rows save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      draft['GlobalHotkeys'] = <dynamic>[
        <String, dynamic>{
          'EGlobalHotkey': 0,
          'Alt': false,
          'Control': true,
          'Shift': false,
          'KeyCode': 65,
        },
      ];
    });
    final rows = doc['GlobalHotkeys'] as List<dynamic>;
    expect(rows.length, 1);
    final row = rows.first as Map<String, dynamic>;
    expect(row['EGlobalHotkey'], 0);
    expect(row['Control'], isTrue);
    expect(row['KeyCode'], 65);
  });

  test('the default hotkey list is empty (nothing registered)', () {
    expect(defaultSettingsJson()['GlobalHotkeys'], isEmpty);
  });
}
