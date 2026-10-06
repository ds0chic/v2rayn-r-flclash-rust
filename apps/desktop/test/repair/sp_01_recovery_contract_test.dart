// SP-01 recovery contract: a corrupt `guiNConfig.json` must surface as a
// visible, non-editable load failure that blocks every write.
//
// The Rust engine fails `open`/`reopen` with `E_FIELD_FORMAT /
// error.config_corrupt` on present-but-empty, truncated or mistyped content
// (see `crates/application/tests/sp01_corrupt_config.rs` for the real
// storage side). Here the corrupt load is injected through a failing bridge
// (fault injection only) and the contract checks the application-visible
// half: no default document is presented, saves are refused without touching
// storage, the corrupt status stays visible as the recovery entry, and a
// repaired source recovers through a plain retrying `load()`.
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/settings/option_setting_window.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

class _CorruptConfigBridge extends SyntheticBridgePort {
  bool failLoad = true;
  int saveJsonCalls = 0;
  int saveGroupCalls = 0;

  @override
  settings.SettingsLoadDto getSettings() {
    if (failLoad) {
      return settings.SettingsLoadDto(
        ok: false,
        revision: BigInt.from(0),
        groupRevisionsJson: '{}',
        settingsJson: '{}',
        error: const contract.ErrorDto(
          code: 'E_FIELD_FORMAT',
          messageKey: 'error.config_corrupt',
          retryable: true,
        ),
      );
    }
    return super.getSettings();
  }

  @override
  settings.SaveSettingsResult saveSettingsJson(
    String settingsJson,
    int expectedRevision,
  ) {
    saveJsonCalls += 1;
    return super.saveSettingsJson(settingsJson, expectedRevision);
  }

  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String patchJson,
    int expectedRevision,
  ) {
    saveGroupCalls += 1;
    return super.saveSettingsGroup(group, patchJson, expectedRevision);
  }
}

ProviderContainer _container(_CorruptConfigBridge bridge) => ProviderContainer(
  overrides: [bridgePortProvider.overrideWithValue(bridge)],
);

void main() {
  test('corrupt load is visible and never an editable default document', () {
    final bridge = _CorruptConfigBridge();
    final container = _container(bridge);
    addTearDown(container.dispose);

    final state = container.read(settingsControllerProvider.notifier).load();

    expect(state.loaded, isFalse);
    expect(state.loadFailed, isTrue);
    // The recovery entry: a visible corrupt status, not a silent default.
    expect(state.status, 'error.config_corrupt');
    expect(state.document, isEmpty);
  });

  test('saves are refused on a failed load without touching storage', () {
    final bridge = _CorruptConfigBridge();
    final container = _container(bridge);
    addTearDown(container.dispose);

    container.read(settingsControllerProvider.notifier).load();

    final whole = container
        .read(settingsControllerProvider.notifier)
        .saveDocument(<String, dynamic>{});
    expect(whole.ok, isFalse);

    final group = container.read(settingsControllerProvider.notifier).saveGroup(
      'GuiItem',
      <String, dynamic>{'AutoRun': true},
    );
    expect(group.ok, isFalse);

    // The real store behind the bridge was never written: the damaged
    // source file survives for recovery instead of being defaulted over.
    expect(bridge.saveJsonCalls, 0);
    expect(bridge.saveGroupCalls, 0);
  });

  test('repair plus retrying load recovers without a restart', () {
    final bridge = _CorruptConfigBridge();
    final container = _container(bridge);
    addTearDown(container.dispose);

    final notifier = container.read(settingsControllerProvider.notifier);
    expect(notifier.load().loadFailed, isTrue);

    // The source is repaired out of band; a plain `load()` retry recovers.
    bridge.failLoad = false;
    final recovered = notifier.load();
    expect(recovered.loaded, isTrue);
    expect(recovered.loadFailed, isFalse);

    final draft = notifier.draft();
    draft['FutureRoot'] = <String, dynamic>{
      'x': <int>[1, 2, 3],
    };
    final saved = notifier.saveDocument(draft);
    expect(saved.ok, isTrue);
    expect(
      container.read(settingsControllerProvider).document['FutureRoot'],
      <String, dynamic>{
        'x': <int>[1, 2, 3],
      },
    );
    // The unknown key round-trips through the synthetic store untouched.
    final stored = jsonDecode(bridge.getSettings().settingsJson);
    expect((stored as Map<String, dynamic>)['FutureRoot'], isNotNull);
  });

  testWidgets('corrupt load shows a retry entry and recovers in place', (
    tester,
  ) async {
    final bridge = _CorruptConfigBridge();
    final container = _container(bridge);
    addTearDown(container.dispose);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: OptionSettingWindow())),
      ),
    );
    await tester.pump();
    await tester.pumpAndSettle();

    // Visible recovery entry: corrupt status plus an inline retry.
    expect(
      find.byKey(const ValueKey('settings-load-retry-inline')),
      findsOneWidget,
    );

    // Repair the source, retry from the entry, and the same window recovers
    // without presenting a defaulted draft in between.
    bridge.failLoad = false;
    await tester.tap(find.byKey(const ValueKey('settings-load-retry-inline')));
    await tester.pump();
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('settings-load-retry-inline')),
      findsNothing,
    );
    expect(container.read(settingsControllerProvider).loaded, isTrue);
  });
}
