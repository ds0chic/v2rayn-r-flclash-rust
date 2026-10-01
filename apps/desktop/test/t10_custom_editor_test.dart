// T10: custom/outbound editor renders, validates JSON + ports, saves.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/features/profiles/custom_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

Future<void> _open(
  WidgetTester tester, {
  required List<c.ProfileDto> saved,
  ConfigType type = ConfigType.custom,
}) async {
  tester.view.physicalSize = const Size(1280, 1024);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final draft = ProfileDraft()..configType = type;
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
  testWidgets('custom editor renders all fields', (tester) async {
    await _open(tester, saved: <c.ProfileDto>[]);
    expect(find.byKey(const ValueKey('custom-editor')), findsOneWidget);
    expect(find.byKey(const ValueKey('custom-remarks')), findsOneWidget);
    expect(find.byKey(const ValueKey('custom-address')), findsOneWidget);
    expect(find.byKey(const ValueKey('custom-config-text')), findsOneWidget);
    expect(find.byKey(const ValueKey('custom-pre-socks-port')), findsOneWidget);
    expect(find.byKey(const ValueKey('custom-display-log')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('custom-singbox-endpoint')),
      findsOneWidget,
    );
  });

  testWidgets('custom editor rejects invalid json text', (tester) async {
    await _open(tester, saved: <c.ProfileDto>[]);
    await tester.enterText(
      find.byKey(const ValueKey('custom-remarks')),
      'custom',
    );
    await tester.enterText(
      find.byKey(const ValueKey('custom-address')),
      'custom.json',
    );
    await tester.enterText(
      find.byKey(const ValueKey('custom-config-text')),
      '{not json',
    );
    await tester.tap(find.byKey(const ValueKey('custom-save')));
    await tester.pumpAndSettle();
    // Still open: the JSON error blocks the save.
    expect(find.byKey(const ValueKey('custom-editor')), findsOneWidget);
    expect(find.textContaining('JSON'), findsWidgets);
  });

  testWidgets('custom editor rejects out-of-range pre-socks port', (
    tester,
  ) async {
    await _open(tester, saved: <c.ProfileDto>[]);
    await tester.enterText(
      find.byKey(const ValueKey('custom-remarks')),
      'custom',
    );
    await tester.enterText(
      find.byKey(const ValueKey('custom-address')),
      'custom.json',
    );
    await tester.enterText(
      find.byKey(const ValueKey('custom-pre-socks-port')),
      '99999',
    );
    await tester.tap(find.byKey(const ValueKey('custom-save')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('custom-editor')), findsOneWidget);
  });

  testWidgets('custom editor cancel persists nothing', (tester) async {
    final saved = <c.ProfileDto>[];
    await _open(tester, saved: saved);
    await tester.enterText(
      find.byKey(const ValueKey('custom-remarks')),
      'draft',
    );
    await tester.tap(find.byKey(const ValueKey('custom-cancel')));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
    expect(find.byKey(const ValueKey('custom-editor')), findsNothing);
  });

  testWidgets('custom editor saves json + flags through', (tester) async {
    final saved = <c.ProfileDto>[];
    await _open(tester, saved: saved, type: ConfigType.outbound);
    await tester.enterText(
      find.byKey(const ValueKey('custom-remarks')),
      'my outbound',
    );
    await tester.enterText(
      find.byKey(const ValueKey('custom-address')),
      'outbound.json',
    );
    await tester.enterText(
      find.byKey(const ValueKey('custom-config-text')),
      '{"tag": "proxy"}',
    );
    await tester.enterText(
      find.byKey(const ValueKey('custom-pre-socks-port')),
      '11820',
    );
    await tester.tap(find.byKey(const ValueKey('custom-save')));
    await tester.pumpAndSettle();
    expect(saved, hasLength(1));
    final dto = saved.single;
    expect(dto.configType, ConfigType.outbound);
    expect(dto.preSocksPort, 11820);
    expect(dto.protoExtra.extraJson, contains('customConfigText'));
    expect(find.byKey(const ValueKey('custom-editor')), findsNothing);
  });
}
