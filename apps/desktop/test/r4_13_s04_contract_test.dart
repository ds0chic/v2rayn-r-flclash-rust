// R4-13.S04 settings-save-to-effect contract (ConstItem instance).
//
// Proves the four ConstItem source URLs persist across a reopen. The real
// consumer fallbacks (stored value when non-blank, else the upstream built-in)
// are asserted in `crates/application/src/dns.rs` (`effective_geo_source_url`,
// `effective_srs_source_url`, `effective_sub_convert_url`,
// `effective_route_rules_template_source_url`) and used by `subs.rs`/`engine.rs`.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('ConstItem source urls save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['ConstItem'] as Map<String, dynamic>;
      g['SubConvertUrl'] = 'https://convert.example/sub?url={0}';
      g['GeoSourceUrl'] = 'https://mirror.example/{0}.dat';
      g['SrsSourceUrl'] = 'https://mirror.example/{0}/{1}.srs';
      g['RouteRulesTemplateSourceUrl'] =
          'https://mirror.example/routing/{0}.json';
    });
    final g = doc['ConstItem'] as Map<String, dynamic>;
    expect(g['SubConvertUrl'], 'https://convert.example/sub?url={0}');
    expect(g['GeoSourceUrl'], 'https://mirror.example/{0}.dat');
    expect(g['SrsSourceUrl'], 'https://mirror.example/{0}/{1}.srs');
    expect(
      g['RouteRulesTemplateSourceUrl'],
      'https://mirror.example/routing/{0}.json',
    );
  });

  test(
    'blank ConstItem urls stay authored (consumer applies the fallback)',
    () {
      final merged = mergeWithSettingsDefaults(<String, dynamic>{
        'ConstItem': <String, dynamic>{
          'GeoSourceUrl': '   ',
          'SrsSourceUrl': null,
        },
      });
      final g = merged['ConstItem'] as Map<String, dynamic>;
      expect(g['GeoSourceUrl'], '   ');
      expect(g['SrsSourceUrl'], isNull);
      expect(g['SubConvertUrl'], isNull);
    },
  );
}
