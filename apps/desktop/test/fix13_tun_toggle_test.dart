import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/tun_toggle.dart';

void main() {
  const helperUnavailable = RuntimeErrorView(
    code: 'E_TUN_HELPER_UNAVAILABLE',
    messageKey: 'error.tun_helper_denied',
  );

  group('toggleTunDesired', () {
    test('failed persist never applies and reports failure', () async {
      var applied = false;
      final result = await toggleTunDesired(
        enabled: true,
        persist: (_) => false,
        apply: () async {
          applied = true;
          return true;
        },
      );
      expect(result.ok, isFalse);
      expect(result.runtimeApplied, isFalse);
      expect(result.error, 'error.settings_save_failed');
      expect(applied, isFalse, reason: 'a failed save must not touch runtime');
    });

    test('successful save then applies the plan', () async {
      var applied = false;
      final result = await toggleTunDesired(
        enabled: true,
        persist: (_) => true,
        apply: () async {
          applied = true;
          return true;
        },
      );
      expect(result.ok, isTrue);
      expect(result.runtimeApplied, isTrue);
      expect(applied, isTrue);
    });

    test('turning off persists false before applying', () async {
      bool? persisted;
      var applied = false;
      final result = await toggleTunDesired(
        enabled: false,
        persist: (v) {
          persisted = v;
          return true;
        },
        apply: () async {
          expect(persisted, isFalse);
          applied = true;
          return true;
        },
      );
      expect(result.ok, isTrue);
      expect(persisted, isFalse);
      expect(applied, isTrue);
    });

    test('rejected apply is not reported as applied', () async {
      final result = await toggleTunDesired(
        enabled: true,
        persist: (_) => true,
        apply: () async => false,
      );
      expect(result.ok, isFalse);
      expect(result.runtimeApplied, isFalse);
      expect(result.error, 'error.tun_apply_failed');
    });
  });

  group('tunActualLabel', () {
    test('switch off is 未启用 regardless of runtime', () {
      const runtime = RuntimeView(
        state: 'Running',
        ports: [11900],
        sessionId: 's',
      );
      expect(tunActualLabel(false, runtime), '未启用');
    });

    test('desired but stopped is 未启用, never a fake active', () {
      const runtime = RuntimeView(state: 'Stopped');
      expect(tunActualLabel(true, runtime), '未启用');
    });

    test('helper refusal surfaces as 失败已回滚', () {
      const runtime = RuntimeView(state: 'Stopped', error: helperUnavailable);
      expect(tunActualLabel(true, runtime), '失败已回滚');
    });

    test('live runtime reads 已请求(未验证), not a fabricated success', () {
      const runtime = RuntimeView(
        state: 'Running',
        ports: [11900],
        sessionId: 's-1',
      );
      expect(tunActualLabel(true, runtime), '已请求(未验证)');
    });

    test('live lease reads the real adapter fact', () {
      const runtime = RuntimeView(
        state: 'Running',
        ports: [11900],
        sessionId: 's-1',
        tun: RuntimeTunView(
          adapterName: 'v2rayn-tun',
          interfaceIndex: 84,
          routeCount: 0,
          dryRun: false,
        ),
      );
      expect(tunActualLabel(true, runtime), '已启用 (v2rayn-tun if=84)');
    });
  });
}
