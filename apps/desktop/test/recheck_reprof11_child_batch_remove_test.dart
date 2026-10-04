// RE-PROF-11: existing children in the group editor support multi-select batch
// removal, mirroring `AddGroupServerViewModel.ChildRemoveAsync` iterating
// `SelectedChildren`. Single page build (locked Flutter resource leak).
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

Future<void> _openWithChildren(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1280, 1024);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final draft = ProfileDraft()
    ..configType = ConfigType.policyGroup
    ..childItems = 'n1,n2';
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
                subItems: const <c.SubItemDto>[],
                onSave: (dto) => c.SaveProfileResult(ok: true, profile: dto),
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
  testWidgets('selecting children enables and runs batch removal', (
    tester,
  ) async {
    await _openWithChildren(tester);
    expect(find.byKey(const ValueKey('group-child-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('group-child-n2')), findsOneWidget);

    final removeAll = tester.widget<OutlinedButton>(
      find.byKey(const ValueKey('group-remove-selected')),
    );
    expect(removeAll.onPressed, isNull, reason: 'disabled with no selection');

    await tester.tap(find.byKey(const ValueKey('group-child-select-n1')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('group-child-select-n2')));
    await tester.pumpAndSettle();
    expect(find.textContaining('移除选中 (2)'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('group-remove-selected')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('group-child-n1')), findsNothing);
    expect(find.byKey(const ValueKey('group-child-n2')), findsNothing);
    expect(find.textContaining('移除选中 (0)'), findsOneWidget);
  });
}
