// UC1-05..07: routing windows UX3 copy (strategy help, expert fold with
// badge/expand on existing values, ghost-field read-only rows).
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';

import 'support/fake_platform_bridge.dart';
import 'support/synthetic_runtime_bridge.dart';

ProviderContainer _container() {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(4),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
      runtimeBridgeProvider.overrideWithValue(SyntheticRuntimeBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

Future<void> _setSize(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1280, 900);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

r.RoutingProfileDto _saveScheme(
  ProviderContainer container, {
  String domainStrategy = '',
  String domainStrategy4Singbox = '',
}) {
  final controller = container.read(routingControllerProvider.notifier);
  final base = controller.newDraft();
  final saved = controller.save(
    r.RoutingProfileDto(
      id: base.id,
      remarks: 'ghost-case',
      url: '',
      ruleSet: '[]',
      ruleNum: 0,
      enabled: true,
      locked: false,
      customIcon: '',
      customRulesetPath4Singbox: '',
      domainStrategy: domainStrategy,
      domainStrategy4Singbox: domainStrategy4Singbox,
      sort: 0,
      isActive: false,
    ),
  );
  expect(saved.ok, isTrue);
  return saved.item!;
}

Future<void> _pumpWith(
  WidgetTester tester,
  ProviderContainer container,
  String key,
  void Function(BuildContext, WidgetRef) open,
) async {
  await _setSize(tester);
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Consumer(
          builder: (context, ref, _) => TextButton(
            key: ValueKey(key),
            onPressed: () => open(context, ref),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(ValueKey(key)));
  await tester.pumpAndSettle();
}

r.RoutingRuleDto _ruleWith({List<String> protocol = const <String>[]}) {
  return r.RoutingRuleDto(
    id: 'r1',
    ruleKind: null,
    port: null,
    network: null,
    inboundTag: const <String>[],
    hasInboundTag: false,
    outboundTag: 'proxy',
    ip: const <String>[],
    hasIp: false,
    domain: const <String>[],
    hasDomain: false,
    protocol: protocol,
    hasProtocol: protocol.isNotEmpty,
    process: const <String>[],
    hasProcess: false,
    enabled: true,
    remarks: null,
    ruleType: 1,
  );
}

void main() {
  testWidgets('strategy rows and scheme guide copy are visible', (
    tester,
  ) async {
    final container = _container();
    await _pumpWith(
      tester,
      container,
      'open-strategy',
      showRoutingSettingWindow,
    );
    expect(find.byType(RoutingSettingWindow), findsOneWidget);
    expect(find.textContaining('绕过大陆'), findsWidgets);
    expect(find.textContaining('AsIs'), findsWidgets);
    // Strategy-row helps render as Tooltip messages (not tree text).
    expect(
      find.byTooltip('默认 AsIs（保持原样）别动；改成 UseIP 会让按域名的规则批量失效（待核实）。'),
      findsOneWidget,
    );
    expect(
      find.byTooltip('用 sing-box 内核才需要看；ipv4_only/ipv6_only 是“断一臂”，别碰。'),
      findsOneWidget,
    );
  });

  testWidgets('ghost strategy fields show read-only and survive save', (
    tester,
  ) async {
    final container = _container();
    final scheme = _saveScheme(
      container,
      domainStrategy: 'UseIP',
      domainStrategy4Singbox: 'ipv4_only',
    );
    await _pumpWith(tester, container, 'open-ruleset', (context, ref) {
      showRoutingRulesetWindow(context, ref, scheme);
    });
    expect(find.byType(RoutingRulesetWindow), findsOneWidget);
    expect(find.textContaining('UseIP'), findsOneWidget);
    expect(find.textContaining('ipv4_only'), findsOneWidget);
    expect(find.textContaining('待核实'), findsWidgets);
    await tester.tap(find.byKey(const ValueKey('ruleset-save')));
    await tester.pumpAndSettle();
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final back = bridge.getRouting(scheme.id);
    expect(back.ok, isTrue);
    expect(back.item!.domainStrategy, 'UseIP');
    expect(back.item!.domainStrategy4Singbox, 'ipv4_only');
  });

  testWidgets('expert match section folds by default for empty rules', (
    tester,
  ) async {
    final container = _container();
    await _pumpWith(tester, container, 'open-details', (context, ref) {
      showRoutingRuleDetailsDialog(context, ref, _ruleWith());
    });
    expect(find.byType(RoutingRuleDetailsDialog), findsOneWidget);
    expect(find.text('专家匹配条件'), findsOneWidget);
    // Folded: protocol chips are offstage.
    expect(find.text('bittorrent'), findsNothing);
    expect(find.textContaining('只管解析不管走路'), findsOneWidget);
    expect(find.textContaining('回落'), findsOneWidget);
  });

  testWidgets('expert match section expands when values already exist', (
    tester,
  ) async {
    final container = _container();
    await _pumpWith(tester, container, 'open-details2', (context, ref) {
      showRoutingRuleDetailsDialog(
        context,
        ref,
        _ruleWith(protocol: const <String>['tls']),
      );
    });
    expect(find.byType(RoutingRuleDetailsDialog), findsOneWidget);
    expect(find.text('bittorrent'), findsOneWidget);
    expect(find.textContaining('已设置 1 项'), findsOneWidget);
  });
}
