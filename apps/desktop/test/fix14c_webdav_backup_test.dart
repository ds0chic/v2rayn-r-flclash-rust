import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_and_restore_view.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

/// Synthetic bridge with injectable WebDAV failures. Overrides only the
/// transport boundary so the controller/status wiring is exercised.
class _FailWebDavBridge extends SyntheticBridgePort {
  _FailWebDavBridge({this.backupError, this.restoreError});

  final c.ErrorDto? backupError;
  final c.ErrorDto? restoreError;

  @override
  Future<c.WebDavOpDto> t16WebdavBackup(c.WebDavConfigDto cfg) async {
    t16Calls.add('webdav_backup');
    final error = backupError;
    if (error != null) {
      return c.WebDavOpDto(
        ok: false,
        bytes: BigInt.zero,
        message: '',
        error: error,
      );
    }
    return super.t16WebdavBackup(cfg);
  }

  @override
  Future<c.RestoreResultDto> t16WebdavRestore(c.WebDavConfigDto cfg) async {
    t16Calls.add('webdav_restore');
    final error = restoreError;
    if (error != null) {
      return c.RestoreResultDto(
        ok: false,
        restored: false,
        message: '',
        error: error,
      );
    }
    return super.t16WebdavRestore(cfg);
  }
}

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

Future<void> fillUrlAndPress(
  WidgetTester tester,
  String buttonKey, {
  String url = 'https://dav.example.invalid',
}) async {
  await tester.enterText(find.byKey(const ValueKey('webdav-url')), url);
  final dynamic button = tester.widget(find.byKey(ValueKey(buttonKey)));
  final VoidCallback? onPressed = button.onPressed as VoidCallback?;
  expect(onPressed, isNotNull, reason: '$buttonKey is disabled');
  onPressed!.call();
  await tester.pumpAndSettle();
}

const _syntheticPassword = 'synthetic-pass-not-logged';

c.ErrorDto _error(String code, String key, {String? detail}) =>
    c.ErrorDto(code: code, messageKey: key, retryable: false, detail: detail);

void main() {
  testWidgets('remote backup success refreshes the remote listing', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await fillUrlAndPress(tester, 'webdav-backup-btn');

    expect(bridge.webdavUploads, isNotEmpty);
    expect(bridge.t16Calls, contains('webdav_list'));
    expect(find.textContaining('远程备份完成'), findsOneWidget);
    expect(find.textContaining('远程文件'), findsOneWidget);
  });

  testWidgets('remote backup failure reports the 401 branch and no success', (
    tester,
  ) async {
    final bridge = _FailWebDavBridge(
      backupError: _error(
        'E_PERMISSION_DENIED',
        'error.webdav_permission',
        detail: 'status 401',
      ),
    );
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await tester.enterText(
      find.byKey(const ValueKey('webdav-pass')),
      _syntheticPassword,
    );
    await fillUrlAndPress(tester, 'webdav-backup-btn');

    expect(find.textContaining('远程备份失败'), findsOneWidget);
    expect(find.textContaining('认证失败'), findsOneWidget);
    expect(find.textContaining('远程备份完成'), findsNothing);
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('backup-status')),
        matching: find.textContaining(_syntheticPassword),
      ),
      findsNothing,
    );
    expect(bridge.t16Calls, isNot(contains('webdav_list')));
  });

  testWidgets('remote restore failure reports the 404 branch', (tester) async {
    final bridge = _FailWebDavBridge(
      restoreError: _error(
        'E_NOT_FOUND',
        'error.webdav_not_found',
        detail: 'status 404',
      ),
    );
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await fillUrlAndPress(tester, 'webdav-restore-btn');

    expect(bridge.t16Calls, contains('webdav_restore'));
    expect(find.textContaining('远程恢复失败'), findsOneWidget);
    expect(find.textContaining('远端路径不存在'), findsOneWidget);
  });

  testWidgets('remote restore success reloads config and reports reopen', (
    tester,
  ) async {
    final bridge = SyntheticBridgePort();
    final container = makeContainer(bridge);
    addTearDown(container.dispose);
    await pumpBackup(tester, container);

    await fillUrlAndPress(tester, 'webdav-restore-btn');

    expect(bridge.t16Calls, contains('webdav_restore'));
    expect(find.textContaining('已重载'), findsOneWidget);
  });
}
