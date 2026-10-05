// R4-13.S21 settings-save-to-effect contract (SystemProxyItem instance).
//
// Storage chain only: the four `ESysProxyType` modes and the PAC/exception
// fields persist and are visible after reopen. The real WinINET write/read is a
// blocked platform path (no isolated machine); the applied-endpoint
// reconciliation guard lives in test/repair/r4_13_s21_repro_test.dart.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  for (final mode in <int>[0, 1, 2, 3]) {
    test('SystemProxyItem mode $mode and PAC fields save -> reopen', () async {
      final bridge = seededBridge();
      final doc = await saveAndReopen(bridge, (draft) {
        final g = draft['SystemProxyItem'] as Map<String, dynamic>;
        g['SysProxyType'] = mode;
        g['SystemProxyExceptions'] = 'localhost;10.*;192.168.*';
        g['NotProxyLocalAddress'] = false;
        g['SystemProxyAdvancedProtocol'] = 'http';
        g['CustomSystemProxyPacPath'] = r'C:\synthetic\pac.txt';
        g['CustomSystemProxyScriptPath'] = r'C:\synthetic\proxy.pac';
      });
      final g = doc['SystemProxyItem'] as Map<String, dynamic>;
      expect(g['SysProxyType'], mode);
      expect(g['SystemProxyExceptions'], 'localhost;10.*;192.168.*');
      expect(g['NotProxyLocalAddress'], isFalse);
      expect(g['SystemProxyAdvancedProtocol'], 'http');
      expect(g['CustomSystemProxyPacPath'], r'C:\synthetic\pac.txt');
      expect(g['CustomSystemProxyScriptPath'], r'C:\synthetic\proxy.pac');
    });
  }

  test('SystemProxyItem defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'SystemProxyItem': <String, dynamic>{},
            })['SystemProxyItem']
            as Map<String, dynamic>;
    expect(g['SysProxyType'], 0);
    expect(g['NotProxyLocalAddress'], isTrue);
    expect(g['SystemProxyAdvancedProtocol'], isNull);
    expect(g['CustomSystemProxyPacPath'], isNull);
    expect(g['CustomSystemProxyScriptPath'], isNull);
  });
}
