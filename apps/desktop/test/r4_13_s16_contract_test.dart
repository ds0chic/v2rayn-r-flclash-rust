// R4-13.S16 settings-save-to-effect contract (Mux4RayItem instance).
//
// Proves the xray mux fields save -> reopen. The xray mux consumer (with the
// frozen 8/16/`reject` fallbacks) is asserted in the Rust
// `r4_13_s16_mux4_ray_reaches_codegen` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('Mux4RayItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['Mux4RayItem'] as Map<String, dynamic>;
      g['Concurrency'] = 4;
      g['XudpConcurrency'] = 9;
      g['XudpProxyUDP443'] = 'skip';
    });
    final g = doc['Mux4RayItem'] as Map<String, dynamic>;
    expect(g['Concurrency'], 4);
    expect(g['XudpConcurrency'], 9);
    expect(g['XudpProxyUDP443'], 'skip');
  });

  test('Mux4RayItem defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'Mux4RayItem': <String, dynamic>{},
            })['Mux4RayItem']
            as Map<String, dynamic>;
    expect(g['Concurrency'], 8);
    expect(g['XudpConcurrency'], 16);
    expect(g['XudpProxyUDP443'], 'reject');
  });
}
