// R4-13.S03 settings-save-to-effect contract (ConfigRoot instance).
//
// Proves the Config root carries every top-level group and a multi-group edit
// persists across a reopen through the real controller.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('Config root exposes every registered group', () {
    final doc = defaultSettingsJson();
    for (final key in defaultSettingsGroups) {
      expect(doc.containsKey(key), isTrue, reason: key);
    }
    expect(defaultSettingsGroups.length, 25);
  });

  test('multi-group edit persists across reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      draft['IndexId'] = 'node-1';
      (draft['CoreBasicItem'] as Map<String, dynamic>)['Loglevel'] = 'info';
      (draft['ConstItem'] as Map<String, dynamic>)['SubConvertUrl'] =
          'https://convert.example/sub?url={0}';
      (draft['ClashUIItem'] as Map<String, dynamic>)['EnableIPv6'] = true;
    });
    expect(doc['IndexId'], 'node-1');
    expect((doc['CoreBasicItem'] as Map<String, dynamic>)['Loglevel'], 'info');
    expect(
      (doc['ConstItem'] as Map<String, dynamic>)['SubConvertUrl'],
      'https://convert.example/sub?url={0}',
    );
    expect((doc['ClashUIItem'] as Map<String, dynamic>)['EnableIPv6'], isTrue);
  });
}
