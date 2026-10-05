// R4-13.S09 settings-save-to-effect contract (GrpcItem instance).
//
// GrpcItem has no UI control upstream (it is edited only through the persisted
// document), so the contract is persistence plus the generator consumer proven
// in the Rust `r4_13_s09_grpc_item_reaches_codegen` test.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('GrpcItem save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['GrpcItem'] as Map<String, dynamic>;
      g['IdleTimeout'] = 11;
      g['HealthCheckTimeout'] = 7;
      g['PermitWithoutStream'] = true;
      g['InitialWindowsSize'] = 65535;
    });
    final g = doc['GrpcItem'] as Map<String, dynamic>;
    expect(g['IdleTimeout'], 11);
    expect(g['HealthCheckTimeout'], 7);
    expect(g['PermitWithoutStream'], isTrue);
    expect(g['InitialWindowsSize'], 65535);
  });

  test('absent GrpcItem fields keep the frozen defaults', () {
    final merged = mergeWithSettingsDefaults(<String, dynamic>{
      'GrpcItem': <String, dynamic>{},
    });
    final g = merged['GrpcItem'] as Map<String, dynamic>;
    expect(g['IdleTimeout'], 60);
    expect(g['HealthCheckTimeout'], 20);
    expect(g['PermitWithoutStream'], isFalse);
    expect(g['InitialWindowsSize'], 0);
  });
}
