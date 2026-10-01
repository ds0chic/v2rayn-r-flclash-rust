// T15b local smoke seed. Guarded by `V2RAYN_T15B_SEED=1` so the normal test
// suite skips it. It writes real profiles into a temporary SQLite data dir
// (via the real bridge) so the release app can be launched against
// `V2RAYN_R_DATA_DIR` for a genuine local speedtest smoke.
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:v2rayn_desktop/bridge/api/contract.dart' as c;
import 'package:v2rayn_desktop/bridge/api/engine.dart' as engine;
import 'package:v2rayn_desktop/bridge/api/mirrors.dart';
import 'package:v2rayn_desktop/bridge/frb_generated.dart';

void main() {
  test('seed local smoke profiles', () async {
    if (Platform.environment['V2RAYN_T15B_SEED'] != '1') {
      markTestSkipped('seed disabled');
      return;
    }
    final dll = Platform.environment['V2RAYN_R_BRIDGE_DLL'];
    final dataDir = Platform.environment['V2RAYN_T15B_DATA'];
    final port = int.tryParse(Platform.environment['V2RAYN_T15B_PORT'] ?? '');
    if (dll == null || dataDir == null || port == null) {
      fail(
        'V2RAYN_R_BRIDGE_DLL / V2RAYN_T15B_DATA / V2RAYN_T15B_PORT required',
      );
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(dll));
    final init = engine.initEngine(dataDir: dataDir);
    expect(init.ok, isTrue, reason: init.error?.messageKey);

    c.ProfileDto local(String remarks, int port) => c.ProfileDto(
      indexId: '',
      configType: ConfigType.vless,
      coreType: CoreType.xray,
      configVersion: 4,
      subid: '',
      isSub: false,
      displayLog: true,
      remarks: remarks,
      address: '127.0.0.1',
      port: port,
      password: '11111111-2222-3333-4444-555555555555',
      username: '',
      network: 'raw',
      security: const c.SecurityDto(streamSecurity: null),
      protoExtra: const c.ProtocolExtraDto(
        vlessEncryption: 'none',
        extraJson: '{}',
      ),
      transportExtra: const c.TransportExtraDto(extraJson: '{}'),
      extraJson: '{}',
    );

    final revision = engine.profileRevision();
    final first = engine.saveProfile(
      draft: local('Local-Smoke-A', port),
      expectedRevision: revision,
    );
    expect(first.ok, isTrue, reason: first.error?.messageKey);
    final second = engine.saveProfile(
      draft: local('Local-Smoke-B', port),
      expectedRevision: first.newRevision!,
    );
    expect(second.ok, isTrue, reason: second.error?.messageKey);
  });
}
