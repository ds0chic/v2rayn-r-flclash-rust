import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/monitor.dart' as m;
import 'package:v2rayn_desktop/features/monitor/clash_ui_config.dart';
import 'package:v2rayn_desktop/features/monitor/connections_view.dart';
import 'package:v2rayn_desktop/features/monitor/monitor_bridge.dart';
import 'package:v2rayn_desktop/features/monitor/proxies_view.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import 'support/fake_monitor_bridge.dart';

/// A settings controller seeded with a fixed document so the Clash monitor
/// consumer can be exercised without the native bridge.
class _FakeSettings extends SettingsController {
  _FakeSettings(this._document);

  final Map<String, dynamic> _document;

  @override
  SettingsViewState build() =>
      SettingsViewState(loaded: true, revision: 1, document: _document);
}

Map<String, dynamic> clashDoc({
  int sorting = 0,
  bool proxiesAuto = false,
  int proxiesInterval = 2,
  bool connectionsAuto = false,
  int connectionsInterval = 2,
}) => <String, dynamic>{
  'ClashUIItem': <String, dynamic>{
    'ProxiesSorting': sorting,
    'ProxiesAutoRefresh': proxiesAuto,
    'ProxiesRefreshInterval': proxiesInterval,
    'ConnectionsAutoRefresh': connectionsAuto,
    'ConnectionsRefreshInterval': connectionsInterval,
  },
};

m.ClashProxyDto node(String name, int delay) => m.ClashProxyDto(
  name: name,
  proxyType: 'Shadowsocks',
  isGroup: false,
  now: null,
  all: const <String>[],
  delay: delay,
  provider: null,
);

Future<FakeMonitorBridge> pumpProxies(
  WidgetTester tester, {
  required Map<String, dynamic> document,
  required FakeMonitorBridge fake,
}) async {
  addTearDown(fake.disposeStreams);
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(fake),
      settingsControllerProvider.overrideWith(() => _FakeSettings(document)),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: ProxiesView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return fake;
}

Future<FakeMonitorBridge> pumpConnections(
  WidgetTester tester, {
  required Map<String, dynamic> document,
  required FakeMonitorBridge fake,
}) async {
  addTearDown(fake.disposeStreams);
  final container = ProviderContainer(
    overrides: [
      monitorBridgeProvider.overrideWithValue(fake),
      settingsControllerProvider.overrideWith(() => _FakeSettings(document)),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: ConnectionsView())),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
  return fake;
}

double nodeY(WidgetTester tester, String name) =>
    tester.getTopLeft(find.byKey(ValueKey('proxies-node-$name'))).dy;

void main() {
  group('clashUiConfigFromDocument', () {
    test('reads ClashUIItem values', () {
      final config = clashUiConfigFromDocument(
        clashDoc(
          sorting: 1,
          proxiesAuto: true,
          proxiesInterval: 5,
          connectionsAuto: true,
          connectionsInterval: 7,
        ),
      );
      expect(config.proxiesSorting, 1);
      expect(config.proxiesAutoRefresh, isTrue);
      expect(config.proxiesRefreshInterval, 5);
      expect(config.connectionsAutoRefresh, isTrue);
      expect(config.connectionsRefreshInterval, 7);
      expect(config.proxiesRefreshEnabled, isTrue);
    });

    test('falls back to upstream defaults when the group is absent', () {
      final config = clashUiConfigFromDocument(<String, dynamic>{});
      expect(config.proxiesSorting, 0);
      expect(config.proxiesAutoRefresh, isFalse);
      expect(config.proxiesRefreshInterval, 2);
      expect(config.connectionsRefreshInterval, 2);
      expect(config.proxiesRefreshEnabled, isFalse);
    });

    test('non-positive interval disables auto refresh', () {
      final config = clashUiConfigFromDocument(
        clashDoc(proxiesAuto: true, proxiesInterval: 0),
      );
      expect(config.proxiesRefreshEnabled, isFalse);
    });
  });

  group('clashUiGroupWith', () {
    test('preserves unknown keys and applies the change', () {
      final document = <String, dynamic>{
        'ClashUIItem': <String, dynamic>{
          'EnableIPv6': true,
          'ProxiesSorting': 0,
        },
      };
      final group = clashUiGroupWith(document, <String, Object>{
        'ProxiesSorting': 1,
      });
      expect(group['EnableIPv6'], isTrue);
      expect(group['ProxiesSorting'], 1);
      // Original document is not mutated.
      expect(document['ClashUIItem']['ProxiesSorting'], 0);
    });
  });

  testWidgets('proxies view sorts by name when ProxiesSorting=1', (
    tester,
  ) async {
    final fake = FakeMonitorBridge(
      clashApiSupported: true,
      proxies: <m.ClashProxyDto>[node('C', 10), node('A', 30), node('B', 20)],
      clashMode: 'Rule',
    );
    await pumpProxies(tester, document: clashDoc(sorting: 1), fake: fake);
    expect(nodeY(tester, 'A'), lessThan(nodeY(tester, 'B')));
    expect(nodeY(tester, 'B'), lessThan(nodeY(tester, 'C')));
  });

  testWidgets('proxies view sorts by delay when ProxiesSorting=0', (
    tester,
  ) async {
    final fake = FakeMonitorBridge(
      clashApiSupported: true,
      proxies: <m.ClashProxyDto>[
        node('A', 300),
        node('B', 100),
        node('C', 200),
      ],
      clashMode: 'Rule',
    );
    await pumpProxies(tester, document: clashDoc(sorting: 0), fake: fake);
    expect(nodeY(tester, 'B'), lessThan(nodeY(tester, 'C')));
    expect(nodeY(tester, 'C'), lessThan(nodeY(tester, 'A')));
  });

  testWidgets('proxies view honors ProxiesAutoRefresh + interval', (
    tester,
  ) async {
    final fake = FakeMonitorBridge(
      clashApiSupported: true,
      proxies: <m.ClashProxyDto>[node('A', 10)],
      clashMode: 'Rule',
    );
    await pumpProxies(
      tester,
      document: clashDoc(proxiesAuto: true, proxiesInterval: 1),
      fake: fake,
    );
    final initial = fake.clashProxiesCount;
    expect(initial, greaterThanOrEqualTo(1));
    await tester.pump(const Duration(seconds: 1));
    await tester.pump();
    expect(fake.clashProxiesCount, greaterThan(initial));
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('connections view honors ConnectionsAutoRefresh + interval', (
    tester,
  ) async {
    final fake = FakeMonitorBridge(clashApiSupported: true);
    await pumpConnections(
      tester,
      document: clashDoc(connectionsAuto: true, connectionsInterval: 1),
      fake: fake,
    );
    final initial = fake.clashConnectionsCount;
    expect(initial, greaterThanOrEqualTo(1));
    await tester.pump(const Duration(seconds: 1));
    await tester.pump();
    expect(fake.clashConnectionsCount, greaterThan(initial));
    await tester.pumpWidget(const SizedBox());
  });
}
