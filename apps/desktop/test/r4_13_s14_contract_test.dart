// R4-13.S14 settings-save-to-effect contract (KcpItem instance).
//
// Proves the kcp transport fields save -> reopen. The xray kcp consumer
// (including the `CwndMultiplier >= 1` correction) is asserted in the Rust
// `r4_13_s14_kcp_item_reaches_codegen` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('KcpItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['KcpItem'] as Map<String, dynamic>;
      g['Mtu'] = 1400;
      g['Tti'] = 40;
      g['UplinkCapacity'] = 15;
      g['DownlinkCapacity'] = 90;
      g['CwndMultiplier'] = 3;
      g['MaxSendingWindow'] = 1024;
    });
    final g = doc['KcpItem'] as Map<String, dynamic>;
    expect(g['Mtu'], 1400);
    expect(g['Tti'], 40);
    expect(g['UplinkCapacity'], 15);
    expect(g['DownlinkCapacity'], 90);
    expect(g['CwndMultiplier'], 3);
    expect(g['MaxSendingWindow'], 1024);
  });

  test('KcpItem defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'KcpItem': <String, dynamic>{},
            })['KcpItem']
            as Map<String, dynamic>;
    expect(g['Mtu'], 1350);
    expect(g['Tti'], 50);
    expect(g['CwndMultiplier'], 1);
    expect(g['MaxSendingWindow'], 2097152);
  });
}
