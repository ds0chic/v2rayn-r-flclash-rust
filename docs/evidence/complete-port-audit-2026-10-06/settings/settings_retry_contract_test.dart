import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

import '../../../../apps/desktop/test/support/counting_runtime_bridge.dart';
import '../../../../apps/desktop/test/support/fake_platform_bridge.dart';

class RejectAutostart extends FakePlatformBridge {
  int attempts = 0;

  @override
  bool setAutostart({
    required String name,
    required bool enabled,
    required String exe,
    required String args,
  }) {
    attempts++;
    return false;
  }
}

class RejectGroupBridge extends SyntheticBridgePort {
  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String json,
    int expectedRevision,
  ) => const settings.SaveSettingsResult(
    ok: false,
    changes: [],
    restartCoreFields: [],
    restartAppFields: [],
    nextLaunchFields: [],
    error: contract.ErrorDto(
      code: 'E_STORAGE_UNAVAILABLE',
      messageKey: 'error.storage_unavailable',
      retryable: true,
    ),
  );
}

ProviderContainer containerFor(
  BridgePort bridge,
  CountingRuntimeBridge runtime, {
  PlatformBridge? platform,
}) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    runtimeBridgeProvider.overrideWithValue(runtime),
    platformBridgeProvider.overrideWithValue(platform ?? FakePlatformBridge()),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
  ],
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('failed autostart is still retryable after reopening settings', () async {
    final platform = RejectAutostart();
    final container = containerFor(
      SyntheticBridgePort(),
      CountingRuntimeBridge(),
      platform: platform,
    );
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final firstDraft = controller.draft();
    (firstDraft['GuiItem'] as Map<String, dynamic>)['AutoRun'] = true;
    final first = await controller.saveAndApply(firstDraft);
    expect(first.saved, isTrue);
    expect(first.ok, isFalse);
    expect(platform.attempts, 1);

    // openOptionSettingWindow invokes load() on every open.
    controller.load();
    final second = await controller.saveAndApply(controller.draft());
    expect(
      platform.attempts,
      2,
      reason: 'reloading persisted desire cannot confirm a failed OS write',
    );
    expect(second.ok, isFalse);
  });

  test('independent settings window can retry a saved but failed apply', () async {
    final runtime = CountingRuntimeBridge(
      applyError: const RuntimeErrorView(
        code: 'E_SYNTHETIC_APPLY',
        messageKey: 'error.synthetic_apply',
      ),
    );
    final container = containerFor(SyntheticBridgePort(), runtime);
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final windowRevision = container.read(settingsControllerProvider).revision;
    final snapshot = controller.draft();
    (snapshot['GuiItem'] as Map<String, dynamic>)['KeepOlderDedupl'] = true;
    final first = await controller.saveAndApply(
      snapshot,
      expectedRevision: windowRevision,
    );
    expect(first.saved, isTrue);
    expect(first.applied, isFalse);
    expect(first.ok, isFalse);

    runtime.applyError = null;
    // _applyOptionDraft always passes the revision captured when opened.
    final retry = await controller.saveAndApply(
      snapshot,
      expectedRevision: windowRevision,
    );
    expect(
      retry.ok,
      isTrue,
      reason: 'retrying this window\'s own saved draft must reach apply',
    );
    expect(runtime.applyCalls, 2);
  });

  test('update option failure must be visible or rolled back', () {
    final bridge = RejectGroupBridge();
    final container = containerFor(bridge, CountingRuntimeBridge());
    addTearDown(container.dispose);
    container.read(settingsControllerProvider.notifier).load();
    final controller = container.read(updateControllerProvider.notifier);
    final before = container.read(updateControllerProvider).prerelease;
    controller.setPrerelease(!before);
    final after = container.read(updateControllerProvider);
    expect(
      after.prerelease == before || after.status?.kind == 'error',
      isTrue,
      reason: 'a failed persisted toggle must not look like success',
    );
  });
}
