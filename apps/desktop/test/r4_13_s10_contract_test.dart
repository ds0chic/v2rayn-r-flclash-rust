// R4-13.S10 settings-save-to-effect contract (GuiItem instance).
//
// Proves the GuiItem flags save -> reopen and that the save path normalizes an
// out-of-list RootCertProvider to `system` (upstream `ConfigHandler.LoadConfig`
// parity). The statistics/speed codegen consumer is asserted in the Rust
// `r4_13_s10_gui_item_reaches_codegen` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('GuiItem flags save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['GuiItem'] as Map<String, dynamic>;
      g['EnableStatistics'] = true;
      g['DisplayRealTimeSpeed'] = true;
      g['KeepOlderDedupl'] = true;
      g['TrayMenuServersLimit'] = 42;
      g['AutoUpdateInterval'] = 30;
    });
    final g = doc['GuiItem'] as Map<String, dynamic>;
    expect(g['EnableStatistics'], isTrue);
    expect(g['DisplayRealTimeSpeed'], isTrue);
    expect(g['KeepOlderDedupl'], isTrue);
    expect(g['TrayMenuServersLimit'], 42);
    expect(g['AutoUpdateInterval'], 30);
  });

  test('out-of-list RootCertProvider is normalized to system', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      (draft['GuiItem'] as Map<String, dynamic>)['RootCertProvider'] =
          'not-a-provider';
    });
    expect(
      (doc['GuiItem'] as Map<String, dynamic>)['RootCertProvider'],
      'system',
    );
  });

  test('GuiItem defaults follow the upstream initializers', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'GuiItem': <String, dynamic>{},
            })['GuiItem']
            as Map<String, dynamic>;
    expect(g['EnableLog'], isTrue);
    expect(g['TrayMenuServersLimit'], 20);
    expect(g['RootCertProvider'], 'system');
  });
}
