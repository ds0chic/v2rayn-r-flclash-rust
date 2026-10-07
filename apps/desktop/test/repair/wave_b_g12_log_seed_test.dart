// Wave B G-12 / FLD-CFG-065 (MainMsgFilter) + FLD-CFG-066 (AutoRefresh):
// seed canonical `MsgUIItem` into the log view (Wave B G-12).
//
// Synthetic-only: FakeMonitorBridge for the log stream/ring plus a stubbed
// settings controller standing in for the persisted settings tree. No
// network/10808/system-proxy/user secrets. Covers: canonical parse/defaults,
// invalid-regex rejection with the old filter kept, persist-first keyword +
// auto-refresh (save failure rolls back with a visible error), save ->
// reopen keeps the initial filter/switch, and live/pause consistency (a
// paused view stays paused across a filter change; a live view keeps
// tailing).
//
// Upstream (frozen 7d6a967): `ConfigItems.MsgUIItem` + `MsgViewModel` filter
// display semantics (filter is presentation-only, collection continues).
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as contract;
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/bridge/api/settings.dart' as settings;
import 'package:v2rayn_desktop/features/monitor/log_ui_config.dart';
import 'package:v2rayn_desktop/features/monitor/logs_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import '../support/fake_monitor_bridge.dart';

m.LogLineDto line(String text, {int level = 2}) =>
    m.LogLineDto(text: text, level: level, truncated: false);

Map<String, dynamic> _msgDoc({Object? filter, Object? autoRefresh}) =>
    <String, dynamic>{
      'MsgUIItem': <String, dynamic>{
        'MainMsgFilter': filter,
        'AutoRefresh': autoRefresh,
      },
    };

/// Settings stand-in: fixed document; [failSave] makes saveGroup always fail
/// (fault injection), success updates state so the provider derives canonical.
class _WaveBLogSettings extends SettingsController {
  _WaveBLogSettings(this.doc, {this.failSave = false});

  final Map<String, dynamic> doc;
  final bool failSave;
  int saveAttempts = 0;

  @override
  SettingsViewState build() =>
      SettingsViewState(loaded: true, revision: 1, document: doc);

  @override
  settings.SaveSettingsResult saveGroup(String group, Object? value) {
    saveAttempts++;
    if (failSave) {
      return const settings.SaveSettingsResult(
        ok: false,
        changes: <settings.SettingsChangeDto>[],
        restartCoreFields: <String>[],
        restartAppFields: <String>[],
        nextLaunchFields: <String>[],
        error: contract.ErrorDto(
          code: 'E_STORAGE_UNAVAILABLE',
          messageKey: 'error.storage_unavailable',
          retryable: true,
        ),
      );
    }
    doc[group] = value;
    state = state.copyWith(document: Map<String, dynamic>.of(doc));
    return const settings.SaveSettingsResult(
      ok: true,
      changes: <settings.SettingsChangeDto>[],
      restartCoreFields: <String>[],
      restartAppFields: <String>[],
      nextLaunchFields: <String>[],
    );
  }
}

({
  ProviderContainer container,
  FakeMonitorBridge bridge,
  MonitorController controller,
})
_controllerHarness() {
  final bridge = FakeMonitorBridge();
  final container = ProviderContainer(
    overrides: [monitorBridgeProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  addTearDown(bridge.disposeStreams);
  return (
    container: container,
    bridge: bridge,
    controller: container.read(monitorControllerProvider.notifier),
  );
}

Future<ProviderContainer> _pumpLogs(
  WidgetTester tester, {
  required _WaveBLogSettings seed,
  required FakeMonitorBridge bridge,
}) async {
  addTearDown(bridge.disposeStreams);
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(bridge),
      settingsControllerProvider.overrideWith(() => seed),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: LogsView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return container;
}

/// Genuine reopen: dispose the page state, then build the page again on the
/// same container so the initial filter/switch must come from canonical.
Future<void> _reopenLogs(
  WidgetTester tester,
  ProviderContainer container,
) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: LogsView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
}

String _keywordText(WidgetTester tester) => tester
    .widget<TextField>(find.byKey(const ValueKey('logs-keyword')))
    .controller!
    .text;

bool _autoRefreshSwitch(WidgetTester tester) =>
    tester.widget<Switch>(find.byKey(const ValueKey('logs-autorefresh'))).value;

void main() {
  group('G-12 canonical parse/validate/encode (synthetic)', () {
    test(
      'FLD-CFG-065: MainMsgFilter parse, missing/null/wrong-type -> empty',
      () {
        expect(
          logUiConfigFromDocument(_msgDoc(filter: 'error')).keyword,
          'error',
        );
        expect(logUiConfigFromDocument(_msgDoc()).keyword, '');
        expect(logUiConfigFromDocument(const <String, dynamic>{}).keyword, '');
        expect(
          logUiConfigFromDocument(_msgDoc(filter: 42)).keyword,
          '',
          reason: 'wrong type must not leak into the filter box',
        );
      },
    );

    test('FLD-CFG-066: AutoRefresh parse, null/missing/wrong-type -> on', () {
      expect(
        logUiConfigFromDocument(_msgDoc(autoRefresh: false)).autoRefresh,
        isFalse,
      );
      expect(
        logUiConfigFromDocument(_msgDoc(autoRefresh: true)).autoRefresh,
        isTrue,
      );
      expect(logUiConfigFromDocument(_msgDoc()).autoRefresh, isTrue);
      expect(
        logUiConfigFromDocument(const <String, dynamic>{}).autoRefresh,
        isTrue,
      );
      expect(
        logUiConfigFromDocument(_msgDoc(autoRefresh: 'off')).autoRefresh,
        isTrue,
      );
    });

    test('FLD-CFG-065: invalid regex rejected, empty/valid accepted', () {
      expect(isValidLogFilter(''), isTrue);
      expect(isValidLogFilter('error'), isTrue);
      expect(isValidLogFilter('err.*404'), isTrue);
      expect(isValidLogFilter('(['), isFalse);
      expect(isValidLogFilter('*'), isFalse);
    });

    test('MsgUIItem group write preserves unknown keys', () {
      final document = <String, dynamic>{
        'MsgUIItem': <String, dynamic>{
          'MainMsgFilter': 'old',
          'AutoRefresh': true,
          'SynUnknown': 7,
        },
      };
      final group = msgUiGroupWith(document, <String, Object?>{
        'MainMsgFilter': 'new',
      });
      expect(group['MainMsgFilter'], 'new');
      expect(group['AutoRefresh'], isTrue);
      expect(group['SynUnknown'], 7);
    });
  });

  group('G-12 controller seed (synthetic fake bridge)', () {
    test('FLD-CFG-065/066: seed applies canonical filter + switch', () {
      final h = _controllerHarness();
      h.controller.setPageVisible('logs', true);
      h.controller.seedLogView(keyword: 'error', autoRefresh: false);
      final state = h.container.read(monitorControllerProvider);
      expect(state.keyword, 'error');
      expect(state.autoRefresh, isFalse);
    });

    test('seed preserves pause flags and the retained tail', () {
      final h = _controllerHarness();
      h.controller.setPageVisible('logs', true);
      h.bridge.logs = <m.LogLineDto>[line('kept')];
      h.controller.seedLogView(keyword: '', autoRefresh: true);
      expect(
        h.container.read(monitorControllerProvider).logs.single.text,
        'kept',
      );
      // Pause arrives through the Rust batch flags (authoritative source).
      h.bridge.emitLogs(
        const <m.LogLineDto>[],
        scrollPaused: true,
        collectingPaused: true,
      );
      // A paused view stays paused across a filter change; tail retained.
      h.controller.seedLogView(keyword: 'kept', autoRefresh: false);
      final state = h.container.read(monitorControllerProvider);
      expect(state.scrollPaused, isTrue);
      expect(state.collectingPaused, isTrue);
      expect(state.logs.single.text, 'kept');
      expect(state.keyword, 'kept');
      // Resuming from canonical resyncs the tail from the ring.
      h.bridge.logs = <m.LogLineDto>[line('kept'), line('resumed')];
      h.controller.seedLogView(keyword: 'kept', autoRefresh: true);
      expect(
        h.container.read(monitorControllerProvider).logs.map((l) => l.text),
        ['kept', 'resumed'],
      );
    });

    test('seed off freezes without resync; seed on resyncs the tail', () {
      final h = _controllerHarness();
      h.controller.setPageVisible('logs', true);
      h.bridge.logs = <m.LogLineDto>[line('ring tail')];
      h.controller.seedLogView(keyword: '', autoRefresh: false);
      expect(
        h.container.read(monitorControllerProvider).logs,
        isEmpty,
        reason: 'freezing must not pull the tail in',
      );
      h.controller.seedLogView(keyword: '', autoRefresh: true);
      expect(
        h.container.read(monitorControllerProvider).logs.single.text,
        'ring tail',
      );
    });
  });

  group('G-12 log page seeding (synthetic widget)', () {
    testWidgets('FLD-CFG-065: open seeds the persisted filter', (tester) async {
      final seed = _WaveBLogSettings(_msgDoc(filter: 'error'));
      final bridge = FakeMonitorBridge();
      bridge.logs = <m.LogLineDto>[
        line('plain info line'),
        line('an error occurred', level: 4),
      ];
      await _pumpLogs(tester, seed: seed, bridge: bridge);
      expect(_keywordText(tester), 'error');
      expect(find.text('an error occurred'), findsOneWidget);
      expect(find.text('plain info line'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets(
      'FLD-CFG-065: type filter -> save -> reopen keeps it, clear unsets',
      (tester) async {
        final seed = _WaveBLogSettings(_msgDoc(filter: ''));
        final bridge = FakeMonitorBridge();
        final container = await _pumpLogs(tester, seed: seed, bridge: bridge);
        expect(_keywordText(tester), '');

        await tester.enterText(
          find.byKey(const ValueKey('logs-keyword')),
          'warn',
        );
        await tester.pump(const Duration(milliseconds: 200));
        await tester.pump();
        expect(seed.saveAttempts, 1);
        expect(
          (seed.doc['MsgUIItem'] as Map<String, dynamic>)['MainMsgFilter'],
          'warn',
        );
        expect(_keywordText(tester), 'warn');

        await _reopenLogs(tester, container);
        expect(_keywordText(tester), 'warn');

        await tester.enterText(find.byKey(const ValueKey('logs-keyword')), '');
        await tester.pump(const Duration(milliseconds: 200));
        await tester.pump();
        expect(
          (seed.doc['MsgUIItem'] as Map<String, dynamic>)['MainMsgFilter'],
          '',
        );
        await _reopenLogs(tester, container);
        expect(_keywordText(tester), '');
        await tester.pumpWidget(const SizedBox());
      },
    );

    testWidgets(
      'FLD-CFG-065: invalid regex rejected, old filter kept + diagnosed',
      (tester) async {
        final seed = _WaveBLogSettings(_msgDoc(filter: 'error'));
        final bridge = FakeMonitorBridge();
        await _pumpLogs(tester, seed: seed, bridge: bridge);
        expect(_keywordText(tester), 'error');

        await tester.enterText(
          find.byKey(const ValueKey('logs-keyword')),
          '([',
        );
        await tester.pump(const Duration(milliseconds: 200));
        await tester.pump();

        expect(seed.saveAttempts, 0, reason: 'rejected before persist');
        expect(
          (seed.doc['MsgUIItem'] as Map<String, dynamic>)['MainMsgFilter'],
          'error',
        );
        expect(_keywordText(tester), 'error');
        expect(find.textContaining('过滤表达式非法'), findsOneWidget);
        await tester.pumpWidget(const SizedBox());
      },
    );

    testWidgets(
      'FLD-CFG-065/066: keyword save failure rolls back + visible error',
      (tester) async {
        final seed = _WaveBLogSettings(
          _msgDoc(filter: 'error'),
          failSave: true,
        );
        final bridge = FakeMonitorBridge();
        await _pumpLogs(tester, seed: seed, bridge: bridge);
        expect(_keywordText(tester), 'error');

        await tester.enterText(
          find.byKey(const ValueKey('logs-keyword')),
          'warn',
        );
        await tester.pump(const Duration(milliseconds: 200));
        await tester.pump();

        expect(seed.saveAttempts, 1);
        expect(
          (seed.doc['MsgUIItem'] as Map<String, dynamic>)['MainMsgFilter'],
          'error',
        );
        expect(_keywordText(tester), 'error');
        expect(find.textContaining('日志选项保存失败'), findsOneWidget);
        await tester.pumpWidget(const SizedBox());
      },
    );

    testWidgets(
      'FLD-CFG-066: refresh save failure keeps the persisted switch',
      (tester) async {
        final seed = _WaveBLogSettings(
          _msgDoc(autoRefresh: true),
          failSave: true,
        );
        final bridge = FakeMonitorBridge();
        await _pumpLogs(tester, seed: seed, bridge: bridge);
        expect(_autoRefreshSwitch(tester), isTrue);

        await tester.tap(find.byKey(const ValueKey('logs-autorefresh')));
        await tester.pump();

        expect(seed.saveAttempts, 1);
        expect(_autoRefreshSwitch(tester), isTrue);
        expect(
          (seed.doc['MsgUIItem'] as Map<String, dynamic>)['AutoRefresh'],
          isTrue,
        );
        expect(find.textContaining('日志选项保存失败'), findsOneWidget);
        await tester.pumpWidget(const SizedBox());
      },
    );

    testWidgets('FLD-CFG-066: off reopens frozen, on keeps tailing + resyncs', (
      tester,
    ) async {
      final seed = _WaveBLogSettings(_msgDoc(autoRefresh: false));
      final bridge = FakeMonitorBridge();
      bridge.logs = <m.LogLineDto>[line('frozen tail')];
      final container = await _pumpLogs(tester, seed: seed, bridge: bridge);
      expect(_autoRefreshSwitch(tester), isFalse);

      // Frozen: a live batch must not enter the view...
      bridge.emitLogs(<m.LogLineDto>[line('while frozen')]);
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pump();
      expect(find.text('while frozen'), findsNothing);

      // ...but the ring kept it; reopen stays frozen on the persisted switch.
      bridge.logs = <m.LogLineDto>[line('frozen tail')];
      await _reopenLogs(tester, container);
      expect(_autoRefreshSwitch(tester), isFalse);

      // Resume: the frozen view resyncs from the tail and tails new lines.
      await tester.tap(find.byKey(const ValueKey('logs-autorefresh')));
      await tester.pump();
      expect(seed.saveAttempts, 1);
      expect(_autoRefreshSwitch(tester), isTrue);
      expect(find.text('frozen tail'), findsOneWidget);
      bridge.emitLogs(<m.LogLineDto>[line('live tail')]);
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pump();
      expect(find.text('live tail'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    });
  });
}
