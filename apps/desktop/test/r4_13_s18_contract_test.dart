// R4-13.S18 settings-save-to-effect contract (RoutingBasicItem instance).
//
// Proves the routing-basic strategy fields save -> reopen. The xray/sing-box
// routing consumer (with the `AsIs` fallback) is asserted in the Rust
// `r4_13_s18_routing_basic_reaches_codegen` test. RoutingIndexId selects the
// active routing profile in the routing engine.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('RoutingBasicItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['RoutingBasicItem'] as Map<String, dynamic>;
      g['DomainStrategy'] = 'IPIfNonMatch';
      g['DomainStrategy4Singbox'] = 'prefer_ipv4';
      g['RoutingIndexId'] = 'profile-1';
    });
    final g = doc['RoutingBasicItem'] as Map<String, dynamic>;
    expect(g['DomainStrategy'], 'IPIfNonMatch');
    expect(g['DomainStrategy4Singbox'], 'prefer_ipv4');
    expect(g['RoutingIndexId'], 'profile-1');
  });

  test('RoutingBasicItem defaults to the AsIs strategy', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'RoutingBasicItem': <String, dynamic>{},
            })['RoutingBasicItem']
            as Map<String, dynamic>;
    expect(g['DomainStrategy'], 'AsIs');
    expect(g['DomainStrategy4Singbox'], isNull);
    expect(g['RoutingIndexId'], isNull);
  });
}
