// FIX-08 draft unit tests (no widgets): rule-id stability, import
// parse/validate, upstream-shape export round-trip, stored-shape
// serialization, and group-level unknown-key preservation.
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

import 'support/fake_platform_bridge.dart';

ProviderContainer _container() {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(SyntheticBridgePort()),
      uiStateStoreProvider.overrideWithValue(MemoryUiStateStore()),
      profileRowCountProvider.overrideWithValue(4),
      platformBridgeProvider.overrideWithValue(FakePlatformBridge()),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('new rule drafts carry stable unique ids', () {
    final controller = _container().read(routingControllerProvider.notifier);
    final ids = <String>{
      for (var i = 0; i < 50; i++) controller.newRuleDraft().id,
    };
    expect(ids, hasLength(50));
    expect(ids.every((id) => id.isNotEmpty), isTrue);
  });

  test('import parse accepts both shapes, refreshes ids, rejects junk', () {
    // Upstream camelCase shape.
    final camel = RoutingController.parseImportedRuleDtos(
      '[{"outboundTag": "proxy", "domain": ["geosite:google"], "enabled": true}]',
    );
    expect(camel, hasLength(1));
    expect(camel.first.id, isNotEmpty);
    expect(camel.first.outboundTag, 'proxy');
    expect(camel.first.domain, ['geosite:google']);

    // Stored snake_case shape with a rule-kind discriminator.
    final snake = RoutingController.parseImportedRuleDtos(
      '[{"outbound_tag": "direct", "port": "443", "rule_type": 2}]',
    );
    expect(snake.first.outboundTag, 'direct');
    expect(snake.first.ruleType, 2);

    // Re-parsing the same text yields different ids (fresh entry ids).
    final again = RoutingController.parseImportedRuleDtos(
      '[{"outboundTag": "proxy", "domain": ["geosite:google"]}]',
    );
    expect(again.first.id, isNot(camel.first.id));

    expect(
      () => RoutingController.parseImportedRuleDtos('not json'),
      throwsFormatException,
    );
    expect(
      () => RoutingController.parseImportedRuleDtos('[]'),
      throwsFormatException,
    );
    expect(
      () =>
          RoutingController.parseImportedRuleDtos('[{"outboundTag": "proxy"}]'),
      throwsFormatException,
    );
  });

  test('draft export is upstream-shaped, id-free and re-importable', () {
    final container = _container();
    final controller = container.read(routingControllerProvider.notifier);
    final draft = controller.newRuleDraft();
    final rules = [
      r.RoutingRuleDto(
        id: draft.id,
        outboundTag: 'proxy',
        inboundTag: const [],
        hasInboundTag: false,
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
      ),
    ];
    final text = RoutingController.exportDraftRulesJson(rules, [draft.id]);
    final decoded = jsonDecode(text) as List;
    expect(decoded, hasLength(1));
    final entry = decoded.first as Map;
    expect(entry.containsKey('id'), isFalse);
    expect(entry['outboundTag'], 'proxy');
    expect(entry['domain'], ['geosite:google']);
    // Round-trip: re-import assigns a fresh id.
    final back = RoutingController.parseImportedRuleDtos(text);
    expect(back, hasLength(1));
    expect(back.first.id, isNot(draft.id));
  });

  test('ruleset serialization keeps ids and matches the stored shape', () {
    final container = _container();
    final controller = container.read(routingControllerProvider.notifier);
    final draft = controller.newRuleDraft();
    final rules = [
      r.RoutingRuleDto(
        id: draft.id,
        outboundTag: 'proxy',
        inboundTag: const [],
        hasInboundTag: false,
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
      ),
    ];
    final text = RoutingController.rulesToRuleSetJson(rules);
    final decoded = jsonDecode(text) as List;
    final entry = decoded.first as Map;
    expect(entry['id'], draft.id);
    expect(entry['outbound_tag'], 'proxy');
    expect(entry['domain'], ['geosite:google']);
    expect(entry['inbound_tag'], isNull);
    expect(entry['rule_type'], 1);
    expect(RoutingController.rulesToRuleSetJson([]), '[]');
  });

  test('settings group save preserves unedited and unknown keys', () {
    final container = _container();
    final settings = container.read(settingsControllerProvider.notifier);
    settings.load();
    final before = Map<String, dynamic>.from(
      container.read(settingsControllerProvider).group('GuiItem'),
    );
    final patch = Map<String, dynamic>.of(before)
      ..['AutoRun'] = true
      ..['FutureToggle'] = true;
    final result = settings.saveGroup('GuiItem', patch);
    expect(result.ok, isTrue);
    final after = container.read(settingsControllerProvider).group('GuiItem');
    expect(after['AutoRun'], isTrue);
    expect(after['FutureToggle'], isTrue);
    // Unedited keys in the same group survived the scoped save.
    for (final key in before.keys) {
      if (key == 'AutoRun') continue;
      expect(after[key], before[key], reason: 'lost $key');
    }
  });
}
