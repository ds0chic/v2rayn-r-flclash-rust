// R4-29 repro: the application self-update path must never latch the window
// busy when the bridge call for the external-upgrade spec throws, and it must
// never fabricate a success or an exit hand-off. A missing own release source
// is a normal (ok=false) result, not an exception.
//
// These assertions encode the R4-29 contract and fail on the pre-fix
// controller (the awaited spec call had no catch; busy stayed true and the
// future completed with an error). Synthetic only: no runner, no exit, no
// network, no port, no install.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

/// A bridge whose app-update spec call throws a native-style exception, and
/// whose check call can be gated to model a hung/slow bridge.
class _FlakyUpdateBridge extends SyntheticBridgePort {
  bool throwOnSpec = false;
  bool throwOnCheck = false;

  @override
  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec() async {
    if (throwOnSpec) {
      throw StateError('native bridge exploded during app update spec');
    }
    return super.t16ApplyAppUpdateSpec();
  }

  @override
  Future<c.UpdateReportDto> t16CheckUpdates(
    List<String> cores,
    bool prerelease,
    bool viaProxy,
  ) async {
    if (throwOnCheck) {
      throw StateError('native bridge exploded during update check');
    }
    return super.t16CheckUpdates(cores, prerelease, viaProxy);
  }
}

/// Recording hand-off so the test proves no runner launch / no exit on failure.
class _Handoff {
  final List<String> launches = <String>[];
  int exits = 0;
}

ProviderContainer _container(_FlakyUpdateBridge bridge, _Handoff handoff) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge),
      updateControllerProvider.overrideWith(
        () => UpdateController(
          launchRunner: (helper, args, cwd) async {
            handoff.launches.add('$helper|${args.join(",")}|$cwd');
          },
          exitApp: () => handoff.exits++,
        ),
      ),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('a thrown spec call completes, reports an error and clears busy', () {
    final bridge = _FlakyUpdateBridge()..throwOnSpec = true;
    final handoff = _Handoff();
    final container = _container(bridge, handoff);
    final notifier = container.read(updateControllerProvider.notifier);

    // Pre-fix this future completes with an error and busy latches true.
    return expectLater(notifier.applyAppUpdate(), completes).then((_) {
      final state = container.read(updateControllerProvider);
      expect(
        state.busy,
        isFalse,
        reason: 'a bridge exception must not latch busy',
      );
      expect(state.status?.isError, isTrue);
      expect(handoff.launches, isEmpty);
      expect(handoff.exits, 0);
    });
  });

  test('a thrown check call completes, reports an error and clears busy', () {
    final bridge = _FlakyUpdateBridge()..throwOnCheck = true;
    final handoff = _Handoff();
    final container = _container(bridge, handoff);
    final notifier = container.read(updateControllerProvider.notifier);

    return expectLater(notifier.checkOnly(), completes).then((_) {
      final state = container.read(updateControllerProvider);
      expect(state.busy, isFalse);
      expect(state.status?.isError, isTrue);
    });
  });
}
