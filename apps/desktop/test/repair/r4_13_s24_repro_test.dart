// R4-13.S24 repro (WebDavItem field consumption guard).
//
// No in-scope WebDAV storage/consume defect was found: the settings document
// and the T16 engine surface both round-trip the four fields, and R4-28 already
// fixed the controller's async/busy/cancel paths. The real TLS endpoint is
// blocked. This file is the regression guard for the field save -> reopen path,
// including the stale-revision rejection that keeps a lost update visible.
//
// Synthetic values only: no real credential is read, stored or logged.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';

void main() {
  test('saved WebDAV fields survive reopen and a stale revision is rejected', () {
    final bridge = SyntheticBridgePort();
    final container = ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);
    final controller = container.read(backupControllerProvider.notifier);
    controller.saveWebdav(
      const c.WebDavConfigDto(
        url: 'https://dav.example/synthetic',
        userName: 'synthetic-user',
        password: 'synthetic-secret',
        dirName: 'synthetic_backup',
      ),
    );

    // The synthetic store bumps its revision; a second write with the previous
    // revision must be reported, never silently dropped as success.
    final stale = bridge.t16WebdavConfigSave(
      const c.WebDavConfigDto(
        url: 'https://dav.example/other',
        userName: 'x',
        password: 'y',
        dirName: 'z',
      ),
      0,
    );
    expect(stale.ok, isFalse);
    expect(stale.error?.code, 'E_REVISION_STALE');

    final reopened = ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
    );
    addTearDown(reopened.dispose);
    final state = reopened.read(backupControllerProvider).webdav;
    expect(state.url, 'https://dav.example/synthetic');
    expect(state.userName, 'synthetic-user');
    expect(state.dirName, 'synthetic_backup');
  });
}
