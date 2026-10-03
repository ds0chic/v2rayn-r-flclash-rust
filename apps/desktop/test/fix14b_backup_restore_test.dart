import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_and_restore_view.dart';
import 'package:v2rayn_desktop/features/backup/backup_picker.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

// FIX-14B targeted UI tests: local restore is driven by the native file picker
// (cancel = no-op), a picked ZIP is recognised/activated, a picked bundle
// directory goes through `backup_restore`, a corrupted archive surfaces a
// structured error without touching the live config, and the window refreshes
// engine-owned state after a restore.

class _FakePicker implements BackupPicker {
  const _FakePicker({this.archive, this.directory});

  final String? archive;
  final String? directory;

  @override
  Future<String?> pickArchive() async => archive;

  @override
  Future<String?> pickDirectory() async => directory;
}

/// The synthetic bridge's import always succeeds; this subclass makes the
/// corrupted-archive path observable without touching the shared fake.
class _FailingImportBridge extends SyntheticBridgePort {
  @override
  c.ImportSummaryDto t16BackupImportUpstream(String path) {
    t16Calls.add('backup_import:$path');
    return c.ImportSummaryDto(
      ok: false,
      status: 'failed',
      sourceVersion: 0,
      importedRows: BigInt.zero,
      warnings: 0,
      errors: 1,
      message: '',
      error: const c.ErrorDto(
        code: 'E_FIELD_FORMAT',
        messageKey: 'error.backup_invalid',
        retryable: false,
      ),
    );
  }
}

ProviderContainer makeContainer(
  SyntheticBridgePort bridge,
  BackupPicker picker,
) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    backupPickerProvider.overrideWithValue(picker),
  ],
);

Future<void> pumpBackup(
  WidgetTester tester,
  ProviderContainer container,
) async {
  await tester.binding.setSurfaceSize(const Size(1100, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: const MaterialApp(home: Scaffold(body: BackupAndRestoreView())),
    ),
  );
  await tester.pump();
}

Future<void> pressKey(WidgetTester tester, String key) async {
  final finder = find.byKey(ValueKey(key));
  final dynamic button = tester.widget(finder);
  final VoidCallback? onPressed = button.onPressed as VoidCallback?;
  expect(onPressed, isNotNull, reason: '$key is disabled');
  onPressed!.call();
  await tester.pump();
}

int coreVersionCalls(SyntheticBridgePort bridge) =>
    bridge.t16Calls.where((call) => call == 'get_core_versions').length;

void main() {
  testWidgets('cancelled archive picker is a no-op', (tester) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge, const _FakePicker());
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await pressKey(tester, 'backup-restore-zip-btn');
    await tester.pumpAndSettle();

    expect(
      bridge.t16Calls.where((call) => call.startsWith('backup_recognize')),
      isEmpty,
    );
    expect(
      bridge.t16Calls.where((call) => call.startsWith('backup_import')),
      isEmpty,
    );
    expect(find.text('就绪'), findsOneWidget);
  });

  testWidgets('archive restore recognizes, imports and refreshes state', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(
      bridge,
      const _FakePicker(archive: '/tmp/good.zip'),
    );
    addTearDown(container.dispose);
    await pumpBackup(tester, container);
    final before = coreVersionCalls(bridge);

    await pressKey(tester, 'backup-restore-zip-btn');
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('backup_recognize:/tmp/good.zip'));
    expect(bridge.t16Calls, contains('backup_import:/tmp/good.zip'));
    expect(coreVersionCalls(bridge), greaterThan(before));
    expect(find.textContaining('本地恢复完成'), findsOneWidget);
  });

  testWidgets('directory restore routes through bundle restore and refreshes', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(
      bridge,
      const _FakePicker(directory: '/tmp/good-bundle'),
    );
    addTearDown(container.dispose);
    await pumpBackup(tester, container);
    final before = coreVersionCalls(bridge);

    await pressKey(tester, 'backup-restore-dir-btn');
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('backup_restore:/tmp/good-bundle'));
    expect(coreVersionCalls(bridge), greaterThan(before));
    expect(find.textContaining('本地恢复完成'), findsOneWidget);
  });

  testWidgets('corrupted zip surfaces the error and keeps existing config', (
    tester,
  ) async {
    final bridge = _FailingImportBridge();
    final container = makeContainer(
      bridge,
      const _FakePicker(archive: '/tmp/corrupt.zip'),
    );
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await pressKey(tester, 'backup-restore-zip-btn');
    await tester.pumpAndSettle();

    expect(find.textContaining('本地恢复失败'), findsOneWidget);
    expect(find.textContaining('E_FIELD_FORMAT'), findsOneWidget);
    expect(find.textContaining('现有配置'), findsOneWidget);
  });

  testWidgets('listed backup shows time/size and restores on selection', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge, const _FakePicker());
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await tester.enterText(
      find.byKey(const ValueKey('backup-list-field')),
      '/tmp/backups',
    );
    await pressKey(tester, 'backup-list-btn');
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('backup_list:/tmp/backups'));
    expect(find.byKey(const ValueKey('backup-list-item-0')), findsOneWidget);
    expect(find.textContaining('项资源'), findsOneWidget);

    await pressKey(tester, 'backup-list-restore-0');
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('backup_restore:/tmp/backups'));
    expect(find.textContaining('本地恢复完成'), findsOneWidget);
  });
}
