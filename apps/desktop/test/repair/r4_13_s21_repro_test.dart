// R4-13.S21 repro (SystemProxyItem applied-mode reconciliation).
//
// Upstream `OptionSettingViewModel.SaveSettingAsync` -> `Reload` -> `LoadCore`
// -> `SysProxyHandler.UpdateSysProxy` re-points the selected system proxy after
// a settings save. R4-24 wired the runtime-session listener; a `SystemProxyItem`
// save whose applied session did not change still never reconciled the platform
// mode. The fix is the settings-change reconciliation in `PlatformController`.
//
// Synthetic data only: no native library, kernel, network, host proxy or user
// data. The synthetic port 11809 is >= 11808 and never 10808.
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

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

Map<String, dynamic> _seedDoc() {
  final doc = defaultSettingsJson();
  (doc['SystemProxyItem'] as Map<String, dynamic>)['SysProxyType'] = 0;
  ((doc['Inbound'] as List).first as Map<String, dynamic>)['LocalPort'] = 11809;
  return doc;
}

void main() {
  test('saving SystemProxyItem re-points the host mode while a session stays applied', () async {
    final bridge = SyntheticBridgePort();
    bridge.saveSettingsJson(jsonEncode(_seedDoc()), 0);
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
        bridgePortProvider.overrideWithValue(bridge),
        platformBridgeProvider.overrideWithValue(platform),
        runtimeBridgeProvider.overrideWithValue(runtime),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      ],
    );
    addTearDown(container.dispose);

    container.read(platformControllerProvider.notifier);
    container.read(settingsControllerProvider.notifier).load();
    await container.read(runtimeControllerProvider.notifier).refresh();
    expect(platform.appliedModes, contains(SysProxyMode.forcedClear));

    final controller = container.read(settingsControllerProvider.notifier);
    final draft = mergeWithSettingsDefaults(controller.draft());
    (draft['SystemProxyItem'] as Map<String, dynamic>)['SysProxyType'] = 1;
    final outcome = await controller.saveAndApply(draft);
    expect(outcome.saved, isTrue);

    expect(
      platform.appliedModes,
      contains(SysProxyMode.forcedChange),
      reason: 'a saved mode change must re-run UpdateSysProxy at the applied endpoint',
    );
    expect(platform.lastServer, contains('11809'));

    // A repeated snapshot of the same session must not rewrite the host.
    final changes = platform.appliedModes
        .where((mode) => mode == SysProxyMode.forcedChange)
        .length;
    await container.read(runtimeControllerProvider.notifier).refresh();
    expect(
      platform.appliedModes
          .where((mode) => mode == SysProxyMode.forcedChange)
          .length,
      changes,
    );
  });
}
