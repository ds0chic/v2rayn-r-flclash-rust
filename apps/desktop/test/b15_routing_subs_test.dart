// B15-DART-ROUTING-SUBS: behavior regression tests (compile on baseline).
// D3a-01 new-rule order, D3a-02 shift anchor, S8-09 controller-side guard.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/profiles/ui_state_store.dart';
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';
import 'package:v2rayn_desktop/features/runtime/runtime_bridge.dart';
import 'package:v2rayn_desktop/features/settings/platform_bridge.dart';
import 'package:v2rayn_desktop/features/subs/subs_controller.dart';
import 'package:v2rayn_desktop/features/subs/sub_setting_window.dart';

import 'support/fake_platform_bridge.dart';
import 'support/subs_harness.dart';
import 'support/synthetic_runtime_bridge.dart';

ProviderContainer _routingContainer({BridgePort? bridge}) {
  final container = ProviderContainer(
    overrides: [
      bridgePortProvider.overrideWithValue(bridge ?? SyntheticBridgePort()),
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
  final saved = controller.save(
    r.RoutingProfileDto(
      id: base.id,
      remarks: 'b15-draft',
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
    ),
  );
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
              key: const ValueKey('b15-open-ruleset'),
              onPressed: () => showRoutingRulesetWindow(context, ref, scheme),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('b15-open-ruleset')));
  await tester.pumpAndSettle();
}

Future<void> _addRule(
  WidgetTester tester,
  String remarks,
  String domain,
) async {
  await tester.tap(find.byKey(const ValueKey('rule-add')));
  await tester.pumpAndSettle();
  await tester.enterText(find.byKey(const ValueKey('rule-remarks')), remarks);
  await tester.enterText(find.byKey(const ValueKey('rule-domain')), domain);
  await tester.pump();
  await tester.tap(find.byKey(const ValueKey('rule-details-save')));
  await tester.pumpAndSettle();
}

/// A bridge that (incorrectly) honours an empty export selection, so the
/// controller-side S8-09 guard is observable.
class _PermissiveExportBridge extends SyntheticBridgePort {
  @override
  r.RoutingRulesTextResult exportRoutingRules(
    String routingId,
    List<String> ids,
  ) => const r.RoutingRulesTextResult(ok: true, text: '[]', ruleCount: 0);
}

void main() {
  // D3a-01: a freshly added rule goes first (upstream `_rules.Insert(0)`).
  testWidgets('newly added rule is inserted at the top', (tester) async {
    final container = _routingContainer();
    await _setSize(tester);
    final scheme = _newScheme(container);
    await _openRuleset(tester, container, scheme);

    await _addRule(tester, 'b15-first', 'first.example.com');
    await _addRule(tester, 'b15-second', 'second.example.com');

    expect(find.text('b15-first'), findsOneWidget);
    expect(find.text('b15-second'), findsOneWidget);
    final secondY = tester.getTopLeft(find.text('b15-second')).dy;
    final firstY = tester.getTopLeft(find.text('b15-first')).dy;
    expect(
      secondY,
      lessThan(firstY),
      reason: 'the later-added rule must render above the earlier one',
    );
  });

  // D3a-02: repeated Shift+click extends from the fixed anchor, not the focus.
  testWidgets('shift-click range stays anchored at the first row', (
    tester,
  ) async {
    final bridge = SeededSubsBridge();
    for (final name in <String>['b15-s2', 'b15-s3', 'b15-s4']) {
      bridge.saveSubItem(
        c.SubItemDto(
          id: '',
          remarks: name,
          url: 'https://example.com/$name',
          moreUrl: '',
          enabled: true,
          userAgent: '',
          sort: 0,
          autoUpdateInterval: 0,
          updateTime: 0,
        ),
      );
    }
    final container = makeSubsContainer(bridge: bridge);
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const MaterialApp(home: Scaffold(body: SubSettingWindow())),
      ),
    );
    await tester.pump();
    final ids = [
      for (final s in container.read(subsControllerProvider).items) s.id,
    ];
    expect(ids.length, 4);

    await tester.tap(find.byKey(ValueKey('sub-row-${ids[0]}')));
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.tap(find.byKey(ValueKey('sub-row-${ids[1]}')));
    await tester.pump();
    await tester.tap(find.byKey(ValueKey('sub-row-${ids[3]}')));
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();

    final selected = container.read(subsControllerProvider).selectedIds;
    expect(
      selected,
      unorderedEquals(ids),
      reason: 'second shift-click must extend from the original anchor',
    );
  });

  // S8-09 controller-side guard: empty selection rejected without the bridge.
  test('exportRules rejects an empty selection before the bridge', () {
    final container = _routingContainer(bridge: _PermissiveExportBridge());
    final controller = container.read(routingControllerProvider.notifier);
    final result = controller.exportRules('any-id', const []);
    expect(result.ok, isFalse);
    expect(result.error?.messageKey, 'error.routing_no_selection');
  });
}
