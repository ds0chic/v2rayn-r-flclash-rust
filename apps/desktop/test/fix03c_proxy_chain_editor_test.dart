// FIX-03C: ProxyChain (102) chain node reopens in the group editor with the
// same child multi-selector / T-U-D-B ordering / five MultipleLoad modes and
// SubChildItems+Filter fields as PolicyGroup; cancel is a no-op and a
// save->reopen keeps order/mode/refs. Generation semantics are covered by the
// Rust synthetic-chain assertions (`crates/application/src/codegen.rs` tests
// and `crates/config_codegen/tests`).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

c.ProfileDto _leaf(String id, String remarks, ConfigType type) => c.ProfileDto(
  indexId: id,
  configType: type,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: 'sub-1',
  isSub: true,
  displayLog: true,
  remarks: remarks,
  address: '192.0.2.1',
  port: 443,
  password: '',
  username: '',
  network: 'raw',
  security: const c.SecurityDto(),
  protoExtra: const c.ProtocolExtraDto(extraJson: '{}'),
  transportExtra: const c.TransportExtraDto(extraJson: '{}'),
  extraJson: '{}',
);

c.SubItemDto _sub(String id, String remarks) => c.SubItemDto(
  id: id,
  remarks: remarks,
  url: 'https://example.com/$id',
  moreUrl: '',
  enabled: true,
  userAgent: '',
  sort: 1,
  autoUpdateInterval: 0,
  updateTime: 0,
);

Future<void> _open(
  WidgetTester tester, {
  required ProfileDraft initial,
  required List<c.ProfileDto> all,
  required List<c.ProfileDto> saved,
  List<c.ProfileDto> Function(String)? preview,
}) async {
  tester.view.physicalSize = const Size(1280, 1024);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (context) => Center(
            child: ElevatedButton(
              onPressed: () async => showGroupEditor(
                context,
                initial: initial,
                allProfiles: all,
                subItems: <c.SubItemDto>[_sub('sub-1', 'demo')],
                previewChildren: preview ?? (_) => const <c.ProfileDto>[],
                onSave: (dto) {
                  saved.add(dto);
                  return c.SaveProfileResult(ok: true, profile: dto);
                },
              ),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
}

Future<void> _pickAll(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('group-pick-open')));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const ValueKey('group-pick-select-all')));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const ValueKey('group-pick-ok')));
  await tester.pumpAndSettle();
}

c.ProfileDto _chainDto() => c.ProfileDto(
  indexId: 'pc1',
  configType: ConfigType.proxyChain,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: 'sub-1',
  isSub: false,
  displayLog: true,
  remarks: 'my chain',
  address: '',
  port: 0,
  password: '',
  username: '',
  network: '',
  security: const c.SecurityDto(),
  protoExtra: const c.ProtocolExtraDto(
    groupType: 'ProxyChain',
    childItems: 'n2,n1',
    subChildItems: 'sub-1',
    filter: '^HK',
    multipleLoad: 4,
    extraJson: '{"futureChain": "kept"}',
  ),
  transportExtra: const c.TransportExtraDto(extraJson: '{}'),
  extraJson: '{}',
);

void main() {
  testWidgets('chain reopen keeps type/group/order/mode/refs on save', (
    tester,
  ) async {
    final n1 = _leaf('n1', 'HK-1', ConfigType.vless);
    final n2 = _leaf('n2', 'US-1', ConfigType.vless);
    final saved = <c.ProfileDto>[];
    await _open(
      tester,
      initial: ProfileDraft.fromDto(_chainDto()),
      all: <c.ProfileDto>[n1, n2],
      saved: saved,
      preview: (_) => <c.ProfileDto>[n2, n1],
    );
    expect(find.byKey(const ValueKey('group-editor')), findsOneWidget);
    // Chain type preserved and NOT offered again as an add-time dropdown.
    expect(find.byKey(const ValueKey('group-config-type')), findsNothing);
    expect(find.byKey(const ValueKey('group-child-n2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-child-n1')), findsOneWidget);
    // Stored order n2,n1 is rendered top-to-bottom.
    expect(
      tester.getTopLeft(find.byKey(const ValueKey('group-child-n2'))).dy,
      lessThan(
        tester.getTopLeft(find.byKey(const ValueKey('group-child-n1'))).dy,
      ),
    );
    expect(find.textContaining('LeastLoad'), findsOneWidget);
    expect(find.textContaining('^HK'), findsOneWidget);

    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'my chain renamed',
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    final out = saved.single;
    expect(out.remarks, 'my chain renamed');
    expect(out.configType, ConfigType.proxyChain);
    expect(out.protoExtra.groupType, 'ProxyChain');
    expect(out.protoExtra.childItems, 'n2,n1');
    expect(out.protoExtra.subChildItems, 'sub-1');
    expect(out.protoExtra.filter, '^HK');
    expect(out.protoExtra.multipleLoad, 4);
  });

  testWidgets('new chain defaults to ProxyChain group type and saves mode', (
    tester,
  ) async {
    final n1 = _leaf('n1', 'HK-1', ConfigType.vless);
    final n2 = _leaf('n2', 'US-1', ConfigType.vless);
    final saved = <c.ProfileDto>[];
    await _open(
      tester,
      initial: ProfileDraft()..configType = ConfigType.proxyChain,
      all: <c.ProfileDto>[n1, n2],
      saved: saved,
    );
    expect(find.byKey(const ValueKey('group-config-type')), findsOneWidget);
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'new chain',
    );
    await _pickAll(tester);
    await tester.tap(find.byKey(const ValueKey('group-multiple-load')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('LeastLoad (最小负载)').last);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    final out = saved.single;
    expect(out.configType, ConfigType.proxyChain);
    expect(out.protoExtra.groupType, 'ProxyChain');
    expect(out.protoExtra.childItems, 'n1,n2');
    expect(out.protoExtra.multipleLoad, 4);
  });

  testWidgets('chain editor exposes all five multiple-load modes', (
    tester,
  ) async {
    await _open(
      tester,
      initial: ProfileDraft()..configType = ConfigType.proxyChain,
      all: <c.ProfileDto>[_leaf('n1', 'leaf', ConfigType.vless)],
      saved: <c.ProfileDto>[],
    );
    await tester.tap(find.byKey(const ValueKey('group-multiple-load')));
    await tester.pumpAndSettle();
    for (final label in <String>[
      'LeastPing',
      'Fallback',
      'Random',
      'RoundRobin',
      'LeastLoad',
    ]) {
      expect(find.textContaining(label), findsWidgets);
    }
  });

  testWidgets('chain editor add/move/remove child then save', (tester) async {
    final n1 = _leaf('n1', 'HK-1', ConfigType.vless);
    final n2 = _leaf('n2', 'US-1', ConfigType.vless);
    final saved = <c.ProfileDto>[];
    await _open(
      tester,
      initial: ProfileDraft()..configType = ConfigType.proxyChain,
      all: <c.ProfileDto>[n1, n2],
      saved: saved,
    );
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'my chain',
    );
    await _pickAll(tester);
    expect(find.byKey(const ValueKey('group-child-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-child-n2')), findsOneWidget);
    // Move n2 top (T) then remove n1: stored order becomes n2.
    await tester.tap(find.byKey(const ValueKey('group-t-n2')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-remove-n1')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(saved.single.protoExtra.childItems, 'n2');
  });

  testWidgets('chain picker offers nested groups/chain but not custom', (
    tester,
  ) async {
    final all = <c.ProfileDto>[
      _leaf('n1', 'leaf', ConfigType.vless),
      _leaf('g2', 'nested', ConfigType.policyGroup),
      _leaf('c2', 'chain', ConfigType.proxyChain),
      _leaf('x1', 'custom file', ConfigType.custom),
    ];
    await _open(
      tester,
      initial: ProfileDraft()..configType = ConfigType.proxyChain,
      all: all,
      saved: <c.ProfileDto>[],
    );
    await tester.tap(find.byKey(const ValueKey('group-pick-open')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-pick-node-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-g2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-x1')), findsNothing);
  });

  testWidgets('chain cancel calls onSave zero times', (tester) async {
    var calls = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Builder(
            builder: (context) => Center(
              child: ElevatedButton(
                onPressed: () async => showGroupEditor(
                  context,
                  initial: ProfileDraft()
                    ..indexId = 'pc1'
                    ..configType = ConfigType.proxyChain
                    ..remarks = 'stored',
                  allProfiles: const <c.ProfileDto>[],
                  subItems: const <c.SubItemDto>[],
                  onSave: (dto) {
                    calls++;
                    return c.SaveProfileResult(ok: true, profile: dto);
                  },
                ),
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'edited but cancelled',
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-cancel')));
    await tester.pumpAndSettle();
    expect(calls, 0);
  });

  testWidgets('chain save then reopen keeps children order and mode', (
    tester,
  ) async {
    final n1 = _leaf('n1', 'HK-1', ConfigType.vless);
    final n2 = _leaf('n2', 'US-1', ConfigType.vless);
    final saved = <c.ProfileDto>[];
    await _open(
      tester,
      initial: ProfileDraft()..configType = ConfigType.proxyChain,
      all: <c.ProfileDto>[n1, n2],
      saved: saved,
    );
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'round trip',
    );
    await _pickAll(tester);
    await tester.tap(find.byKey(const ValueKey('group-multiple-load')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Random (随机)').last);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    final out = saved.single;

    final reopened = <c.ProfileDto>[];
    await _open(
      tester,
      initial: ProfileDraft.fromDto(out),
      all: <c.ProfileDto>[n1, n2],
      saved: reopened,
      preview: (_) => <c.ProfileDto>[n1, n2],
    );
    expect(find.byKey(const ValueKey('group-editor')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-child-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-child-n2')), findsOneWidget);
    expect(find.textContaining('Random'), findsOneWidget);
    // Saving the untouched reopen produces the same chain fields.
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(reopened.single.configType, ConfigType.proxyChain);
    expect(reopened.single.protoExtra.groupType, 'ProxyChain');
    expect(reopened.single.protoExtra.childItems, 'n1,n2');
    expect(reopened.single.protoExtra.multipleLoad, 2);
  });
}
