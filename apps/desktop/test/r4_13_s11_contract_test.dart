// R4-13.S11 settings-save-to-effect contract (HappyEyeballs4RayItem instance).
//
// Proves the four happy-eyeballs fields save -> reopen. The xray DNS consumer
// is asserted in the Rust `r4_13_s11_happy_eyeballs_reaches_codegen` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('HappyEyeballs fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['HappyEyeballs4RayItem'] as Map<String, dynamic>;
      g['TryDelayMs'] = 300;
      g['PrioritizeIPv6'] = true;
      g['Interleave'] = 2;
      g['MaxConcurrentTry'] = 3;
    });
    final g = doc['HappyEyeballs4RayItem'] as Map<String, dynamic>;
    expect(g['TryDelayMs'], 300);
    expect(g['PrioritizeIPv6'], isTrue);
    expect(g['Interleave'], 2);
    expect(g['MaxConcurrentTry'], 3);
  });

  test('HappyEyeballs defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'HappyEyeballs4RayItem': <String, dynamic>{},
            })['HappyEyeballs4RayItem']
            as Map<String, dynamic>;
    expect(g['TryDelayMs'], 250);
    expect(g['PrioritizeIPv6'], isFalse);
    expect(g['Interleave'], 1);
    expect(g['MaxConcurrentTry'], 4);
  });
}
