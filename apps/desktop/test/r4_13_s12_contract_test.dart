// R4-13.S12 settings-save-to-effect contract (HysteriaItem instance).
//
// Proves the hysteria bandwidth/hop fields save -> reopen. The hysteria2
// generator consumer is asserted in the Rust
// `r4_13_s12_hysteria_reaches_codegen` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('Hysteria fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['HysteriaItem'] as Map<String, dynamic>;
      g['UpMbps'] = 55;
      g['DownMbps'] = 66;
      g['HopInterval'] = 42;
    });
    final g = doc['HysteriaItem'] as Map<String, dynamic>;
    expect(g['UpMbps'], 55);
    expect(g['DownMbps'], 66);
    expect(g['HopInterval'], 42);
  });

  test('Hysteria defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'HysteriaItem': <String, dynamic>{},
            })['HysteriaItem']
            as Map<String, dynamic>;
    expect(g['UpMbps'], 100);
    expect(g['DownMbps'], 100);
    expect(g['HopInterval'], 30);
  });
}
