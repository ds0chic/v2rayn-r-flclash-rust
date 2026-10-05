// R4-20 contract: policy group / proxy chain resolution and editor persistence.
//
// The Dart editor preview must mirror the persisted generation consumer
// (`application::groups::resolve_children` / `resolve_sub_children` ->
// `config_codegen` group/chain expansion). Synthetic data only; no native
// library, kernel, network, user data or ports are touched.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

c.ProfileDto _node(
  String id,
  String remarks, {
  String subid = 'sub-1',
  ConfigType type = ConfigType.vless,
  String address = '192.0.2.1',
  String password = '11111111-1111-1111-1111-111111111111',
  String? ssMethod,
}) => c.ProfileDto(
  indexId: id,
  configType: type,
  coreType: CoreType.xray,
  configVersion: 4,
  subid: subid,
  isSub: true,
  displayLog: true,
  remarks: remarks,
  address: address,
  port: 443,
  password: password,
  username: '',
  network: 'raw',
  security: const c.SecurityDto(),
  protoExtra: c.ProtocolExtraDto(extraJson: '{}', ssMethod: ssMethod),
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

Future<void> _openGroup(
  WidgetTester tester, {
  required ProfileDraft initial,
  required List<c.ProfileDto> all,
  required List<c.ProfileDto> saved,
  List<c.SubItemDto> subs = const <c.SubItemDto>[],
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
                subItems: subs,
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
  group('resolveGroupPreview (preview == generation semantics)', () {
    test('subscription source survives a cross-subscription id change', () {
      // A subscription node's IndexId can change on update; a sub-child group
      // references `subid` + remarks filter, so it must keep resolving.
      final before = resolveGroupPreview(
        all: <c.ProfileDto>[_node('old-id', 'JP-1', subid: 's1')],
        childIds: const <String>[],
        subChildItems: 'self',
        filter: '^JP',
        ownerSubId: 's1',
      );
      final after = resolveGroupPreview(
        all: <c.ProfileDto>[_node('new-id', 'JP-1', subid: 's1')],
        childIds: const <String>[],
        subChildItems: 'self',
        filter: '^JP',
        ownerSubId: 's1',
      );
      expect(before.map((p) => p.indexId), <String>['old-id']);
      expect(after.map((p) => p.indexId), <String>['new-id']);
    });

    test('nested group child is retained as a member (no expansion)', () {
      final nested = _node('g2', 'inner', type: ConfigType.policyGroup);
      final explicit = resolveGroupPreview(
        all: <c.ProfileDto>[_node('c1', 'A'), nested],
        childIds: const <String>['g2', 'c1'],
      );
      expect(explicit.map((p) => p.indexId), <String>['g2', 'c1']);
    });

    test('cycle members resolve one level without hanging', () {
      // a -> b, b -> a. The preview is one level (upstream
      // `GetChildProfileItemsByProtocolExtra`), and the save-time Rust
      // `validate_group` rejects the cycle with `E_GRAPH_CYCLE`.
      final a = _node('a', 'A', type: ConfigType.policyGroup);
      final b = _node('b', 'B', type: ConfigType.policyGroup);
      final out = resolveGroupPreview(
        all: <c.ProfileDto>[a, b],
        childIds: const <String>['b'],
      );
      expect(out.map((p) => p.indexId), <String>['b']);
    });

    test('region filter selects only matching remarks in index order', () {
      final out = resolveGroupPreview(
        all: <c.ProfileDto>[
          _node('jp2', 'JP-2'),
          _node('jp1', 'JP-1'),
          _node('us1', 'US-1'),
        ],
        childIds: const <String>[],
        subChildItems: 'sub-1',
        filter: '^JP',
      );
      expect(out.map((p) => p.indexId), <String>['jp1', 'jp2']);
    });

    test('unsupported Shadowsocks method is dropped from the preview', () {
      final out = resolveGroupPreview(
        all: <c.ProfileDto>[
          _node('ok', 'HK-vless'),
          _node(
            'ss-bad',
            'HK-ss',
            type: ConfigType.shadowsocks,
            password: 'secret',
            ssMethod: 'plain',
          ),
          _node(
            'ss-ok',
            'HK-ss-ok',
            type: ConfigType.shadowsocks,
            password: 'secret',
            ssMethod: 'aes-256-gcm',
          ),
        ],
        childIds: const <String>[],
        subChildItems: 'sub-1',
        filter: '^HK',
      );
      expect(out.map((p) => p.indexId), <String>['ok', 'ss-ok']);
    });
  });

  testWidgets('save persists the draft child order and sub source', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final all = <c.ProfileDto>[_node('c1', 'C1'), _node('c2', 'C2')];
    await _openGroup(
      tester,
      initial: ProfileDraft()
        ..indexId = 'g1'
        ..configType = ConfigType.policyGroup
        ..coreType = CoreType.xray
        ..remarks = 'g'
        ..multipleLoad = 0
        ..childItems = 'c2,c1'
        ..subChildItems = 'sub-1'
        ..filter = '^C',
      all: all,
      saved: saved,
      subs: <c.SubItemDto>[_sub('sub-1', 'demo')],
    );
    await tester.ensureVisible(find.byKey(const ValueKey('group-save')));
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();

    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.childItems, 'c2,c1');
    expect(saved.single.protoExtra.subChildItems, 'sub-1');
    expect(saved.single.protoExtra.filter, '^C');
  });

  testWidgets('cancel after edits persists nothing', (tester) async {
    final saved = <c.ProfileDto>[];
    final all = <c.ProfileDto>[_node('c1', 'C1'), _node('c2', 'C2')];
    await _openGroup(
      tester,
      initial: ProfileDraft()
        ..indexId = 'g1'
        ..configType = ConfigType.policyGroup
        ..remarks = 'g'
        ..childItems = 'c1',
      all: all,
      saved: saved,
    );
    await tester.tap(find.byKey(const ValueKey('group-remove-c1')));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('group-cancel')));
    await tester.tap(find.byKey(const ValueKey('group-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
    expect(find.byKey(const ValueKey('group-editor')), findsNothing);
  });
}
