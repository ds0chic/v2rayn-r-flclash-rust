// R4-13.S24 settings-save-to-effect contract (WebDavItem instance).
//
// Storage chain: the four `WebDavItem` fields persist through both the settings
// document and the T16 WebDAV config surface (R4-28 hand-off). The real TLS
// endpoint is a blocked platform path. Values are synthetic; the real user's
// password is never read or logged.
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/bridge_port.dart';
import 'package:v2rayn_desktop/features/backup/backup_controller.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_controller.dart';
import 'package:v2rayn_desktop/features/settings/settings_defaults.dart';

import 'support/r4_13_settings_support.dart';

void main() {
  test('WebDavItem fields save -> reopen', () async {
    final bridge = seededBridge();
    final doc = await saveAndReopen(bridge, (draft) {
      final g = draft['WebDavItem'] as Map<String, dynamic>;
      g['Url'] = 'https://dav.example/synthetic';
      g['UserName'] = 'synthetic-user';
      g['Password'] = 'synthetic-secret';
      g['DirName'] = 'synthetic_backup';
    });
    final g = doc['WebDavItem'] as Map<String, dynamic>;
    expect(g['Url'], 'https://dav.example/synthetic');
    expect(g['UserName'], 'synthetic-user');
    expect(g['Password'], 'synthetic-secret');
    expect(g['DirName'], 'synthetic_backup');
  });

  test('WebDavItem defaults are the frozen upstream values', () {
    final g =
        mergeWithSettingsDefaults(<String, dynamic>{
              'WebDavItem': <String, dynamic>{},
            })['WebDavItem']
            as Map<String, dynamic>;
    expect(g['Url'], isNull);
    expect(g['UserName'], isNull);
    expect(g['Password'], isNull);
    expect(g['DirName'], isNull);
  });

  test('T16 WebDAV config save -> reopen through the backup controller', () {
    final bridge = SyntheticBridgePort();
    const cfg = c.WebDavConfigDto(
      url: 'https://dav.example/synthetic',
      userName: 'synthetic-user',
      password: 'synthetic-secret',
      dirName: 'synthetic_backup',
    );

    final container = ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
    );
    addTearDown(container.dispose);
    container.read(backupControllerProvider.notifier).saveWebdav(cfg);
    expect(
      container.read(backupControllerProvider).webdav.url,
      'https://dav.example/synthetic',
    );

    final reopened = ProviderContainer(
      overrides: [bridgePortProvider.overrideWithValue(bridge)],
    );
    addTearDown(reopened.dispose);
    final state = reopened.read(backupControllerProvider);
    expect(state.webdav.userName, 'synthetic-user');
    expect(state.webdav.dirName, 'synthetic_backup');
  });
}
