import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/tun_toggle.dart';

/// SP-10 label contracts (CP-12 / TUN-A03): the TUN label reads only live
/// runtime facts over the runtime_tun chain. A dry-run lease is simulated,
/// never enabled; desired=false with a live lease is still-enabled, never
/// closed. Red before the SP-10 label fix.
void main() {
  group('sp10 tunActualLabel reads live facts', () {
    test('dry-run lease reads simulated, never enabled', () {
      const runtime = RuntimeView(
        state: 'Running',
        ports: [11900],
        sessionId: 's-1',
        tun: RuntimeTunView(
          adapterName: 'v2rayn-tun',
          interfaceIndex: 9,
          routeCount: 1,
          dryRun: true,
        ),
      );
      expect(tunActualLabel(true, runtime), '模拟 (v2rayn-tun if=9)');
    });

    test('desired off with a live lease reads still-enabled, never closed', () {
      const runtime = RuntimeView(
        state: 'Running',
        ports: [11900],
        sessionId: 's-1',
        tun: RuntimeTunView(
          adapterName: 'v2rayn-tun',
          interfaceIndex: 9,
          routeCount: 1,
          dryRun: false,
        ),
      );
      expect(tunActualLabel(false, runtime), '仍启用(关闭待确认)');
    });
  });
}
