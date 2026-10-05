import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/tun_toggle.dart';

/// R4-25 synthetic contract: the TUN toggle code path
/// (switch -> persist desired -> authorize/apply -> actual-state readback)
/// must persist before applying, never touch a running session on a failed
/// save, and never fabricate an active/closed result from the desired flag.
///
/// These assertions pin the Dart seam that R3-04/R3-05 (net-host) and
/// R4-12/R4-24 (settings/apply reconcile) feed. Real auto-route TUN, global
/// takeover, DNS/IPv6/exclude and crash/sleep recovery stay `blocked` and
/// require an authorized isolated VM; nothing here touches the host network.
void main() {
  const helperUnavailable = RuntimeErrorView(
    code: 'E_TUN_HELPER_UNAVAILABLE',
    messageKey: 'error.tun_helper_denied',
  );

  group('R4-25 toggle persists the frozen target before applying', () {
    test(
      'enable writes the desired flag, then applies the real plan',
      () async {
        final order = <String>[];
        final result = await toggleTunDesired(
          enabled: true,
          persist: (enabled) {
            order.add('persist:$enabled');
            return true;
          },
          apply: () async {
            order.add('apply');
            return true;
          },
        );
        expect(order, ['persist:true', 'apply']);
        expect(result.ok, isTrue);
        expect(result.runtimeApplied, isTrue);
      },
    );

    test('disable persists false first, then re-applies', () async {
      final order = <String>[];
      final result = await toggleTunDesired(
        enabled: false,
        persist: (enabled) {
          order.add('persist:$enabled');
          return true;
        },
        apply: () async {
          order.add('apply');
          return true;
        },
      );
      expect(order, ['persist:false', 'apply']);
      expect(result.ok, isTrue);
    });
  });

  group('R4-25 failed save keeps the old runtime and never fakes success', () {
    test(
      'a failed persist does not apply and reports the field error',
      () async {
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
        expect(
          applied,
          isFalse,
          reason: 'a failed save must not touch the running session',
        );
      },
    );

    test('authorization denial / cancel after save is never ok', () async {
      await expectLater(
        toggleTunDesired(
          enabled: true,
          persist: (_) => true,
          apply: () async => throw StateError('E_TUN_HELPER_UNAVAILABLE'),
        ),
        throwsA(isA<StateError>()),
      );
    });
  });

  group('R4-25 actual TUN state comes from the runtime, not the switch', () {
    test('switch off reads 未启用 even with a Running runtime', () {
      const runtime = RuntimeView(
        state: 'Running',
        ports: [11900],
        sessionId: 's',
      );
      expect(tunActualLabel(false, runtime), '未启用');
    });

    test('desired but stopped reads 未启用, never a fake active', () {
      const runtime = RuntimeView(state: 'Stopped');
      expect(tunActualLabel(true, runtime), '未启用');
    });

    test('helper refusal reads 失败已回滚', () {
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
  });
}
