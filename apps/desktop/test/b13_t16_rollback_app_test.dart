// B13 (B14 request): BridgePort carries the app-upgrade rollback seam;
// FrbBridgePort passes it straight through, SyntheticBridgePort stubs it.
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';

void main() {
  test('rollback app upgrade defaults to a no-op success', () {
    final bridge = SyntheticBridgePort(count: 5);
    final result = bridge.t16RollbackAppUpgrade();
    expect(result.ok, isTrue);
    expect(result.restored, isNull);
    expect(bridge.t16Calls, contains('rollback_app_upgrade'));
  });

  test('rollback app upgrade serves a queued result', () {
    final bridge = SyntheticBridgePort(count: 5);
    bridge.rollbackAppResults.add(
      const c.AppRollbackResultDto(ok: true, restored: '7.25.4'),
    );
    final result = bridge.t16RollbackAppUpgrade();
    expect(result.ok, isTrue);
    expect(result.restored, '7.25.4');
  });
}
