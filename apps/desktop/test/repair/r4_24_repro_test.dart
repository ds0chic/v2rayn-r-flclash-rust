// R4-24 repro. Expected to FAIL pre-fix:
//  - Upstream `MainWindowViewModel.LoadCore` -> `SysProxyHandler.UpdateSysProxy`
//    re-points the selected system proxy / PAC after *every* successful apply.
//    The current desktop wiring only reconciled the persisted mode on launch
//    restore and on an explicit status-bar/tray toggle, so selecting a mode and
//    then applying a new inbound port left the host proxy/PAC pointing at the
//    previous endpoint (plan D19).
//  - The repro drives a real runtime apply success (a new session id + applied
//    port) and asserts the platform bridge received the mode apply with the
//    *actual applied* endpoint, not the configured desired one.
//
// Synthetic data only: no native library, kernel, network, user data or host
// proxy write. The port (11809) is >= 11808 and never 10808.
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import '../support/fake_platform_bridge.dart';

class _RunningRuntimeBridge implements RuntimeBridge {
  _RunningRuntimeBridge(this._view);

  RuntimeView _view;

  @override
  Future<RuntimeView> snapshot() async => _view;

  @override
  String? activeProfileId() => 'synthetic-active';

  @override
  Future<RuntimeActionResult> applyActive({
    required BigInt expectedRevision,
  }) async => const RuntimeActionResult(ok: true);

  @override
  Future<RuntimeActionResult> stop() async {
    _view = const RuntimeView();
    return const RuntimeActionResult(ok: true);
  }

  @override
  Stream<RuntimeEvent> events() => const Stream<RuntimeEvent>.empty();
}

class _DocSettingsController extends SettingsController {
  _DocSettingsController(this._doc);
  final Map<String, dynamic> _doc;

  @override
  SettingsViewState build() => SettingsViewState(loaded: true, document: _doc);
}

Map<String, dynamic> _forcedChangeDoc(int appliedPort) => <String, dynamic>{
  'SystemProxyItem': <String, dynamic>{'SysProxyType': 1},
  'Inbound': <Map<String, dynamic>>[
    <String, dynamic>{'LocalPort': appliedPort, 'Protocol': 0},
  ],
};

void main() {
  test(
    'apply success re-points the selected mode at the actual applied endpoint',
    () async {
      final platform = FakePlatformBridge();
      final runtime = _RunningRuntimeBridge(
        const RuntimeView(
          state: 'Running',
          hostAlive: true,
          ports: <int>[11809],
          sessionId: 'session-1',
        ),
      );
      final container = ProviderContainer(
        overrides: [
          bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
          platformBridgeProvider.overrideWithValue(platform),
          runtimeBridgeProvider.overrideWithValue(runtime),
          settingsControllerProvider.overrideWith(
            () => _DocSettingsController(_forcedChangeDoc(11809)),
          ),
        ],
      );
      addTearDown(container.dispose);

      // The platform listener is registered before the runtime publishes the
      // session; the refresh is the "apply success" transition.
      container.read(platformControllerProvider.notifier);
      await container.read(runtimeControllerProvider.notifier).refresh();

      expect(
        platform.appliedModes,
        contains(SysProxyMode.forcedChange),
        reason: 'a new applied session must re-run UpdateSysProxy',
      );
      expect(
        platform.lastServer,
        'socks=127.0.0.1:11809',
        reason: 'the applied port/protocol must drive the host proxy',
      );

      // A repeated snapshot of the same session must not rewrite the host.
      await container.read(runtimeControllerProvider.notifier).refresh();
      expect(platform.appliedModes, hasLength(1));
    },
  );
}
