// R4-13.S19 settings-save-to-effect contract (SimpleDNSItem instance).
//
// Proves the SimpleDNS fields save -> reopen. The generator SimpleDns consumer
// is asserted in the Rust `r4_13_s19_simple_dns_reaches_codegen` test. The DNS
// settings window edits the remaining fields; this instance covers the
// persisted document -> codegen chain.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('SimpleDNSItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['SimpleDNSItem'] as Map<String, dynamic>;
      g['UseSystemHosts'] = true;
      g['FakeIP'] = true;
      g['GlobalFakeIp'] = false;
      g['FakeIPRange'] = '198.19.0.0/16';
      g['RemoteDNS'] = 'https://dns.example/dns-query';
      g['ServeStale'] = true;
      g['ParallelQuery'] = true;
      g['EnableHappyEyeballs'] = true;
    });
    final g = doc['SimpleDNSItem'] as Map<String, dynamic>;
    expect(g['UseSystemHosts'], isTrue);
    expect(g['FakeIP'], isTrue);
    expect(g['GlobalFakeIp'], isFalse);
    expect(g['FakeIPRange'], '198.19.0.0/16');
    expect(g['RemoteDNS'], 'https://dns.example/dns-query');
    expect(g['ServeStale'], isTrue);
    expect(g['ParallelQuery'], isTrue);
    expect(g['EnableHappyEyeballs'], isTrue);
  });

  test('SimpleDNSItem absent fields keep the built-in defaults', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'SimpleDNSItem': <String, dynamic>{},
            })['SimpleDNSItem']
            as Map<String, dynamic>;
    expect(g['DirectDNS'], '119.29.29.29');
    expect(g['BlockAAAAQuery'], isFalse);
    expect(g['ServeStale'], isFalse);
  });
}
