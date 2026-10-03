// FIX-03B: Custom/Outbound editor Browse (file import -> Address rewrite ->
// default Remarks) and Edit (external open with full-path resolution).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/profiles/custom_editor_dialog.dart';
import 'package:v2rayn_desktop/features/profiles/profile_draft.dart';

class _Harness {
  String? pickedPath;
  c.CustomFileResult Function(String)? importResult;
  final List<String> imported = <String>[];
  final List<String> opened = <String>[];
  bool openSucceeds = true;
  String dataDir = r'C:\data';
}

c.CustomFileResult _importOk(String _) =>
    const c.CustomFileResult(ok: true, fileName: 'stored.json');

c.CustomFileResult _importFail(String _) => const c.CustomFileResult(
  ok: false,
  error: c.ErrorDto(
    code: 'E_NOT_FOUND',
    messageKey: 'error.custom_file_not_found',
    retryable: false,
  ),
);

Future<List<c.ProfileDto>> _openEditor(
  WidgetTester tester,
  _Harness h, {
  ConfigType type = ConfigType.custom,
  String initialRemarks = '',
  String initialAddress = '',
}) async {
  tester.view.physicalSize = const Size(1600, 1200);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final saved = <c.ProfileDto>[];
  final draft = ProfileDraft()
    ..configType = type
    ..remarks = initialRemarks
    ..address = initialAddress;
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (context) => Center(
            child: ElevatedButton(
              onPressed: () => showCustomEditor(
                context,
                initial: draft,
                onSave: (dto) {
                  saved.add(dto);
                  return c.SaveProfileResult(ok: true, profile: dto);
                },
                customImportFile: (source) {
                  h.imported.add(source);
                  return (h.importResult ?? _importOk)(source);
                },
                dataDir: () => h.dataDir,
                pickFile: () async => h.pickedPath,
                openFile: (path) async {
                  h.opened.add(path);
                  return h.openSucceeds;
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
  return saved;
}

Future<void> _browse(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('custom-browse')));
  await tester.pumpAndSettle();
}

Future<void> _edit(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('custom-edit')));
  await tester.pumpAndSettle();
}

void main() {
  group('resolveCustomConfigPath', () {
    test('absolute path is used as-is', () {
      expect(
        resolveCustomConfigPath(address: r'C:\abs\x.json', dataDir: r'C:\data'),
        r'C:\abs\x.json',
      );
      expect(
        resolveCustomConfigPath(address: '/unix/x.json', dataDir: '/d'),
        '/unix/x.json',
      );
    });

    test('relative name joins dataDir/config', () {
      expect(
        resolveCustomConfigPath(address: 'a.json', dataDir: r'C:\data'),
        '${r'C:\data'}\\config\\a.json',
      );
    });
  });

  group('defaultCustomRemarks', () {
    test('matches upstream custom/outbound prefixes', () {
      final now = DateTime(2026, 10, 4, 9, 8, 7);
      expect(
        defaultCustomRemarks(ConfigType.custom, now),
        'import custom@2026/10/04 09:08:07',
      );
      expect(
        defaultCustomRemarks(ConfigType.outbound, now),
        'import custom outbound@2026/10/04 09:08:07',
      );
    });
  });

  group('SyntheticBridgePort custom seam', () {
    test('records source and returns a stored name with the extension', () {
      final b = SyntheticBridgePort();
      final r = b.customImportFile('C:/tmp/my.yaml');
      expect(r.ok, isTrue);
      expect(r.fileName, endsWith('.yaml'));
      expect(b.importedCustomFiles, <String>['C:/tmp/my.yaml']);
      expect(b.dataDir(), isNotEmpty);
    });

    test('reports a structured failure when forced', () {
      final b = SyntheticBridgePort()..failCustomImport = true;
      final r = b.customImportFile('C:/tmp/my.yaml');
      expect(r.ok, isFalse);
      expect(r.error?.messageKey, 'error.custom_file_not_found');
    });
  });

  group('custom editor Browse', () {
    testWidgets('copies the picked file and rewrites Address', (tester) async {
      final h = _Harness()..pickedPath = 'C:/tmp/src.json';
      final saved = await _openEditor(tester, h);
      await _browse(tester);
      expect(h.imported, <String>['C:/tmp/src.json']);
      expect(find.text('stored.json'), findsOneWidget);
      await tester.tap(find.byKey(const ValueKey('custom-save')));
      await tester.pumpAndSettle();
      expect(saved.single.address, 'stored.json');
      expect(saved.single.remarks, startsWith('import custom@'));
    });

    testWidgets('outbound empty remarks gets the outbound prefix', (
      tester,
    ) async {
      final h = _Harness()..pickedPath = 'C:/tmp/src.yaml';
      final saved = await _openEditor(tester, h, type: ConfigType.outbound);
      await _browse(tester);
      await tester.tap(find.byKey(const ValueKey('custom-save')));
      await tester.pumpAndSettle();
      expect(saved.single.configType, ConfigType.outbound);
      expect(saved.single.remarks, startsWith('import custom outbound@'));
    });

    testWidgets('cancelled picker leaves the draft untouched', (tester) async {
      final h = _Harness();
      await _openEditor(tester, h);
      await _browse(tester);
      expect(h.imported, isEmpty);
      expect(find.text('stored.json'), findsNothing);
    });

    testWidgets('failed import keeps Address and shows a file error', (
      tester,
    ) async {
      final h = _Harness()
        ..pickedPath = 'C:/tmp/src.json'
        ..importResult = _importFail;
      await _openEditor(tester, h);
      await _browse(tester);
      expect(h.imported, <String>['C:/tmp/src.json']);
      expect(find.byKey(const ValueKey('custom-file-error')), findsOneWidget);
      expect(find.text('stored.json'), findsNothing);
    });

    testWidgets('non-empty remarks are not overwritten on browse', (
      tester,
    ) async {
      final h = _Harness()..pickedPath = 'C:/tmp/src.json';
      final saved = await _openEditor(tester, h, initialRemarks: 'mine');
      await _browse(tester);
      await tester.tap(find.byKey(const ValueKey('custom-save')));
      await tester.pumpAndSettle();
      expect(saved.single.remarks, 'mine');
    });
  });

  group('custom editor Edit', () {
    testWidgets('relative Address resolves under dataDir/config', (
      tester,
    ) async {
      final h = _Harness();
      await _openEditor(tester, h, initialAddress: 'a.json');
      await _edit(tester);
      expect(h.opened.single, '${r'C:\data'}\\config\\a.json');
      expect(find.byKey(const ValueKey('custom-file-error')), findsNothing);
    });

    testWidgets('absolute Address is opened directly', (tester) async {
      final h = _Harness();
      await _openEditor(tester, h, initialAddress: r'C:\abs\x.json');
      await _edit(tester);
      expect(h.opened.single, r'C:\abs\x.json');
    });

    testWidgets('missing file shows an explicit error', (tester) async {
      final h = _Harness()..openSucceeds = false;
      await _openEditor(tester, h, initialAddress: 'gone.json');
      await _edit(tester);
      expect(find.byKey(const ValueKey('custom-file-error')), findsOneWidget);
      expect(find.textContaining('gone.json'), findsWidgets);
    });

    testWidgets('empty Address is rejected before opening', (tester) async {
      final h = _Harness();
      await _openEditor(tester, h);
      await _edit(tester);
      expect(h.opened, isEmpty);
      expect(find.byKey(const ValueKey('custom-file-error')), findsOneWidget);
    });
  });
}
