import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_and_restore_view.dart';
import 'package:v2rayn_desktop/features/backup/backup_picker.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

// FIX-14 targeted UI tests: the backup window reports that an upstream import
// activates the restored settings/active node and that a local restore reloads
// the config + resources, while keeping the existing error path intact.

class _FakePicker implements BackupPicker {
  const _FakePicker({this.directory});

  final String? directory;

  @override
  Future<String?> pickArchive() async => null;

  @override
  Future<String?> pickDirectory() async => directory;
}

ProviderContainer makeContainer(
  SyntheticBridgePort bridge, {
  BackupPicker? picker,
}) => ProviderContainer(
  overrides: [
    bridgePortProvider.overrideWithValue(bridge),
    if (picker != null) backupPickerProvider.overrideWithValue(picker),
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

void main() {
  testWidgets('local restore success reports config and resource reload', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(
      bridge,
      picker: const _FakePicker(directory: '/tmp/good-bundle'),
    );
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await pressKey(tester, 'backup-restore-dir-btn');
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('backup_restore:/tmp/good-bundle'));
    expect(find.textContaining('已重载'), findsOneWidget);
  });

  testWidgets(
    'upstream import success reports activated settings/active node',
    (tester) async {
      final bridge = SyntheticBridgePort();
      final container = makeContainer(bridge);
      addTearDown(container.dispose);
      await pumpBackup(tester, container);

      await tester.enterText(
        find.byKey(const ValueKey('backup-archive-field')),
        '/tmp/upstream.zip',
      );
      await pressKey(tester, 'backup-import-btn');

      expect(bridge.t16Calls, contains('backup_import:/tmp/upstream.zip'));
      expect(find.textContaining('活动节点已激活'), findsOneWidget);
    },
  );

  testWidgets('broken bundle keeps the structured error path', (tester) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(
      bridge,
      picker: const _FakePicker(directory: '/tmp/broken'),
    );
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await pressKey(tester, 'backup-restore-dir-btn');
    await tester.pumpAndSettle();

    expect(find.textContaining('本地恢复失败'), findsOneWidget);
    expect(find.textContaining('E_FIELD_FORMAT'), findsOneWidget);
  });
}
