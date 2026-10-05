// R4-13.S17 settings-save-to-effect contract (Mux4SboxItem instance).
//
// Proves the sing-box mux fields save -> reopen. The sing-box mux consumer
// (with the `h2mux` fallback) is asserted in the Rust
// `r4_13_s17_mux4_sbox_reaches_codegen` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('Mux4SboxItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['Mux4SboxItem'] as Map<String, dynamic>;
      g['Protocol'] = 'smux';
      g['MaxConnections'] = 12;
      g['Padding'] = true;
    });
    final g = doc['Mux4SboxItem'] as Map<String, dynamic>;
    expect(g['Protocol'], 'smux');
    expect(g['MaxConnections'], 12);
    expect(g['Padding'], isTrue);
  });

  test('Mux4SboxItem defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'Mux4SboxItem': <String, dynamic>{},
            })['Mux4SboxItem']
            as Map<String, dynamic>;
    expect(g['Protocol'], 'h2mux');
    expect(g['MaxConnections'], 8);
    expect(g['Padding'], isNull);
  });
}
