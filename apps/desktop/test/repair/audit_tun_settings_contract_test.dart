import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_controller.dart';
import 'package:v2rayn_desktop/features/runtime/tun_toggle.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import '../support/counting_runtime_bridge.dart';
import '../support/fake_platform_bridge.dart';

/// Regression contracts from the 2026-10-05 TUN/settings audit
/// (`docs/evidence/tun-settings-audit-2026-10-05`). Each test asserts the
/// *correct* expected behavior; they were red before the Phase-1 fixes.
class ThrowingLoadBridge extends SyntheticBridgePort {
  @override
  settings.SettingsLoadDto getSettings() =>
      throw StateError('synthetic load failure');
}

class RejectProxyBridge extends FakePlatformBridge {
  @override
  PlatformActionResult setSystemProxy({
    required int mode,
    String? server,
    String? bypass,
    String? autoConfigUrl,
  }) => const PlatformActionResult(
    ok: false,
    error: PlatformErrorView(
      code: 'E_PLATFORM_BACKEND',
      messageKey: 'error.platform_backend',
    ),
  );
}

class _RejectAutostart extends FakePlatformBridge {
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

  test('whole save must refresh group revisions before next group save', () {
    final bridge = SyntheticBridgePort();
    final container = containerFor(bridge, CountingRuntimeBridge());
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final draft = controller.draft();
    (draft['GuiItem'] as Map<String, dynamic>)['KeepOlderDedupl'] = true;
    final saved = controller.saveDocument(draft);
    expect(saved.ok, isTrue);
    final patched = controller.saveGroup(
      'GuiItem',
      controller.draft()['GuiItem'],
    );
    expect(
      patched.ok,
      isTrue,
      reason: 'a group save after successful whole save must not use stale revision',
    );
  });

  test('TUN toggle must not report applied when runtime rejected the plan', () async {
    final runtime = CountingRuntimeBridge(
      applyError: const RuntimeErrorView(
        code: 'E_TUN_HELPER_UNAVAILABLE',
        messageKey: 'error.tun_helper_denied',
      ),
    );
    final container = containerFor(SyntheticBridgePort(), runtime);
    addTearDown(container.dispose);
    final controller = container.read(runtimeControllerProvider.notifier);
    final result = await toggleTunDesired(
      enabled: false,
      persist: (_) => true,
      apply: controller.applyActive,
    );
    expect(
      container.read(runtimeControllerProvider).error?.code,
      'E_TUN_HELPER_UNAVAILABLE',
    );
    expect(
      result.runtimeApplied,
      isFalse,
      reason: 'void completion is not an applied result',
    );
    expect(result.ok, isFalse);
  });

  test('settings load exception must not make defaults a successful load', () {
    final container = containerFor(ThrowingLoadBridge(), CountingRuntimeBridge());
    addTearDown(container.dispose);
    final result = container.read(settingsControllerProvider.notifier).load();
    expect(
      result.loaded,
      isFalse,
      reason: 'loading failed; an editable default draft is not the stored config',
    );
  });

  test('settings apply must include system proxy failure in its outcome', () async {
    final container = containerFor(
      SyntheticBridgePort(),
      CountingRuntimeBridge(
        initial: const RuntimeView(
          state: 'Running',
          hostAlive: true,
          ports: [11977],
          sessionId: 'synthetic-old-session',
        ),
      ),
      platform: RejectProxyBridge(),
    );
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    container.read(platformControllerProvider.notifier);
    await container.read(runtimeControllerProvider.notifier).refresh();
    final draft = controller.draft();
    (draft['SystemProxyItem'] as Map<String, dynamic>)['SysProxyType'] = 1;
    final outcome = await controller.saveAndApply(draft);
    expect(
      container.read(platformControllerProvider).error?.code,
      'E_PLATFORM_BACKEND',
    );
    expect(
      outcome.ok,
      isFalse,
      reason: 'the system proxy consumer failed even though the core apply succeeded',
    );
  });

  test('failed autostart must retry when clicking save again', () async {
    final platform = _RejectAutostart();
    final container = containerFor(
      SyntheticBridgePort(),
      CountingRuntimeBridge(),
      platform: platform,
    );
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final draft = controller.draft();
    (draft['GuiItem'] as Map<String, dynamic>)['AutoRun'] = true;
    final first = await controller.saveAndApply(draft);
    expect(first.saved, isTrue);
    expect(first.ok, isFalse);
    expect(platform.attempts, 1);
    final second = await controller.saveAndApply(draft);
    expect(
      platform.attempts,
      2,
      reason: 'same persisted value does not mean OS effect succeeded',
    );
    expect(second.ok, isFalse);
  });

  test('old full draft must not overwrite a newer group edit', () {
    final container = containerFor(SyntheticBridgePort(), CountingRuntimeBridge());
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final oldDraft = controller.draft();
    final ui = Map<String, dynamic>.from(oldDraft['UiItem'] as Map);
    ui['CurrentLanguage'] = 'en';
    expect(controller.saveGroup('UiItem', ui).ok, isTrue);
    expect(
      (container.read(settingsControllerProvider).document['UiItem'] as Map)['CurrentLanguage'],
      'en',
    );
    final result = controller.saveDocument(oldDraft);
    expect(
      result.ok,
      isFalse,
      reason: 'editor must submit its captured revision, not the latest controller revision',
    );
  });
}
