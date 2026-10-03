// FIX-03: special nodes reopen in dedicated editors; cancel is a no-op and
// every extra/reference/raw field survives the edit round-trip.
//
// Upstream: ProfilesViewModel.EditServerAsync dispatches Custom/Outbound ->
// AddServer2, PolicyGroup/ProxyChain (IsGroupType) -> AddGroup, the rest ->
// AddServer. Manual child selection excludes only Custom
// (AddGroupServerViewModel.AddChildAsync); nested groups are ring-checked at
// save time by Rust `validate_group`.
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/custom_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_actions.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

c.ProfileDto _leaf(
  String id,
  String remarks,
  ConfigType type, {
  String subid = 'sub-1',
}) => c.ProfileDto(
  indexId: id,
  configType: type,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: subid,
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

Future<void> _openCustom(
  WidgetTester tester, {
  required ProfileDraft initial,
  required List<c.ProfileDto> saved,
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
              onPressed: () async => showCustomEditor(
                context,
                initial: initial,
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

Future<void> _openGroup(
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

void main() {
  test('dispatch routes every config type to the upstream editor', () {
    const custom = <ConfigType>[ConfigType.custom, ConfigType.outbound];
    const group = <ConfigType>[ConfigType.policyGroup, ConfigType.proxyChain];
    for (final t in ConfigType.values) {
      final kind = resolveEditorKind(t);
      if (custom.contains(t)) {
        expect(kind, SpecialEditorKind.custom, reason: t.name);
      } else if (group.contains(t)) {
        expect(kind, SpecialEditorKind.group, reason: t.name);
      } else {
        expect(kind, SpecialEditorKind.generic, reason: t.name);
      }
    }
  });

  testWidgets(
    'custom reopen keeps extra/reference/raw fields on remarks-only save',
    (tester) async {
      final dto = c.ProfileDto(
        indexId: 'custom-1',
        configType: ConfigType.custom,
        coreType: CoreType.mihomo,
        configVersion: 4,
        subid: 'sub-9',
        isSub: false,
        preSocksPort: 11820,
        displayLog: false,
        remarks: 'my custom',
        address: 'custom.yaml',
        port: 0,
        password: '',
        username: '',
        network: 'raw',
        security: const c.SecurityDto(
          streamSecurity: 'tls',
          sni: 'example.com',
        ),
        protoExtra: const c.ProtocolExtraDto(
          isSingboxEndpoint: true,
          extraJson:
              '{"customConfigText": "mode: rule\\n", "futureFlag": true}',
        ),
        transportExtra: const c.TransportExtraDto(
          extraJson: '{"futureTransport": 1}',
        ),
        extraJson: '{"futureTop": "kept"}',
      );
      final saved = <c.ProfileDto>[];
      await _openCustom(
        tester,
        initial: ProfileDraft.fromDto(dto),
        saved: saved,
      );
      expect(find.byKey(const ValueKey('custom-editor')), findsOneWidget);
      await tester.enterText(
        find.byKey(const ValueKey('custom-remarks')),
        'my custom renamed',
      );
      await tester.tap(find.byKey(const ValueKey('custom-save')));
      await tester.pumpAndSettle();
      expect(saved, hasLength(1));
      final out = saved.single;
      expect(out.remarks, 'my custom renamed');
      expect(out.configType, ConfigType.custom);
      expect(out.coreType, CoreType.mihomo);
      expect(out.subid, 'sub-9');
      expect(out.isSub, isFalse);
      expect(out.preSocksPort, 11820);
      expect(out.displayLog, isFalse);
      expect(out.address, 'custom.yaml');
      expect(out.security.streamSecurity, 'tls');
      expect(out.security.sni, 'example.com');
      final proto =
          jsonDecode(out.protoExtra.extraJson) as Map<String, dynamic>;
      expect(proto['customConfigText'], 'mode: rule\n');
      expect(proto['futureFlag'], isTrue);
      expect(out.protoExtra.isSingboxEndpoint, isTrue);
      expect(jsonDecode(out.transportExtra.extraJson)['futureTransport'], 1);
      expect(jsonDecode(out.extraJson)['futureTop'], 'kept');
    },
  );

  testWidgets('custom cancel calls onSave zero times', (tester) async {
    var calls = 0;
    final draft = ProfileDraft()
      ..indexId = 'custom-1'
      ..configType = ConfigType.custom
      ..remarks = 'stored'
      ..address = 'custom.json';
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
                onPressed: () async => showCustomEditor(
                  context,
                  initial: draft,
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
      find.byKey(const ValueKey('custom-remarks')),
      'edited but cancelled',
    );
    await tester.tap(find.byKey(const ValueKey('custom-cancel')));
    await tester.pumpAndSettle();
    expect(calls, 0);
    expect(draft.remarks, 'stored');
  });

  testWidgets('group reopen keeps children/order/modes/refs on save', (
    tester,
  ) async {
    final n1 = _leaf('n1', 'HK-1', ConfigType.vless);
    final n2 = _leaf('n2', 'US-1', ConfigType.vless);
    final nested = _leaf('g2', 'nested group', ConfigType.policyGroup);
    final outbound = _leaf('o1', 'out', ConfigType.outbound);
    final dto = c.ProfileDto(
      indexId: 'g1',
      configType: ConfigType.policyGroup,
      coreType: CoreType.xray,
      configVersion: 4,
      subid: 'sub-1',
      isSub: false,
      displayLog: true,
      remarks: 'my group',
      address: '',
      port: 0,
      password: '',
      username: '',
      network: '',
      security: const c.SecurityDto(),
      protoExtra: const c.ProtocolExtraDto(
        groupType: 'PolicyGroup',
        childItems: 'n2,n1',
        subChildItems: 'sub-1',
        filter: '^HK',
        multipleLoad: 3,
        extraJson: '{"futureGroup": "kept"}',
      ),
      transportExtra: const c.TransportExtraDto(extraJson: '{}'),
      extraJson: '{}',
    );
    final saved = <c.ProfileDto>[];
    await _openGroup(
      tester,
      initial: ProfileDraft.fromDto(dto),
      all: <c.ProfileDto>[n1, n2, nested, outbound],
      saved: saved,
      preview: (_) => <c.ProfileDto>[n2, n1],
    );
    // Stored children render in order; stored mode/filter render back.
    expect(find.byKey(const ValueKey('group-child-n2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-child-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-preview-n2')), findsOneWidget);
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'my group renamed',
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    final out = saved.single;
    expect(out.remarks, 'my group renamed');
    expect(out.configType, ConfigType.policyGroup);
    expect(out.protoExtra.childItems, 'n2,n1');
    expect(out.protoExtra.subChildItems, 'sub-1');
    expect(out.protoExtra.filter, '^HK');
    expect(out.protoExtra.multipleLoad, 3);
    expect(out.protoExtra.groupType, 'PolicyGroup');
    expect(jsonDecode(out.protoExtra.extraJson)['futureGroup'], 'kept');
  });

  testWidgets('group picker offers nested groups/outbound but not custom', (
    tester,
  ) async {
    final all = <c.ProfileDto>[
      _leaf('n1', 'leaf', ConfigType.vless),
      _leaf('g2', 'nested', ConfigType.policyGroup),
      _leaf('c2', 'chain', ConfigType.proxyChain),
      _leaf('o1', 'outbound', ConfigType.outbound),
      _leaf('x1', 'custom file', ConfigType.custom),
    ];
    await _openGroup(
      tester,
      initial: ProfileDraft()..configType = ConfigType.policyGroup,
      all: all,
      saved: <c.ProfileDto>[],
    );
    await tester.tap(find.byKey(const ValueKey('group-pick-open')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-picker')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-g2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-c2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-o1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-node-x1')), findsNothing);
  });

  testWidgets('group editor exposes all five multiple-load modes', (
    tester,
  ) async {
    await _openGroup(
      tester,
      initial: ProfileDraft()..configType = ConfigType.policyGroup,
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

  testWidgets('group cancel calls onSave zero times', (tester) async {
    var calls = 0;
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
                  initial: ProfileDraft()
                    ..indexId = 'g1'
                    ..configType = ConfigType.policyGroup
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
}
