// T11: routing controller (CRUD, move, import/export, rule mode) plus the
// routing settings window render/close through the synthetic bridge.
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';

import 'support/profiles_harness.dart';

r.RoutingRuleDto _rule(String id, String outbound) => r.RoutingRuleDto(
  id: id,
  inboundTag: const [],
  hasInboundTag: false,
  outboundTag: outbound,
  ip: const [],
  hasIp: false,
  domain: const ['geosite:google'],
  hasDomain: true,
  protocol: const [],
  hasProtocol: false,
  process: const [],
  hasProcess: false,
  enabled: true,
  ruleType: 1,
);

void main() {
  test('routing controller seeds one active builtin scheme', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    final state = container.read(routingControllerProvider);
    expect(state.items, hasLength(1));
    expect(state.selectedId, isNotNull);
    expect(state.ruleMode, 'Rule');
  });

  test('routing save rejects empty remarks', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    final controller = container.read(routingControllerProvider.notifier);
    final result = controller.save(controller.newDraft());
    expect(result.ok, isFalse);
    expect(result.error?.fieldPath, 'remarks');
  });

  test('routing rules move preserves order (T/U/D/B)', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    final controller = container.read(routingControllerProvider.notifier);
    final saved = controller.save(
      const r.RoutingProfileDto(
        id: '',
        remarks: 'order',
        url: '',
        ruleSet: '[]',
        ruleNum: 0,
        enabled: true,
        locked: false,
        customIcon: '',
        customRulesetPath4Singbox: '',
        domainStrategy: '',
        domainStrategy4Singbox: '',
        sort: 9,
        isActive: false,
      ),
    );
    expect(saved.ok, isTrue);
    final id = saved.item!.id;
    controller.saveRules(id, [_rule('a', 'proxy'), _rule('b', 'direct')]);
    // Move 'b' (index 1) up: order becomes b, a.
    final moved = controller.moveRule(id, 1, 1);
    expect(moved.ok, isTrue);
    final rules = container.read(routingControllerProvider).rules;
    expect(rules.map((e) => e.id), ['b', 'a']);
  });

  test('routing import/export round-trips through the bridge', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    final controller = container.read(routingControllerProvider.notifier);
    final saved = controller.save(
      const r.RoutingProfileDto(
        id: '',
        remarks: 'rt',
        url: '',
        ruleSet: '[]',
        ruleNum: 0,
        enabled: true,
        locked: false,
        customIcon: '',
        customRulesetPath4Singbox: '',
        domainStrategy: '',
        domainStrategy4Singbox: '',
        sort: 9,
        isActive: false,
      ),
    );
    final id = saved.item!.id;
    final imported = controller.importRules(
      id,
      '[{"outboundTag": "proxy"}, {"outboundTag": "direct"}]',
    );
    expect(imported.ok, isTrue);
    expect(imported.ruleCount, 2);
    final exported = controller.exportRules(id, const []);
    expect(exported.ok, isTrue);
    expect(exported.ruleCount, 2);
    final bad = controller.importRules(id, 'not json');
    expect(bad.ok, isFalse);
  });

  test('routing mode switches Rule/Global/Direct', () {
    final container = makeContainer();
    addTearDown(container.dispose);
    final controller = container.read(routingControllerProvider.notifier);
    expect(controller.setRuleMode('Global').ok, isTrue);
    expect(container.read(routingControllerProvider).ruleMode, 'Global');
    expect(controller.setRuleMode('Bogus').ok, isFalse);
    expect(controller.setRuleMode('Direct').ok, isTrue);
  });

  testWidgets('routing settings window renders list and closes', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 900);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final container = makeContainer();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: RoutingSettingWindow())),
      ),
    );
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('routing-setting-window')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('routing-list')), findsOneWidget);
    expect(find.text('V4-绕过大陆(Whitelist)'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('routing-domain-strategy')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('routing-close')));
    await tester.pumpAndSettle();
  });
}
