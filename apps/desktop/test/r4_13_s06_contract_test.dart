// R4-13.S06 settings-save-to-effect contract (CoreTypeItem instance).
//
// Proves the per-config-type core rows persist across a reopen. The real
// consumer (`AppSettings::core_for` -> engine core decision) is asserted in the
// Rust `r4_13_s06_core_type_item_reaches_core_selection` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('CoreTypeItem rows save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      draft['CoreTypeItem'] = <dynamic>[
        <String, dynamic>{'ConfigType': 1, 'CoreType': 2},
        <String, dynamic>{'ConfigType': 5, 'CoreType': 1},
      ];
    });
    final rows = doc['CoreTypeItem'] as List<dynamic>;
    expect(rows.length, 2);
    expect((rows[0] as Map<String, dynamic>)['ConfigType'], 1);
    expect((rows[0] as Map<String, dynamic>)['CoreType'], 2);
    expect((rows[1] as Map<String, dynamic>)['CoreType'], 1);
  });

  test('absent CoreTypeItem stays null until the window seeds it', () {
    final merged = mergeWithSettingsDefaults(<String, dynamic>{});
    expect(merged['CoreTypeItem'], isNull);
  });
}
