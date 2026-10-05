// R4-13.S22 repro (TunModeItem -> generated config guard).
//
// The persisted -> generated consumer chain is asserted in Rust:
//   crates/application/src/codegen.rs
//     ::tests::r4_13_s22_tun_fields_reach_generated_config
// which proves `AutoRoute` / `StrictRoute` / `Stack` / `IcmpRouting` / `Mtu` /
// IPv4+IPv6 / `RouteExcludeAddress` reach the real sing-box and xray TUN
// inbounds. This Dart guard covers the storage half: a partial TUN edit must
// merge with the frozen defaults without coercing the explicit mode-sensitive
// values, and persist across a reopen.
//
// Synthetic data only: no native library, kernel, device, route or user data.
import 'package:flutter_test/flutter_test.dart';

import '../support/r4_13_settings_support.dart';

void main() {
  test(
    'a partial TunModeItem edit keeps explicit values and defaults the rest',
    () async {
      final bridge = seededBridge(<String, dynamic>{
        'TunModeItem': <String, dynamic>{
          'EnableTun': true,
          'AutoRoute': false,
          'Stack': 'system',
        },
      });
      final doc = await saveAndReopen(bridge, (draft) {
        final g = draft['TunModeItem'] as Map<String, dynamic>;
        g['Mtu'] = 1400;
      });
      final g = doc['TunModeItem'] as Map<String, dynamic>;
      expect(g['EnableTun'], isTrue);
      expect(
        g['AutoRoute'],
        isFalse,
        reason: 'an explicit false must not be coerced by the default merge',
      );
      expect(g['Stack'], 'system');
      expect(g['Mtu'], 1400);
      expect(
        g['StrictRoute'],
        isTrue,
        reason: 'absent field keeps its default',
      );
      expect(
        g['IcmpRouting'],
        'rule',
        reason: 'absent field keeps its default',
      );
    },
  );
}
