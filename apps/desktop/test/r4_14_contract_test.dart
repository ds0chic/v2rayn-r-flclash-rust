// R4-14 路由原版提交与导入 contract tests.
//
// Synthetic, in-memory fakes only: no native library, core, port, system
// proxy/route/TUN, registry or real window is touched.
//
// Covers the card's must-pass scenarios at the window/host boundary:
//  * `一键导入规则集` really commits the backend import and shows the schemes;
//  * a failed commit never changes the in-memory list and shows the error;
//  * Ctrl+A + Delete removes all selected schemes through per-scheme commits;
//  * the original sub-editor 确定 / strategy immediate commits still hold.
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/routing.dart' as r;
import 'package:v2rayn_desktop/features/routing/routing_windows.dart';

r.RoutingProfileDto _profile(
  String id,
  String remarks, {
  int sort = 0,
  bool active = false,
}) => r.RoutingProfileDto(
  id: id,
  remarks: remarks,
  url: '',
  ruleSet: '[]',
  ruleNum: 0,
  enabled: true,
  locked: false,
  customIcon: '',
  customRulesetPath4Singbox: '',
  domainStrategy: '',
  domainStrategy4Singbox: '',
  sort: sort,
  isActive: active,
);

r.RoutingRuleDto _rule(String id) => r.RoutingRuleDto(
  id: id,
  inboundTag: const <String>[],
  hasInboundTag: false,
  outboundTag: 'proxy',
  ip: const <String>[],
  hasIp: false,
  domain: const <String>[],
  hasDomain: false,
  protocol: const <String>[],
  hasProtocol: false,
  process: const <String>[],
  hasProcess: false,
  enabled: true,
  ruleType: 1,
);

RoutingEditorSnapshot _snapshot() => RoutingEditorSnapshot(
  schemes: <RoutingSchemeSnapshot>[
    RoutingSchemeSnapshot(
      profile: _profile('a', 'V4-绕过大陆', sort: 1, active: true),
      rules: const <r.RoutingRuleDto>[],
    ),
    RoutingSchemeSnapshot(
      profile: _profile('b', 'V4-全局', sort: 2),
      rules: const <r.RoutingRuleDto>[],
    ),
  ],
  domainStrategy: 'AsIs',
  domainStrategySbox: '',
  outboundTags: const <String>['proxy', 'direct', 'block'],
);

class _FakeCommitHost implements RoutingEditorHost, RoutingCommitHost {
  _FakeCommitHost(this.snapshot, {this.failCommit = false});

  final RoutingEditorSnapshot snapshot;
  bool failCommit;
  int saveCalls = 0;
  int closeCalls = 0;
  final List<String> actions = <String>[];

  @override
  Future<RoutingEditorSnapshot> loadSnapshot() async => snapshot;

  @override
  Future<RoutingEditorOutcome> save(RoutingDraft draft) async {
    saveCalls++;
    return const RoutingEditorOutcome(ok: true);
  }

  @override
  Future<void> close() async {
    closeCalls++;
  }

  @override
  Future<RoutingEditorOutcome> commit(String actionJson) async {
    actions.add(actionJson);
    if (failCommit) {
      return const RoutingEditorOutcome(ok: false, message: '保存路由设置失败');
    }
    if (actionJson.contains('importBuiltin')) {
      return RoutingEditorOutcome(
        ok: true,
        schemes: <RoutingSchemeSnapshot>[
          RoutingSchemeSnapshot(
            profile: _profile('imp-1', 'V4-绕过大陆(Whitelist)', sort: 3),
            rules: <r.RoutingRuleDto>[_rule('ir1')],
          ),
          RoutingSchemeSnapshot(
            profile: _profile('imp-2', 'V4-黑名单(Blacklist)', sort: 4),
            rules: <r.RoutingRuleDto>[_rule('ir2')],
          ),
          RoutingSchemeSnapshot(
            profile: _profile('imp-3', 'V4-全局(Global)', sort: 5),
            rules: <r.RoutingRuleDto>[_rule('ir3')],
          ),
        ],
      );
    }
    return const RoutingEditorOutcome(ok: true);
  }
}

Future<void> _pump(WidgetTester tester, RoutingEditorHost host) async {
  tester.view.physicalSize = const Size(1400, 1000);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(MaterialApp(home: RoutingEditorWindow(host: host)));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('一键导入规则集 commits the real backend import', (tester) async {
    final host = _FakeCommitHost(_snapshot());
    await _pump(tester, host);

    await tester.tap(find.byKey(const ValueKey('routing-import-builtin')));
    await tester.pumpAndSettle();

    expect(host.actions, hasLength(1));
    expect(host.actions.first, contains('"kind":"importBuiltin"'));
    // Imported schemes are visible in the same window.
    expect(find.text('V4-绕过大陆(Whitelist)'), findsOneWidget);
    expect(find.text('V4-黑名单(Blacklist)'), findsOneWidget);
    expect(find.text('V4-全局(Global)'), findsOneWidget);
  });

  testWidgets('failed commit leaves the list unchanged and shows the error', (
    tester,
  ) async {
    final host = _FakeCommitHost(_snapshot(), failCommit: true);
    await _pump(tester, host);

    await tester.tap(find.byKey(const ValueKey('routing-add')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('ruleset-remarks')),
      '新方案',
    );
    await tester.tap(find.byKey(const ValueKey('ruleset-save')));
    await tester.pumpAndSettle();

    expect(host.actions, hasLength(1));
    expect(host.actions.first, contains('"kind":"saveScheme"'));
    // Save failed: in-memory state must not change.
    expect(find.text('新方案'), findsNothing);
    expect(find.byKey(const ValueKey('routing-error')), findsOneWidget);
    expect(find.text('保存路由设置失败'), findsOneWidget);
  });

  testWidgets('Ctrl+A + Delete removes all selected schemes', (tester) async {
    final host = _FakeCommitHost(_snapshot());
    await _pump(tester, host);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pumpAndSettle();

    await tester.sendKeyEvent(LogicalKeyboardKey.delete);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('routing-delete-confirm')));
    await tester.pumpAndSettle();

    final deletes = host.actions
        .where((a) => a.contains('"kind":"deleteScheme"'))
        .toList();
    expect(deletes, hasLength(2));
    expect(find.byKey(const ValueKey('routing-row-a')), findsNothing);
    expect(find.byKey(const ValueKey('routing-row-b')), findsNothing);
  });

  testWidgets('sub-editor 确定 commits immediately; 取消 never saves the draft', (
    tester,
  ) async {
    final host = _FakeCommitHost(_snapshot());
    await _pump(tester, host);

    await tester.tap(find.byKey(const ValueKey('routing-add')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('ruleset-remarks')),
      '新增方案',
    );
    await tester.tap(find.byKey(const ValueKey('ruleset-save')));
    await tester.pumpAndSettle();
    expect(host.actions.first, contains('"kind":"saveScheme"'));
    expect(find.text('新增方案'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('routing-cancel')));
    await tester.pumpAndSettle();
    expect(host.closeCalls, 1);
    expect(host.saveCalls, 0);
  });

  testWidgets('strategy change commits immediately', (tester) async {
    final host = _FakeCommitHost(_snapshot());
    await _pump(tester, host);

    await tester.tap(find.byKey(const ValueKey('routing-domain-strategy')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('UseIP').last);
    await tester.pumpAndSettle();

    expect(host.actions, hasLength(1));
    expect(host.actions.first, contains('"kind":"strategy"'));
  });
}
