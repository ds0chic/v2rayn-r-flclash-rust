// T10: group editor renders, validates, cancels cleanly and saves children.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/group_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

c.ProfileDto _node(String id, String remarks) => c.ProfileDto(
  indexId: id,
  configType: ConfigType.vless,
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
  required List<c.ProfileDto> saved,
  ProfileDraft? initial,
}) async {
  tester.view.physicalSize = const Size(1280, 1024);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final draft =
      initial ?? (ProfileDraft()..configType = ConfigType.policyGroup);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (context) => Center(
            child: ElevatedButton(
              onPressed: () async => showGroupEditor(
                context,
                initial: draft,
                allProfiles: <c.ProfileDto>[
                  _node('n1', 'HK-1'),
                  _node('n2', 'US-1'),
                ],
                subItems: <c.SubItemDto>[_sub('sub-1', 'demo')],
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
  testWidgets('group editor renders all sections', (tester) async {
    await _open(tester, saved: <c.ProfileDto>[]);
    expect(find.byKey(const ValueKey('group-editor')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-remarks')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-multiple-load')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-pick-open')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-child-count')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-sub-child')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-filter')), findsOneWidget);
  });

  testWidgets('group editor requires remarks and children', (tester) async {
    await _open(tester, saved: <c.ProfileDto>[]);
    // Empty remarks: the form validator blocks the save in place.
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-editor')), findsOneWidget);
    expect(find.text('必填'), findsOneWidget);
    // Remarks filled but no children and no subscription: server-style error.
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'lonely group',
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-editor')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-error')), findsOneWidget);
  });

  testWidgets('group editor cancel persists nothing', (tester) async {
    final saved = <c.ProfileDto>[];
    await _open(tester, saved: saved);
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'draft remark',
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
    expect(find.byKey(const ValueKey('group-editor')), findsNothing);
  });

  testWidgets('group editor add/move/remove child then save', (tester) async {
    final saved = <c.ProfileDto>[];
    await _open(tester, saved: saved);
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'my group',
    );
    // Multi-select picker: check both nodes, confirm appends in list order.
    await tester.tap(find.byKey(const ValueKey('group-pick-open')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-picker')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('group-pick-select-all')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-pick-ok')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-child-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-child-n2')), findsOneWidget);

    // Move n2 up (U) so the order becomes n2,n1.
    await tester.tap(find.byKey(const ValueKey('group-u-n2')));
    await tester.pumpAndSettle();
    // Remove n1 again.
    await tester.tap(find.byKey(const ValueKey('group-remove-n1')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-child-n1')), findsNothing);

    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.childItems, 'n2');
    expect(saved.single.remarks, 'my group');
    expect(find.byKey(const ValueKey('group-editor')), findsNothing);
  });

  testWidgets('group editor picker cancel adds nothing', (tester) async {
    final saved = <c.ProfileDto>[];
    await _open(tester, saved: saved);
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'my group',
    );
    await tester.tap(find.byKey(const ValueKey('group-pick-open')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-pick-select-all')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-pick-cancel')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-child-n1')), findsNothing);
    expect(find.byKey(const ValueKey('group-child-n2')), findsNothing);
  });

  testWidgets('group editor keeps sub-child source and filter', (tester) async {
    final saved = <c.ProfileDto>[];
    await _open(tester, saved: saved);
    await tester.enterText(
      find.byKey(const ValueKey('group-remarks')),
      'sub group',
    );
    await tester.enterText(find.byKey(const ValueKey('group-filter')), '^HK');
    await tester.tap(find.byKey(const ValueKey('group-sub-child')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('demo').last);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    expect(saved.single.protoExtra.subChildItems, 'sub-1');
    expect(saved.single.protoExtra.filter, '^HK');
  });
}
