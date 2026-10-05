// R4-13.S02 settings-save-to-effect contract (ClashUIItem instance).
//
// Proves the ClashUIItem fields save through the real controller and reopen
// with the frozen defaults for null/absent fields. The codegen/mihomo consumer
// mapping (EnableIPv6, EnableMixinContent) is asserted in the Rust
// `r4_13_s02_clash_ui_item_reaches_mihomo_merge` test; the refresh/column
// fields are Clash view preferences.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('ClashUIItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['ClashUIItem'] as Map<String, dynamic>;
      g['EnableIPv6'] = true;
      g['EnableMixinContent'] = true;
      g['ProxiesSorting'] = 3;
      g['ProxiesAutoRefresh'] = true;
      g['ProxiesRefreshInterval'] = 5;
      g['ConnectionsAutoRefresh'] = true;
      g['ConnectionsRefreshInterval'] = 7;
    });
    final g = doc['ClashUIItem'] as Map<String, dynamic>;
    expect(g['EnableIPv6'], isTrue);
    expect(g['EnableMixinContent'], isTrue);
    expect(g['ProxiesSorting'], 3);
    expect(g['ProxiesAutoRefresh'], isTrue);
    expect(g['ProxiesRefreshInterval'], 5);
    expect(g['ConnectionsAutoRefresh'], isTrue);
    expect(g['ConnectionsRefreshInterval'], 7);
  });

  test('ClashUIItem null/absent fields get frozen defaults', () {
    final merged = mergeWithSettingsDefaults(<String, dynamic>{
      'ClashUIItem': <String, dynamic>{'EnableIPv6': null},
    });
    final g = merged['ClashUIItem'] as Map<String, dynamic>;
    expect(g['EnableIPv6'], isFalse);
    expect(g['ProxiesSorting'], 0);
    expect(g['ProxiesRefreshInterval'], 2);
    expect(g['ConnectionsRefreshInterval'], 2);
    expect(g['ConnectionsColumnItem'], isEmpty);
  });
}
