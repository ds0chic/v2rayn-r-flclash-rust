import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';
import 'package:v2rayn_desktop/features/update/check_update_view.dart';
import 'package:v2rayn_desktop/features/update/update_controller.dart';

/// SP-27: the persisted `CheckUpdateItem` group write always fails, so the
/// update window must keep the old toggles and surface an error instead of
/// optimistically showing the new selection as saved.
class _RejectGroupBridge extends SyntheticBridgePort {
  int saveAttempts = 0;

  @override
  settings.SaveSettingsResult saveSettingsGroup(
    String group,
    String json,
    int expectedRevision,
  ) {
    saveAttempts++;
    return const settings.SaveSettingsResult(
      ok: false,
      changes: [],
      restartCoreFields: [],
      restartAppFields: [],
      nextLaunchFields: [],
      error: c.ErrorDto(
        code: 'E_STORAGE_UNAVAILABLE',
        messageKey: 'error.storage_unavailable',
        retryable: true,
      ),
    );
  }
}

/// SP-27: this build has no own release source, so the app self-update entry
/// must report `error.update_app_source_unconfigured`, never a fake success.
class _UnconfiguredAppBridge extends SyntheticBridgePort {
  @override
  Future<c.ExternalSpecDto> t16ApplyAppUpdateSpec() async {
    t16Calls.add('app_update_spec');
    return const c.ExternalSpecDto(
      ok: false,
      helperExe: null,
      source: null,
      installRoot: null,
      waitForPid: 0,
      args: <String>[],
      error: c.ErrorDto(
        code: 'E_UNAVAILABLE',
        messageKey: 'error.update_app_source_unconfigured',
        retryable: false,
      ),
    );
  }
}

ProviderContainer _containerFor(SyntheticBridgePort bridge) =>
    ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
    );

/// SP-27 UI wiring: recorded runner hand-off (no real process, no exit).
class _Handoff {
  final List<String> launches = <String>[];
  int exits = 0;
}

ProviderContainer _wiredContainer(
  SyntheticBridgePort bridge,
  _Handoff handoff,
) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    updateControllerProvider.overrideWith(
      () => UpdateController(
        launchRunner: (helper, args, cwd) async =>
            handoff.launches.add('$helper|${args.join(",")}|$cwd'),
        exitApp: () => handoff.exits++,
      ),
    ),
  ],
);

Future<void> _pumpUpdate(
  WidgetTester tester,
  ProviderContainer container,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: CheckUpdateView())),
    ),
  );
  await tester.pump();
}

void main() {
  test('failed persist keeps the old prerelease toggle and reports error', () {
    final bridge = _RejectGroupBridge();
    final container = _containerFor(bridge);
    addTearDown(container.dispose);
    container.read(settingsControllerProvider.notifier).load();

    expect(container.read(updateControllerProvider).prerelease, isFalse);
    container.read(updateControllerProvider.notifier).setPrerelease(true);

    final after = container.read(updateControllerProvider);
    expect(
      after.prerelease,
      isFalse,
      reason: 'a failed persisted toggle must not look like success',
    );
    expect(after.status?.kind, 'error');
    expect(bridge.saveAttempts, 1);
  });

  test('failed persist keeps the old core selection and reports error', () {
    final bridge = _RejectGroupBridge();
    final container = _containerFor(bridge);
    addTearDown(container.dispose);
    container.read(settingsControllerProvider.notifier).load();

    final before = container.read(updateControllerProvider).selected;
    expect(before, contains('mihomo'));
    container
        .read(updateControllerProvider.notifier)
        .toggleCore('mihomo', false);

    final after = container.read(updateControllerProvider);
    expect(
      after.selected,
      contains('mihomo'),
      reason: 'a failed persisted toggle must not look like success',
    );
    expect(after.status?.kind, 'error');
  });

  testWidgets('failed persist leaves the checkbox off with a visible error', (
    tester,
  ) async {
    final bridge = _RejectGroupBridge();
    final container = _containerFor(bridge);
    addTearDown(container.dispose);
    await _pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-prerelease')));
    await tester.pump();

    final tile = tester.widget<CheckboxListTile>(
      find.byKey(const ValueKey('update-prerelease')),
    );
    expect(
      tile.value,
      isFalse,
      reason: 'a failed persisted toggle must not look like success',
    );
    expect(find.textContaining('更新选项保存失败'), findsOneWidget);
  });

  testWidgets('successful persist still applies the new toggle', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = _containerFor(bridge);
    addTearDown(container.dispose);
    await _pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-prerelease')));
    await tester.pump();

    final tile = tester.widget<CheckboxListTile>(
      find.byKey(const ValueKey('update-prerelease')),
    );
    expect(tile.value, isTrue);
    expect(find.textContaining('更新选项保存失败'), findsNothing);
  });

  testWidgets(
    'unconfigured app source is blocked and the action is retryable',
    (tester) async {
      final bridge = _UnconfiguredAppBridge();
      final container = _containerFor(bridge);
      addTearDown(container.dispose);
      await _pumpUpdate(tester, container);

      await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
      await tester.pumpAndSettle();
      expect(find.textContaining('应用自身发行源未配置'), findsOneWidget);

      // The failure clears `busy`, so the user can retry instead of latching.
      await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
      await tester.pumpAndSettle();
      expect(
        bridge.t16Calls.where((call) => call == 'app_update_spec'),
        hasLength(2),
      );
      expect(find.textContaining('应用自身发行源未配置'), findsOneWidget);
    },
  );

  testWidgets('app spec carries the current per-operation flags', (
    tester,
  ) async {
    // Default selection: stable channel, via proxy.
    final bridge = SyntheticBridgePort()..proxyAvailable = true;
    final handoff = _Handoff();
    final container = _wiredContainer(bridge, handoff);
    addTearDown(container.dispose);
    await _pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('app_update_spec_with_flags:false:true'));
    expect(handoff.launches, hasLength(1));
    expect(handoff.exits, 1);
  });

  testWidgets('toggled flags are forwarded to the app spec entry', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort()..proxyAvailable = true;
    final handoff = _Handoff();
    final container = _wiredContainer(bridge, handoff);
    addTearDown(container.dispose);
    await _pumpUpdate(tester, container);

    await tester.tap(find.byKey(const ValueKey('update-prerelease')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('update-via-proxy')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('update-app-spec-btn')));
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('app_update_spec_with_flags:true:false'));
    expect(handoff.launches, hasLength(1));
    expect(handoff.exits, 1);
  });

  test(
    'defaults path keeps the no-arg entry for genuine-default callers',
    () async {
      final bridge = SyntheticBridgePort()..proxyAvailable = true;
      final handoff = _Handoff();
      final container = _wiredContainer(bridge, handoff);
      addTearDown(container.dispose);
      container.read(settingsControllerProvider.notifier).load();

      await container
          .read(updateControllerProvider.notifier)
          .applyAppUpdateWithDefaults();

      expect(bridge.t16Calls, contains('app_update_spec'));
      expect(
        bridge.t16Calls.any(
          (call) => call.startsWith('app_update_spec_with_flags:'),
        ),
        isFalse,
      );
      expect(handoff.launches, hasLength(1));
      expect(handoff.exits, 1);
    },
  );

  test('checkAndApply freezes one flag set across check and apply', () async {
    final bridge = SyntheticBridgePort()..proxyAvailable = true;
    final handoff = _Handoff();
    final container = _wiredContainer(bridge, handoff);
    addTearDown(container.dispose);
    container.read(settingsControllerProvider.notifier).load();
    container.read(updateControllerProvider.notifier).setPrerelease(true);

    await container.read(updateControllerProvider.notifier).checkAndApply();

    final checks = bridge.t16Calls
        .where((call) => call.startsWith('check_updates:'))
        .toList();
    final applies = bridge.t16Calls
        .where((call) => call.startsWith('apply_core:'))
        .toList();
    expect(checks, hasLength(1));
    expect(applies, hasLength(1));
    expect(checks.single, endsWith(':true:true'));
    expect(applies.single, endsWith(':true:true'));
  });
}
