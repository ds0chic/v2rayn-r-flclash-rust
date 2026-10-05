// R4-13.S20 settings-save-to-effect contract (SpeedTestItem instance).
//
// Proves the speedtest fields save -> reopen. The speedtest engine consumer
// (with the `>=10` timeout/concurrency corrections and blank-URL fallbacks) is
// asserted in the Rust `r4_13_s20_speed_test_reaches_engine` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('SpeedTestItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['SpeedTestItem'] as Map<String, dynamic>;
      g['SpeedTestTimeout'] = 20;
      g['MixedConcurrencyCount'] = 16;
      g['SpeedTestUrl'] = 'https://speed.example/10mb.test';
      g['SpeedPingTestUrl'] = 'https://speed.example/ping';
      g['UdpTestTarget'] = 'ntp:example.org';
      g['IPAPIUrl'] = 'https://ip.example/json';
      g['SpeedTestPageSize'] = 5;
      g['SpeedTestDelayInterval'] = 2;
    });
    final g = doc['SpeedTestItem'] as Map<String, dynamic>;
    expect(g['SpeedTestTimeout'], 20);
    expect(g['MixedConcurrencyCount'], 16);
    expect(g['SpeedTestUrl'], 'https://speed.example/10mb.test');
    expect(g['SpeedPingTestUrl'], 'https://speed.example/ping');
    expect(g['UdpTestTarget'], 'ntp:example.org');
    expect(g['IPAPIUrl'], 'https://ip.example/json');
    expect(g['SpeedTestPageSize'], 5);
    expect(g['SpeedTestDelayInterval'], 2);
  });

  test('SpeedTestItem defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'SpeedTestItem': <String, dynamic>{},
            })['SpeedTestItem']
            as Map<String, dynamic>;
    expect(g['SpeedTestTimeout'], 10);
    expect(g['MixedConcurrencyCount'], 10);
    expect(g['SpeedTestUrl'], 'https://cachefly.cachefly.net/50mb.test');
    expect(g['UdpTestTarget'], 'ntp:pool.ntp.org');
  });
}
