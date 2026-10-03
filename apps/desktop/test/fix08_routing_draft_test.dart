// FIX-08 (SET-07/08/09/10): the routing scheme editor holds one draft.
// New rules enter with unique ids; move/import/export work on the draft;
// deleting down to an empty list still saves; errors keep the window open;
// the top strategy row edits the global RoutingBasicItem, not the scheme.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
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
import 'package:v2rayn_desktop/features/settings/settings_controller.dart';

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

r.RoutingProfileDto _newScheme(ProviderContainer container) {
  final controller = container.read(routingControllerProvider.notifier);
  final base = controller.newDraft();
  final draft = r.RoutingProfileDto(
    id: base.id,
    remarks: 'draft-case',
    url: '',
    ruleSet: '[]',
    ruleNum: 0,
    enabled: true,
    locked: false,
    customIcon: '',
    customRulesetPath4Singbox: '',
    domainStrategy: '',
    domainStrategy4Singbox: '',
    sort: 0,
    isActive: false,
  );
  final saved = controller.save(draft);
  expect(saved.ok, isTrue);
  return saved.item!;
}

Future<void> _addRule(
  WidgetTester tester,
  String outbound,
  String domain,
) async {
  await tester.tap(find.byKey(const ValueKey('rule-add')));
  await tester.pumpAndSettle();
  expect(
    find.byKey(const ValueKey('routing-rule-details-window')),
    findsOneWidget,
  );
  await tester.enterText(find.byKey(const ValueKey('rule-outbound')), outbound);
  await tester.enterText(find.byKey(const ValueKey('rule-domain')), domain);
  await tester.pump();
  await tester.tap(find.byKey(const ValueKey('rule-details-save')));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('ruleset draft: unique ids, draft move, delete-all saves', (
    tester,
  ) async {
    final container = _container();
    await _setSize(tester);
    final scheme = _newScheme(container);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Consumer(
            builder: (context, ref, _) => TextButton(
              key: const ValueKey('fix08-open-ruleset'),
              onPressed: () => showRoutingRulesetWindow(context, ref, scheme),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('fix08-open-ruleset')));
    await tester.pumpAndSettle();

    await _addRule(tester, 'proxy', 'geosite:google');
    await _addRule(tester, 'direct', 'geosite:cn');
    expect(find.byKey(const ValueKey('rule-row-0')), findsOneWidget);
    expect(find.byKey(const ValueKey('rule-row-1')), findsOneWidget);
    // Nothing reached storage yet: the draft is window-local.
    expect(bridge.listRoutingRules(scheme.id).rules, isEmpty);

    // Move the first row down inside the draft.
    await tester.tap(find.byKey(const ValueKey('rule-down-0')));
    await tester.pump();
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('rule-row-0')),
        matching: find.text('direct'),
      ),
      findsOneWidget,
    );
    expect(bridge.listRoutingRules(scheme.id).rules, isEmpty);

    // Import from the clipboard appends to the same draft, not to storage.
    // (The file flow shares the parse/merge path; the clipboard is mocked
    // here so no native channel is involved.)
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          if (call.method == 'Clipboard.getData') {
            return <String, dynamic>{
              'text': '[{"outboundTag": "block", "domain": ["geosite:private"], "enabled": true}]',
            };
          }
          return null;
        });
    addTearDown(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, null);
    });
    await tester.tap(find.byKey(const ValueKey('rule-import-clipboard')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    await tester.tap(find.byKey(const ValueKey('routing-import-append')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('rule-row-2')), findsOneWidget);
    expect(bridge.listRoutingRules(scheme.id).rules, isEmpty);

    // Select all three rows and delete them, then save the empty list.
    // (Row taps wait out the double-tap timeout before onTap fires.)
    await tester.tap(find.byKey(const ValueKey('rule-row-0')));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.tap(find.byKey(const ValueKey('rule-row-1')));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.tap(find.byKey(const ValueKey('rule-row-2')));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.tap(find.byKey(const ValueKey('rule-remove')));
    await tester.pump();
    expect(find.text('暂无规则，请新增或导入'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('ruleset-save')));
    await tester.pumpAndSettle();

    expect(find.byType(RoutingRulesetWindow), findsNothing);
    final stored = bridge.getRouting(scheme.id);
    expect(stored.ok, isTrue);
    expect(stored.item!.ruleNum, 0);
    expect(bridge.listRoutingRules(scheme.id).rules, isEmpty);
  });

  testWidgets('routing top strategy edits the global object, not the scheme', (
    tester,
  ) async {
    final container = _container();
    await _setSize(tester);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Consumer(
            builder: (context, ref, _) => TextButton(
              key: const ValueKey('fix08-open-routing'),
              onPressed: () => showRoutingSettingWindow(context, ref),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('fix08-open-routing')));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('routing-domain-strategy')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('UseIP').last);
    await tester.pumpAndSettle();

    final doc = container.read(settingsControllerProvider).document;
    final basic = doc['RoutingBasicItem'] as Map;
    expect(basic['DomainStrategy'], 'UseIP');
    // The selected scheme row was not rewritten with the strategy.
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final schemes = bridge.listRoutings().items;
    expect(schemes, isNotEmpty);
    for (final scheme in schemes) {
      expect(scheme.domainStrategy, isNot('UseIP'));
    }
  });
}
