// RE-PROF-10: the group/chain editor "refresh preview" resolves the current
// draft, not the persisted node.
//
// Upstream `AddGroupServerViewModel.UpdatePreviewList` feeds
// `GetUpdatedProtocolExtra()` (current ChildItems/mode/SubChildItems/Filter)
// into `GroupProfileManager.GetChildProfileItemsByProtocolExtra`; the preview
// therefore follows unsaved edits and a brand-new group can preview before it
// is saved. The persisted generation path is Rust
// `application::groups::resolve_children`, which this Dart port mirrors.
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

Future<void> _openGroup(
  WidgetTester tester, {
  required ProfileDraft initial,
  required List<c.ProfileDto> all,
  List<c.SubItemDto> subs = const <c.SubItemDto>[],
  List<c.ProfileDto>? saved,
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
                  saved?.add(dto);
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

Future<void> _refresh(WidgetTester tester) async {
  final button = find.byKey(const ValueKey('group-preview-refresh'));
  await tester.ensureVisible(button);
  await tester.tap(button);
  await tester.pumpAndSettle();
}

Future<void> _pick(WidgetTester tester, String id) async {
  await tester.tap(find.byKey(const ValueKey('group-pick-open')));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(ValueKey('group-pick-node-$id')));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const ValueKey('group-pick-ok')));
  await tester.pumpAndSettle();
}

void main() {
  group('resolveGroupPreview', () {
    test('subscription matches first: eligible nodes + remarks filter', () {
      final all = <c.ProfileDto>[
        _node('n1', 'HK-1'),
        _node('n2', 'US-1'),
        _node('n3', 'HK-2', subid: 'sub-2'),
        _node('g2', 'HK-group', type: ConfigType.policyGroup),
        _node('o1', 'HK-out', type: ConfigType.outbound),
      ];
      final out = resolveGroupPreview(
        all: all,
        childIds: const <String>[],
        subChildItems: 'sub-1',
        filter: '^HK',
      );
      // g2 (PolicyGroup) is dropped, o1 (Outbound) is kept, sorted by id.
      expect(out.map((p) => p.indexId), <String>['n1', 'o1']);
    });

    test(
      'explicit children keep list order and de-dupe against sub matches',
      () {
        final all = <c.ProfileDto>[
          _node('a', 'A', subid: 's1'),
          _node('b', 'B', subid: 's1'),
          _node('c', 'C', subid: 's1'),
        ];
        final explicitOnly = resolveGroupPreview(
          all: all,
          childIds: const <String>['c', 'a', 'c'],
        );
        expect(explicitOnly.map((p) => p.indexId), <String>['c', 'a']);

        final merged = resolveGroupPreview(
          all: all,
          childIds: const <String>['b', 'a'],
          subChildItems: 's1',
          filter: '^A',
        );
        // Sub match `a` first, then explicit `b`; the repeated `a` is skipped.
        expect(merged.map((p) => p.indexId), <String>['a', 'b']);
      },
    );

    test('self sentinel uses owner subid and invalid filter matches all', () {
      final all = <c.ProfileDto>[
        _node('a', 'A', subid: 'own'),
        _node('b', 'B', subid: 'own'),
      ];
      final out = resolveGroupPreview(
        all: all,
        childIds: const <String>[],
        subChildItems: 'self',
        filter: '(',
        ownerSubId: 'own',
      );
      expect(out.map((p) => p.indexId), <String>['a', 'b']);
    });
  });

  testWidgets('edit draft to B + filter then refresh shows B, not stored A', (
    tester,
  ) async {
    final all = <c.ProfileDto>[_node('a', 'A-node'), _node('b', 'B-node')];
    await _openGroup(
      tester,
      initial: ProfileDraft()
        ..indexId = 'g1'
        ..configType = ConfigType.policyGroup
        ..remarks = 'g'
        ..childItems = 'a',
      all: all,
    );
    expect(find.byKey(const ValueKey('group-preview-a')), findsOneWidget);

    // Replace stored child A with draft child B.
    await tester.tap(find.byKey(const ValueKey('group-remove-a')));
    await tester.pumpAndSettle();
    await _pick(tester, 'b');
    await tester.enterText(find.byKey(const ValueKey('group-filter')), '^B');
    await _refresh(tester);

    expect(find.byKey(const ValueKey('group-preview-b')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-preview-a')), findsNothing);
  });

  testWidgets('cancel after a changed preview persists nothing', (
    tester,
  ) async {
    final saved = <c.ProfileDto>[];
    final all = <c.ProfileDto>[_node('a', 'A-node'), _node('b', 'B-node')];
    await _openGroup(
      tester,
      initial: ProfileDraft()
        ..indexId = 'g1'
        ..configType = ConfigType.policyGroup
        ..remarks = 'g'
        ..childItems = 'a',
      all: all,
      saved: saved,
    );
    await tester.tap(find.byKey(const ValueKey('group-remove-a')));
    await tester.pumpAndSettle();
    await _pick(tester, 'b');
    await _refresh(tester);
    expect(find.byKey(const ValueKey('group-preview-b')), findsOneWidget);

    await tester.ensureVisible(find.byKey(const ValueKey('group-cancel')));
    await tester.tap(find.byKey(const ValueKey('group-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
    expect(find.byKey(const ValueKey('group-editor')), findsNothing);
  });

  testWidgets('a new group previews a child before it is saved', (
    tester,
  ) async {
    final all = <c.ProfileDto>[_node('a', 'A-node')];
    await _openGroup(
      tester,
      initial: ProfileDraft()
        ..configType = ConfigType.policyGroup
        ..remarks = 'new group',
      all: all,
    );
    expect(find.byKey(const ValueKey('group-preview-empty')), findsOneWidget);
    expect(find.text('新建节点保存后可预览'), findsNothing);

    await _pick(tester, 'a');
    await _refresh(tester);
    expect(find.byKey(const ValueKey('group-preview-a')), findsOneWidget);
  });

  testWidgets('subscription children are filtered by the draft remarks regex', (
    tester,
  ) async {
    final all = <c.ProfileDto>[
      _node('n1', 'HK-1'),
      _node('n2', 'US-1'),
      _node('n3', 'HK-2', subid: 'sub-2'),
      _node('g2', 'HK-group', type: ConfigType.policyGroup),
    ];
    await _openGroup(
      tester,
      initial: ProfileDraft()
        ..indexId = 'g1'
        ..configType = ConfigType.policyGroup
        ..remarks = 'g'
        ..subChildItems = 'sub-1'
        ..filter = '^HK',
      all: all,
      subs: <c.SubItemDto>[_sub('sub-1', 'demo')],
    );
    expect(find.byKey(const ValueKey('group-preview-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-preview-n2')), findsNothing);
    expect(find.byKey(const ValueKey('group-preview-n3')), findsNothing);
    expect(find.byKey(const ValueKey('group-preview-g2')), findsNothing);

    // Editing only the filter re-resolves the draft.
    await tester.enterText(find.byKey(const ValueKey('group-filter')), '^US');
    await _refresh(tester);
    expect(find.byKey(const ValueKey('group-preview-n1')), findsNothing);
    expect(find.byKey(const ValueKey('group-preview-n2')), findsOneWidget);
  });

  testWidgets('chain draft reorder is reflected by the refresh preview', (
    tester,
  ) async {
    final all = <c.ProfileDto>[_node('n1', 'HK-1'), _node('n2', 'US-1')];
    await _openGroup(
      tester,
      initial: ProfileDraft()
        ..indexId = 'pc1'
        ..configType = ConfigType.proxyChain
        ..remarks = 'pc'
        ..childItems = 'n2,n1',
      all: all,
    );
    expect(find.byKey(const ValueKey('group-preview-n2')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-preview-n1')), findsOneWidget);
    expect(
      tester.getTopLeft(find.byKey(const ValueKey('group-preview-n2'))).dy,
      lessThan(
        tester.getTopLeft(find.byKey(const ValueKey('group-preview-n1'))).dy,
      ),
    );

    // Move n1 to the top of the draft; refresh must follow the new order.
    await tester.tap(find.byKey(const ValueKey('group-t-n1')));
    await tester.pumpAndSettle();
    await _refresh(tester);
    expect(
      tester.getTopLeft(find.byKey(const ValueKey('group-preview-n1'))).dy,
      lessThan(
        tester.getTopLeft(find.byKey(const ValueKey('group-preview-n2'))).dy,
      ),
    );
  });
}
