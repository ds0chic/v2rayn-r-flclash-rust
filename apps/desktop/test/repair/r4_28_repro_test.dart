// R4-28 repro: backup/restore/WebDAV must never latch the window busy and a
// cancelled WebDAV operation must not commit its late result.
//
// These assertions encode the R4-28 contract and fail on the pre-fix
// controller (no unified catch/finally; WebDAV ops ignored the cancel
// generation). Synthetic only: no native library, no network, no port use and
// no user data.
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

/// A bridge whose WebDAV/local calls can throw a native-style exception or be
/// held open on a gate, so the controller's error/cancel handling is exercised.
class _FlakyBridge extends SyntheticBridgePort {
  final Completer<void> webdavGate = Completer<void>();
  final Completer<void> localGate = Completer<void>();
  bool throwOnWebdavCheck = false;
  bool throwOnLocal = false;
  int webdavBackupCalls = 0;

  @override
  Future<c.WebDavCheckDto> t16WebdavCheck(c.WebDavConfigDto cfg) async {
    if (throwOnWebdavCheck) {
      throw StateError('native bridge exploded at ${cfg.url}');
    }
    return super.t16WebdavCheck(cfg);
  }

  @override
  Future<c.BackupResultDto> t16BackupLocal(String destRoot) async {
    await localGate.future;
    if (throwOnLocal) {
      throw StateError('disk copy failed');
    }
    return super.t16BackupLocal(destRoot);
  }

  @override
  Future<c.WebDavOpDto> t16WebdavBackup(c.WebDavConfigDto cfg) async {
    webdavBackupCalls += 1;
    await webdavGate.future;
    return super.t16WebdavBackup(cfg);
  }
}

const _cfg = c.WebDavConfigDto(
  url: 'http://127.0.0.1:11818',
  userName: 'synthetic',
  password: 'synthetic-pass-not-logged',
  dirName: 'v2rayN_backup',
);

ProviderContainer _container(BridgePort bridge) {
  final container = ProviderContainer(
    overrides: [bridgePortProvider.overrideWithValue(bridge)],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test(
    'a thrown WebDAV check completes, reports an error and clears busy',
    () async {
      final bridge = _FlakyBridge()..throwOnWebdavCheck = true;
      final container = _container(bridge);
      final notifier = container.read(backupControllerProvider.notifier);

      // Pre-fix this future completes with an error and busy stays true.
      await expectLater(notifier.webdavCheck(_cfg), completes);
      final state = container.read(backupControllerProvider);
      expect(
        state.busy,
        isFalse,
        reason: 'a bridge exception must not latch busy',
      );
      expect(state.status?.isError, isTrue);
    },
  );

  test('a thrown local backup completes, reports and clears busy', () async {
    final bridge = _FlakyBridge()
      ..throwOnLocal = true
      ..localGate.complete();
    final container = _container(bridge);
    final notifier = container.read(backupControllerProvider.notifier);

    await expectLater(notifier.localBackup(r'C:\synthetic\dest'), completes);
    final state = container.read(backupControllerProvider);
    expect(state.busy, isFalse);
    expect(state.status?.isError, isTrue);
  });

  test('cancel during a slow remote backup discards the late result', () async {
    final bridge = _FlakyBridge();
    final container = _container(bridge);
    final notifier = container.read(backupControllerProvider.notifier);

    final pending = notifier.webdavBackup(_cfg);
    await Future<void>.delayed(Duration.zero);
    expect(container.read(backupControllerProvider).busy, isTrue);
    notifier.cancel();
    expect(container.read(backupControllerProvider).busy, isFalse);

    bridge.webdavGate.complete();
    await pending;
    final state = container.read(backupControllerProvider);
    expect(state.status?.message, '已取消');
    expect(
      state.status?.isSuccess,
      isFalse,
      reason: 'a cancelled upload must not be reported as a success',
    );
  });
}
