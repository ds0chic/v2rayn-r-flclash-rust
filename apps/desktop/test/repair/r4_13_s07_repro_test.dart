// R4-13.S07 repro: the UI load-parity merge dropped the legacy fragment
// scalars.
//
// Upstream `ConfigHandler.LoadConfig:181-187` seeds `Fragment4RayItem.Lengths`
// / `Delays` from the legacy `Length` / `Interval` when the lists are empty.
// The pre-fix `mergeWithSettingsDefaults` kept the empty lists as-is, so the
// option window showed/ persisted blank ranges instead of the stored value.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

void main() {
  test('empty fragment lists are seeded from legacy Length/Interval', () {
    final merged = mergeWithSettingsDefaults(<String, dynamic>{
      'Fragment4RayItem': <String, dynamic>{
        'Packets': '1-2',
        'Lengths': <dynamic>[],
        'Delays': <dynamic>[],
        'Length': '100-200',
        'Interval': '30-40',
      },
    });
    final fragment = merged['Fragment4RayItem'] as Map<String, dynamic>;
    expect(fragment['Lengths'], <String>['100-200']);
    expect(fragment['Delays'], <String>['30-40']);
  });
}
