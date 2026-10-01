import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_and_restore_view.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

ProviderContainer makeContainer(SyntheticBridgePort bridge) =>
    ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
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

/// Invoke a button's `onPressed` directly. The window's content scrolls, so a
/// coordinate tap can land on the scroll viewport; calling the registered
/// callback still exercises the full controller -> bridge wiring.
Future<void> pressKey(WidgetTester tester, String key) async {
  final finder = find.byKey(ValueKey(key));
  final dynamic button = tester.widget(finder);
  final VoidCallback? onPressed = button.onPressed as VoidCallback?;
  expect(onPressed, isNotNull, reason: '$key is disabled');
  onPressed!.call();
  await tester.pump();
}

void main() {
  testWidgets('local backup calls the real bridge and reports the result', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await tester.enterText(
      find.byKey(const ValueKey('backup-dest-field')),
      '/tmp/backups',
    );
    await pressKey(tester, 'backup-local-btn');
    await tester.pump();

    expect(bridge.t16Calls, contains('backup_local:/tmp/backups'));
    expect(find.textContaining('本地备份完成'), findsOneWidget);
  });

  testWidgets('restore validates first and surfaces a structured error', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await tester.enterText(
      find.byKey(const ValueKey('backup-bundle-field')),
      '/tmp/broken',
    );
    await pressKey(tester, 'backup-restore-btn');
    await tester.pump();

    expect(bridge.t16Calls, contains('backup_restore:/tmp/broken'));
    expect(find.textContaining('本地恢复失败'), findsOneWidget);
    expect(find.textContaining('E_FIELD_FORMAT'), findsOneWidget);
  });

  testWidgets('import upstream ZIP reports the candidate-flow summary', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await tester.enterText(
      find.byKey(const ValueKey('backup-archive-field')),
      '/tmp/upstream.zip',
    );
    await pressKey(tester, 'backup-import-btn');
    await tester.pump();

    expect(bridge.t16Calls, contains('backup_import:/tmp/upstream.zip'));
    expect(find.textContaining('导入完成'), findsOneWidget);
  });

  testWidgets('WebDAV check failure is shown without credentials', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort()..webdavCheckOk = false;
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await tester.enterText(
      find.byKey(const ValueKey('webdav-url')),
      'https://dav.example.invalid',
    );
    await pressKey(tester, 'webdav-check-btn');
    await tester.pumpAndSettle();

    expect(bridge.t16Calls, contains('webdav_check'));
    expect(find.textContaining('WebDAV 连接失败'), findsOneWidget);
    expect(find.textContaining('E_PERMISSION_DENIED'), findsOneWidget);
  });

  testWidgets('WebDAV remote backup uploads through the bridge', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await tester.enterText(
      find.byKey(const ValueKey('webdav-url')),
      'https://dav.example.invalid',
    );
    await pressKey(tester, 'webdav-backup-btn');
    await tester.pumpAndSettle();

    expect(bridge.webdavUploads, isNotEmpty);
    expect(find.textContaining('远程备份完成'), findsOneWidget);
  });

  testWidgets('cleanup and open-directory actions route through the bridge', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await pressKey(tester, 'backup-cleanup-btn');
    await tester.pump();
    expect(bridge.t16Calls, contains('cleanup_logs_tmp'));
    expect(find.textContaining('已清理 3 个文件'), findsOneWidget);

    await pressKey(tester, 'backup-open-dir-btn');
    await tester.pump();
    expect(bridge.t16Calls, contains('open_config_dir'));
    expect(find.textContaining('已打开配置目录'), findsOneWidget);
  });
}
