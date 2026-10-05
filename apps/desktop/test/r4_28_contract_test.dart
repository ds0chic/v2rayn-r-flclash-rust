// R4-28 contract: local ZIP / WebDAV backup + restore, WebDAV 401/404/timeout/
// unreachable branches, upload failure is not a false success, busy clears on
// every path, cancel discards a late result and one operation runs at a time.
//
// Synthetic only: no native library, no network, no port use, no user data and
// no real disk writes. The real HTTP/TLS side is covered by the Rust
// `crates/application/tests/t16_webdav.rs` loopback suite.
import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

c.ErrorDto _err(String code, String key, {String? detail}) =>
    c.ErrorDto(code: code, messageKey: key, retryable: false, detail: detail);

/// Synthetic bridge with per-call result injection and optional gating. Only
/// the transport seam is overridden so the controller wiring is exercised.
class _ContractBridge extends SyntheticBridgePort {
  c.ErrorDto? checkError;
  c.ErrorDto? listError;
  c.ErrorDto? uploadError;
  c.ErrorDto? restoreError;
  c.ErrorDto? localBackupError;
  c.ErrorDto? localRestoreError;
  int deleteCalls = 0;
  int webdavBackupCalls = 0;
  int webdavRestoreCalls = 0;
  Completer<void>? webdavBackupGate;
  Completer<void>? localBackupGate;

  @override
  Future<c.WebDavCheckDto> t16WebdavCheck(c.WebDavConfigDto cfg) async {
    t16Calls.add('webdav_check');
    final error = checkError;
    if (error != null) {
      return c.WebDavCheckDto(
        ok: false,
        createdDir: false,
        status: 0,
        message: '',
        error: error,
      );
    }
    return super.t16WebdavCheck(cfg);
  }

  @override
  Future<c.WebDavListDto> t16WebdavList(c.WebDavConfigDto cfg) async {
    t16Calls.add('webdav_list');
    final error = listError;
    if (error != null) {
      return c.WebDavListDto(
        ok: false,
        items: const <c.WebDavEntryDto>[],
        error: error,
      );
    }
    return super.t16WebdavList(cfg);
  }

  @override
  Future<c.WebDavOpDto> t16WebdavBackup(c.WebDavConfigDto cfg) async {
    webdavBackupCalls += 1;
    t16Calls.add('webdav_backup');
    if (webdavBackupGate != null) {
      await webdavBackupGate!.future;
    }
    final error = uploadError;
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
    webdavRestoreCalls += 1;
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

  @override
  Future<c.BackupResultDto> t16BackupLocal(String destRoot) async {
    if (localBackupGate != null) {
      await localBackupGate!.future;
    }
    final error = localBackupError;
    if (error != null) {
      return c.BackupResultDto(ok: false, root: null, error: error);
    }
    return super.t16BackupLocal(destRoot);
  }

  @override
  Future<c.RestoreResultDto> t16BackupRestore(String bundleDir) async {
    t16Calls.add('backup_restore:$bundleDir');
    final error = localRestoreError;
    if (error != null) {
      return c.RestoreResultDto(
        ok: false,
        restored: false,
        message: '',
        error: error,
      );
    }
    return super.t16BackupRestore(bundleDir);
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
  test('the backup bridge seam is asynchronous', () {
    final bridge = SyntheticBridgePort();
    expect(bridge.t16BackupLocal('x'), isA<Future<c.BackupResultDto>>());
    expect(bridge.t16BackupList('x'), isA<Future<c.BackupListDto>>());
    expect(bridge.t16BackupRestore('x'), isA<Future<c.RestoreResultDto>>());
    expect(bridge.t16BackupRecognize('x'), isA<Future<c.RecognitionDto>>());
    expect(bridge.t16WebdavCheck(_cfg), isA<Future<c.WebDavCheckDto>>());
    expect(bridge.t16WebdavList(_cfg), isA<Future<c.WebDavListDto>>());
    expect(bridge.t16WebdavBackup(_cfg), isA<Future<c.WebDavOpDto>>());
    expect(bridge.t16WebdavRestore(_cfg), isA<Future<c.RestoreResultDto>>());
  });

  test('local backup commits the manifest and clears busy', () async {
    final bridge = _ContractBridge();
    final container = _container(bridge);
    final notifier = container.read(backupControllerProvider.notifier);
    await notifier.localBackup(r'C:\synthetic\dest');
    final state = container.read(backupControllerProvider);
    expect(state.busy, isFalse);
    expect(state.status?.isSuccess, isTrue);
    expect(state.bundles, hasLength(1));
  });

  test(
    'a failed local restore keeps the previous bundles and reports the error',
    () async {
      final bridge = _ContractBridge()
        ..localRestoreError = _err(
          'E_FIELD_FORMAT',
          'error.backup_invalid',
          detail: 'missing guiNDB.db',
        );
      final container = _container(bridge);
      final notifier = container.read(backupControllerProvider.notifier);
      await notifier.localBackup(r'C:\synthetic\dest');
      final before = container.read(backupControllerProvider).bundles;

      await notifier.restoreBundle('/tmp/broken');
      final state = container.read(backupControllerProvider);
      expect(state.busy, isFalse);
      expect(state.status?.isError, isTrue);
      expect(
        state.bundles,
        before,
        reason: 'a failed restore must not drop the current listing/data',
      );
    },
  );

  test(
    'WebDAV check maps 401, 404, timeout and unreachable branches',
    () async {
      final cases = <String, String>{
        'E_PERMISSION_DENIED': '认证失败',
        'E_NOT_FOUND': '远端路径不存在',
        'E_TIMEOUT': '连接超时',
        'E_UNAVAILABLE': '无法连接远端',
      };
      for (final entry in cases.entries) {
        final bridge = _ContractBridge()
          ..checkError = _err(entry.key, 'error.webdav_branch');
        final container = _container(bridge);
        final notifier = container.read(backupControllerProvider.notifier);
        await notifier.webdavCheck(_cfg);
        final state = container.read(backupControllerProvider);
        expect(state.busy, isFalse);
        expect(state.status?.isError, isTrue);
        expect(
          state.status?.detail,
          contains(entry.value),
          reason: '${entry.key} must map to its concrete branch',
        );
        expect(state.status?.detail, isNot(contains('synthetic-pass')));
      }
    },
  );

  test(
    'a failed upload is an error, never a success, and skips the list',
    () async {
      final bridge = _ContractBridge()
        ..uploadError = _err(
          'E_PERMISSION_DENIED',
          'error.webdav_permission',
          detail: 'status 401',
        );
      final container = _container(bridge);
      final notifier = container.read(backupControllerProvider.notifier);

      await notifier.webdavBackup(_cfg);
      final state = container.read(backupControllerProvider);
      expect(state.busy, isFalse);
      expect(state.status?.isSuccess, isFalse);
      expect(state.status?.message, contains('远程备份失败'));
      expect(
        bridge.t16Calls,
        isNot(contains('webdav_list')),
        reason: 'a rejected upload must not refresh the remote listing',
      );
      expect(state.status?.detail, isNot(contains('synthetic-pass')));
    },
  );

  test('a successful upload refreshes the remote listing', () async {
    final bridge = _ContractBridge();
    final container = _container(bridge);
    final notifier = container.read(backupControllerProvider.notifier);
    await notifier.webdavBackup(_cfg);
    final state = container.read(backupControllerProvider);
    expect(state.status?.isSuccess, isTrue);
    expect(bridge.t16Calls, contains('webdav_list'));
    expect(state.remotes, isNotEmpty);
  });

  test(
    'a thrown WebDAV restore clears busy and reports, then a retry succeeds',
    () async {
      // First call: bridge throws at the transport seam.
      final container = _container(_ThrowingRestoreBridge());
      final notifier = container.read(backupControllerProvider.notifier);
      await expectLater(notifier.webdavRestore(_cfg), completes);
      var state = container.read(backupControllerProvider);
      expect(state.busy, isFalse);
      expect(state.status?.isError, isTrue);

      // Retry on a healthy bridge succeeds.
      final container2 = _container(_ContractBridge());
      final notifier2 = container2.read(backupControllerProvider.notifier);
      await notifier2.webdavRestore(_cfg);
      state = container2.read(backupControllerProvider);
      expect(state.busy, isFalse);
      expect(state.status?.isSuccess, isTrue);
    },
  );

  test(
    'cancel discards a late WebDAV upload and the window stays usable',
    () async {
      final bridge = _ContractBridge()..webdavBackupGate = Completer<void>();
      final container = _container(bridge);
      final notifier = container.read(backupControllerProvider.notifier);

      final pending = notifier.webdavBackup(_cfg);
      await Future<void>.delayed(Duration.zero);
      expect(container.read(backupControllerProvider).busy, isTrue);

      notifier.cancel();
      expect(container.read(backupControllerProvider).busy, isFalse);
      bridge.webdavBackupGate!.complete();
      await pending;

      final state = container.read(backupControllerProvider);
      expect(state.status?.message, '已取消');
      expect(state.status?.isSuccess, isFalse);

      // The window can still run a fresh operation afterwards.
      bridge.webdavBackupGate = null;
      await notifier.webdavBackup(_cfg);
      expect(
        container.read(backupControllerProvider).status?.isSuccess,
        isTrue,
      );
    },
  );

  test('only one WebDAV operation runs at a time (task drain)', () async {
    final bridge = _ContractBridge()..webdavBackupGate = Completer<void>();
    final container = _container(bridge);
    final notifier = container.read(backupControllerProvider.notifier);

    final first = notifier.webdavBackup(_cfg);
    await Future<void>.delayed(Duration.zero);
    await notifier.webdavBackup(_cfg); // rejected while busy

    expect(
      bridge.webdavBackupCalls,
      1,
      reason: 'a second upload must not race the first',
    );

    bridge.webdavBackupGate!.complete();
    await first;
    expect(container.read(backupControllerProvider).busy, isFalse);
  });
}

class _ThrowingRestoreBridge extends SyntheticBridgePort {
  @override
  Future<c.RestoreResultDto> t16WebdavRestore(c.WebDavConfigDto cfg) async {
    throw StateError('native webdav restore exploded');
  }
}
