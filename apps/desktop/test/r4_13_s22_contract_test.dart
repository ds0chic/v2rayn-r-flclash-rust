// R4-13.S22 settings-save-to-effect contract (TunModeItem instance).
//
// Storage chain only: every `TunModeItem` field persists and is visible after
// reopen. The persisted -> generated (sing-box / xray TUN inbound) consumer
// chain is asserted in the Rust `r4_13_s22_tun_fields_reach_generated_config`
// test. Real device creation / routing is a blocked platform path.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('TunModeItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['TunModeItem'] as Map<String, dynamic>;
      g['EnableTun'] = true;
      g['AutoRoute'] = false;
      g['StrictRoute'] = true;
      g['Stack'] = 'system';
      g['Mtu'] = 1400;
      g['EnableIPv6Address'] = true;
      g['IcmpRouting'] = 'direct';
      g['EnableLegacyProtect'] = false;
      g['RouteExcludeAddress'] = <String>['10.0.0.0/8'];
      g['IPv4Address'] = '172.18.0.1/30';
      g['IPv6Address'] = 'fd00::1/64';
    });
    final g = doc['TunModeItem'] as Map<String, dynamic>;
    expect(g['EnableTun'], isTrue);
    expect(g['AutoRoute'], isFalse);
    expect(g['StrictRoute'], isTrue);
    expect(g['Stack'], 'system');
    expect(g['Mtu'], 1400);
    expect(g['EnableIPv6Address'], isTrue);
    expect(g['IcmpRouting'], 'direct');
    expect(g['EnableLegacyProtect'], isFalse);
    expect(g['RouteExcludeAddress'], <dynamic>['10.0.0.0/8']);
    expect(g['IPv4Address'], '172.18.0.1/30');
    expect(g['IPv6Address'], 'fd00::1/64');
  });

  test('TunModeItem defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'TunModeItem': <String, dynamic>{},
            })['TunModeItem']
            as Map<String, dynamic>;
    expect(g['EnableTun'], isFalse);
    expect(g['AutoRoute'], isTrue);
    expect(g['StrictRoute'], isTrue);
    expect(g['Stack'], isNull);
    expect(g['Mtu'], 9000);
    expect(g['IcmpRouting'], 'rule');
    expect(g['EnableLegacyProtect'], isTrue);
    expect(g['RouteExcludeAddress'], isNull);
  });
}
