// FIX-08C (ACT-RR-002/004/005/008, F-ROUTING-003/004): routing draft import,
// delete confirmation and the outbound selector's Custom exclusion.
// Import failures and cancelled deletes leave the one draft untouched.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
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

r.RoutingProfileDto _newScheme(ProviderContainer container) {
  final controller = container.read(routingControllerProvider.notifier);
  final base = controller.newDraft();
  final draft = r.RoutingProfileDto(
    id: base.id,
    remarks: 'fix08c-draft',
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

Future<void> _openRuleset(
  WidgetTester tester,
  ProviderContainer container,
  r.RoutingProfileDto scheme,
) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        home: Scaffold(
          body: Consumer(
            builder: (context, ref, _) => TextButton(
              key: const ValueKey('fix08c-open-ruleset'),
              onPressed: () => showRoutingRulesetWindow(context, ref, scheme),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('fix08c-open-ruleset')));
  await tester.pumpAndSettle();
}

Future<void> _addRule(
  WidgetTester tester,
  String outbound,
  String domain,
) async {
  await tester.tap(find.byKey(const ValueKey('rule-add')));
  await tester.pumpAndSettle();
  await tester.enterText(find.byKey(const ValueKey('rule-outbound')), outbound);
  await tester.enterText(find.byKey(const ValueKey('rule-domain')), domain);
  await tester.pump();
  await tester.tap(find.byKey(const ValueKey('rule-details-save')));
  await tester.pumpAndSettle();
}

void _mockClipboard(String text) {
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(SystemChannels.platform, (call) async {
        if (call.method == 'Clipboard.getData') {
          return <String, dynamic>{'text': text};
        }
        return null;
      });
  addTearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null);
  });
}

void main() {
  testWidgets('import failure keeps the draft; success enters it', (
    tester,
  ) async {
    final container = _container();
    await _setSize(tester);
    final scheme = _newScheme(container);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    await _openRuleset(tester, container, scheme);

    // Invalid text: no dialog, no draft change, error surfaced.
    _mockClipboard('not json');
    await tester.tap(find.byKey(const ValueKey('rule-import-clipboard')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('routing-import-mode-dialog')),
      findsNothing,
    );
    expect(find.textContaining('导入失败'), findsOneWidget);
    expect(find.byKey(const ValueKey('rule-row-0')), findsNothing);
    expect(bridge.listRoutingRules(scheme.id).rules, isEmpty);

    // Valid upstream camelCase text appends into the draft only.
    _mockClipboard(
      '[{"outboundTag": "block", "domain": ["geosite:private"], "enabled": true}]',
    );
    await tester.tap(find.byKey(const ValueKey('rule-import-clipboard')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('routing-import-mode-dialog')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('routing-import-append')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('rule-row-0')), findsOneWidget);
    expect(bridge.listRoutingRules(scheme.id).rules, isEmpty);
  });

  testWidgets('deleting rules asks for confirmation; cancel keeps the draft', (
    tester,
  ) async {
    final container = _container();
    await _setSize(tester);
    final scheme = _newScheme(container);
    await _openRuleset(tester, container, scheme);
    await _addRule(tester, 'proxy', 'geosite:google');

    await tester.tap(find.byKey(const ValueKey('rule-row-0')));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.tap(find.byKey(const ValueKey('rule-remove')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('rule-delete-confirm-dialog')),
      findsOneWidget,
    );

    // Cancel keeps the selected rule (the selection is retained).
    await tester.tap(find.byKey(const ValueKey('rule-delete-cancel')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('rule-row-0')), findsOneWidget);

    // Confirm removes it.
    await tester.tap(find.byKey(const ValueKey('rule-remove')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('rule-delete-confirm')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('rule-row-0')), findsNothing);
    expect(find.text('暂无规则，请新增或导入'), findsOneWidget);
  });

  testWidgets('outbound selector excludes Custom profiles', (tester) async {
    final container = _container();
    await _setSize(tester);
    final bridge = container.read(bridgePortProvider) as SyntheticBridgePort;
    final profiles = bridge.queryAllProfiles();
    final custom = profiles.where((p) => p.configType == ConfigType.custom);
    final normal = profiles.where((p) => p.configType != ConfigType.custom);
    expect(custom, isNotEmpty, reason: 'synthetic seed must include Custom');
    expect(normal, isNotEmpty);

    final draft = container
        .read(routingControllerProvider.notifier)
        .newRuleDraft();
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          home: Scaffold(
            body: Consumer(
              builder: (context, ref, _) => TextButton(
                key: const ValueKey('fix08c-open-details'),
                onPressed: () =>
                    showRoutingRuleDetailsDialog(context, ref, draft),
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('fix08c-open-details')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('rule-select-profile')));
    await tester.pumpAndSettle();

    expect(
      find.byKey(ValueKey('rule-outbound-${custom.first.remarks}')),
      findsNothing,
    );
    final chosen = normal.first.remarks;
    expect(find.byKey(ValueKey('rule-outbound-$chosen')), findsOneWidget);
    await tester.tap(find.byKey(ValueKey('rule-outbound-$chosen')));
    await tester.pumpAndSettle();
    final field = tester.widget<TextField>(
      find.byKey(const ValueKey('rule-outbound')),
    );
    expect(field.controller!.text, chosen);
  });
}
