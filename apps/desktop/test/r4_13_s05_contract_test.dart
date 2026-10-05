// R4-13.S05 settings-save-to-effect contract (CoreBasicItem instance).
//
// Proves one settings group (CoreBasicItem) saves through the real bridge and
// the value is consumed by the real plan apply, with failure visible instead of
// a fake success. Synthetic bridge / runtime only: no native library, core,
// port, host proxy/TUN/registry or user data.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/app/shell/ui_shell_controller.dart';
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import 'support/counting_runtime_bridge.dart';
import 'support/fake_platform_bridge.dart';

/// SyntheticBridgePort that reports the edited CoreBasicItem field as a
/// `restart_core` change, like the real Rust `save_settings_json`.
class _CoreBasicBridge extends SyntheticBridgePort {
  @override
  settings.SaveSettingsResult saveSettingsJson(
    String settingsJson,
    int expectedRevision,
  ) {
    final result = super.saveSettingsJson(settingsJson, expectedRevision);
    if (!result.ok) return result;
    return settings.SaveSettingsResult(
      ok: true,
      newRevision: result.newRevision,
      changes: result.changes,
      restartCoreFields: const <String>['CoreBasicItem.Loglevel'],
      restartAppFields: const <String>[],
      nextLaunchFields: const <String>[],
    );
  }
}

ProviderContainer _container(BridgePort bridge, RuntimeBridge runtime) =>
    ProviderContainer(
      overrides: [
        bridgePortProvider.overrideWithValue(bridge),
        runtimeBridgeProvider.overrideWithValue(runtime),
        platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
        uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      ],
    );

Map<String, dynamic> _draftWithLoglevel(
  SettingsController controller,
  String level, {
  bool fragment = false,
}) {
  final draft = controller.draft();
  final core = draft['CoreBasicItem'] as Map<String, dynamic>;
  core['Loglevel'] = level;
  core['EnableFragment'] = fragment;
  return draft;
}

void main() {
  test(
    'CoreBasicItem save awaits the real apply and persists for reopen',
    () async {
      final bridge = _CoreBasicBridge();
      final runtime = CountingRuntimeBridge();
      final container = _container(bridge, runtime);
      addTearDown(container.dispose);

      final controller = container.read(settingsControllerProvider.notifier);
      controller.load();
      final outcome = await controller.saveAndApply(
        _draftWithLoglevel(controller, 'debug', fragment: true),
      );

      expect(outcome.ok, isTrue);
      expect(outcome.saved, isTrue);
      expect(outcome.applied, isTrue);
      expect(runtime.applyCalls, 1, reason: 'save must drive a real apply');
      expect(outcome.statusKey, 'settings.saved_need_core_restart');
      expect(
        container.read(uiShellControllerProvider).message,
        contains('重启服务'),
        reason: 'restart hint must be visible in the main window',
      );

      // Reopen from the same persisted bridge: the saved value is visible.
      final reopened = _container(bridge, CountingRuntimeBridge());
      addTearDown(reopened.dispose);
      reopened.read(settingsControllerProvider.notifier).load();
      final doc = reopened.read(settingsControllerProvider).document;
      final core = doc['CoreBasicItem'] as Map<String, dynamic>;
      expect(core['Loglevel'], 'debug');
      expect(core['EnableFragment'], isTrue);
    },
  );

  test('apply failure is reported and never faked as success', () async {
    final bridge = _CoreBasicBridge();
    final runtime = CountingRuntimeBridge(
      applyError: const RuntimeErrorView(
        code: 'E_CORE',
        messageKey: 'error.core_start_failed',
      ),
    );
    final container = _container(bridge, runtime);
    addTearDown(container.dispose);

    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final outcome = await controller.saveAndApply(
      _draftWithLoglevel(controller, 'error'),
    );

    expect(outcome.ok, isFalse);
    expect(outcome.saved, isTrue, reason: 'the document is still persisted');
    expect(outcome.applied, isFalse);
    expect(outcome.message, contains('应用失败'));
    expect(container.read(uiShellControllerProvider).message, contains('应用失败'));

    // The saved value is still visible on reopen even when the apply failed.
    final reopened = _container(bridge, CountingRuntimeBridge());
    addTearDown(reopened.dispose);
    reopened.read(settingsControllerProvider.notifier).load();
    final core =
        reopened.read(settingsControllerProvider).document['CoreBasicItem']
            as Map<String, dynamic>;
    expect(core['Loglevel'], 'error');
  });

  test('SendThrough/BindInterface trim and empty->null on save', () async {
    final bridge = _CoreBasicBridge();
    final runtime = CountingRuntimeBridge();
    final container = _container(bridge, runtime);
    addTearDown(container.dispose);

    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final draft = controller.draft();
    final core = draft['CoreBasicItem'] as Map<String, dynamic>;
    core['SendThrough'] = '  192.0.2.10  ';
    core['BindInterface'] = '   ';
    final outcome = await controller.saveAndApply(draft);
    expect(outcome.ok, isTrue);

    final reopened = _container(bridge, CountingRuntimeBridge());
    addTearDown(reopened.dispose);
    reopened.read(settingsControllerProvider.notifier).load();
    final restored =
        reopened.read(settingsControllerProvider).document['CoreBasicItem']
            as Map<String, dynamic>;
    expect(restored['SendThrough'], '192.0.2.10');
    expect(restored['BindInterface'], isNull);
  });

  testWidgets('in-process 确定 keeps the window open on apply failure', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final bridge = _CoreBasicBridge();
    final runtime = CountingRuntimeBridge(
      applyError: const RuntimeErrorView(
        code: 'E_CORE',
        messageKey: 'error.core_start_failed',
      ),
    );
    final container = _container(bridge, runtime);
    addTearDown(container.dispose);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: OptionSettingWindow(standalone: true)),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('settings-save')));
    await tester.pumpAndSettle();

    expect(runtime.applyCalls, 1);
    expect(
      find.byKey(const ValueKey('settings-save')),
      findsOneWidget,
      reason: 'a failed apply must not close the window as a success',
    );
    expect(find.textContaining('应用失败'), findsOneWidget);
  });
}
