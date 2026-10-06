// SP-12 saved-but-not-applied retry contract (synthetic only).
//
// No native library, core, port, host proxy/TUN/registry or user data.
// Ports are never bound; autostart goes through a counting fake.
import 'dart:convert';

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

import '../support/counting_runtime_bridge.dart';
import '../support/fake_platform_bridge.dart';

class _RejectGroupBridge extends SyntheticBridgePort {
  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String patchJson,
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

class CountingAutostart extends FakePlatformBridge {
  int attempts = 0;
  bool failWrites = false;

  @override
  bool setAutostart({
    required String name,
    required bool enabled,
    required String exe,
    required String args,
  }) {
    attempts++;
    if (failWrites) return false;
    autostart[name] = enabled;
    return true;
  }
}

ProviderContainer containerFor(
  BridgePort bridge,
  CountingRuntimeBridge runtime,
  PlatformBridge platform,
) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    runtimeBridgeProvider.overrideWithValue(runtime),
    platformBridgeProvider.overrideWithValue(platform),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
  ],
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'save returns new revision; core failure keeps saved fact visible',
    () async {
      final bridge = SyntheticBridgePort();
      final runtime = CountingRuntimeBridge(
        applyError: const RuntimeErrorView(
          code: 'E_SYNTHETIC_APPLY',
          messageKey: 'error.synthetic_apply',
        ),
      );
      final platform = CountingAutostart();
      final container = containerFor(bridge, runtime, platform);
      addTearDown(container.dispose);

      final controller = container.read(settingsControllerProvider.notifier);
      controller.load();
      final before = container.read(settingsControllerProvider).revision;
      final draft = controller.draft();
      (draft['GuiItem'] as Map<String, dynamic>)['KeepOlderDedupl'] = true;

      final first = await controller.saveAndApply(draft);
      expect(
        first.saved,
        isTrue,
        reason: 'persisted fact must survive apply failure',
      );
      expect(first.ok, isFalse);
      expect(first.applied, isFalse);
      expect(first.newRevision, isNotNull);
      expect(first.newRevision, greaterThan(before));
      expect(first.contentHash, isNotNull);
      expect(first.coreOk, isFalse);

      final state = container.read(settingsControllerProvider);
      expect(state.revision, first.newRevision);
      expect(state.lastReceipt.save, 'committed');
      expect(state.lastReceipt.core, 'failed');
      expect(state.lastReceipt.savedRevision, first.newRevision);
      expect(state.lastReceipt.contentHash, first.contentHash);

      // desired (persisted document) is updated while applied (runtime) is not.
      expect((state.document['GuiItem'] as Map)['KeepOlderDedupl'], isTrue);
      expect(container.read(runtimeControllerProvider).isRunning, isFalse);

      // Reopen from the same persisted bridge: the saved value is visible.
      final reopened = containerFor(
        bridge,
        CountingRuntimeBridge(),
        FakePlatformBridge(),
      );
      addTearDown(reopened.dispose);
      reopened.read(settingsControllerProvider.notifier).load();
      final doc = reopened.read(settingsControllerProvider).document;
      expect((doc['GuiItem'] as Map)['KeepOlderDedupl'], isTrue);
    },
  );

  test('retry executes only the failed core phase with hash check', () async {
    final bridge = SyntheticBridgePort();
    final runtime = CountingRuntimeBridge(
      applyError: const RuntimeErrorView(
        code: 'E_SYNTHETIC_APPLY',
        messageKey: 'error.synthetic_apply',
      ),
    );
    final platform = CountingAutostart();
    final container = containerFor(bridge, runtime, platform);
    addTearDown(container.dispose);

    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final draft = controller.draft();
    (draft['GuiItem'] as Map<String, dynamic>)['KeepOlderDedupl'] = true;
    final first = await controller.saveAndApply(draft);
    expect(first.saved, isTrue);
    expect(first.ok, isFalse);
    expect(runtime.applyCalls, 1);

    final revisionAfterSave = bridge.settingsRevision();
    final autostartAttemptsAfterSave = platform.attempts;

    runtime.applyError = null;
    final retry = await controller.retrySettingsApply(
      draft,
      savedRevision: first.newRevision!,
      savedContentHash: first.contentHash!,
      phases: const {'core'},
    );
    expect(retry.ok, isTrue);
    expect(retry.saved, isTrue);
    expect(retry.coreOk, isTrue);
    expect(
      runtime.applyCalls,
      2,
      reason: 'only the failed core phase runs again',
    );
    expect(
      bridge.settingsRevision(),
      revisionAfterSave,
      reason: 'retry must not re-persist (no new revision)',
    );
    expect(
      platform.attempts,
      autostartAttemptsAfterSave,
      reason: 'retry must not repeat the succeeded autostart phase',
    );
  });

  test(
    'independent window retry with stale revision still reaches apply',
    () async {
      final bridge = SyntheticBridgePort();
      final runtime = CountingRuntimeBridge(
        applyError: const RuntimeErrorView(
          code: 'E_SYNTHETIC_APPLY',
          messageKey: 'error.synthetic_apply',
        ),
      );
      final container = containerFor(bridge, runtime, FakePlatformBridge());
      addTearDown(container.dispose);

      final controller = container.read(settingsControllerProvider.notifier);
      controller.load();
      final windowRevision = container
          .read(settingsControllerProvider)
          .revision;
      final snapshot = controller.draft();
      (snapshot['GuiItem'] as Map<String, dynamic>)['KeepOlderDedupl'] = true;
      final first = await controller.saveAndApply(
        snapshot,
        expectedRevision: windowRevision,
      );
      expect(first.saved, isTrue);
      expect(first.applied, isFalse);

      runtime.applyError = null;
      // The window retries with the revision captured when it opened.
      final retry = await controller.saveAndApply(
        snapshot,
        expectedRevision: windowRevision,
      );
      expect(
        retry.ok,
        isTrue,
        reason: 'retrying this window\'s own saved draft must reach apply',
      );
      expect(retry.saved, isTrue);
      expect(runtime.applyCalls, 2);
    },
  );

  test('stale retry with diverged content runs no phase', () async {
    final bridge = SyntheticBridgePort();
    final runtime = CountingRuntimeBridge();
    final container = containerFor(bridge, runtime, FakePlatformBridge());
    addTearDown(container.dispose);

    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final draft = controller.draft();
    (draft['GuiItem'] as Map<String, dynamic>)['KeepOlderDedupl'] = true;
    final first = await controller.saveAndApply(draft);
    expect(first.ok, isTrue);

    // External edit advances the persisted document away from the saved hash.
    final external = controller.draft();
    (external['GuiItem'] as Map<String, dynamic>)['KeepOlderDedupl'] = false;
    (external['GuiItem'] as Map<String, dynamic>)['AutoRun'] = true;
    controller.saveDocument(external);
    final callsBefore = runtime.applyCalls;

    final retry = await controller.retrySettingsApply(
      draft,
      savedRevision: first.newRevision!,
      savedContentHash: first.contentHash!,
      phases: const {'core'},
    );
    expect(retry.ok, isFalse);
    expect(retry.phaseErrors, contains('E_RETRY_STALE'));
    expect(
      runtime.applyCalls,
      callsBefore,
      reason: 'a diverged retry must not apply stale content',
    );
  });

  test(
    'failed autostart is retried after reopen, never confirmed by reload',
    () async {
      final bridge = SyntheticBridgePort();
      final runtime = CountingRuntimeBridge();
      final platform = CountingAutostart()..failWrites = true;
      final container = containerFor(bridge, runtime, platform);
      addTearDown(container.dispose);

      final controller = container.read(settingsControllerProvider.notifier);
      controller.load();
      final draft = controller.draft();
      (draft['GuiItem'] as Map<String, dynamic>)['AutoRun'] = true;
      final first = await controller.saveAndApply(draft);
      expect(first.saved, isTrue);
      expect(first.ok, isFalse);
      expect(first.autostartOk, isFalse);
      expect(platform.attempts, 1);

      // Reopening reloads the persisted desire; the OS fact is still unwritten.
      controller.load();
      final second = await controller.saveAndApply(controller.draft());
      expect(
        platform.attempts,
        2,
        reason: 'reloading persisted desire cannot confirm a failed OS write',
      );
      expect(second.ok, isFalse);
      expect(second.saved, isTrue);
    },
  );

  test(
    'group save failure stays visible instead of looking like success',
    () async {
      final bridge = _RejectGroupBridge();
      final container = containerFor(
        bridge,
        CountingRuntimeBridge(),
        FakePlatformBridge(),
      );
      addTearDown(container.dispose);

      final controller = container.read(settingsControllerProvider.notifier);
      controller.load();
      final result = controller.saveGroup('CheckUpdateItem', <String, dynamic>{
        'CheckPreReleaseUpdate': true,
      });
      expect(result.ok, isFalse);
      expect(result.error, isNotNull);
      expect(
        container.read(settingsControllerProvider).status,
        isNotNull,
        reason: 'a failed group save must leave a visible status',
      );
      expect(
        jsonEncode(container.read(settingsControllerProvider).document),
        isNotEmpty,
      );
    },
  );
}
