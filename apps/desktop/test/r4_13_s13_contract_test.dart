// R4-13.S13 settings-save-to-effect contract (Inbound instance).
//
// Proves the first Inbound listener row persists across a reopen. The real
// generator consumer (every field except the caller-owned local port) is
// asserted in the Rust `r4_13_s13_inbound_reaches_codegen` test. A real
// listener binding needs an isolated environment and is registered as blocked.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('Inbound listener fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final row =
          (draft['Inbound'] as List<dynamic>).first as Map<String, dynamic>;
      row['LocalPort'] = 11808;
      row['UdpEnabled'] = false;
      row['SniffingEnabled'] = false;
      row['RouteOnly'] = true;
      row['AllowLANConn'] = true;
      row['NewPort4LAN'] = true;
      row['SecondLocalPortEnabled'] = true;
      row['User'] = 'u';
      row['Pass'] = 'p';
      row['DestOverride'] = <dynamic>['tls'];
    });
    final row = (doc['Inbound'] as List<dynamic>).first as Map<String, dynamic>;
    expect(row['LocalPort'], 11808);
    expect(row['UdpEnabled'], isFalse);
    expect(row['SniffingEnabled'], isFalse);
    expect(row['RouteOnly'], isTrue);
    expect(row['AllowLANConn'], isTrue);
    expect(row['NewPort4LAN'], isTrue);
    expect(row['SecondLocalPortEnabled'], isTrue);
    expect(row['User'], 'u');
    expect(row['Pass'], 'p');
    expect(row['DestOverride'], <dynamic>['tls']);
  });

  test('the seeded default carries one socks listener', () {
    final doc = defaultSettingsJson();
    final rows = doc['Inbound'] as List<dynamic>;
    expect(rows.length, 1);
    final row = rows.first as Map<String, dynamic>;
    expect(row['Protocol'], 0);
    expect(row['UdpEnabled'], isTrue);
  });
}
