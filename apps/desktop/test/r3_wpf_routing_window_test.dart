// R3-WPF-ROUTING-WINDOW: the routing settings UI runs in the independent
// window (second engine) with a snapshot/draft host. These tests cover the
// Wave K structure, the 确定/取消/Esc semantics and the draft relayed back to
// the main engine via the host.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/routing/routing_controller.dart';
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';

class _FakeHost implements RoutingEditorHost {
  _FakeHost({required this.snapshot, this.saveOk = true});

  final RoutingEditorSnapshot snapshot;
  final bool saveOk;
  int saveCalls = 0;
  int closeCalls = 0;
  RoutingDraft? lastDraft;

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async => snapshot;

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    saveCalls++;
    lastDraft = draft;
    return RoutingEditorOutcome(
      ok: saveOk,
      message: saveOk ? null : '保存路由设置失败',
    );
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }
}

r.RoutingRuleDto _rule(String remarks) => r.RoutingRuleDto(
  id: RoutingController.newRuleId(),
  ruleKind: 'field',
  outboundTag: 'proxy',
  inboundTag: const <String>[],
  hasInboundTag: false,
  ip: const <String>[],
  hasIp: false,
  domain: const <String>['example.com'],
  hasDomain: true,
  protocol: const <String>[],
  hasProtocol: false,
  process: const <String>[],
  hasProcess: false,
  enabled: true,
  remarks: remarks,
  ruleType: 1,
);

RoutingSchemeSnapshot _scheme({
  required String id,
  required String remarks,
  bool active = false,
  int ruleCount = 0,
}) => RoutingSchemeSnapshot(
  profile: r.RoutingProfileDto(
    id: id,
    remarks: remarks,
    url: '',
    ruleSet: '[]',
    ruleNum: ruleCount,
    enabled: true,
    locked: false,
    customIcon: '',
    customRulesetPath4Singbox: '',
    domainStrategy: '',
    domainStrategy4Singbox: '',
    sort: 0,
    isActive: active,
  ),
  rules: <r.RoutingRuleDto>[for (var i = 0; i < ruleCount; i++) _rule('规则 $i')],
);

RoutingEditorSnapshot _snapshot() => RoutingEditorSnapshot(
  schemes: <RoutingSchemeSnapshot>[
    _scheme(id: 'a', remarks: 'V4-绕过大陆(Whitelist)', active: true, ruleCount: 2),
    _scheme(id: 'b', remarks: 'V4-全局(Global)', ruleCount: 1),
  ],
  domainStrategy: 'AsIs',
  domainStrategySbox: '',
  outboundTags: const <String>['proxy', 'direct', 'block'],
);

Future<void> _pump(WidgetTester tester, _FakeHost host) async {
  tester.view.physicalSize = const Size(1400, 1000);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(MaterialApp(home: RoutingEditorWindow(host: host)));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('renders the Wave K structure from the host snapshot', (
    tester,
  ) async {
    final host = _FakeHost(snapshot: _snapshot());
    await _pump(tester, host);

    expect(find.text('添加规则集'), findsOneWidget);
    expect(find.text('一键导入规则集'), findsOneWidget);
    expect(find.text('域名解析策略'), findsOneWidget);
    expect(find.text('sing-box 域名解析策略'), findsOneWidget);
    expect(find.text('预定义规则集列表'), findsOneWidget);
    for (final column in <String>['别名', '数量', '排序', '可选地址 (Url)', '自定义图标']) {
      expect(
        find.text(column),
        findsOneWidget,
        reason: 'missing column $column',
      );
    }
    for (final key in <String>[
      'routing-add',
      'routing-import-builtin',
      'routing-block-title',
      'routing-list',
      'routing-ok',
      'routing-cancel',
    ]) {
      expect(find.byKey(ValueKey<String>(key)), findsOneWidget);
    }
    expect(find.text('确定'), findsOneWidget);
    expect(find.text('取消'), findsOneWidget);
    expect(host.saveCalls, 0);
    expect(host.closeCalls, 0);
  });

  testWidgets('确定 relays the whole draft to the host then closes', (
    tester,
  ) async {
    final host = _FakeHost(snapshot: _snapshot());
    await _pump(tester, host);

    await tester.tap(find.byKey(const ValueKey('routing-ok')));
    await tester.pumpAndSettle();

    expect(host.saveCalls, 1);
    expect(host.closeCalls, 1);
    expect(host.lastDraft!.schemes.length, 2);
    expect(host.lastDraft!.domainStrategy, 'AsIs');
  });

  testWidgets('editing a scheme feeds back into the relayed draft', (
    tester,
  ) async {
    final host = _FakeHost(snapshot: _snapshot());
    await _pump(tester, host);

    await tester.tap(find.byKey(const ValueKey('routing-add')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('routing-ruleset-window')),
      findsOneWidget,
    );

    await tester.enterText(
      find.byKey(const ValueKey('ruleset-remarks')),
      '新增方案',
    );
    await tester.tap(find.byKey(const ValueKey('ruleset-save')));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('routing-ok')));
    await tester.pumpAndSettle();

    expect(host.saveCalls, 1);
    expect(host.lastDraft!.schemes.length, 3);
    expect(
      host.lastDraft!.schemes.any((s) => s.profile.remarks == '新增方案'),
      isTrue,
    );
  });

  testWidgets('取消 discards the draft: no save, window closes', (tester) async {
    final host = _FakeHost(snapshot: _snapshot());
    await _pump(tester, host);

    await tester.tap(find.byKey(const ValueKey('routing-cancel')));
    await tester.pumpAndSettle();

    expect(host.saveCalls, 0);
    expect(host.closeCalls, 1);
  });

  testWidgets('Esc closes without writing the draft', (tester) async {
    final host = _FakeHost(snapshot: _snapshot());
    await _pump(tester, host);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();

    expect(host.saveCalls, 0);
    expect(host.closeCalls, 1);
  });

  testWidgets('a failed save keeps the window open and shows the error', (
    tester,
  ) async {
    final host = _FakeHost(snapshot: _snapshot(), saveOk: false);
    await _pump(tester, host);

    await tester.tap(find.byKey(const ValueKey('routing-ok')));
    await tester.pumpAndSettle();

    expect(host.saveCalls, 1);
    expect(host.closeCalls, 0);
    expect(find.text('保存路由设置失败'), findsOneWidget);
  });
}
