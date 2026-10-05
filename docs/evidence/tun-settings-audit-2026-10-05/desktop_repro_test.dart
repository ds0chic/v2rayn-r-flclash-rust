import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import '../../../apps/desktop/test/support/counting_runtime_bridge.dart';
import '../../../apps/desktop/test/support/fake_platform_bridge.dart';

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

ProviderContainer _container(PlatformBridge platform) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
    platformBridgeProvider.overrideWithValue(platform),
    runtimeBridgeProvider.overrideWithValue(CountingRuntimeBridge()),
    uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
  ],
);

void main() {
  test('failed autostart must retry when clicking save again', () async {
    final platform = _RejectAutostart();
    final container = _container(platform);
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
    print('audit: autostart attempts=${platform.attempts}; second.ok=${second.ok}');
    expect(platform.attempts, 2, reason: 'same persisted value does not mean OS effect succeeded');
    expect(second.ok, isFalse);
  });

  test('old full draft must not overwrite a newer group edit', () {
    final container = _container(FakePlatformBridge());
    addTearDown(container.dispose);
    final controller = container.read(settingsControllerProvider.notifier);
    controller.load();
    final oldDraft = controller.draft();
    final ui = Map<String, dynamic>.from(oldDraft['UiItem'] as Map);
    ui['CurrentLanguage'] = 'en';
    expect(controller.saveGroup('UiItem', ui).ok, isTrue);
    expect((container.read(settingsControllerProvider).document['UiItem'] as Map)['CurrentLanguage'], 'en');
    final result = controller.saveDocument(oldDraft);
    print('audit: old draft save.ok=${result.ok}; language=${(container.read(settingsControllerProvider).document['UiItem'] as Map)['CurrentLanguage']}');
    expect(result.ok, isFalse, reason: 'editor must submit its captured revision, not the latest controller revision');
  });
}
